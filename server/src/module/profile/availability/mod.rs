pub mod handler;
pub mod models;
pub mod repo;
pub mod service;

use std::sync::Arc;

use axum::{
    routing::{get, post},
    Router,
};
use sqlx::PgPool;

pub use handler::AvailabilityState;
pub use repo::{AvailabilityRepo, AvailabilityRepoPsql};
pub use service::AvailabilityService;

pub fn router(pool: PgPool) -> Router {
    let repo: Arc<dyn AvailabilityRepo> = Arc::new(AvailabilityRepoPsql::new(pool));
    let service = Arc::new(AvailabilityService::new(repo));
    let state = AvailabilityState { service };

    Router::new()
        .route(
            "/v1/availability/schedule",
            post(handler::update_schedule_availability),
        )
        .route(
            "/v1/availability/instant",
            post(handler::update_instant_availability),
        )
        .route("/v1/availability", get(handler::get_availability))
        .with_state(state)
}
