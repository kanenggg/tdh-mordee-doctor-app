use backoff::ExponentialBackoff;
use dotenvy::dotenv;
use jiff::Zoned;
use server::config::AppConfig;
use server::module::notification::repo::{NotificationDoc, BROADCAST_DOCTOR_ID};
use server::repo::firestore_repo::{FirestoreRepo, FirestoreRepoTrait};
use std::time::Duration as StdDuration;
use tracing::info;
use tracing_subscriber::EnvFilter;

const COLLECTION_NAME: &str = "notifications";

/// Seed a single broadcast Announcement that every doctor sees.
///
/// Broadcasts are stored in the `notifications` collection with the sentinel
/// `doctorId = BROADCAST_DOCTOR_ID` (0). The read path merges these with each
/// doctor's personal announcements, so all doctors see this record. Runs against
/// whatever Firestore project the default config points at (dev by default).
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Install AWS LC rustls crypto provider before any TLS operations
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install rustls CryptoProvider");

    // Load .env file for local development
    dotenv().ok();

    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env().add_directive("generate_test_announcement=info".parse()?),
        )
        .init();

    info!("Seeding one broadcast announcement (doctorId={BROADCAST_DOCTOR_ID})");

    // Load configuration
    let cfg = AppConfig::load_from_dir(None)?;
    info!(
        "Configuration loaded (firestore project: {})",
        cfg.firestore.gcp_project_id
    );

    // Initialize Firestore with retry config
    let retry_config = ExponentialBackoff {
        initial_interval: StdDuration::from_millis(cfg.retry.base_delay_ms),
        max_interval: StdDuration::from_millis(cfg.retry.max_delay_ms),
        multiplier: 2.0,
        max_elapsed_time: None,
        ..Default::default()
    };

    let firestore = FirestoreRepo::new(&cfg.firestore, retry_config).await?;
    info!("Firestore connection established");

    let notification_id = format!("announcement_broadcast_{}", uuid::Uuid::new_v4());

    let announcement = NotificationDoc::Announcement {
        notification_id: notification_id.clone(),
        is_read: false,
        title: "Welcome to Mordee Doctor".to_string(),
        sub_title: "A new broadcast announcement visible to every doctor.".to_string(),
        sent_at: Zoned::now(),
        content_url: "https://example.com/announcements/welcome".to_string(),
        icon_url: "https://example.com/icons/marketing.png".to_string(),
        category: "Marketing".to_string(),
    };

    let doc = to_doc_with_doctor_id(&announcement, BROADCAST_DOCTOR_ID);

    firestore
        .set_doc(COLLECTION_NAME, &notification_id, &doc)
        .await?;

    info!("✅ Created broadcast announcement: {notification_id}");
    info!("Verify with: GET /notifications/v1?type=Announcement (as any doctor)");

    Ok(())
}

/// Serialize a NotificationDoc and inject the doctorId field for flat collection storage.
fn to_doc_with_doctor_id(notification: &NotificationDoc, doctor_id: i32) -> serde_json::Value {
    let mut val = serde_json::to_value(notification).expect("serialize notification");
    if let serde_json::Value::Object(map) = &mut val {
        map.insert("doctorId".to_string(), serde_json::json!(doctor_id));
    }
    val
}
