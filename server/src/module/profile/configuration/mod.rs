pub mod handler;
pub mod models;
pub mod repo;
pub mod service;

use std::sync::Arc;

use axum::Router;
use sqlx::PgPool;

pub use repo::{DoctorConfigurationRepo, DoctorConfigurationRepoPsql};
pub use service::DoctorConfigurationService;

pub fn router(pool: PgPool) -> Router {
    let repo: Arc<dyn DoctorConfigurationRepo> = Arc::new(DoctorConfigurationRepoPsql::new(pool));
    let service = Arc::new(DoctorConfigurationService::new(repo));

    handler::routes(service)
}
