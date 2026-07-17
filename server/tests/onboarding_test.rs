use axum::http::StatusCode;
use axum::Router;
use axum_test::TestServer;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tdh_protocol::common::Localized;

use server::core::error::AppResult;
use server::model::onboarding::{
    Address, Documents, EducationRequest, OnBoardingRequest, OnBoardingStatus, OnBoardingStub,
    SelectedWorkPlaceRequest, Specialty, Subspecialty,
};
use server::model::ref_data::{
    AcademicPosition, District, MedicalSchool, PostalCode, Profession, Province, SubDistrict,
    WorkPlace,
};
use server::module::onboarding;
use server::module::onboarding::repo::OnBoardingRepo;
use server::module::onboarding::service::OnboardingService;
use server::module::onboarding::validation::OnboardingValidatorImp;
use server::module::onboarding::OnboardingState;

// ============================================================================
// Auth header helper
// ============================================================================

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

// ============================================================================
// Mock OnBoardingRepo with in-memory storage
// ============================================================================

struct MockOnboardingRepo {
    store: Arc<std::sync::Mutex<HashMap<String, OnBoardingStub>>>,
}

impl MockOnboardingRepo {
    fn new() -> Self {
        Self {
            store: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }
}

#[async_trait::async_trait]
impl OnBoardingRepo for MockOnboardingRepo {
    async fn get_doctor_profile_draft(&self, doctor_id: i32) -> AppResult<Option<OnBoardingStub>> {
        let store = self.store.lock().unwrap();
        Ok(store.get(&doctor_id.to_string()).cloned())
    }

    async fn save_doctor_profile_draft(
        &self,
        doctor_account_id: i32,
        _doctor_profile_id: i32,
        request: &OnBoardingRequest,
    ) -> AppResult<()> {
        let mut store = self.store.lock().unwrap();
        let stub = OnBoardingStub::from(request.clone());
        store.insert(doctor_account_id.to_string(), stub);
        Ok(())
    }

    async fn submit_doctor_profile_draft(
        &self,
        doctor_account_id: i32,
        _doctor_profile_id: i32,
        request: &OnBoardingRequest,
    ) -> AppResult<()> {
        let mut store = self.store.lock().unwrap();
        let mut stub = OnBoardingStub::from(request.clone());
        stub.status = OnBoardingStatus::PendingApproval;
        store.insert(doctor_account_id.to_string(), stub);
        Ok(())
    }

    async fn get_onboarding_status(
        &self,
        doctor_account_id: i32,
    ) -> AppResult<Option<OnBoardingStatus>> {
        let store = self.store.lock().unwrap();
        Ok(store
            .get(&doctor_account_id.to_string())
            .map(|stub| stub.status.clone()))
    }
}

// ============================================================================
// Test helpers
// ============================================================================

fn localized(th: &str, en: &str) -> Localized {
    Localized {
        th: th.to_string(),
        en: en.to_string(),
    }
}

/// Input sent by the frontend for POST /onboarding/v1
fn create_valid_onboarding_request() -> OnBoardingRequest {
    let empty_loc = || Localized {
        th: String::new(),
        en: String::new(),
    };

    OnBoardingRequest {
        profession: Profession {
            id: 1,
            name: empty_loc(),
            abbr: empty_loc(),
        },
        academic_position: AcademicPosition {
            id: 1,
            name: empty_loc(),
            abbr: empty_loc(),
        },
        citizen_id: "1101700203450".to_string(),
        first_name: localized("จอห์น", "John"),
        last_name: localized("โด", "Doe"),
        address: Address {
            address_detail: "123 Main St".to_string(),
            sub_district: SubDistrict {
                id: 100105,
                name: empty_loc(),
                district_id: 1001,
                zip_code: String::new(),
            },
            district: District {
                id: 1001,
                name: empty_loc(),
                province_id: 1,
            },
            province: Province {
                id: 1,
                name: empty_loc(),
            },
            postal_code: PostalCode {
                id: 1,
                district_id: 1001,
            },
        },
        work_place: SelectedWorkPlaceRequest {
            primary: vec![WorkPlace {
                id: 1,
                name: empty_loc(),
            }],
            additional: vec![],
        },
        education: EducationRequest {
            license_number: "12345".to_string(),
            medical_school: MedicalSchool {
                id: 1,
                name: empty_loc(),
            },
            specialties: Specialty {
                id: 1,
                name: empty_loc(),
                subspecialty: Subspecialty {
                    id: 1,
                    name: empty_loc(),
                    medical_school: MedicalSchool {
                        id: 1,
                        name: empty_loc(),
                    },
                },
                medical_school: MedicalSchool {
                    id: 1,
                    name: empty_loc(),
                },
            },
            additional_specialties: vec![],
            special_interests: vec!["Anemia".to_string()],
        },
        documents: Documents {
            profile_image_url: "https://example.com/profile.jpg".to_string(),
            id_card_image_url: "https://example.com/id.jpg".to_string(),
            book_bank_image_url: "https://example.com/bank.jpg".to_string(),
            med_license_image_url: "https://example.com/license.jpg".to_string(),
            certificate_image_urls: vec![],
        },
    }
}

/// Stub used for direct validator tests — same data as the request, descriptions empty.
fn create_valid_onboarding_stub() -> OnBoardingStub {
    OnBoardingStub::from(create_valid_onboarding_request())
}

fn create_test_server() -> TestServer {
    let repo: Arc<dyn OnBoardingRepo> = Arc::new(MockOnboardingRepo::new());
    let service = Arc::new(OnboardingService::new(
        Arc::clone(&repo),
        OnboardingValidatorImp::new(),
    ));

    let state = OnboardingState { service };

    let app = Router::new()
        .route(
            "/",
            axum::routing::get(onboarding::handler::get_doctor_profile_draft)
                .post(onboarding::handler::save_doctor_profile_draft),
        )
        .route(
            "/submit",
            axum::routing::post(onboarding::handler::submit_doctor_profile_draft),
        )
        .with_state(state)
        // Mirror production wiring so the `RequestId` extension is present.
        .layer(axum::middleware::from_fn(
            server::core::gcp_logging_middleware,
        ));

    TestServer::new(app).unwrap()
}

// ============================================================================
// Tests
// ============================================================================

#[tokio::test]
async fn get_onboarding_returns_not_found_for_new_doctor() {
    let server = create_test_server();

    let response = server
        .get("/")
        .add_header(AUTH_HEADER, doctor_identity(123))
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);
    let body = response.text();
    assert!(body.contains("OnBoardingNotFound"));
}

#[tokio::test]
async fn post_saves_draft_onboarding() {
    let server = create_test_server();

    let response = server
        .post("/")
        .add_header(AUTH_HEADER, doctor_identity(123))
        .json(&create_valid_onboarding_request())
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);

    let get_response = server
        .get("/")
        .add_header(AUTH_HEADER, doctor_identity(123))
        .await;

    assert_eq!(get_response.status_code(), StatusCode::OK);
    let body = get_response.text();
    assert!(body.contains("OnBoardingResponse"));
    assert!(body.contains("John"));
}

#[tokio::test]
async fn save_always_sets_status_to_draft() {
    let server = create_test_server();

    server
        .post("/")
        .add_header(AUTH_HEADER, doctor_identity(123))
        .json(&create_valid_onboarding_request())
        .await;

    let get_response = server
        .get("/")
        .add_header(AUTH_HEADER, doctor_identity(123))
        .await;

    let body = get_response.text();
    assert!(body.contains("OnBoardingResponse"));
    assert!(body.contains(r#""__type":"Draft""#));
    assert!(!body.contains("Approved"));
}

#[tokio::test]
async fn post_ignores_extra_fields_like_description() {
    let server = create_test_server();

    // Simulate the old frontend shape that sends description as a plain string
    let payload = json!({
        "profession": { "id": 1, "description": "Doctor (Male)", "name": { "th": "", "en": "" }, "abbr": { "th": "", "en": "" } },
        "academicPosition": { "id": 1, "description": "Professor", "name": { "th": "", "en": "" }, "abbr": { "th": "", "en": "" } },
        "citizenId": "1234567890123",
        "firstName": { "th": "จอห์น", "en": "John" },
        "lastName": { "th": "โด", "en": "Doe" },
        "address": {
            "addressDetail": "123 Main St",
            "subDistrict": { "id": 100105, "name": { "th": "", "en": "" }, "districtId": 1001, "zipCode": "" },
            "district": { "id": 1001, "name": { "th": "", "en": "" }, "provinceId": 1 },
            "province": { "id": 1, "name": { "th": "", "en": "" } },
            "postalCode": { "id": 1, "description": "10200", "districtId": 1001 }
        },
        "workPlace": {
            "primary": [{ "id": 1, "description": "Asoke Skin Hospital", "name": { "th": "", "en": "" } }],
            "additional": []
        },
        "education": {
            "licenseNumber": "12345",
            "medicalSchool": { "id": 1, "description": "Thammasat University", "name": { "th": "", "en": "" } },
            "specialties": {
                "id": 1,
                "name": { "th": "", "en": "" },
                "subspecialty": { "id": 1, "name": { "th": "", "en": "" }, "medicalSchool": { "id": 1, "name": { "th": "", "en": "" } } },
                "medicalSchool": { "id": 1, "name": { "th": "", "en": "" } }
            },
            "additionalSpecialties": [],
            "specialInterests": ["Anemia"]
        },
        "documents": {
            "profileImageUrl": "https://example.com/profile.jpg",
            "idCardImageUrl": "https://example.com/id.jpg",
            "bookBankImageUrl": "https://example.com/bank.jpg",
            "medLicenseImageUrl": "https://example.com/license.jpg",
            "certificateImageUrls": []
        }
    });

    let response = server
        .post("/")
        .add_header(AUTH_HEADER, doctor_identity(123))
        .json(&payload)
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);
}

#[tokio::test]
async fn submit_changes_status_to_pending_approval() {
    let server = create_test_server();

    let response = server
        .post("/submit")
        .add_header(AUTH_HEADER, doctor_identity(123))
        .json(&create_valid_onboarding_request())
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);

    let get_response = server
        .get("/")
        .add_header(AUTH_HEADER, doctor_identity(123))
        .await;

    let body = get_response.text();
    assert!(body.contains(r#""__type":"PendingApproval""#));
}

#[tokio::test]
async fn submit_validates_required_fields_before_saving() {
    let server = create_test_server();

    // Empty patch: all fields default — citizen_id is "" (not 13 chars), fails validation
    let response = server
        .post("/submit")
        .add_header(AUTH_HEADER, doctor_identity(999))
        .json(&json!({}))
        .await;

    assert_eq!(response.status_code(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn submit_validates_required_documents() {
    let server = create_test_server();
    let mut input = create_valid_onboarding_request();
    input.documents.profile_image_url = String::new();

    let response = server
        .post("/submit")
        .add_header(AUTH_HEADER, doctor_identity(123))
        .json(&input)
        .await;

    assert_eq!(response.status_code(), StatusCode::BAD_REQUEST);
    let body: serde_json::Value = response.json();
    assert_eq!(body["error"], "profile_image_url is required");
    assert_eq!(body["field"], "profile_image_url");
}

#[tokio::test]
async fn validation_requires_all_documents() {
    let validator = OnboardingValidatorImp::new();
    let mut info = create_valid_onboarding_stub();

    info.documents.profile_image_url = String::new();
    assert!(validator.validate_onboarding_submission(&info).is_err());

    info.documents.profile_image_url = "https://example.com/profile.jpg".to_string();
    info.documents.id_card_image_url = String::new();
    assert!(validator.validate_onboarding_submission(&info).is_err());
}

#[tokio::test]
async fn validation_limits_certificates_to_six() {
    let validator = OnboardingValidatorImp::new();
    let mut info = create_valid_onboarding_stub();

    info.documents.certificate_image_urls = vec!["url".to_string(); 7];
    assert!(validator.validate_onboarding_submission(&info).is_err());
}

#[tokio::test]
async fn validation_allows_six_certificates() {
    let validator = OnboardingValidatorImp::new();
    let mut info = create_valid_onboarding_stub();

    info.documents.certificate_image_urls = vec!["url".to_string(); 6];
    assert!(validator.validate_onboarding_submission(&info).is_ok());
}

#[tokio::test]
async fn status_transition_from_approved_to_draft_is_invalid() {
    let validator = OnboardingValidatorImp::new();

    assert!(validator
        .validate_onboarding_status_transition(
            &OnBoardingStatus::Approved,
            &OnBoardingStatus::Draft
        )
        .is_err());
}

#[tokio::test]
async fn partial_first_save_only_citizen_id_uses_defaults_for_rest() {
    let server = create_test_server();

    let partial_patch = json!({ "citizenId": "9876543210987" });

    let post_response = server
        .post("/")
        .add_header(AUTH_HEADER, doctor_identity(456))
        .json(&partial_patch)
        .await;

    assert_eq!(post_response.status_code(), StatusCode::OK);

    let get_response = server
        .get("/")
        .add_header(AUTH_HEADER, doctor_identity(456))
        .await;

    assert_eq!(get_response.status_code(), StatusCode::OK);
    let body = get_response.text();
    assert!(body.contains("OnBoardingResponse"));
    assert!(body.contains("9876543210987"));
}

#[tokio::test]
async fn omitted_address_section_saves_with_default_address() {
    let server = create_test_server();

    let patch = json!({
        "citizenId": "1111111111111",
        "firstName": { "th": "สมชาย", "en": "Somchai" },
        "lastName": { "th": "ใจดี", "en": "Jaidee" }
    });

    let post_response = server
        .post("/")
        .add_header(AUTH_HEADER, doctor_identity(789))
        .json(&patch)
        .await;

    assert_eq!(post_response.status_code(), StatusCode::OK);

    let get_response = server
        .get("/")
        .add_header(AUTH_HEADER, doctor_identity(789))
        .await;

    assert_eq!(get_response.status_code(), StatusCode::OK);
    let body = get_response.text();
    assert!(body.contains("OnBoardingResponse"));
    assert!(body.contains("Somchai"));
}

#[tokio::test]
async fn citizen_id_must_be_13_digits() {
    let validator = OnboardingValidatorImp::new();

    // Valid: exactly 13 ASCII digits → passes the full submission.
    let ok = create_valid_onboarding_stub();
    assert!(validator.validate_onboarding_submission(&ok).is_ok());

    // Too short.
    let mut short = create_valid_onboarding_stub();
    short.citizen_id = "123456789012".to_string();
    assert!(validator.validate_onboarding_submission(&short).is_err());

    // Right length but contains a non-digit.
    let mut non_digit = create_valid_onboarding_stub();
    non_digit.citizen_id = "11017002034X0".to_string();
    assert!(validator
        .validate_onboarding_submission(&non_digit)
        .is_err());
}
