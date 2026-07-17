use backoff::ExponentialBackoff;
use dotenvy::dotenv;
use server::config::AppConfig;
use server::repo::firestore_repo::{FirestoreRepo, FirestoreRepoTrait, QueryFilter, QueryOp};
use std::time::Duration;
use tracing::info;
use tracing_subscriber::EnvFilter;

const DOCTOR_ID: i32 = 2443;
const COLLECTION: &str = "notifications";

/// Clear all test notifications for doctorId 2443
/// Deletes all documents from the flat notifications collection matching this doctor
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Install AWS LC rustls crypto provider before any TLS operations
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install rustls CryptoProvider");

    dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env().add_directive("clear_test_notifications=info".parse()?),
        )
        .init();

    info!(
        "Starting test notification cleanup for doctorId: {}",
        DOCTOR_ID
    );

    let cfg = AppConfig::load_from_dir(None)?;
    info!("Configuration loaded successfully");

    let retry_config = ExponentialBackoff {
        initial_interval: Duration::from_millis(cfg.retry.base_delay_ms),
        max_interval: Duration::from_millis(cfg.retry.max_delay_ms),
        multiplier: 2.0,
        max_elapsed_time: None,
        ..Default::default()
    };

    let firestore = FirestoreRepo::new(&cfg.firestore, retry_config).await?;
    info!("Firestore connection established");

    let filters = vec![QueryFilter {
        field: "doctorId".to_string(),
        op: QueryOp::Eq,
        value: serde_json::json!(DOCTOR_ID),
    }];

    let notifications = firestore
        .query_collection::<serde_json::Value>(COLLECTION, filters, None, None, None)
        .await?;

    let total = notifications.len();
    info!("Found {} notifications to delete", total);

    if total == 0 {
        info!("No notifications found to delete");
        return Ok(());
    }

    for (i, notif) in notifications.iter().enumerate() {
        let notification_id = notif["notificationId"]
            .as_str()
            .or_else(|| notif["notification_id"].as_str())
            .unwrap_or_else(|| {
                notif["__name__"]
                    .as_str()
                    .and_then(|s| s.rsplit('/').next())
                    .unwrap_or("unknown")
            });

        firestore
            .db
            .fluent()
            .delete()
            .from(COLLECTION)
            .document_id(notification_id)
            .execute()
            .await
            .map_err(|e| {
                info!("Failed to delete {}: {}", notification_id, e);
                e
            })?;

        info!("Deleted {}/{}: {}", i + 1, total, notification_id);
    }

    info!(
        "Successfully deleted {} notifications for doctorId {}",
        total, DOCTOR_ID
    );
    Ok(())
}
