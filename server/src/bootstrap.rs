use anyhow::Result;
use axum::{middleware, Router};
use backoff::ExponentialBackoff;
use deadpool_redis::{Config as RedisConfig, Runtime};
use opentelemetry_sdk::trace::SdkTracerProvider;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use std::time::Duration;
use tokio::signal;
use tokio_util::sync::CancellationToken;
use tower_http::cors::CorsLayer;
use tracing::info;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::config::AppConfig;
use crate::core::{self};
use crate::module;
use crate::openapi;
use crate::repo::{FirebaseRepo, FirestoreRepo};

pub struct Dependencies {
    pub firestore: FirestoreRepo,
    pub firebase: FirebaseRepo,
    #[allow(dead_code)]
    pub gcp_token: Arc<core::GcpTokenProvider>,
    pub pubsub_publisher: Arc<module::webhook::PubsubPublisher>,
    pub pg_pool: sqlx::PgPool,
    pub redis_pool: deadpool_redis::Pool,
    pub ranking_repo: Arc<dyn module::ranking::RankingRepoTrait>,
    pub ranking_cache: Arc<dyn module::ranking::RankingCacheTrait>,
    pub privilege_svc: Arc<dyn module::ranking::PrivilegeServiceTrait>,
}

pub struct AppRouters {
    pub notification: Router,
    pub consultation: Router,
    pub ranking: Router,
    pub profile: Router,
    pub timeslot: Router,
    pub appointment: Router,
    pub onboarding: Router,
    pub backoffice: Router,
    pub internal_doctor_management: Router,
    pub internal_doctor_directory: Router,
    pub internal_onboarding: Router,
    pub ehr: Router,
}

pub fn init_infrastructure_with_config(
    config_dir: Option<String>,
) -> Result<(AppConfig, Option<SdkTracerProvider>)> {
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install rustls CryptoProvider");

    dotenvy::dotenv().ok();

    let cfg = AppConfig::load_from_dir(config_dir)?;
    let tracer_provider = core::init_telemetry(&cfg.telemetry)?;
    core::set_startup_time();
    Ok((cfg, tracer_provider))
}

pub async fn init_repos_and_services(cfg: &AppConfig) -> Result<Dependencies> {
    let gcp_token = Arc::new(core::GcpTokenProvider::new());

    let retry_config = ExponentialBackoff {
        initial_interval: Duration::from_millis(cfg.retry.base_delay_ms),
        max_interval: Duration::from_millis(cfg.retry.max_delay_ms),
        multiplier: 2.0,
        max_elapsed_time: None,
        ..Default::default()
    };

    let firestore = FirestoreRepo::new(&cfg.firestore, retry_config.clone()).await?;
    let firebase =
        FirebaseRepo::new(&cfg.firebase, gcp_token.clone(), retry_config.clone()).await?;

    let pg_pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&cfg.postgres.database_url())
        .await?;

    let redis_pool = RedisConfig::from_url(cfg.redis.url.clone())
        .create_pool(Some(Runtime::Tokio1))
        .map_err(|e| anyhow::anyhow!("Redis pool creation failed: {}", e))?;

    info!("Redis pool created successfully");

    let ranking_cache: Arc<dyn module::ranking::RankingCacheTrait> = Arc::new(
        module::ranking::cache::RankingCache::new(redis_pool.clone()),
    );

    let ranking_repo: Arc<dyn module::ranking::RankingRepoTrait> =
        Arc::new(module::ranking::repo::RankingRepo::new(
            pg_pool.clone(),
            cfg.ranking.platform_fee_multiplier,
        ));

    let privilege_svc: Arc<dyn module::ranking::PrivilegeServiceTrait> =
        Arc::new(module::ranking::privilege::PrivilegeService::new(
            cfg.service.privilege_service_base_uri.clone(),
        ));

    let pubsub_publisher = Arc::new(module::webhook::PubsubPublisher::new(&cfg.pubsub).await?);
    info!(
        "Pub/Sub publisher ready (topics: appointments={}, consultations={}, system={}, broadcast={}, doctor_notifications={})",
        cfg.pubsub.topics.appointments,
        cfg.pubsub.topics.consultations,
        cfg.pubsub.topics.system,
        cfg.pubsub.topics.broadcast,
        cfg.pubsub.topics.doctor_notifications,
    );

    Ok(Dependencies {
        firestore,
        firebase,
        gcp_token,
        pubsub_publisher,
        pg_pool,
        redis_pool,
        ranking_repo,
        ranking_cache,
        privilege_svc,
    })
}

pub async fn init_routers(cfg: &AppConfig, deps: &mut Dependencies) -> Result<AppRouters> {
    let (notification_router, _) = module::notification::router(deps.firestore.clone(), cfg);

    let timeslot_repo: Arc<dyn module::timeslot::TimeslotRepo> = Arc::new(
        module::timeslot::TimeslotRepoImpl::new(deps.pg_pool.clone(), deps.redis_pool.clone()),
    );

    let consultation_router = module::consultation::router(
        deps.pg_pool.clone(),
        &cfg.paseto.summarization_key,
        cfg.service.biz_jade_service_base_uri.clone(),
        cfg.service.consultation_base_uri.clone(),
        deps.pubsub_publisher.clone(),
        cfg.pubsub.topics.consultations.clone(),
        timeslot_repo,
    )?;

    let ranking_router = module::ranking::router(
        deps.ranking_repo.clone(),
        deps.ranking_cache.clone(),
        deps.privilege_svc.clone(),
    );

    use crate::doctor_actor::repo::DoctorTimeslotRepoImpl;
    let doctor_timeslot_repo: Arc<dyn crate::doctor_actor::repo::DoctorTimeslotRepo> = Arc::new(
        DoctorTimeslotRepoImpl::new(deps.pg_pool.clone(), deps.redis_pool.clone()),
    );

    let timeslot_router = module::timeslot::router(
        deps.pg_pool.clone(),
        cfg,
        deps.pubsub_publisher.clone(),
        doctor_timeslot_repo,
        cfg.service.consultation_base_uri.clone(),
    )
    .await?;

    let (appointment_router, consultation) = module::appointment::router(cfg)?;

    let gcs_signed_url_generator: Arc<dyn core::gcs_signed_url::GcsSignedUrlGenerator> = Arc::new(
        core::gcs_signed_url::GcpGcsSignedUrlGenerator::new(&cfg.gcs, deps.gcp_token.clone()),
    );
    let ekyc_router = module::ekyc::router(
        consultation.clone(),
        cfg.service.eagle_base_uri.clone(),
        gcs_signed_url_generator,
    );

    let appointment_router = appointment_router.merge(ekyc_router);

    // Shared GCP KMS client (citizen_id is KMS-encrypted at save, decrypted for display).
    let kms: Arc<dyn core::kms::KmsClient> = Arc::new(core::kms::GcpKmsGateway::new(
        deps.gcp_token.clone(),
        cfg.kms.doctor_profile_pii_data.clone(),
    ));

    let (onboarding_router, _) = module::onboarding::router(deps.pg_pool.clone(), kms.clone());
    let profile_router = module::profile::router(deps.pg_pool.clone(), kms.clone());
    let backoffice_routers = module::backoffice::routers(
        deps.firestore.clone(),
        cfg.firestore.collections.doctors.clone(),
        deps.pg_pool.clone(),
        deps.pubsub_publisher.clone(),
        cfg.pubsub.topics.doctor_profile_approved.clone(),
        cfg.pubsub.topics.doctor_profile_status_updated.clone(),
        cfg.service.doctor_profile_immediate_delivery_enabled,
    );

    let ehr_router = module::ehr::router(
        cfg.service.ehr_service_base_uri.clone(),
        cfg.service.biz_apm_base_uri.clone(),
        cfg.service.biz_jade_service_base_uri.clone(),
        deps.pg_pool.clone(),
    );

    Ok(
        AppRouters {
            notification: notification_router,
            consultation: consultation_router,
            ranking: ranking_router,
            profile: profile_router,
            timeslot: timeslot_router,
            appointment: appointment_router,
            onboarding: onboarding_router,
            backoffice: backoffice_routers.backoffice,
            internal_doctor_management: backoffice_routers.internal_doctor_management,
            internal_doctor_directory: backoffice_routers.internal_doctor_directory,
            internal_onboarding: backoffice_routers.internal_onboarding,
            ehr: ehr_router,
        },
        // timeslot_worker_handle,
    )
}

pub fn build_app(routers: AppRouters) -> Router {
    Router::new()
        .merge(SwaggerUi::new("/swagger").url("/api-docs/openapi.json", openapi::ApiDoc::openapi()))
        .merge(core::health_router())
        .nest("/notifications/v1", routers.notification)
        .nest("/consultation/v1", routers.consultation)
        .nest("/ranking/v1", routers.ranking)
        .nest("/profile", routers.profile)
        .nest("/timeslot", routers.timeslot)
        .nest("/appointment/v1", routers.appointment)
        .nest("/onboarding/v1", routers.onboarding)
        .nest("/internal/v1", routers.internal_doctor_management)
        .nest("/internal/v1/doctors", routers.internal_doctor_directory)
        .nest("/internal/v1/onboarding", routers.internal_onboarding)
        .nest("/backoffice/v1", routers.backoffice)
        .nest("/ehr/v1", routers.ehr)
        .layer(middleware::from_fn(core::gcp_logging_middleware))
        .layer(CorsLayer::permissive())
}

pub async fn shutdown_signal(
    tracer_provider: Option<SdkTracerProvider>,
    cancel_token: CancellationToken,
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

    info!("Shutdown signal received, cancelling subscribers");
    cancel_token.cancel();
    core::shutdown_telemetry(tracer_provider).await;
}
