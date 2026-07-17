pub mod handler;
pub mod repo;

use axum::Router;
use sqlx::PgPool;
use std::sync::Arc;

pub fn router(pool: PgPool) -> Router {
    handler::router(Arc::new(repo::ApprovedDoctorDirectoryRepo::new(pool)))
}
