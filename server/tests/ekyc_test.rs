//! Integration tests for `GET /appointment/v1/{bookingId}/ekyc`.
//!
//! Uses `axum-test` for the HTTP layer, `wiremock` for the eagle eKYC
//! upstream, and a hand-rolled mock `ConsultationClientTrait` for the
//! consultation lookup.

use std::sync::Arc;

use async_trait::async_trait;
use axum::Router;
use axum_test::TestServer;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use server::core::error::{AppError, AppResult};
use server::core::gcs_signed_url::GcsSignedUrlGenerator;
use server::module::appointment::external::consultation_client::{
    ConsultationAppointmentTime, ConsultationDetail, ConsultationIdentity, ConsultationLookup,
    ConsultationPrescreen,
};
use server::module::appointment::external::ConsultationClientTrait;
use server::module::ekyc;
use server::module::ekyc::service::{EkycClient, EkycService, EkycServiceTrait};

const PATIENT_ID: i32 = 42;
const BOOKING_ID: &str = "260511ABC";

// ----------------------------------------------------------------------------
// Mock ConsultationClient
// ----------------------------------------------------------------------------

struct MockConsultation {
    result: ConsultationLookup,
}

struct MockGcsSigner {
    fail: bool,
}

#[async_trait]
impl GcsSignedUrlGenerator for MockGcsSigner {
    async fn generate_signed_url(&self, source_url: &str) -> AppResult<String> {
        if self.fail {
            return Err(AppError::UpstreamError("signer failed".to_string()));
        }
        Ok(format!("signed:{source_url}"))
    }
}

#[async_trait]
impl ConsultationClientTrait for MockConsultation {
    async fn get_appointment(&self, booking_id: &str) -> AppResult<ConsultationLookup> {
        assert_eq!(booking_id, BOOKING_ID);
        match &self.result {
            ConsultationLookup::Found(detail) => {
                assert_eq!(detail.booking_id, BOOKING_ID);
                Ok(ConsultationLookup::Found(ConsultationDetail {
                    booking_id: booking_id.to_string(),
                    appointment_time: ConsultationAppointmentTime {
                        start_time: detail.appointment_time.start_time,
                        end_time: detail.appointment_time.end_time,
                    },
                    status: detail.status.clone(),
                    booking_type: detail.booking_type.clone(),
                    consultation_channel: detail.consultation_channel.clone(),
                    patient: ConsultationIdentity {
                        account_id: detail.patient.account_id,
                        profile_id: detail.patient.profile_id,
                    },
                    doctor: ConsultationIdentity {
                        account_id: detail.doctor.account_id,
                        profile_id: detail.doctor.profile_id,
                    },
                    prescreen: ConsultationPrescreen {
                        symptom: detail.prescreen.symptom.clone(),
                        duration: detail.prescreen.duration,
                        duration_unit: detail.prescreen.duration_unit.clone(),
                        attachments: detail.prescreen.attachments.clone(),
                        allergies: detail.prescreen.allergies.clone(),
                    },
                    payment_tx_id: detail.payment_tx_id,
                    payment_tx_ref_id: detail.payment_tx_ref_id.clone(),
                }))
            }
            ConsultationLookup::NotFound => Ok(ConsultationLookup::NotFound),
        }
    }
}

fn fixture_consultation() -> ConsultationDetail {
    ConsultationDetail {
        booking_id: BOOKING_ID.to_string(),
        appointment_time: ConsultationAppointmentTime {
            start_time: 1645940400,
            end_time: 1645941300,
        },
        status: "Booked".to_string(),
        booking_type: "Schedule".to_string(),
        consultation_channel: "Video".to_string(),
        patient: ConsultationIdentity {
            account_id: PATIENT_ID,
            profile_id: 200,
        },
        doctor: ConsultationIdentity {
            account_id: 555,
            profile_id: 400,
        },
        prescreen: ConsultationPrescreen {
            symptom: "rash".to_string(),
            duration: 7,
            duration_unit: "day".to_string(),
            attachments: vec![],
            allergies: vec![],
        },
        payment_tx_id: 0,
        payment_tx_ref_id: String::new(),
    }
}

fn doctor_identity_header(doctor_id: i32) -> String {
    json!({
        "accountId": doctor_id,
        "accountType": 2,
        "userProfileId": 1,
        "userMainProfileId": 1,
        "tenantId": 1,
    })
    .to_string()
}

fn build_server(service: Arc<dyn EkycServiceTrait>) -> TestServer {
    let app = Router::new().nest("/appointment/v1", ekyc::router_with_service(service));
    TestServer::new(app).unwrap()
}

fn build_service(
    consultation: Arc<dyn ConsultationClientTrait>,
    eagle_base_uri: String,
) -> Arc<dyn EkycServiceTrait> {
    build_service_with_signer(
        consultation,
        eagle_base_uri,
        Arc::new(MockGcsSigner { fail: false }),
    )
}

fn build_service_with_signer(
    consultation: Arc<dyn ConsultationClientTrait>,
    eagle_base_uri: String,
    signer: Arc<dyn GcsSignedUrlGenerator>,
) -> Arc<dyn EkycServiceTrait> {
    Arc::new(EkycService::new(
        consultation,
        Arc::new(EkycClient::new(eagle_base_uri)),
        signer,
    ))
}

// ----------------------------------------------------------------------------
// Tests
// ----------------------------------------------------------------------------

#[tokio::test]
async fn happy_path_returns_ekyc_available() {
    let eagle = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/internal/v1/user/{}", PATIENT_ID)))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "__type": "validKycUserInfo",
            "ekycSessionResult": {
                "documentImageUrl": "https://example.com/doc.png",
                "selfieImageUrl": "https://example.com/selfie.png",
                "firstName": "Somchai",
                "lastName": "Jaidee",
                "birthDate": "1999-04-27",
                "gender": "M"
            }
        })))
        .mount(&eagle)
        .await;

    let consultation = Arc::new(MockConsultation {
        result: ConsultationLookup::Found(fixture_consultation()),
    });
    let service = build_service(consultation, eagle.uri());
    let server = build_server(service);

    let resp = server
        .get(&format!("/appointment/v1/{}/ekyc", BOOKING_ID))
        .add_header("tdh-sec-iam-user-identity", doctor_identity_header(555))
        .await;

    resp.assert_status_ok();
    let body: serde_json::Value = resp.json();
    assert_eq!(body["__type"], "EkycAvailable");
    assert_eq!(
        body["documentImageUrl"],
        "signed:https://example.com/doc.png"
    );
    assert_eq!(
        body["livenessImageUrl"],
        "signed:https://example.com/selfie.png"
    );
    assert_eq!(body["fullName"], "Somchai Jaidee");
    assert_eq!(body["birthDate"], "1999-04-27");
    assert_eq!(body["gender"], "M");
}

#[tokio::test]
async fn no_ekyc_returns_ekyc_not_available() {
    let eagle = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/internal/v1/user/{}", PATIENT_ID)))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "__type": "noKycUserInfo" })),
        )
        .mount(&eagle)
        .await;

    let consultation = Arc::new(MockConsultation {
        result: ConsultationLookup::Found(fixture_consultation()),
    });
    let service = build_service(consultation, eagle.uri());
    let server = build_server(service);

    let resp = server
        .get(&format!("/appointment/v1/{}/ekyc", BOOKING_ID))
        .add_header("tdh-sec-iam-user-identity", doctor_identity_header(555))
        .await;

    resp.assert_status_ok();
    let body: serde_json::Value = resp.json();
    assert_eq!(body["__type"], "EkycNotAvailable");
}

#[tokio::test]
async fn missing_appointment_returns_appointment_not_found() {
    let eagle = MockServer::start().await;

    let consultation = Arc::new(MockConsultation {
        result: ConsultationLookup::NotFound,
    });
    let service = build_service(consultation, eagle.uri());
    let server = build_server(service);

    let resp = server
        .get(&format!("/appointment/v1/{}/ekyc", BOOKING_ID))
        .add_header("tdh-sec-iam-user-identity", doctor_identity_header(555))
        .await;

    resp.assert_status_ok();
    let body: serde_json::Value = resp.json();
    assert_eq!(body["__type"], "AppointmentNotFound");
}

#[tokio::test]
async fn eagle_failure_returns_502() {
    let eagle = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/internal/v1/user/{}", PATIENT_ID)))
        .respond_with(ResponseTemplate::new(503))
        .mount(&eagle)
        .await;

    let consultation = Arc::new(MockConsultation {
        result: ConsultationLookup::Found(fixture_consultation()),
    });
    let service = build_service(consultation, eagle.uri());
    let server = build_server(service);

    let resp = server
        .get(&format!("/appointment/v1/{}/ekyc", BOOKING_ID))
        .add_header("tdh-sec-iam-user-identity", doctor_identity_header(555))
        .await;

    resp.assert_status(axum::http::StatusCode::BAD_GATEWAY);
}

#[tokio::test]
async fn signer_failure_returns_502() {
    let eagle = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/internal/v1/user/{}", PATIENT_ID)))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "__type": "validKycUserInfo",
            "ekycSessionResult": {
                "documentImageUrl": "gs://private-bucket/doc.png",
                "selfieImageUrl": "gs://private-bucket/selfie.png"
            }
        })))
        .mount(&eagle)
        .await;

    let consultation = Arc::new(MockConsultation {
        result: ConsultationLookup::Found(fixture_consultation()),
    });
    let service = build_service_with_signer(
        consultation,
        eagle.uri(),
        Arc::new(MockGcsSigner { fail: true }),
    );
    let server = build_server(service);

    let resp = server
        .get(&format!("/appointment/v1/{}/ekyc", BOOKING_ID))
        .add_header("tdh-sec-iam-user-identity", doctor_identity_header(555))
        .await;

    resp.assert_status(axum::http::StatusCode::BAD_GATEWAY);
}
