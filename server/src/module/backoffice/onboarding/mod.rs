pub mod handler;
pub mod outbox_delivery;
pub mod pending;
pub mod repo;
pub mod service;

use crate::module::backoffice::onboarding::outbox_delivery::ImmediateDoctorProfileDelivery;
use crate::module::webhook::PubsubPublisher;
use axum::{routing::post, Router};
pub use handler::BackofficeOnboardingState;
pub use repo::BackofficeOnboardingRepo;
pub use service::BackofficeOnboardingService;
use std::sync::Arc;

pub struct OnboardingRouters {
    pub backoffice: Router,
    pub internal: Router,
}

pub fn router(pool: sqlx::PgPool, publisher: Arc<PubsubPublisher>, topic: String) -> Router {
    let immediate_delivery = Some(Arc::new(ImmediateDoctorProfileDelivery::new(
        pool.clone(),
        publisher,
        topic,
    )));
    let routers = routers(pool, immediate_delivery);
    routers.internal.merge(routers.backoffice)
}

pub fn routers(
    pool: sqlx::PgPool,
    immediate_delivery: Option<Arc<ImmediateDoctorProfileDelivery>>,
) -> OnboardingRouters {
    let pending_routes = pending::router(pool.clone());
    let repo = BackofficeOnboardingRepo::new(pool);
    let service = Arc::new(BackofficeOnboardingService::new(repo, immediate_delivery));
    let state = BackofficeOnboardingState { service };

    let onboarding_routes = Router::new()
        .route("/approve", post(handler::approve_doctor))
        .route("/reject", post(handler::reject_doctor))
        .with_state(state);

    OnboardingRouters {
        backoffice: Router::new().nest("/pending", pending_routes),
        internal: onboarding_routes,
    }
}
