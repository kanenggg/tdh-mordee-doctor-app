use axum::http::StatusCode;
use axum::Router;
use axum_test::TestServer;
use serde_json::{json, Value};

mod common;
use common::setup_postgres;

fn backoffice_identity(account_id: i32) -> String {
    json!({
        "accountId": account_id,
        "accountType": 4,
        "userProfileId": 456,
        "userMainProfileId": 456,
        "tenantId": 1
    })
    .to_string()
}

fn doctor_identity(account_id: i32) -> String {
    json!({
        "accountId": account_id,
        "accountType": 2,
        "userProfileId": 456,
        "userMainProfileId": 456,
        "tenantId": 1
    })
    .to_string()
}

fn pending_app(pool: sqlx::PgPool) -> TestServer {
    let app = Router::new().nest(
        "/pending",
        server::module::backoffice::onboarding::pending::router(pool),
    );
    TestServer::new(app).unwrap()
}

async fn seed_draft(pool: &sqlx::PgPool, account_id: i32, profile_id: i32, status: &str) {
    sqlx::query(
        r#"INSERT INTO doctor_profile_draft
           (doctor_account_id, doctor_profile_id, citizen_id, profession, academic_position,
            first_name, last_name, license_number, primary_medical_school, specialty,
            additional_specialties, special_interest, address_detail, sub_district, district,
            province, postal_code, work_place, additional_workplace, profile_image_url,
            id_card_image_url, book_bank_image_url, medical_license_image_url,
            education_license_image_url, status, created_at)
           VALUES ($1, $2, '1234567890123', '[{"locale":"en","value":"General Practice"}]'::jsonb,
            '[{"locale":"en","value":"Attending"}]'::jsonb,
            '{"th":"สมชาย","en":"Somchai"}'::jsonb,
            '{"th":"ใจดี","en":"Jaidee"}'::jsonb,
            'LIC-123', '[{"locale":"en","value":"Chula"}]'::jsonb,
            '{"id": 10, "name": {"en":"Family Medicine", "th":"เวชศาสตร์ครอบครัว"}}'::jsonb,
            '[{"id": 11, "name": {"en":"Diabetes", "th":"เบาหวาน"}}]'::jsonb,
            ARRAY['telehealth']::text[], '123 Safe St',
            '{"id": 1, "name": {"en":"Sub", "th":"แขวง"}}'::jsonb,
            '{"id": 2, "name": {"en":"District", "th":"เขต"}}'::jsonb,
            '{"id": 3, "name": {"en":"Bangkok", "th":"กรุงเทพ"}}'::jsonb,
            10110,
            '[{"id": 7, "name": {"en":"Hospital", "th":"โรงพยาบาล"}}]'::jsonb,
            '[]'::jsonb,
            'https://cdn.example/profile.jpg',
            'gs://private/id-card.jpg', 'gs://private/book-bank.jpg', 'gs://private/license.jpg',
            ARRAY['gs://private/education-license.jpg']::text[], $3::doctor_profile_status_enum,
            now())"#,
    )
    .bind(account_id)
    .bind(profile_id)
    .bind(status)
    .execute(pool)
    .await
    .expect("seed draft");
}

#[tokio::test]
async fn list_pending_approvals_returns_only_pending_display_safe_summaries() {
    let (_pg, pool) = setup_postgres().await;
    seed_draft(&pool, 7001, 9701, "PendingApproval").await;
    seed_draft(&pool, 7002, 9702, "Draft").await;
    let server = pending_app(pool);

    let resp = server
        .get("/pending")
        .add_header("tdh-sec-iam-user-identity", backoffice_identity(9001))
        .await;

    resp.assert_status_ok();
    let body: Value = resp.json();
    assert_eq!(body["page"], 1);
    assert_eq!(body["limit"], 20);
    assert_eq!(body["data"].as_array().unwrap().len(), 1);
    assert_eq!(body["data"][0]["doctorAccountId"], 7001);
    assert_eq!(body["data"][0]["status"], "PendingApproval");
    assert_eq!(
        body["data"][0]["profileImageUrl"],
        "https://cdn.example/profile.jpg"
    );

    let raw = body.to_string();
    assert!(!raw.contains("1234567890123"));
    assert!(!raw.contains("citizenId"));
    assert!(!raw.contains("gs://private"));
    assert!(!raw.contains("idCardImageUrl"));
    assert!(!raw.contains("addressDetail"));
}

#[tokio::test]
async fn get_pending_approval_detail_returns_display_safe_fields_and_redaction_markers() {
    let (_pg, pool) = setup_postgres().await;
    seed_draft(&pool, 7101, 9801, "PendingApproval").await;
    let server = pending_app(pool);

    let resp = server
        .get("/pending/7101")
        .add_header("tdh-sec-iam-user-identity", backoffice_identity(9001))
        .await;

    resp.assert_status_ok();
    let body: Value = resp.json();
    assert_eq!(body["__type"], "PendingDoctorApproval");
    assert_eq!(body["doctorAccountId"], 7101);
    assert_eq!(body["licenseNumber"], "LIC-123");
    assert_eq!(body["address"]["postalCode"], 10110);
    assert_eq!(
        body["redactedFields"],
        json!([
            "citizenId",
            "idCardImageUrl",
            "bookBankImageUrl",
            "medicalLicenseImageUrl",
            "educationLicenseImageUrl"
        ])
    );

    let raw = body.to_string();
    assert!(!raw.contains("1234567890123"));
    assert!(!raw.contains("gs://private"));
    assert!(!raw.contains("citizenId\":"));
    assert!(!raw.contains("idCardImageUrl\":"));
    assert!(!raw.contains("bookBankImageUrl\":"));
}

#[tokio::test]
async fn get_pending_approval_detail_for_unknown_or_non_pending_profile_returns_typed_not_found() {
    let (_pg, pool) = setup_postgres().await;
    seed_draft(&pool, 7201, 9901, "Draft").await;
    let server = pending_app(pool);

    let non_pending = server
        .get("/pending/7201")
        .add_header("tdh-sec-iam-user-identity", backoffice_identity(9001))
        .await;
    non_pending.assert_status_ok();
    assert_eq!(
        non_pending.json::<Value>(),
        json!({ "__type": "PendingDoctorApprovalNotFound" })
    );

    let missing = server
        .get("/pending/999999")
        .add_header("tdh-sec-iam-user-identity", backoffice_identity(9001))
        .await;
    missing.assert_status_ok();
    assert_eq!(
        missing.json::<Value>(),
        json!({ "__type": "PendingDoctorApprovalNotFound" })
    );
}

#[tokio::test]
async fn pending_approval_endpoints_require_backoffice_identity() {
    let (_pg, pool) = setup_postgres().await;
    seed_draft(&pool, 7301, 9911, "PendingApproval").await;
    let server = pending_app(pool);

    let missing = server.get("/pending").await;
    assert_eq!(missing.status_code(), StatusCode::UNAUTHORIZED);

    let wrong_account_type = server
        .get("/pending/7301")
        .add_header("tdh-sec-iam-user-identity", doctor_identity(7301))
        .await;
    assert_eq!(wrong_account_type.status_code(), StatusCode::UNAUTHORIZED);
}
