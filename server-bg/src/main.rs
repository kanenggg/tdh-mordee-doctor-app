use clap::Parser;
use sqlx::postgres::PgPoolOptions;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::signal;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::error;
use tracing::info;

use server_bg::config::AppConfig;

#[derive(Parser, Debug)]
#[command(name = "server-bg")]
#[command(about = "TDH Doctor App background event service", long_about = None)]
struct Args {
    #[arg(short, long)]
    config_dir: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let (cfg, tracer_provider) = init_infrastructure(args.config_dir)?;
    let deps = init_dependencies(&cfg).await?;
    let cancellation = CancellationToken::new();
    let relay_handle = start_doctor_profile_outbox_relay(&cfg, &deps, cancellation.clone()).await?;

    let patient_service = Arc::new(common::patient::PatientService::new(
        cfg.service.iam_gatekeeper_base_uri.clone(),
    ));
    let calendar_service = doctor_calendar_service(&cfg, &deps);
    let doctor_notification_service =
        doctor_notification_service(&cfg, &deps, patient_service.clone());

    let notification_service = Arc::new(
        server_bg::module::doctor_notification::service::DoctorNotificationDeliveryService::new(
            deps.notification_repo.clone(),
            deps.fcm_service.clone(),
            deps.cloud_tasks_service.clone(),
        ),
    );
    let task_service = Arc::new(
        server_bg::module::doctor_notification::service::ScheduledNotificationTaskService::new(
            deps.cloud_tasks_service.clone(),
            notification_service.clone(),
        ),
    );

    let app = server_bg::build_app(
        calendar_service,
        doctor_notification_service,
        notification_service,
        task_service,
    );

    let addr: SocketAddr = format!("{}:{}", cfg.http_server.host, cfg.http_server.port).parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("Server-bg listening on {addr}");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal(tracer_provider, cancellation))
    .await?;

    if let Some(handle) = relay_handle {
        handle.await??;
    }

    Ok(())
}

struct BgDependencies {
    firebase: common::repo::FirebaseRepo,
    pubsub_publisher: Arc<common::messaging::PubsubPublisher>,
    cloud_tasks_service: Arc<common::messaging::CloudTasksService>,
    fcm_service: Arc<server_bg::infra::fcm_service::FcmService>,
    notification_repo: Arc<dyn common::notification::NotificationRepoTrait>,
    pg_pool: Option<sqlx::PgPool>,
}

fn init_infrastructure(
    config_dir: Option<String>,
) -> anyhow::Result<(
    AppConfig,
    Option<opentelemetry_sdk::trace::SdkTracerProvider>,
)> {
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install rustls CryptoProvider");

    dotenvy::dotenv().ok();

    let cfg = AppConfig::load_from_dir(Some(default_config_dir(config_dir)))?;
    let telemetry_config = cfg.telemetry.to_common();
    let tracer_provider = common::core::init_telemetry(&telemetry_config)?;
    common::core::set_startup_time();
    Ok((cfg, tracer_provider))
}

fn default_config_dir(config_dir: Option<String>) -> String {
    match config_dir {
        Some(dir) => dir,
        None if Path::new("server-bg/config/default.toml").is_file() => "server-bg/config".into(),
        None => "./config".into(),
    }
}

async fn init_dependencies(cfg: &AppConfig) -> anyhow::Result<BgDependencies> {
    let gcp_token = Arc::new(common::core::GcpTokenProvider::new());
    let retry_config = backoff::ExponentialBackoff {
        initial_interval: Duration::from_millis(cfg.retry.base_delay_ms),
        max_interval: Duration::from_millis(cfg.retry.max_delay_ms),
        multiplier: 2.0,
        max_elapsed_time: None,
        ..Default::default()
    };

    let firestore_config = cfg.firestore.to_common();
    let firebase_config = cfg.firebase.to_common();
    let pubsub_config = cfg.pubsub.to_common();
    let cloud_tasks_config = cfg.cloud_tasks.to_common();

    let firestore =
        common::repo::FirestoreRepo::new(&firestore_config, retry_config.clone()).await?;
    let firebase =
        common::repo::FirebaseRepo::new(&firebase_config, gcp_token.clone(), retry_config).await?;
    let pubsub_publisher = Arc::new(common::messaging::PubsubPublisher::new(&pubsub_config).await?);
    let cloud_tasks_service = Arc::new(common::messaging::CloudTasksService::new(
        &cloud_tasks_config,
        gcp_token.clone(),
    ));
    let fcm_service = Arc::new(server_bg::infra::fcm_service::FcmService::new(
        Arc::new(cfg.fcm.clone()),
        gcp_token,
        Arc::new(cfg.retry.clone()),
    ));
    let notification_repo: Arc<dyn common::notification::NotificationRepoTrait> =
        Arc::new(common::notification::NotificationRepo::new(
            firestore.clone(),
            cfg.firestore.collections.notifications.clone(),
            cfg.firestore.collections.fcm_tokens.clone(),
        ));
    let pg_pool = if cfg.doctor_profile_outbox.enabled {
        let postgres = cfg
            .postgres
            .as_ref()
            .expect("validated when relay is enabled");
        Some(
            PgPoolOptions::new()
                .max_connections(5)
                .connect(&postgres.database_url())
                .await?,
        )
    } else {
        None
    };

    Ok(BgDependencies {
        firebase,
        pubsub_publisher,
        cloud_tasks_service,
        fcm_service,
        notification_repo,
        pg_pool,
    })
}

async fn start_doctor_profile_outbox_relay(
    cfg: &AppConfig,
    deps: &BgDependencies,
    cancellation: CancellationToken,
) -> anyhow::Result<Option<JoinHandle<anyhow::Result<()>>>> {
    if !cfg.doctor_profile_outbox.enabled {
        info!("doctor profile outbox relay disabled");
        return Ok(None);
    }
    use server_bg::module::doctor_profile_outbox::{
        DoctorProfileOutboxRelay, PostgresDoctorProfileOutboxRepo,
        PubsubDoctorProfileOutboxPublisher,
    };
    let repo = Arc::new(PostgresDoctorProfileOutboxRepo::new(
        deps.pg_pool
            .clone()
            .expect("validated when relay is enabled"),
    ));
    let publisher = Arc::new(PubsubDoctorProfileOutboxPublisher::new(
        deps.pubsub_publisher.clone(),
        cfg.pubsub.topics.doctor_profile_approved.clone(),
        cfg.pubsub.topics.doctor_profile_status_updated.clone(),
    ));
    let relay = Arc::new(DoctorProfileOutboxRelay::new(repo, publisher).configured(
        cfg.doctor_profile_outbox.batch_size,
        cfg.doctor_profile_outbox.lease_seconds,
        Duration::from_secs(cfg.doctor_profile_outbox.publish_timeout_seconds),
    ));
    let poll = Duration::from_secs(cfg.doctor_profile_outbox.poll_seconds);
    Ok(Some(tokio::spawn(async move {
        let mut interval = tokio::time::interval(poll);
        loop {
            tokio::select! {
                _ = cancellation.cancelled() => {
                    info!("doctor profile outbox relay stopped");
                    return Ok(());
                }
                _ = interval.tick() => {
                    if let Err(error) = relay.run_once().await {
                        error!(error = ?error, "doctor profile outbox relay iteration failed");
                    }
                }
            }
        }
    })))
}

fn doctor_calendar_service(
    cfg: &AppConfig,
    deps: &BgDependencies,
) -> Arc<server_bg::module::doctor_calendar::service::DoctorCalendarService> {
    let event_repo = Arc::new(
        server_bg::module::doctor_calendar::domain::RtdbConsultationEventRepo::new(
            deps.firebase.clone(),
            cfg.firebase.collections.appointments.clone(),
        ),
    );

    Arc::new(server_bg::module::doctor_calendar::service::DoctorCalendarService::new(event_repo))
}

fn doctor_notification_service(
    cfg: &AppConfig,
    deps: &BgDependencies,
    patient_service: Arc<common::patient::PatientService>,
) -> Arc<server_bg::module::doctor_notification::service::DoctorNotificationService> {
    let notification_publisher = Arc::new(
        server_bg::module::doctor_notification::domain::ConsultationNotificationPublisher::new(
            deps.pubsub_publisher.clone(),
            patient_service.clone(),
            deps.cloud_tasks_service.clone(),
            deps.notification_repo.clone(),
            cfg.pubsub.topics.doctor_notifications.clone(),
        ),
    );

    Arc::new(
        server_bg::module::doctor_notification::service::DoctorNotificationService::new(
            deps.pubsub_publisher.clone(),
            notification_publisher,
            deps.notification_repo.clone(),
            deps.cloud_tasks_service.clone(),
            patient_service,
            cfg.pubsub.topics.doctor_notifications.clone(),
        ),
    )
}

async fn shutdown_signal(
    tracer_provider: Option<opentelemetry_sdk::trace::SdkTracerProvider>,
    cancellation: CancellationToken,
) {
    let ctrl_c = async {
        signal::ctrl_c().await.expect("ctrl-c handler failed");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("SIGTERM handler failed")
            .recv()
            .await
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("Shutdown signal received, cancelling server-bg");
    cancellation.cancel();
    common::core::shutdown_telemetry(tracer_provider).await;
}

#[cfg(test)]
mod tests {
    #[test]
    fn default_config_dir_points_to_server_bg_default_config() {
        let dir = super::default_config_dir(None);

        assert!(std::path::Path::new(&dir).join("default.toml").is_file());
    }

    #[test]
    fn explicit_config_dir_wins() {
        assert_eq!(
            super::default_config_dir(Some("custom/config".to_string())),
            "custom/config"
        );
    }
}
