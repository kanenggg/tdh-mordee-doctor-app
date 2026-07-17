use axum::Router;
use sqlx::PgPool;
use std::sync::Arc;

use crate::core::kms::KmsClient;

pub mod availability;
pub mod common;
pub mod configuration;
pub mod consultation_setting;
pub mod handler;
pub mod repo;

pub fn router(pool: PgPool, kms: Arc<dyn KmsClient>) -> Router {
    let profile_repo: Arc<dyn repo::ProfileRepoTrait> =
        Arc::new(repo::ProfileRepoImp::new(pool.clone(), kms));
    Router::new()
        .merge(consultation_setting::router(pool.clone()))
        .merge(configuration::router(pool.clone()))
        .merge(availability::router(pool))
        .merge(handler::routes(profile_repo))
}
