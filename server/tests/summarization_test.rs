//! Integration tests for consultation summarization endpoints.
//!
//! Uses testcontainers for PostgreSQL and JSON fixtures for test data.
//! Tests scenarios from qa/SummaryNote.feature
//!
//! Test coverage:
//! - GET /consultation/v1/summarization/:appointment_id
//!   - PendingRecord (no draft exists)
//!   - SummarizationRecord with Draft status
//!   - SummarizationRecord with Submitted status
//! - POST /consultation/v1/summarization/draft
//!   - Success (save draft)
//!   - AlreadySubmitted (cannot save after submission)
//!   - Partial saves (only summary_note, only prescription, only follow_up)
//! - POST /consultation/v1/summarization/submit
//!   - Success
//!   - AlreadySubmitted (re-submit)
//!   - Validation errors (missing required fields)

use async_trait::async_trait;
use axum::http::{HeaderName, StatusCode};
use axum::middleware::{self, Next};
use axum::{extract::Request, response::Response, routing::get, routing::post, Router};
use axum_test::TestServer;
use sqlx::PgPool;
use std::sync::Arc;

use server::core::error::AppResult;
use server::core::RequestId;
use server::module::consultation::summarization::{
    ConsultationSummarizationServiceStub, FollowUpReservationRepo, JadeServiceStub,
    OverlappingTimeslot, ReservedTimeslot, SummarizationEncryptor, SummarizationPublisher,
    SummarizationRepoPsql, SummarizationService, SummarizationState,
};
use tdh_protocol::biz_apm::consultation_event::ConsultationEvent;

mod common;
use common::setup_postgres;

fn auth_header_name() -> HeaderName {
    HeaderName::from_static("tdh-sec-iam-user-identity")
}

async fn test_request_id_middleware(mut request: Request, next: Next) -> Response {
    request
        .extensions_mut()
        .insert(RequestId("test-request-id".to_string()));
    next.run(request).await
}

const TEST_PASETO_KEY: &str = "0102030405060708091011121314151617181920212223242526272829303132";

fn doctor_identity(account_id: i32, profile_id: i32) -> String {
    serde_json::json!({
        "accountId": account_id,
        "accountType": 2,
        "userProfileId": profile_id,
        "userMainProfileId": profile_id,
        "tenantId": 1
    })
    .to_string()
}

struct SummarizationPublisherStub;

#[async_trait]
impl SummarizationPublisher for SummarizationPublisherStub {
    async fn publish_event(&self, _topic: &str, _event: &ConsultationEvent) -> AppResult<()> {
        Ok(())
    }
}

struct FollowUpReservationRepoStub;

#[async_trait]
impl FollowUpReservationRepo for FollowUpReservationRepoStub {
    async fn find_overlapping_timeslots(
        &self,
        _doctor_id: i32,
        _start_time: i64,
        _end_time: i64,
    ) -> AppResult<Vec<OverlappingTimeslot>> {
        Ok(vec![])
    }

    async fn reserve_follow_up(
        &self,
        _doctor_id: i32,
        _start_time: i64,
        _end_time: i64,
    ) -> AppResult<ReservedTimeslot> {
        Err(server::core::error::AppError::BadRequest(
            "No available timeslot found".to_string(),
        ))
    }

    async fn get_reserved_follow_up(
        &self,
        _appointment_id: &str,
    ) -> AppResult<Option<ReservedTimeslot>> {
        Ok(None)
    }
}

fn load_fixture(path: &str) -> serde_json::Value {
    let content =
        std::fs::read_to_string(format!("tests/fixtures/summarization/{}", path)).unwrap();
    serde_json::from_str(&content).unwrap()
}

fn create_test_server(pool: PgPool) -> TestServer {
    let encryptor = Arc::new(SummarizationEncryptor::from_hex(TEST_PASETO_KEY).unwrap());
    let summarization_repo = Arc::new(SummarizationRepoPsql::new(pool));
    let jade_service = Arc::new(JadeServiceStub);
    let consultation_service = Arc::new(ConsultationSummarizationServiceStub);
    let publisher = Arc::new(SummarizationPublisherStub);
    let follow_up_repo: Arc<dyn FollowUpReservationRepo> = Arc::new(FollowUpReservationRepoStub);

    let summarization_service = Arc::new(SummarizationService::new(
        summarization_repo,
        encryptor,
        jade_service,
        consultation_service,
        publisher,
        "consultations".to_string(),
        follow_up_repo,
    ));

    let state = SummarizationState {
        service: summarization_service,
    };

    let router = Router::new()
        .route(
            "/consultation/v1/summarization/{appointment_id}",
            get(server::module::consultation::summarization::handler::get_summary_note),
        )
        .route(
            "/consultation/v1/summarization/draft",
            post(server::module::consultation::summarization::handler::save_draft),
        )
        .route(
            "/consultation/v1/summarization/submit",
            post(server::module::consultation::summarization::handler::submit),
        )
        .with_state(state)
        .layer(middleware::from_fn(test_request_id_middleware));

    TestServer::new(router).unwrap()
}

// ============================================================================
// GET /consultation/v1/summarization/:appointment_id Tests
// ============================================================================

#[tokio::test]
async fn get_summarization_pending_record() {
    let (_container, pool) = setup_postgres().await;
    let server = create_test_server(pool);

    let response = server
        .get("/consultation/v1/summarization/APT-NOT-EXISTS")
        .add_header(auth_header_name(), doctor_identity(123, 100))
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);

    let expected = load_fixture("responses/pending_record.json");
    let actual: serde_json::Value = response.json();
    assert_eq!(actual["__type"], expected["__type"]);
}

#[tokio::test]
async fn get_summarization_existing_draft() {
    let (_container, pool) = setup_postgres().await;
    let server = create_test_server(pool);

    let draft_request = load_fixture("requests/save_draft_full.json");
    let save_response = server
        .post("/consultation/v1/summarization/draft")
        .add_header(auth_header_name(), doctor_identity(123, 100))
        .json(&draft_request)
        .await;

    assert_eq!(save_response.status_code(), StatusCode::OK);

    let response = server
        .get("/consultation/v1/summarization/APT-001")
        .add_header(auth_header_name(), doctor_identity(123, 100))
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);

    let actual: serde_json::Value = response.json();
    assert_eq!(actual["__type"], "SummarizationRecord");
    assert_eq!(actual["status"], "Draft");
    assert_eq!(actual["doctorAccountId"], 123);
    assert_eq!(actual["appointmentId"], "APT-001");

    assert!(actual["summaryNote"].is_object());
    assert_eq!(
        actual["summaryNote"]["chiefComplaint"],
        draft_request["summaryNote"]["chiefComplaint"]
    );
}

#[tokio::test]
async fn get_summarization_already_submitted() {
    let (_container, pool) = setup_postgres().await;
    let server = create_test_server(pool);

    let submit_request = load_fixture("requests/submit_full.json");
    let submit_response = server
        .post("/consultation/v1/summarization/submit")
        .add_header(auth_header_name(), doctor_identity(123, 100))
        .json(&submit_request)
        .await;

    assert_eq!(submit_response.status_code(), StatusCode::OK);

    let response = server
        .get("/consultation/v1/summarization/APT-001")
        .add_header(auth_header_name(), doctor_identity(123, 100))
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);

    let actual: serde_json::Value = response.json();
    assert_eq!(actual["__type"], "SummarizationRecord");
    assert_eq!(actual["status"], "Submitted");
    assert_eq!(actual["doctorAccountId"], 123);
}

#[tokio::test]
async fn get_summarization_unauthorized_returns_pending() {
    let (_container, pool) = setup_postgres().await;
    let server = create_test_server(pool);

    let draft_request = load_fixture("requests/save_draft_full.json");
    let save_response = server
        .post("/consultation/v1/summarization/draft")
        .add_header(auth_header_name(), doctor_identity(123, 100))
        .json(&draft_request)
        .await;

    assert_eq!(save_response.status_code(), StatusCode::OK);

    let response = server
        .get("/consultation/v1/summarization/APT-001")
        .add_header(auth_header_name(), doctor_identity(456, 200))
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);

    let actual: serde_json::Value = response.json();
    assert_eq!(actual["__type"], "PendingRecord");
}

// ============================================================================
// POST /consultation/v1/summarization/draft Tests
// ============================================================================

#[tokio::test]
async fn save_draft_success() {
    let (_container, pool) = setup_postgres().await;
    let server = create_test_server(pool);

    let request = load_fixture("requests/save_draft_full.json");

    let response = server
        .post("/consultation/v1/summarization/draft")
        .add_header(auth_header_name(), doctor_identity(123, 100))
        .json(&request)
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);

    let expected = load_fixture("responses/save_success.json");
    let actual: serde_json::Value = response.json();
    assert_eq!(actual["__type"], expected["__type"]);
}

#[tokio::test]
async fn save_draft_already_submitted() {
    let (_container, pool) = setup_postgres().await;
    let server = create_test_server(pool);

    let submit_request = load_fixture("requests/submit_full.json");
    let submit_response = server
        .post("/consultation/v1/summarization/submit")
        .add_header(auth_header_name(), doctor_identity(123, 100))
        .json(&submit_request)
        .await;

    assert_eq!(submit_response.status_code(), StatusCode::OK);

    let draft_request = load_fixture("requests/save_draft_full.json");
    let response = server
        .post("/consultation/v1/summarization/draft")
        .add_header(auth_header_name(), doctor_identity(123, 100))
        .json(&draft_request)
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);

    let expected = load_fixture("responses/save_already_submitted.json");
    let actual: serde_json::Value = response.json();
    assert_eq!(actual["__type"], expected["__type"]);
}

#[tokio::test]
async fn save_draft_unauthorized() {
    let (_container, pool) = setup_postgres().await;
    let server = create_test_server(pool);

    let draft_request = load_fixture("requests/save_draft_full.json");
    let save_response = server
        .post("/consultation/v1/summarization/draft")
        .add_header(auth_header_name(), doctor_identity(123, 100))
        .json(&draft_request)
        .await;

    assert_eq!(save_response.status_code(), StatusCode::OK);

    let response = server
        .post("/consultation/v1/summarization/draft")
        .add_header(auth_header_name(), doctor_identity(456, 200))
        .json(&draft_request)
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);

    let actual: serde_json::Value = response.json();
    assert_eq!(actual["__type"], "SaveDraftResult.Unauthorized");
}

#[tokio::test]
async fn save_partial_draft_summary_only() {
    let (_container, pool) = setup_postgres().await;
    let server = create_test_server(pool);

    let request = load_fixture("requests/save_draft_partial_summary_only.json");

    let response = server
        .post("/consultation/v1/summarization/draft")
        .add_header(auth_header_name(), doctor_identity(123, 100))
        .json(&request)
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);

    let actual: serde_json::Value = response.json();
    assert_eq!(actual["__type"], "SaveDraftResult.Success");

    let get_response = server
        .get("/consultation/v1/summarization/APT-002")
        .add_header(auth_header_name(), doctor_identity(123, 100))
        .await;

    let draft: serde_json::Value = get_response.json();
    assert_eq!(draft["__type"], "SummarizationRecord");
    assert!(draft["summaryNote"].is_object());
    assert!(draft["prescription"].is_null());
    assert!(draft["followUpInfo"].is_null());
}

#[tokio::test]
async fn save_partial_draft_prescription_only() {
    let (_container, pool) = setup_postgres().await;
    let server = create_test_server(pool);

    let request = load_fixture("requests/save_draft_partial_prescription_only.json");

    let response = server
        .post("/consultation/v1/summarization/draft")
        .add_header(auth_header_name(), doctor_identity(123, 100))
        .json(&request)
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);

    let actual: serde_json::Value = response.json();
    assert_eq!(actual["__type"], "SaveDraftResult.Success");

    let get_response = server
        .get("/consultation/v1/summarization/APT-003")
        .add_header(auth_header_name(), doctor_identity(123, 100))
        .await;

    let draft: serde_json::Value = get_response.json();
    assert_eq!(draft["__type"], "SummarizationRecord");
    assert!(draft["summaryNote"].is_null());
    assert!(draft["prescription"].is_object());
    assert!(draft["followUpInfo"].is_null());
}

#[tokio::test]
async fn save_partial_draft_followup_only() {
    let (_container, pool) = setup_postgres().await;
    let server = create_test_server(pool);

    let request = load_fixture("requests/save_draft_partial_followup_only.json");

    let response = server
        .post("/consultation/v1/summarization/draft")
        .add_header(auth_header_name(), doctor_identity(123, 100))
        .json(&request)
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);

    let actual: serde_json::Value = response.json();
    assert_eq!(actual["__type"], "SaveDraftResult.Success");

    let get_response = server
        .get("/consultation/v1/summarization/APT-004")
        .add_header(auth_header_name(), doctor_identity(123, 100))
        .await;

    let draft: serde_json::Value = get_response.json();
    assert_eq!(draft["__type"], "SummarizationRecord");
    assert!(draft["summaryNote"].is_null());
    assert!(draft["prescription"].is_null());
    assert!(draft["followUpInfo"].is_object());
}

#[tokio::test]
async fn save_draft_update_existing() {
    let (_container, pool) = setup_postgres().await;
    let server = create_test_server(pool);

    let initial_request = load_fixture("requests/save_draft_partial_summary_only.json");
    let mut initial_request = initial_request.as_object().unwrap().clone();
    initial_request.insert("appointmentId".to_string(), serde_json::json!("APT-UPDATE"));

    let response = server
        .post("/consultation/v1/summarization/draft")
        .add_header(auth_header_name(), doctor_identity(123, 100))
        .json(&serde_json::Value::Object(initial_request))
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);

    let full_request = load_fixture("requests/save_draft_full.json");
    let mut full_request = full_request.as_object().unwrap().clone();
    full_request.insert("appointmentId".to_string(), serde_json::json!("APT-UPDATE"));

    let response = server
        .post("/consultation/v1/summarization/draft")
        .add_header(auth_header_name(), doctor_identity(123, 100))
        .json(&serde_json::Value::Object(full_request))
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);

    let get_response = server
        .get("/consultation/v1/summarization/APT-UPDATE")
        .add_header(auth_header_name(), doctor_identity(123, 100))
        .await;

    let draft: serde_json::Value = get_response.json();
    assert_eq!(draft["__type"], "SummarizationRecord");
    assert!(draft["summaryNote"].is_object());
    assert!(draft["prescription"].is_object());
    assert!(draft["followUpInfo"].is_object());
}

// ============================================================================
// POST /consultation/v1/summarization/submit Tests
// ============================================================================

#[tokio::test]
async fn submit_success() {
    let (_container, pool) = setup_postgres().await;
    let server = create_test_server(pool);

    let request = load_fixture("requests/submit_full.json");

    let response = server
        .post("/consultation/v1/summarization/submit")
        .add_header(auth_header_name(), doctor_identity(123, 100))
        .json(&request)
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);

    let expected = load_fixture("responses/submit_success.json");
    let actual: serde_json::Value = response.json();
    assert_eq!(actual["__type"], expected["__type"]);

    let get_response = server
        .get("/consultation/v1/summarization/APT-001")
        .add_header(auth_header_name(), doctor_identity(123, 100))
        .await;

    let record: serde_json::Value = get_response.json();
    assert_eq!(record["__type"], "SummarizationRecord");
    assert_eq!(record["status"], "Submitted");
}

#[tokio::test]
async fn submit_already_submitted() {
    let (_container, pool) = setup_postgres().await;
    let server = create_test_server(pool);

    let request = load_fixture("requests/submit_full.json");

    let response1 = server
        .post("/consultation/v1/summarization/submit")
        .add_header(auth_header_name(), doctor_identity(123, 100))
        .json(&request)
        .await;

    assert_eq!(response1.status_code(), StatusCode::OK);

    let response2 = server
        .post("/consultation/v1/summarization/submit")
        .add_header(auth_header_name(), doctor_identity(123, 100))
        .json(&request)
        .await;

    assert_eq!(response2.status_code(), StatusCode::OK);

    let expected = load_fixture("responses/submit_already_submitted.json");
    let actual: serde_json::Value = response2.json();
    assert_eq!(actual["__type"], expected["__type"]);
}

#[tokio::test]
async fn submit_unauthorized() {
    let (_container, pool) = setup_postgres().await;
    let server = create_test_server(pool);

    let draft_request = load_fixture("requests/save_draft_full.json");
    let save_response = server
        .post("/consultation/v1/summarization/draft")
        .add_header(auth_header_name(), doctor_identity(123, 100))
        .json(&draft_request)
        .await;

    assert_eq!(save_response.status_code(), StatusCode::OK);

    let submit_request = load_fixture("requests/submit_full.json");
    let response = server
        .post("/consultation/v1/summarization/submit")
        .add_header(auth_header_name(), doctor_identity(456, 200))
        .json(&submit_request)
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);

    let actual: serde_json::Value = response.json();
    assert_eq!(actual["__type"], "SubmitResponse.Unauthorized");
}

#[tokio::test]
async fn submit_after_draft() {
    let (_container, pool) = setup_postgres().await;
    let server = create_test_server(pool);

    let draft_request = load_fixture("requests/save_draft_full.json");
    let draft_response = server
        .post("/consultation/v1/summarization/draft")
        .add_header(auth_header_name(), doctor_identity(123, 100))
        .json(&draft_request)
        .await;

    assert_eq!(draft_response.status_code(), StatusCode::OK);

    let submit_request = load_fixture("requests/submit_full.json");
    let response = server
        .post("/consultation/v1/summarization/submit")
        .add_header(auth_header_name(), doctor_identity(123, 100))
        .json(&submit_request)
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);

    let actual: serde_json::Value = response.json();
    assert_eq!(actual["__type"], "SubmitResponse.Success");
}

#[tokio::test]
async fn submit_missing_summary_note() {
    let (_container, pool) = setup_postgres().await;
    let server = create_test_server(pool);

    let request = load_fixture("requests/submit_missing_summary_note.json");

    let response = server
        .post("/consultation/v1/summarization/submit")
        .add_header(auth_header_name(), doctor_identity(123, 100))
        .json(&request)
        .await;

    assert_eq!(response.status_code(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn submit_missing_prescription() {
    let (_container, pool) = setup_postgres().await;
    let server = create_test_server(pool);

    let request = load_fixture("requests/submit_missing_prescription.json");

    let response = server
        .post("/consultation/v1/summarization/submit")
        .add_header(auth_header_name(), doctor_identity(123, 100))
        .json(&request)
        .await;

    assert_eq!(response.status_code(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn submit_missing_follow_up() {
    let (_container, pool) = setup_postgres().await;
    let server = create_test_server(pool);

    let request = load_fixture("requests/submit_missing_follow_up.json");

    let response = server
        .post("/consultation/v1/summarization/submit")
        .add_header(auth_header_name(), doctor_identity(123, 100))
        .json(&request)
        .await;

    assert_eq!(response.status_code(), StatusCode::UNPROCESSABLE_ENTITY);
}

// ============================================================================
// GET + SAVE round-trip tests (no external service calls)
// ============================================================================

#[tokio::test]
async fn save_draft_with_prescription_then_get_verifies_all_fields() {
    let (_container, pool) = setup_postgres().await;
    let server = create_test_server(pool);

    let request = load_fixture("requests/save_draft_with_prescription.json");

    let save_response = server
        .post("/consultation/v1/summarization/draft")
        .add_header(auth_header_name(), doctor_identity(123, 100))
        .json(&request)
        .await;

    assert_eq!(save_response.status_code(), StatusCode::OK);
    let save_result: serde_json::Value = save_response.json();
    assert_eq!(save_result["__type"], "SaveDraftResult.Success");

    let get_response = server
        .get("/consultation/v1/summarization/APT-RX-001")
        .add_header(auth_header_name(), doctor_identity(123, 100))
        .await;

    assert_eq!(get_response.status_code(), StatusCode::OK);

    let actual: serde_json::Value = get_response.json();
    assert_eq!(actual["__type"], "SummarizationRecord");
    assert_eq!(actual["status"], "Draft");
    assert_eq!(actual["appointmentId"], "APT-RX-001");
    assert_eq!(actual["doctorAccountId"], 123);

    // Verify summary note fields
    let note = &actual["summaryNote"];
    assert_eq!(note["chiefComplaint"], "Fever and body aches");
    assert_eq!(note["diagnosis"], "Viral fever");
    assert_eq!(note["recommendations"], "Rest and hydration");
    assert_eq!(
        note["presentIllness"],
        "Patient has fever for 2 days with body aches"
    );
    assert_eq!(note["icd10"][0]["code"], "A08.4");
    assert_eq!(note["illnessDuration"]["value"], 2);
    assert_eq!(note["illnessDuration"]["unit"], "days");

    // Verify prescription with drug allergies
    let rx = &actual["prescription"];
    assert_eq!(rx["drugAllergyInfo"]["__type"], "HasDrugAllergies");
    assert_eq!(
        rx["drugAllergyInfo"]["drugAllergies"][0]["displayText"],
        "Penicillin"
    );
    assert_eq!(
        rx["drugAllergyInfo"]["drugAllergies"][1]["displayText"],
        "Sulfa drugs"
    );

    // Verify prescription items
    assert_eq!(rx["prescriptionItems"]["__type"], "Prescription");
    let item = &rx["prescriptionItems"]["items"][0];
    assert_eq!(item["medicineId"], 1);
    assert_eq!(item["medicineName"], "Paracetamol 500mg");
    assert_eq!(item["dose"]["value"], 500);
    assert_eq!(item["dose"]["unit"], "mg");
    assert_eq!(item["quantity"], 10);
    assert_eq!(item["route"]["description"], "Oral");
    assert_eq!(item["frequency"]["description"], "3 times a day");
    assert_eq!(item["duration"]["value"], 3);
    assert_eq!(item["duration"]["unit"], "days");
    assert_eq!(item["remark"], "Maximum 4g per day");
    assert_eq!(
        item["noteToPatient"],
        "Take with food to avoid stomach upset"
    );

    // Verify follow-up is NoFollowUp
    let fu = &actual["followUpInfo"];
    assert_eq!(fu["__type"], "NoFollowUp");
}

#[tokio::test]
async fn save_draft_with_schedule_followup_then_get_verifies_all_fields() {
    let (_container, pool) = setup_postgres().await;
    let server = create_test_server(pool);

    let request = load_fixture("requests/save_draft_with_schedule_followup.json");

    let save_response = server
        .post("/consultation/v1/summarization/draft")
        .add_header(auth_header_name(), doctor_identity(123, 100))
        .json(&request)
        .await;

    assert_eq!(save_response.status_code(), StatusCode::OK);

    let get_response = server
        .get("/consultation/v1/summarization/APT-FU-001")
        .add_header(auth_header_name(), doctor_identity(123, 100))
        .await;

    assert_eq!(get_response.status_code(), StatusCode::OK);

    let actual: serde_json::Value = get_response.json();
    assert_eq!(actual["__type"], "SummarizationRecord");
    assert_eq!(actual["status"], "Draft");

    // Verify summary note
    let note = &actual["summaryNote"];
    assert_eq!(note["chiefComplaint"], "Hypertension follow-up");
    assert_eq!(note["diagnosis"], "Essential hypertension");
    assert_eq!(note["icd10"][0]["code"], "I10");
    assert_eq!(note["illnessDuration"]["value"], 30);
    assert_eq!(note["noteToStaff"], "Monitor blood pressure");

    // Verify prescription is NoPrescription
    let rx = &actual["prescription"];
    assert_eq!(rx["drugAllergyInfo"]["__type"], "NoDrugAllergies");
    assert_eq!(rx["prescriptionItems"]["__type"], "NoPrescription");

    // Verify ScheduleAppointment follow-up
    let fu = &actual["followUpInfo"];
    assert_eq!(fu["__type"], "ScheduleAppointment");
    assert_eq!(fu["followStartDatetime"], 1717430400);
    assert_eq!(fu["followEndDatetime"], 1717432200);
    assert_eq!(fu["visitTypes"], serde_json::json!(["FollowUp"]));
    assert_eq!(
        fu["noteToPatient"],
        "Please come back for follow-up in 2 weeks"
    );
    assert_eq!(fu["noteToStaff"], "Monitor blood pressure");
}

#[tokio::test]
async fn save_draft_complete_then_get_verifies_all_sections() {
    let (_container, pool) = setup_postgres().await;
    let server = create_test_server(pool);

    let request = load_fixture("requests/save_draft_complete.json");

    let save_response = server
        .post("/consultation/v1/summarization/draft")
        .add_header(auth_header_name(), doctor_identity(123, 100))
        .json(&request)
        .await;

    assert_eq!(save_response.status_code(), StatusCode::OK);

    let get_response = server
        .get("/consultation/v1/summarization/APT-COMPLETE-001")
        .add_header(auth_header_name(), doctor_identity(123, 100))
        .await;

    assert_eq!(get_response.status_code(), StatusCode::OK);

    let actual: serde_json::Value = get_response.json();
    assert_eq!(actual["__type"], "SummarizationRecord");
    assert_eq!(actual["status"], "Draft");
    assert_eq!(actual["doctorAccountId"], 123);

    // Verify summary note with multiple ICD-10 codes
    let note = &actual["summaryNote"];
    assert_eq!(note["chiefComplaint"], "Severe sore throat with fever");
    assert_eq!(note["diagnosis"], "Acute pharyngitis - bacterial");
    assert_eq!(note["icd10"].as_array().unwrap().len(), 2);
    assert_eq!(note["icd10"][0]["code"], "J02.9");
    assert_eq!(note["icd10"][1]["code"], "R50.9");
    assert_eq!(
        note["noteToStaff"],
        "Patient has known penicillin allergy - flagged in system"
    );

    // Verify prescription with multiple items
    let rx = &actual["prescription"];
    assert_eq!(rx["drugAllergyInfo"]["__type"], "HasDrugAllergies");
    assert_eq!(
        rx["drugAllergyInfo"]["drugAllergies"][0]["displayText"],
        "Penicillin"
    );

    let items = rx["prescriptionItems"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["medicineName"], "Amoxicillin 500mg");
    assert_eq!(items[0]["quantity"], 21);
    assert_eq!(
        items[0]["cautions"],
        "Allergic to penicillin - do not prescribe"
    );
    assert_eq!(items[1]["medicineName"], "Ibuprofen 400mg");
    assert_eq!(items[1]["quantity"], 10);
    assert_eq!(items[1]["cautions"], "Avoid on empty stomach");

    // Verify ScheduleAppointment follow-up
    let fu = &actual["followUpInfo"];
    assert_eq!(fu["__type"], "ScheduleAppointment");
    assert_eq!(fu["visitTypes"], serde_json::json!(["FollowUp"]));
    assert_eq!(fu["followStartDatetime"], 1718035200);
    assert_eq!(fu["followEndDatetime"], 1718037000);
    assert_eq!(
        fu["noteToPatient"],
        "Schedule follow-up consultation in 1 week"
    );
    assert_eq!(fu["noteToStaff"], "Review lab results before next visit");
}

#[tokio::test]
async fn save_draft_then_update_then_get_reflects_latest() {
    let (_container, pool) = setup_postgres().await;
    let server = create_test_server(pool);

    // First save: summary only
    let initial = serde_json::json!({
        "appointmentId": "APT-EVOLVE",
        "summaryNote": {
            "chiefComplaint": "Initial complaint",
            "presentIllness": "Initial illness",
            "diagnosis": "Initial diagnosis",
            "recommendations": "Initial recs",
            "icd10": [],
            "illnessDuration": { "value": 1, "unit": "days" },
            "noteToStaff": null
        }
    });

    let r = server
        .post("/consultation/v1/summarization/draft")
        .add_header(auth_header_name(), doctor_identity(123, 100))
        .json(&initial)
        .await;
    assert_eq!(r.status_code(), StatusCode::OK);

    // Second save: full data replaces the draft
    let updated = load_fixture("requests/save_draft_complete.json");
    let mut updated = updated.as_object().unwrap().clone();
    updated.insert("appointmentId".to_string(), serde_json::json!("APT-EVOLVE"));

    let r = server
        .post("/consultation/v1/summarization/draft")
        .add_header(auth_header_name(), doctor_identity(123, 100))
        .json(&serde_json::Value::Object(updated))
        .await;
    assert_eq!(r.status_code(), StatusCode::OK);

    // GET should reflect the second (updated) save
    let get_response = server
        .get("/consultation/v1/summarization/APT-EVOLVE")
        .add_header(auth_header_name(), doctor_identity(123, 100))
        .await;

    let actual: serde_json::Value = get_response.json();
    assert_eq!(actual["__type"], "SummarizationRecord");
    assert_eq!(
        actual["summaryNote"]["chiefComplaint"],
        "Severe sore throat with fever"
    );
    assert!(
        actual["prescription"]["prescriptionItems"]["items"]
            .as_array()
            .unwrap()
            .len()
            == 2
    );
    assert_eq!(actual["followUpInfo"]["__type"], "ScheduleAppointment");
}

#[tokio::test]
async fn get_nonexistent_appointment_returns_pending_record() {
    let (_container, pool) = setup_postgres().await;
    let server = create_test_server(pool);

    let response = server
        .get("/consultation/v1/summarization/DOES-NOT-EXIST")
        .add_header(auth_header_name(), doctor_identity(999, 500))
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);

    let actual: serde_json::Value = response.json();
    assert_eq!(actual["__type"], "PendingRecord");
    // PendingRecord should have no other fields beyond __type
    assert!(actual.as_object().unwrap().len() == 1);
}

// ============================================================================
// Fixture Loading Tests (verify fixtures are valid JSON)
// ============================================================================

#[test]
fn fixtures_load_correctly() {
    let _ = load_fixture("requests/save_draft_full.json");
    let _ = load_fixture("requests/save_draft_partial_summary_only.json");
    let _ = load_fixture("requests/save_draft_partial_prescription_only.json");
    let _ = load_fixture("requests/save_draft_partial_followup_only.json");
    let _ = load_fixture("requests/save_draft_with_prescription.json");
    let _ = load_fixture("requests/save_draft_with_schedule_followup.json");
    let _ = load_fixture("requests/save_draft_complete.json");
    let _ = load_fixture("requests/submit_full.json");
    let _ = load_fixture("requests/submit_missing_summary_note.json");
    let _ = load_fixture("requests/submit_missing_prescription.json");
    let _ = load_fixture("requests/submit_missing_follow_up.json");

    let _ = load_fixture("responses/pending_record.json");
    let _ = load_fixture("responses/existing_draft.json");
    let _ = load_fixture("responses/existing_submitted.json");
    let _ = load_fixture("responses/save_success.json");
    let _ = load_fixture("responses/save_already_submitted.json");
    let _ = load_fixture("responses/submit_success.json");
    let _ = load_fixture("responses/submit_already_submitted.json");
}
