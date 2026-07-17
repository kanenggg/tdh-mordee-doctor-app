use serde_json::Value;
use sqlx::PgPool;

use server::module::ehr::past_visit_detail::repo::{DoctorBasicRepo, DoctorBasicRepoTrait};

mod common;
use common::setup_postgres;

const FIXTURE_PATH: &str = "tests/fixtures/past_visit_detail/doctor_profile_basic.json";

fn load_fixture() -> Value {
    let raw = std::fs::read_to_string(FIXTURE_PATH)
        .unwrap_or_else(|e| panic!("failed to read fixture {}: {}", FIXTURE_PATH, e));
    serde_json::from_str(&raw).expect("fixture must be valid JSON")
}

async fn seed_doctor_profile(pool: &PgPool, fixture: &Value) {
    let doctor_account_id = fixture["doctorAccountId"].as_i64().unwrap() as i32;
    let doctor_profile_id = fixture["doctorProfileId"].as_i64().unwrap() as i32;
    let citizen_id = fixture["citizenId"].as_str().unwrap();

    sqlx::query(
        r#"
        INSERT INTO doctor_profile (
            doctor_id,
            doctor_account_id, doctor_profile_id, citizen_id,
            first_name, last_name, specialty,
            department_id, license_number, address_detail,
            sub_district, district, province, postal_code,
            profile_image_url, id_card_image_url, book_bank_image_url, medical_license_image_url,
            is_active
        ) VALUES (
            gen_random_uuid(),
            $1, $2, $3,
            $4, $5, $6,
            1, 'L-0001', 'Test Address',
            '{}'::jsonb, '{}'::jsonb, '{}'::jsonb, 10000,
            '', '', '', '',
            true
        )
        "#,
    )
    .bind(doctor_account_id)
    .bind(doctor_profile_id)
    .bind(citizen_id)
    .bind(&fixture["firstName"])
    .bind(&fixture["lastName"])
    .bind(&fixture["specialty"])
    .execute(pool)
    .await
    .expect("failed to seed doctor_profile row");
}

#[tokio::test]
async fn get_doctor_basic_returns_record_matching_fixture_contract() {
    let (_container, pool) = setup_postgres().await;
    let fixture = load_fixture();
    seed_doctor_profile(&pool, &fixture).await;

    let doctor_account_id = fixture["doctorAccountId"].as_i64().unwrap() as i32;
    let repo = DoctorBasicRepo::new(pool);
    let info = repo
        .get_doctor_basic(doctor_account_id)
        .await
        .expect("repo call should succeed")
        .expect("doctor row should exist");

    // Fixture contract: each field's JSON shape must round-trip exactly.
    assert_eq!(
        serde_json::to_value(&info.first_name).unwrap(),
        fixture["firstName"],
        "firstName JSON drifted from fixture",
    );
    assert_eq!(
        serde_json::to_value(&info.last_name).unwrap(),
        fixture["lastName"],
        "lastName JSON drifted from fixture",
    );
    assert_eq!(
        serde_json::to_value(&info.specialty).unwrap(),
        fixture["specialty"],
        "specialty JSON drifted from fixture",
    );
}

#[tokio::test]
async fn get_doctor_basic_returns_none_when_account_missing() {
    let (_container, pool) = setup_postgres().await;
    let repo = DoctorBasicRepo::new(pool);

    let result = repo
        .get_doctor_basic(424242)
        .await
        .expect("repo call should succeed");

    assert!(
        result.is_none(),
        "expected None for unknown doctor_account_id"
    );
}
