use axum::http::{HeaderName, StatusCode};
use axum::Router;
use axum_test::TestServer;
use serde_json::{json, Value};
use sqlx::PgPool;
use testcontainers::ContainerAsync;
use testcontainers_modules::postgres::Postgres;

mod common;
use common::setup_postgres;

fn auth_header_name() -> HeaderName {
    HeaderName::from_static("tdh-sec-iam-user-identity")
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

fn patient_identity(account_id: i32) -> String {
    json!({
        "accountId": account_id,
        "accountType": 1,
        "userProfileId": 456,
        "userMainProfileId": 456,
        "tenantId": 1
    })
    .to_string()
}

/// The checked-in migrations create a stale `doctor_profile` (keyed by
/// `doctor_account_id`, without the `doctor_id` UUID column). The live table has since
/// been migrated; recreate it here to match the production DDL so this test exercises
/// the real shape. The `channel_type_enum` / `language_code_enum` types already exist
/// from the base migration.
async fn setup() -> (ContainerAsync<Postgres>, PgPool) {
    let (container, pool) = setup_postgres().await;

    sqlx::raw_sql(
        r#"
        DROP TABLE IF EXISTS doctor_profile CASCADE;
        CREATE TABLE doctor_profile (
            doctor_id                   uuid                     not null,
            doctor_account_id           integer                  not null,
            doctor_profile_id           integer                  not null,
            citizen_id                  text                     not null,
            profession                  jsonb default '[]'::jsonb not null,
            academic_position           jsonb default '[]'::jsonb not null,
            first_name                  jsonb default '[]'::jsonb not null,
            last_name                   jsonb default '[]'::jsonb not null,
            department_id               integer                  not null,
            license_number              varchar(50)              not null,
            primary_medical_school      jsonb default '[]'::jsonb not null,
            specialty                   jsonb default '{}'::jsonb not null,
            additional_specialties      jsonb default '[]'::jsonb not null,
            special_interest            text[] default '{}'::text[] not null,
            supported_languages         language_code_enum[] default '{th,en}'::language_code_enum[] not null,
            channel_type                channel_type_enum[] default '{voice,chat,video}'::channel_type_enum[] not null,
            duration_minutes            int4                     not null,
            doctor_fee_amount           numeric(10, 2)           not null,
            doctor_fee_currency         varchar(3) default 'THB'::character varying not null,
            address_detail              text                     not null,
            sub_district                jsonb                    not null,
            district                    jsonb                    not null,
            province                    jsonb                    not null,
            postal_code                 integer                  not null,
            work_place                  jsonb default '[]'::jsonb not null,
            additional_workplace        jsonb default '[]'::jsonb not null,
            profile_image_url           varchar(500)             not null,
            id_card_image_url           varchar(500)             not null,
            book_bank_image_url         varchar(500)             not null,
            medical_license_image_url   varchar(500)             not null,
            education_license_image_url text[] default '{}'::text[] not null,
            is_active                   boolean default false    not null,
            created_at                  timestamp with time zone default now(),
            updated_at                  timestamp with time zone,
            CONSTRAINT doctor_profile_pkey PRIMARY KEY (doctor_id),
            CONSTRAINT doctor_profile_account_key UNIQUE (doctor_account_id),
            CONSTRAINT doctor_duration_duration_minutes_check CHECK ((duration_minutes = ANY (ARRAY[15, 25, 50]))),
            CONSTRAINT doctor_fee_amount_positive_check CHECK ((doctor_fee_amount >= (0)::numeric))
        );
        "#,
    )
    .execute(&pool)
    .await
    .expect("failed to create doctor_profile fixture table");

    (container, pool)
}

/// Inserts a minimal doctor_profile row for the given account and returns the generated
/// `doctor_id` UUID as text.
async fn seed_doctor(pool: &PgPool, account_id: i32) -> String {
    let (doctor_id,): (String,) = sqlx::query_as(
        r#"
        INSERT INTO doctor_profile (
            doctor_id, doctor_account_id, doctor_profile_id, citizen_id, department_id,
            license_number, duration_minutes, doctor_fee_amount, address_detail,
            sub_district, district, province, postal_code, profile_image_url,
            id_card_image_url, book_bank_image_url, medical_license_image_url
        ) VALUES (
            gen_random_uuid(), $1, 1, '1234567890123', 1,
            'LIC-1', 25, 500, 'addr',
            '{}'::jsonb, '{}'::jsonb, '{}'::jsonb, 10000, 'p.png',
            'id.png', 'bank.png', 'lic.png'
        )
        RETURNING doctor_id::text
        "#,
    )
    .bind(account_id)
    .fetch_one(pool)
    .await
    .expect("failed to seed doctor_profile row");
    doctor_id
}

fn create_test_server(pool: PgPool) -> TestServer {
    let kms = std::sync::Arc::new(common::MockKms);
    let app = Router::new()
        .nest("/profile", server::module::profile::router(pool, kms))
        // Mirror production wiring so the `RequestId` extension is present.
        .layer(axum::middleware::from_fn(
            server::core::gcp_logging_middleware,
        ));
    TestServer::new(app).unwrap()
}

#[tokio::test]
async fn missing_auth_returns_401() {
    let (_container, pool) = setup().await;
    let server = create_test_server(pool);

    let response = server.get("/profile/v1/doctor-id").await;

    assert_eq!(response.status_code(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn non_doctor_returns_403() {
    let (_container, pool) = setup().await;
    let server = create_test_server(pool);

    let response = server
        .get("/profile/v1/doctor-id")
        .add_header(auth_header_name(), patient_identity(999))
        .await;

    assert_eq!(response.status_code(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn unknown_doctor_returns_not_found_200() {
    let (_container, pool) = setup().await;
    let server = create_test_server(pool);

    let response = server
        .get("/profile/v1/doctor-id")
        .add_header(auth_header_name(), doctor_identity(404))
        .await;

    response.assert_status_ok();
    assert_eq!(
        response.json::<Value>(),
        json!({ "__type": "DoctorIdNotFound" })
    );
}

#[tokio::test]
async fn returns_doctor_id() {
    let (_container, pool) = setup().await;
    let expected = seed_doctor(&pool, 123).await;
    let server = create_test_server(pool);

    let response = server
        .get("/profile/v1/doctor-id")
        .add_header(auth_header_name(), doctor_identity(123))
        .await;

    response.assert_status_ok();
    let body: Value = response.json();
    assert_eq!(body["__type"], "DoctorId");
    assert_eq!(body["doctorId"], json!(expected));
}
