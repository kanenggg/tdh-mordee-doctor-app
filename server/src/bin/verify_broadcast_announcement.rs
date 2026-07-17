use backoff::ExponentialBackoff;
use dotenvy::dotenv;
use server::config::AppConfig;
use server::module::notification::repo::{
    NotificationDoc, NotificationRepo, NotificationRepoTrait,
};
use server::repo::firestore_repo::FirestoreRepo;
use std::time::Duration as StdDuration;
use tracing::info;
use tracing_subscriber::EnvFilter;

/// Verify that every doctor can see broadcast announcements.
///
/// Exercises the real `get_notifications` read path (personal + broadcast merge
/// with per-doctor read overlay) against the configured Firestore project (dev by
/// default) for several distinct doctor ids. Because the broadcast query is keyed
/// on the sentinel `doctorId = 0` independent of the requesting doctor, a spread of
/// arbitrary ids — including ones with no personal notifications — demonstrates
/// that ALL doctors see the announcement.
///
/// Pass doctor ids as CLI args to override the default spread:
///   cargo run --bin verify_broadcast_announcement -- 2443 1001 99999
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install rustls CryptoProvider");

    dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive("verify_broadcast_announcement=info".parse()?),
        )
        .init();

    let cfg = AppConfig::load_from_dir(None)?;
    info!(
        "Configuration loaded (firestore project: {})",
        cfg.firestore.gcp_project_id
    );

    let retry_config = ExponentialBackoff {
        initial_interval: StdDuration::from_millis(cfg.retry.base_delay_ms),
        max_interval: StdDuration::from_millis(cfg.retry.max_delay_ms),
        multiplier: 2.0,
        max_elapsed_time: None,
        ..Default::default()
    };

    let firestore = FirestoreRepo::new(&cfg.firestore, retry_config).await?;
    info!("Firestore connection established");

    let repo = NotificationRepo::new(
        firestore,
        cfg.firestore.collections.notifications.clone(),
        cfg.firestore.collections.fcm_tokens.clone(),
    );

    // Doctor ids to check: CLI args, or a default spread of distinct ids.
    // 2443 has personal test data; the others are arbitrary ids that should have
    // no personal notifications, proving the broadcast shows for any doctor.
    let doctor_ids: Vec<String> = {
        let args: Vec<String> = std::env::args().skip(1).collect();
        if args.is_empty() {
            vec![
                "2443".to_string(),
                "1001".to_string(),
                "55555".to_string(),
                "99999".to_string(),
            ]
        } else {
            args
        }
    };

    let mut all_saw_broadcast = true;

    for doctor_id in &doctor_ids {
        let docs = repo
            .get_notifications(doctor_id, "Announcement", None, None, 100)
            .await?;

        let broadcasts: Vec<&NotificationDoc> = docs
            .iter()
            .filter(|d| d.notification_id().starts_with("announcement_broadcast_"))
            .collect();

        if broadcasts.is_empty() {
            all_saw_broadcast = false;
            info!(
                "doctor {doctor_id}: ❌ NO broadcast announcements visible (total announcements: {})",
                docs.len()
            );
        } else {
            let summary: Vec<String> = broadcasts
                .iter()
                .map(|d| {
                    let is_read = match d {
                        NotificationDoc::Announcement { is_read, .. } => *is_read,
                        NotificationDoc::Alert { is_read, .. } => *is_read,
                    };
                    format!("{} (isRead={})", d.notification_id(), is_read)
                })
                .collect();
            info!(
                "doctor {doctor_id}: ✅ sees {} broadcast(s): {} | total announcements: {}",
                broadcasts.len(),
                summary.join(", "),
                docs.len()
            );
        }
    }

    if all_saw_broadcast {
        info!(
            "✅ PASS: all {} doctors checked can see the broadcast announcement(s)",
            doctor_ids.len()
        );
    } else {
        anyhow::bail!("FAIL: at least one doctor could not see the broadcast announcement");
    }

    Ok(())
}
