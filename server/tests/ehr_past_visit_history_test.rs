//! Integration test for `GET /past-visit` (patient past-visit history).
//!
//! Wiremocks the biz-apm consultation service (`GET /internal/v1/appointments`)
//! and asserts the gateway response is mapped into the `PastVisits` result.

use axum::{middleware, Router};
use axum_test::TestServer;
use serde_json::json;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use server::core::RequestId;
use server::module::ehr::past_visit_history;

const PATIENT_ACCOUNT_ID: i32 = 232;
const PATIENT_PROFILE_ID: i32 = 232;

async fn inject_request_id(
    mut req: axum::extract::Request,
    next: middleware::Next,
) -> axum::response::Response {
    req.extensions_mut()
        .insert(RequestId("test-req-id".to_string()));
    next.run(req).await
}

fn appointments_body() -> serde_json::Value {
    json!({
        "appointments": [
            {
                "bookingId": "BK-1001",
                "appointmentTime": { "startTime": 1_700_000_000_i64, "endTime": 1_700_000_900_i64 },
                "doctor": { "accountId": 901, "profileId": 801, "firstName": "Thanawat", "lastName": "Sukgasem" }
            },
            {
                "bookingId": "BK-1002",
                "appointmentTime": { "startTime": 1_700_100_000_i64, "endTime": 1_700_100_900_i64 },
                "doctor": { "accountId": 902, "profileId": 802, "firstName": "Somchai", "lastName": "Jaidee" }
            }
        ]
    })
}

async fn build_server(base_uri: String) -> TestServer {
    let app: Router =
        past_visit_history::router(base_uri).layer(middleware::from_fn(inject_request_id));
    TestServer::new(app).unwrap()
}

#[tokio::test]
async fn returns_mapped_past_visits() {
    let apm = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/internal/v1/appointments"))
        .and(header("X-Request-Id", "test-req-id"))
        .and(query_param(
            "patientAccountId",
            PATIENT_ACCOUNT_ID.to_string(),
        ))
        .and(query_param(
            "patientProfileId",
            PATIENT_PROFILE_ID.to_string(),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(appointments_body()))
        .mount(&apm)
        .await;

    let server = build_server(apm.uri()).await;

    let resp = server
        .get("/past-visit")
        .add_query_param("patientAccountId", PATIENT_ACCOUNT_ID)
        .add_query_param("patientProfileId", PATIENT_PROFILE_ID)
        .await;

    resp.assert_status_ok();
    let body: serde_json::Value = resp.json();

    assert_eq!(body["__type"], "PastVisits");
    let visits = body["pastVisits"].as_array().expect("pastVisits array");
    assert_eq!(visits.len(), 2);
    assert_eq!(visits[0]["bookingId"], "BK-1001");
    assert_eq!(visits[0]["consultationStartTime"], 1_700_000_000_i64);
    assert_eq!(visits[0]["consultationEndTime"], 1_700_000_900_i64);
    assert_eq!(visits[0]["doctorInfo"]["doctorName"], "Thanawat Sukgasem");
    assert_eq!(visits[1]["bookingId"], "BK-1002");
    assert_eq!(visits[1]["doctorInfo"]["doctorName"], "Somchai Jaidee");
}

#[tokio::test]
async fn maps_unauthorized_to_unauthorized_variant() {
    let apm = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/internal/v1/appointments"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&apm)
        .await;

    let server = build_server(apm.uri()).await;

    let resp = server
        .get("/past-visit")
        .add_query_param("patientAccountId", PATIENT_ACCOUNT_ID)
        .add_query_param("patientProfileId", PATIENT_PROFILE_ID)
        .await;

    resp.assert_status_ok();
    let body: serde_json::Value = resp.json();
    assert_eq!(body["__type"], "Unauthorized");
}

#[tokio::test]
async fn maps_not_found_to_not_found_variant() {
    let apm = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/internal/v1/appointments"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&apm)
        .await;

    let server = build_server(apm.uri()).await;

    let resp = server
        .get("/past-visit")
        .add_query_param("patientAccountId", PATIENT_ACCOUNT_ID)
        .add_query_param("patientProfileId", PATIENT_PROFILE_ID)
        .await;

    resp.assert_status_ok();
    let body: serde_json::Value = resp.json();
    assert_eq!(body["__type"], "NotFound");
}
