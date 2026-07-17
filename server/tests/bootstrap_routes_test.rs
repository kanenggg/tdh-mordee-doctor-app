use axum::{
    http::StatusCode,
    routing::{get, patch, post},
    Router,
};
use axum_test::TestServer;
use server::bootstrap::{build_app, AppRouters};
use utoipa::OpenApi;

async fn approve_stub() -> StatusCode {
    StatusCode::NO_CONTENT
}

async fn consultation_configuration_stub() -> StatusCode {
    StatusCode::ACCEPTED
}

async fn update_doctor_active_status_stub() -> StatusCode {
    StatusCode::NO_CONTENT
}

async fn pending_stub() -> StatusCode {
    StatusCode::OK
}

async fn approved_doctor_directory_stub() -> StatusCode {
    StatusCode::OK
}

fn app_routers(
    backoffice: Router,
    internal_doctor_management: Router,
    internal_doctor_directory: Router,
    internal_onboarding: Router,
) -> AppRouters {
    AppRouters {
        notification: Router::new(),
        consultation: Router::new(),
        ranking: Router::new(),
        profile: Router::new(),
        timeslot: Router::new(),
        appointment: Router::new(),
        onboarding: Router::new(),
        backoffice,
        internal_doctor_management,
        internal_doctor_directory,
        internal_onboarding,
        ehr: Router::new(),
    }
}

#[tokio::test]
async fn contexts_mount_only_their_owned_routes() {
    let onboarding = Router::new()
        .route("/approve", post(approve_stub))
        .route("/reject", post(approve_stub));
    let doctor_management = Router::new()
        .route(
            "/consultation-configuration",
            patch(consultation_configuration_stub),
        )
        .route(
            "/doctor-active-status",
            patch(update_doctor_active_status_stub),
        );
    let backoffice = Router::new().nest(
        "/onboarding",
        Router::new().route("/pending", get(pending_stub)),
    );
    let server = TestServer::new(build_app(app_routers(
        backoffice,
        doctor_management,
        Router::new(),
        onboarding,
    )))
    .unwrap();

    server
        .post("/internal/v1/onboarding/approve")
        .await
        .assert_status(StatusCode::NO_CONTENT);
    server
        .post("/internal/onboarding/v1/approve")
        .await
        .assert_status(StatusCode::NOT_FOUND);
    server
        .post("/backoffice/v1/onboarding/approve")
        .await
        .assert_status(StatusCode::NOT_FOUND);

    server
        .post("/internal/v1/onboarding/reject")
        .await
        .assert_status(StatusCode::NO_CONTENT);
    server
        .post("/internal/onboarding/v1/reject")
        .await
        .assert_status(StatusCode::NOT_FOUND);

    server
        .patch("/internal/v1/consultation-configuration")
        .await
        .assert_status(StatusCode::ACCEPTED);
    server
        .patch("/backoffice/v1/consultation-configuration")
        .await
        .assert_status(StatusCode::NOT_FOUND);
    server
        .patch("/internal/onboarding/v1/consultation-configuration")
        .await
        .assert_status(StatusCode::NOT_FOUND);
    server
        .patch("/internal/v1/doctor-active-status")
        .await
        .assert_status(StatusCode::NO_CONTENT);
    server
        .patch("/internal/v1/doctor-status")
        .await
        .assert_status(StatusCode::NOT_FOUND);
    server
        .post("/internal/v1/deactivate")
        .await
        .assert_status(StatusCode::NOT_FOUND);
    server
        .post("/backoffice/v1/deactivate")
        .await
        .assert_status(StatusCode::NOT_FOUND);
    server
        .post("/internal/onboarding/v1/deactivate")
        .await
        .assert_status(StatusCode::NOT_FOUND);

    server
        .get("/internal/onboarding/v1/pending")
        .await
        .assert_status(StatusCode::NOT_FOUND);
    server
        .get("/backoffice/v1/onboarding/pending")
        .await
        .assert_status(StatusCode::OK);
}

#[tokio::test]
async fn approved_doctor_directory_is_available_under_internal_v1() {
    let directory = Router::new()
        .route("/", get(approved_doctor_directory_stub))
        .route("/{doctor_account_id}", get(approved_doctor_directory_stub));
    let server = TestServer::new(build_app(app_routers(
        Router::new(),
        Router::new(),
        directory,
        Router::new(),
    )))
    .unwrap();

    server
        .get("/internal/v1/doctors")
        .await
        .assert_status(StatusCode::OK);
    server
        .get("/internal/v1/doctors/42")
        .await
        .assert_status(StatusCode::OK);
}

#[test]
fn openapi_documents_each_context_at_its_own_prefix() {
    let document = server::openapi::ApiDoc::openapi();

    for path in [
        "/internal/v1/onboarding/approve",
        "/internal/v1/onboarding/reject",
        "/internal/v1/consultation-configuration",
        "/internal/v1/doctor-active-status",
        "/backoffice/v1/onboarding/pending",
        "/backoffice/v1/onboarding/pending/{doctor_account_id}",
        "/internal/v1/doctors",
        "/internal/v1/doctors/{doctor_account_id}",
    ] {
        assert!(document.paths.paths.contains_key(path), "missing {path}");
    }

    let active_status = document
        .paths
        .paths
        .get("/internal/v1/doctor-active-status")
        .and_then(|item| item.patch.as_ref())
        .expect("doctor active-status PATCH operation");
    assert!(
        !active_status.responses.responses.contains_key("401"),
        "internal doctor active-status must not document backoffice authentication"
    );

    for path in [
        "/internal/onboarding/v1/approve",
        "/internal/onboarding/v1/reject",
        "/internal/onboarding/v1/consultation-configuration",
        "/internal/onboarding/v1/deactivate",
        "/internal/v1/deactivate",
        "/internal/v1/doctor-status",
        "/backoffice/v1/consultation-configuration",
        "/backoffice/v1/deactivate",
        "/backoffice/v1/onboarding/approve",
        "/backoffice/v1/onboarding/reject",
    ] {
        assert!(
            !document.paths.paths.contains_key(path),
            "obsolete path remains: {path}"
        );
    }

    for path in [
        "/internal/onboarding/v1/pending",
        "/internal/onboarding/v1/pending/{doctor_account_id}",
    ] {
        assert!(
            !document.paths.paths.contains_key(path),
            "pending path incorrectly moved: {path}"
        );
    }
}
