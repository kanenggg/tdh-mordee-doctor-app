use axum::http::StatusCode;
use axum::Router;
use axum_test::TestServer;
use serde_json::{json, Value};
use std::sync::Arc;

use server::core::error::AppResult;
use server::model::onboarding::{OnBoardingRequest, OnBoardingStatus, OnBoardingStub};
use server::module::onboarding;
use server::module::onboarding::repo::OnBoardingRepo;
use server::module::onboarding::service::OnboardingService;
use server::module::onboarding::validation::OnboardingValidatorImp;
use server::module::onboarding::OnboardingState;

const AUTH_HEADER: &str = "tdh-sec-iam-user-identity";

fn doctor_identity(account_id: i32) -> String {
    json!({
        "accountId": account_id,
        "accountType": 2,
        "userProfileId": 100,
        "userMainProfileId": 100,
        "tenantId": 1
    })
    .to_string()
}

/// Mock whose only meaningful method is `get_onboarding_status`, returning a preconfigured
/// value. The other trait methods are unused stubs for this endpoint.
struct MockStatusRepo {
    status: Option<OnBoardingStatus>,
}

#[async_trait::async_trait]
impl OnBoardingRepo for MockStatusRepo {
    async fn get_doctor_profile_draft(&self, _doctor_id: i32) -> AppResult<Option<OnBoardingStub>> {
        Ok(None)
    }
    async fn save_doctor_profile_draft(
        &self,
        _doctor_account_id: i32,
        _doctor_profile_id: i32,
        _request: &OnBoardingRequest,
    ) -> AppResult<()> {
        Ok(())
    }
    async fn submit_doctor_profile_draft(
        &self,
        _doctor_account_id: i32,
        _doctor_profile_id: i32,
        _request: &OnBoardingRequest,
    ) -> AppResult<()> {
        Ok(())
    }
    async fn get_onboarding_status(
        &self,
        _doctor_account_id: i32,
    ) -> AppResult<Option<OnBoardingStatus>> {
        Ok(self.status.clone())
    }
}

fn create_test_server(status: Option<OnBoardingStatus>) -> TestServer {
    let repo: Arc<dyn OnBoardingRepo> = Arc::new(MockStatusRepo { status });
    let service = Arc::new(OnboardingService::new(
        Arc::clone(&repo),
        OnboardingValidatorImp::new(),
    ));
    let state = OnboardingState { service };
    let app = Router::new()
        .route(
            "/status",
            axum::routing::get(onboarding::handler::get_onboarding_status),
        )
        .with_state(state)
        // Mirror production wiring so the `RequestId` extension is present.
        .layer(axum::middleware::from_fn(
            server::core::gcp_logging_middleware,
        ));
    TestServer::new(app).unwrap()
}

#[tokio::test]
async fn returns_approved_status() {
    let server = create_test_server(Some(OnBoardingStatus::Approved));
    let response = server
        .get("/status")
        .add_header(AUTH_HEADER, doctor_identity(123))
        .await;
    assert_eq!(response.status_code(), StatusCode::OK);
    assert_eq!(response.json::<Value>(), json!({ "__type": "Approved" }));
}

#[tokio::test]
async fn returns_pending_approval_status() {
    let server = create_test_server(Some(OnBoardingStatus::PendingApproval));
    let response = server
        .get("/status")
        .add_header(AUTH_HEADER, doctor_identity(123))
        .await;
    assert_eq!(
        response.json::<Value>(),
        json!({ "__type": "PendingApproval" })
    );
}

#[tokio::test]
async fn rejected_status_includes_status_reason() {
    let server = create_test_server(Some(OnBoardingStatus::Rejected {
        reason: "documents unclear".to_string(),
    }));
    let response = server
        .get("/status")
        .add_header(AUTH_HEADER, doctor_identity(123))
        .await;
    assert_eq!(
        response.json::<Value>(),
        json!({ "__type": "Rejected", "status_reason": "documents unclear" })
    );
}

#[tokio::test]
async fn deactivated_status_includes_status_reason() {
    let server = create_test_server(Some(OnBoardingStatus::Deactivated {
        reason: "license expired".to_string(),
    }));
    let response = server
        .get("/status")
        .add_header(AUTH_HEADER, doctor_identity(123))
        .await;
    assert_eq!(
        response.json::<Value>(),
        json!({ "__type": "Deactivated", "status_reason": "license expired" })
    );
}

#[tokio::test]
async fn returns_not_found_when_no_draft() {
    let server = create_test_server(None);
    let response = server
        .get("/status")
        .add_header(AUTH_HEADER, doctor_identity(404))
        .await;
    assert_eq!(response.status_code(), StatusCode::OK);
    assert_eq!(
        response.json::<Value>(),
        json!({ "__type": "OnBoardingNotFound" })
    );
}

#[tokio::test]
async fn missing_auth_returns_401() {
    let server = create_test_server(Some(OnBoardingStatus::Approved));
    let response = server.get("/status").await;
    assert_eq!(response.status_code(), StatusCode::UNAUTHORIZED);
}
