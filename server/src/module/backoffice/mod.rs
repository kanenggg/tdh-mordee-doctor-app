pub mod consultation_configuration;
pub mod doctor_directory;
pub mod doctor_management;
pub mod handlers;
pub mod onboarding;
pub mod repo;

use crate::module::backoffice::onboarding::outbox_delivery::ImmediateDoctorProfileDelivery;
use crate::module::webhook::PubsubPublisher;
use crate::repo::firestore_repo::FirestoreRepo;
use axum::{routing::get, Router};
use sqlx::PgPool;
use std::sync::Arc;

pub struct BackofficeRouters {
    pub backoffice: Router,
    pub internal_doctor_management: Router,
    pub internal_doctor_directory: Router,
    pub internal_onboarding: Router,
}

pub fn routers(
    firestore: FirestoreRepo,
    collection: String,
    pool: PgPool,
    publisher: Arc<PubsubPublisher>,
    doctor_profile_approved_topic: String,
    doctor_profile_status_updated_topic: String,
    doctor_profile_immediate_delivery_enabled: bool,
) -> BackofficeRouters {
    let backoffice_repo: Arc<repo::BackofficeRepo> = Arc::new(repo::BackofficeRepo::new(
        firestore.clone(),
        collection.clone(),
    ));

    let doctor_directory_router = doctor_directory::router(pool.clone());
    let approved_immediate_delivery = doctor_profile_immediate_delivery_enabled.then(|| {
        Arc::new(ImmediateDoctorProfileDelivery::new(
            pool.clone(),
            publisher.clone(),
            doctor_profile_approved_topic,
        ))
    });
    let status_updated_immediate_delivery = doctor_profile_immediate_delivery_enabled.then(|| {
        Arc::new(ImmediateDoctorProfileDelivery::new(
            pool.clone(),
            publisher,
            doctor_profile_status_updated_topic,
        ))
    });
    let doctor_management_router = doctor_management::router(
        pool.clone(),
        approved_immediate_delivery.clone(),
        status_updated_immediate_delivery,
    );
    let onboarding_routers = onboarding::routers(pool, approved_immediate_delivery);

    compose_routers(
        doctor_routes(backoffice_repo),
        doctor_management_router,
        onboarding_routers.internal,
        onboarding_routers.backoffice,
        doctor_directory_router,
    )
}

fn compose_routers(
    doctor_router: Router,
    doctor_management_router: Router,
    internal_onboarding_router: Router,
    backoffice_onboarding_router: Router,
    internal_doctor_directory: Router,
) -> BackofficeRouters {
    BackofficeRouters {
        backoffice: Router::new()
            .nest("/doctors", doctor_router)
            .nest("/onboarding", backoffice_onboarding_router),
        internal_doctor_management: doctor_management_router,
        internal_doctor_directory,
        internal_onboarding: internal_onboarding_router,
    }
}

fn doctor_routes(repo: Arc<repo::BackofficeRepo>) -> Router {
    Router::new()
        .route(
            "/",
            get(handlers::list_doctors).post(handlers::create_doctor),
        )
        .route(
            "/{id}",
            get(handlers::get_doctor).put(handlers::update_doctor),
        )
        .with_state(repo)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{http::StatusCode, routing::get};
    use axum_test::TestServer;

    async fn doctor_stub() -> StatusCode {
        StatusCode::OK
    }

    async fn onboarding_stub() -> StatusCode {
        StatusCode::NO_CONTENT
    }

    async fn doctor_management_stub() -> StatusCode {
        StatusCode::ACCEPTED
    }

    #[tokio::test]
    async fn contexts_mount_only_their_owned_routes() {
        let route_groups = compose_routers(
            Router::new().route("/", get(doctor_stub)),
            Router::new()
                .route("/consultation-configuration", get(doctor_management_stub))
                .route("/doctor-active-status", get(doctor_management_stub)),
            Router::new()
                .route("/approve", get(onboarding_stub))
                .route("/reject", get(onboarding_stub)),
            Router::new().route("/pending", get(onboarding_stub)),
            Router::new().route("/", get(doctor_stub)),
        );
        let app = Router::new()
            .nest("/backoffice/v1", route_groups.backoffice)
            .nest("/internal/v1", route_groups.internal_doctor_management)
            .nest(
                "/internal/v1/doctors",
                route_groups.internal_doctor_directory,
            )
            .nest("/internal/v1/onboarding", route_groups.internal_onboarding);
        let server = TestServer::new(app).unwrap();

        server
            .get("/backoffice/v1/doctors")
            .await
            .assert_status(StatusCode::OK);
        server
            .get("/internal/v1/onboarding/approve")
            .await
            .assert_status(StatusCode::NO_CONTENT);
        server
            .get("/internal/v1/onboarding/reject")
            .await
            .assert_status(StatusCode::NO_CONTENT);
        server
            .get("/internal/onboarding/v1/approve")
            .await
            .assert_status(StatusCode::NOT_FOUND);
        server
            .get("/internal/onboarding/v1/reject")
            .await
            .assert_status(StatusCode::NOT_FOUND);
        server
            .get("/internal/v1/consultation-configuration")
            .await
            .assert_status(StatusCode::ACCEPTED);
        server
            .get("/internal/v1/doctor-active-status")
            .await
            .assert_status(StatusCode::ACCEPTED);
        server
            .get("/internal/v1/doctor-status")
            .await
            .assert_status(StatusCode::NOT_FOUND);
        server
            .get("/internal/v1/deactivate")
            .await
            .assert_status(StatusCode::NOT_FOUND);
        server
            .get("/backoffice/v1/consultation-configuration")
            .await
            .assert_status(StatusCode::NOT_FOUND);
        server
            .get("/backoffice/v1/deactivate")
            .await
            .assert_status(StatusCode::NOT_FOUND);
        server
            .get("/internal/onboarding/v1/consultation-configuration")
            .await
            .assert_status(StatusCode::NOT_FOUND);
        server
            .get("/internal/onboarding/v1/deactivate")
            .await
            .assert_status(StatusCode::NOT_FOUND);
        server
            .get("/internal/onboarding/v1/pending")
            .await
            .assert_status(StatusCode::NOT_FOUND);
        server
            .get("/backoffice/v1/onboarding/pending")
            .await
            .assert_status(StatusCode::NO_CONTENT);
        server
            .get("/internal/v1/doctors")
            .await
            .assert_status(StatusCode::OK);
        server
            .get("/internal/onboarding/v1/doctors")
            .await
            .assert_status(StatusCode::NOT_FOUND);
    }
}
