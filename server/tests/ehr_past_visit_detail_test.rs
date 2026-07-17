//! Integration test for `GET /past-visit/{bookingId}`.
//!
//! Wiremocks both APM and Jade; injects a fake doctor repo so the test
//! exercises the gateway+mapping flow without standing up Postgres.

use std::sync::Arc;

use async_trait::async_trait;
use axum::{middleware, Router};
use axum_test::TestServer;
use serde_json::json;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use server::core::error::AppResult;
use server::core::RequestId;
use server::model::doctor_specialty::{MedicalSchool, Specialty};
use server::model::localize::Localized;
use server::module::ehr::past_visit_detail::repo::{DoctorBasicInfo, DoctorBasicRepoTrait};
use server::module::ehr::past_visit_detail::{
    routes, JadePolicy, PastVisitDetailService, PastVisitGateway,
};

const BOOKING_ID: &str = "BK-12345";
const DOCTOR_ACCOUNT_ID: i32 = 555;

struct StubDoctorRepo {
    info: Option<DoctorBasicInfo>,
}

#[async_trait]
impl DoctorBasicRepoTrait for StubDoctorRepo {
    async fn get_doctor_basic(&self, doctor_account_id: i32) -> AppResult<Option<DoctorBasicInfo>> {
        assert_eq!(doctor_account_id, DOCTOR_ACCOUNT_ID);
        Ok(self.info.clone())
    }
}

async fn inject_request_id(
    mut req: axum::extract::Request,
    next: middleware::Next,
) -> axum::response::Response {
    req.extensions_mut()
        .insert(RequestId("test-req-id".to_string()));
    next.run(req).await
}

fn fast_policy() -> JadePolicy {
    JadePolicy {
        max_attempts: 1,
        base_backoff_ms: 0,
        max_backoff_ms: 0,
        per_attempt_timeout_secs: 2,
    }
}

fn doctor_info() -> DoctorBasicInfo {
    DoctorBasicInfo {
        first_name: Localized {
            th: "ธนวัฒน์".into(),
            en: "Thanawat".into(),
        },
        last_name: Localized {
            th: "สุขเกษม".into(),
            en: "Sukgasem".into(),
        },
        specialty: Some(Specialty {
            id: 7,
            name: Localized {
                th: "ผิวหนัง".into(),
                en: "Dermatology".into(),
            },
            subspecialty: None,
            medical_school: MedicalSchool {
                id: 1,
                name: Localized {
                    th: "จุฬาฯ".into(),
                    en: "Chulalongkorn".into(),
                },
            },
        }),
    }
}

fn apm_success_body() -> serde_json::Value {
    json!({
        "__type": "Success",
        "bookingId": BOOKING_ID,
        "appointmentTime": { "startTime": 1_700_000_000, "endTime": 1_700_000_900 },
        "consultationChannel": "video",
        "doctor": {
            "doctorId": 1,
            "doctorAccountId": DOCTOR_ACCOUNT_ID,
            "doctorProfileId": 2
        },
        "summaryNote": {
            "presentIllness": "rash",
            "chiefComplaint": "itchy skin",
            "diagnosis": "contact dermatitis",
            "recommendations": "avoid allergen",
            "icd10": [{ "code": "L23", "description": "Allergic contact dermatitis" }],
            "drugAllergies": null,
            "illnessDuration": { "value": 3, "unit": "day" },
            "noteToStaff": "",
            "prescriptionId": 9001
        },
        "followUp": { "__type": "AsNeeded" }
    })
}

fn jade_prescription_body() -> serde_json::Value {
    json!({
        "__type": "Prescription",
        "prescriptionId": 9001,
        "bookingId": BOOKING_ID,
        "bizUnitId": 1,
        "bizCenterId": 1,
        "doctorId": "1",
        "patientId": 100,
        "prescriptionCode": "RX-0001",
        "prescriptionExpiredAt": null,
        "acknowledgeAllergy": true,
        "allergies": [],
        "totalAmount": 250.0,
        "items": [{
            "prescriptionItemId": 1,
            "medicineId": 42,
            "pricePlanId": 1,
            "medicineName": "Hydrocortisone 1% Cream 30g",
            "dosage": 1,
            "dose": null,
            "route": null,
            "frequency": null,
            "duration": 7,
            "durationUnit": null,
            "indication": null,
            "mealInstruction": {
                "id": 11,
                "description": "Usually applied once or twice daily unless otherwise directed by a doctor."
            },
            "container": { "id": 5, "description": "Box" },
            "caution": null,
            "remark": null,
            "patientDetail": null,
            "unitCost": 250.0,
            "totalAmount": 250.0
        }]
    })
}

#[tokio::test]
async fn happy_path_returns_mapped_past_visit_detail() {
    let apm = MockServer::start().await;
    let jade = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path(format!(
            "/internal/v1/appointment/{}/consultation-summary",
            BOOKING_ID
        )))
        .and(header("X-Request-Id", "test-req-id"))
        .respond_with(ResponseTemplate::new(200).set_body_json(apm_success_body()))
        .mount(&apm)
        .await;

    Mock::given(method("GET"))
        .and(path(format!("/prescription/{}", BOOKING_ID)))
        .and(header("X-Request-Id", "test-req-id"))
        .respond_with(ResponseTemplate::new(200).set_body_json(jade_prescription_body()))
        .mount(&jade)
        .await;

    let gateway = Arc::new(PastVisitGateway::new(apm.uri(), jade.uri(), fast_policy()));
    let doctor_repo: Arc<dyn DoctorBasicRepoTrait> = Arc::new(StubDoctorRepo {
        info: Some(doctor_info()),
    });
    let service = Arc::new(PastVisitDetailService::new(gateway, doctor_repo));

    let app: Router = routes(service).layer(middleware::from_fn(inject_request_id));
    let server = TestServer::new(app).unwrap();

    let resp = server.get(&format!("/past-visit/{}", BOOKING_ID)).await;

    resp.assert_status_ok();
    let body: serde_json::Value = resp.json();

    assert_eq!(body["__type"], "PastVisitDetail");
    assert_eq!(body["appointmentId"], BOOKING_ID);
    assert_eq!(body["consultationChannel"], "Video");
    assert_eq!(body["doctor"]["name"], "Dr. Thanawat Sukgasem");
    assert_eq!(body["doctor"]["specialties"][0], "Dermatology");
    assert_eq!(body["summaryNote"]["diagnosis"], "contact dermatitis");
    assert_eq!(body["followUp"]["__type"], "AsNeeded");

    let items = body["prescriptionItems"].as_array().expect("items array");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["medId"], 42);
    assert_eq!(items[0]["name"], "Hydrocortisone 1% Cream 30g");
    assert_eq!(items[0]["quantity"], 1);
    assert_eq!(items[0]["unit"], "Box");
    assert_eq!(
        items[0]["dosageInstructions"],
        "Usually applied once or twice daily unless otherwise directed by a doctor."
    );
}
