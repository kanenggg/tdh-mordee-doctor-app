pub mod common;
pub mod get_my_available_time_slots;
pub mod handler;
pub mod model;
pub mod repo;
pub mod service;

pub use model::*;
pub use repo::{TimeslotRepo, TimeslotRepoImpl};
pub use service::{CachedReserveResponse, IdempotencyCache, RateLimiter, TimeslotService};

use axum::{routing::get, Router};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::core::error::AppResult;
use crate::module::timeslot::handler::TimeslotState;
use crate::module::webhook::PubsubPublisher;
use crate::{
    config::AppConfig,
    module::timeslot::get_my_available_time_slots::handler::get_my_available_timeslots,
};
use deadpool_redis::{Config as RedisConfig, Runtime};

pub async fn router(
    pg_pool: sqlx::PgPool,
    cfg: &AppConfig,
    pubsub_publisher: Arc<PubsubPublisher>,
    doctor_timeslot_repo: Arc<dyn crate::doctor_actor::repo::DoctorTimeslotRepo>,
    consultation_base_uri: String,
) -> AppResult<Router> {
    let redis_url = cfg.redis.url.clone();

    let redis_pool = RedisConfig::from_url(redis_url.clone())
        .create_pool(Some(Runtime::Tokio1))
        .map_err(|e| {
            crate::core::error::AppError::InternalError(format!(
                "Redis pool creation failed: {}",
                e
            ))
        })?;

    let client = redis::Client::open(redis_url.clone()).map_err(|e| {
        crate::core::error::AppError::InternalError(format!("Redis connection failed: {}", e))
    })?;
    let redis_manager = redis::aio::ConnectionManager::new(client)
        .await
        .map_err(|e| {
            crate::core::error::AppError::InternalError(format!("Redis manager failed: {}", e))
        })?;

    let idempotency_cache = IdempotencyCache::new(&redis_url).await?;

    let repo = Arc::new(TimeslotRepoImpl::new(pg_pool.clone(), redis_pool));
    let rate_limiter = RateLimiter::new(
        pg_pool.clone(),
        cfg.timeslot.rate_limit.daily_limit,
        cfg.timeslot.rate_limit.weekly_limit,
    );
    let service = Arc::new(TimeslotService::new(repo, rate_limiter, pubsub_publisher));

    let consultation_duration_repo: Arc<
        dyn crate::module::timeslot::get_my_available_time_slots::repo::ConsultationDurationRepo,
    > = Arc::new(
        crate::module::timeslot::get_my_available_time_slots::repo::ConsultationDurationRepoImpl::new(
            pg_pool.clone(),
        ),
    );

    let reserved_timeslots_client: Arc<
        dyn crate::module::timeslot::get_my_available_time_slots::gateway::ReservedTimeslotsClientTrait,
    > = Arc::new(
        crate::module::timeslot::get_my_available_time_slots::gateway::ReservedTimeslotsClient::new(
            consultation_base_uri,
        ),
    );

    let state = TimeslotState {
        service,
        idempotency_cache: Arc::new(Mutex::new(idempotency_cache)),
        redis: redis_manager,
        config: cfg.timeslot.clone(),
        doctor_timeslot_repo,
        consultation_duration_repo,
        reserved_timeslots_client,
    };

    let router = Router::new()
        // .route("/available", get(handler::get_available_timeslot))
        // .route("/my-available", get(handler::get_my_available_timeslots))
        .route(
            "/v1/me/available-timeslots",
            get(get_my_available_timeslots),
        )
        .with_state(state);

    Ok(router)
}
