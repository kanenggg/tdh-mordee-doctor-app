pub mod handler;
pub mod repo;
pub mod service;

use crate::module::backoffice::onboarding::outbox_delivery::ImmediateDoctorProfileDelivery;
use axum::{routing::patch, Router};
use sqlx::PgPool;
use std::sync::Arc;

pub fn router(
    pool: PgPool,
    approved_immediate_delivery: Option<Arc<ImmediateDoctorProfileDelivery>>,
    status_updated_immediate_delivery: Option<Arc<ImmediateDoctorProfileDelivery>>,
) -> Router {
    let repo = repo::DoctorManagementRepo::new(pool);
    let service = Arc::new(service::DoctorManagementService::new(
        repo,
        approved_immediate_delivery,
        status_updated_immediate_delivery,
    ));
    let state = handler::DoctorManagementState { service };

    Router::new()
        .route(
            "/consultation-configuration",
            patch(handler::update_consultation_configuration),
        )
        .route(
            "/doctor-active-status",
            patch(handler::update_doctor_active_status),
        )
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use axum_test::TestServer;
    use sqlx::postgres::PgPoolOptions;

    #[tokio::test]
    async fn router_exposes_doctor_active_status_and_removes_old_routes() {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost/test")
            .unwrap();
        let server = TestServer::new(router(pool, None, None)).unwrap();

        server
            .patch("/doctor-active-status")
            .await
            .assert_status(StatusCode::INTERNAL_SERVER_ERROR);
        server
            .patch("/doctor-status")
            .await
            .assert_status(StatusCode::NOT_FOUND);
        server
            .post("/deactivate")
            .await
            .assert_status(StatusCode::NOT_FOUND);
    }

    #[test]
    fn exposes_active_status_handler_name() {
        let _handler = handler::update_doctor_active_status;
    }
}
