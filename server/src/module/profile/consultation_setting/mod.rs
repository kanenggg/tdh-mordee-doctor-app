pub mod handler;
pub mod model;
pub mod repo;
pub mod service;
pub mod validate;
pub mod window;

use std::sync::Arc;

use axum::{routing::get, Router};
use sqlx::PgPool;

pub use handler::ConsultationSettingState;
pub use repo::{ConsultationSettingRepo, ConsultationSettingRepoPsql};
pub use service::ConsultationSettingService;

pub fn router(pool: PgPool) -> Router {
    let repo: Arc<dyn ConsultationSettingRepo> = Arc::new(ConsultationSettingRepoPsql::new(pool));
    let service = Arc::new(ConsultationSettingService::new(repo));
    let state = ConsultationSettingState { service };

    Router::new()
        .route(
            "/v1/consultation-setting/schedule-config/{bizUnit}",
            get(handler::get_schedule_config).put(handler::update_schedule_config),
        )
        .with_state(state)
}
