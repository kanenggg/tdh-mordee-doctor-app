pub mod cache;
pub mod handlers;
pub mod language;
pub mod models;
pub mod privilege;
pub mod repo;
#[allow(dead_code)]
pub mod subscriber;

use axum::{routing::get, Router};
use std::sync::Arc;

pub use cache::RankingCacheTrait;
pub use privilege::PrivilegeServiceTrait;
pub use repo::RankingRepoTrait;

pub fn router(
    repo: Arc<dyn RankingRepoTrait>,
    cache: Arc<dyn RankingCacheTrait>,
    privilege_svc: Arc<dyn PrivilegeServiceTrait>,
) -> Router {
    let state = handlers::RankingState {
        repo,
        cache,
        privilege_svc,
    };

    Router::new()
        .route("/doctors/instant", get(handlers::list_instant_doctors))
        .route("/doctors/scheduled", get(handlers::list_scheduled_doctors))
        .route("/doctor/{doctor_uuid}", get(handlers::get_doctor_profile))
        .with_state(state)
}
