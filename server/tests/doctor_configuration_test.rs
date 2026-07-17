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

/// Consultation config lives in its own `doctor_consultation_config` table, now
/// keyed by `doctor_id` (uuid). The repo resolves the request's `doctor_account_id`
/// to that uuid via `doctor_profile`, so the fixture creates a minimal
/// `doctor_profile` mapping table alongside it. The migration that adds these is
/// generated from the schema source; create them here to match the production DDL
/// so these tests exercise the real shape. The `channel_type_enum` /
/// `language_code_enum` types already exist from the base migration.
async fn setup() -> (ContainerAsync<Postgres>, PgPool) {
    let (container, pool) = setup_postgres().await;

    sqlx::raw_sql(
        r#"
        DROP TABLE IF EXISTS doctor_consultation_config CASCADE;
        DROP TABLE IF EXISTS doctor_profile_event_outbox CASCADE;
        DROP TABLE IF EXISTS doctor_profile CASCADE;
        DROP TABLE IF EXISTS doctor_fee_transaction CASCADE;
        CREATE TABLE doctor_profile (
            doctor_id         uuid    not null,
            doctor_account_id integer not null,
            doctor_profile_id integer not null default 0,
            department_id integer not null default 0,
            is_active boolean not null default true,
            profession jsonb not null default '{"id":0,"name":{"th":"","en":""},"abbr":{"th":"","en":""}}',
            specialty jsonb not null default '{"id":0,"name":{"th":"","en":""},"medicalSchool":{"id":0,"name":{"th":"","en":""}}}',
            work_place jsonb not null default '[]',
            academic_position jsonb not null default '{"id":0,"name":{"th":"","en":""},"abbr":{"th":"","en":""}}',
            first_name jsonb not null default '{"th":"","en":""}',
            last_name jsonb not null default '{"th":"","en":""}',
            profile_image_url text not null default '',
            profile_version bigint not null default 0,
            created_at timestamptz not null default now(),
            updated_at timestamptz,
            CONSTRAINT doctor_profile_pkey PRIMARY KEY (doctor_id),
            CONSTRAINT doctor_profile_account_key UNIQUE (doctor_account_id)
        );
        CREATE TABLE doctor_consultation_config (
            doctor_id           uuid    not null,
            supported_languages language_code_enum[] default '{th,en}'::language_code_enum[] not null,
            channel_types       channel_type_enum[]  default '{voice,chat,video}'::channel_type_enum[] not null,
            duration_minutes    int4,
            doctor_fee_amount   numeric(10, 2),
            created_at          timestamp with time zone default now() not null,
            updated_at          timestamp with time zone,
            CONSTRAINT doctor_configuration_pkey PRIMARY KEY (doctor_id),
            CONSTRAINT doctor_duration_minutes_check CHECK (duration_minutes IS NULL OR duration_minutes = ANY (ARRAY[15, 25, 50])),
            CONSTRAINT doctor_fee_amount_positive_check CHECK (doctor_fee_amount IS NULL OR doctor_fee_amount >= (0)::numeric)
        );
        CREATE TABLE doctor_fee_transaction (
            transaction_id      integer GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
            doctor_id           uuid                     not null,
            doctor_fee_amount   numeric(10, 2)           not null,
            previous_fee_amount numeric(10, 2),
            change_reason       text,
            action_by           integer                  not null,
            created_at          timestamp with time zone not null default now(),
            CONSTRAINT doctor_fee_transaction_amount_positive_check CHECK (doctor_fee_amount >= 0)
        );
        CREATE INDEX IF NOT EXISTS idx_doctor_fee_transaction_account_created
            ON doctor_fee_transaction (doctor_id, created_at DESC);
        CREATE TABLE doctor_profile_event_outbox (
            event_id uuid primary key, doctor_id uuid not null references doctor_profile(doctor_id),
            doctor_account_id integer not null, event_type text not null, schema_version integer not null,
            profile_version bigint not null, occurred_at timestamptz not null, payload jsonb not null,
            attempts integer not null default 0, available_at timestamptz not null default now(),
            lease_token uuid, leased_until timestamptz, published_at timestamptz, last_error text,
            created_at timestamptz not null default now(), unique(doctor_id, profile_version)
        );
        "#,
    )
    .execute(&pool)
    .await
    .expect("failed to create doctor_consultation_config fixture tables");

    (container, pool)
}

/// Inserts a `doctor_profile` mapping row and a `doctor_consultation_config` row
/// keyed by the generated `doctor_id`, for the given account id and configuration.
async fn seed_doctor(
    pool: &PgPool,
    account_id: i32,
    channels: &str,
    languages: &str,
    duration_minutes: i32,
    fee_amount: f64,
) {
    sqlx::query(
        "INSERT INTO doctor_profile (doctor_id, doctor_account_id) VALUES (gen_random_uuid(), $1)",
    )
    .bind(account_id)
    .execute(pool)
    .await
    .expect("failed to seed doctor_profile row");

    sqlx::query(
        r#"
        INSERT INTO doctor_consultation_config (
            doctor_id, supported_languages, channel_types,
            duration_minutes, doctor_fee_amount
        )
        SELECT doctor_id, $2::language_code_enum[], $3::channel_type_enum[], $4, $5
          FROM doctor_profile
         WHERE doctor_account_id = $1
        "#,
    )
    .bind(account_id)
    .bind(format!("{{{languages}}}"))
    .bind(format!("{{{channels}}}"))
    .bind(duration_minutes)
    .bind(fee_amount)
    .execute(pool)
    .await
    .expect("failed to seed doctor_consultation_config row");
}

/// Inserts only a `doctor_profile` mapping row, with no matching
/// `doctor_consultation_config` row. Models a doctor who exists but has never
/// saved a consultation configuration yet.
async fn seed_doctor_profile_only(pool: &PgPool, account_id: i32) {
    sqlx::query(
        "INSERT INTO doctor_profile (doctor_id, doctor_account_id) VALUES (gen_random_uuid(), $1)",
    )
    .bind(account_id)
    .execute(pool)
    .await
    .expect("failed to seed doctor_profile row");
}

fn create_test_server(pool: PgPool) -> TestServer {
    let kms = std::sync::Arc::new(common::MockKms);
    let app = Router::new().nest("/profile", server::module::profile::router(pool, kms));
    TestServer::new(app).unwrap()
}

#[tokio::test]
async fn missing_auth_returns_401() {
    let (_container, pool) = setup().await;
    let server = create_test_server(pool);

    let response = server.get("/profile/v1/doctor-configuration").await;

    assert_eq!(response.status_code(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn non_doctor_auth_returns_403() {
    let (_container, pool) = setup().await;
    let server = create_test_server(pool);

    let response = server
        .get("/profile/v1/doctor-configuration")
        .add_header(auth_header_name(), patient_identity(999))
        .await;

    assert_eq!(response.status_code(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn get_returns_configuration() {
    let (_container, pool) = setup().await;
    seed_doctor(&pool, 123, "voice,video", "th,en", 25, 500.0).await;
    let server = create_test_server(pool);

    let response = server
        .get("/profile/v1/doctor-configuration")
        .add_header(auth_header_name(), doctor_identity(123))
        .await;

    response.assert_status_ok();
    let body: Value = response.json();
    assert_eq!(body["__type"], "DoctorConfigurationResponse");
    assert_eq!(body["channel"], json!(["voice", "video"]));
    assert_eq!(body["language"], json!(["th", "en"]));
    assert_eq!(body["duration"], 25);
    assert_eq!(body["fee"]["amount"], 500.0);
    assert_eq!(body["fee"]["currency"], "THB");
}

#[tokio::test]
async fn get_unknown_doctor_returns_not_found_200() {
    let (_container, pool) = setup().await;
    let server = create_test_server(pool);

    let response = server
        .get("/profile/v1/doctor-configuration")
        .add_header(auth_header_name(), doctor_identity(404))
        .await;

    response.assert_status_ok();
    assert_eq!(
        response.json::<Value>(),
        json!({ "__type": "DoctorConfigurationNotFound" })
    );
}

#[tokio::test]
async fn patch_channel_persists() {
    let (_container, pool) = setup().await;
    seed_doctor(&pool, 123, "voice,chat,video", "th,en", 25, 500.0).await;
    let server = create_test_server(pool);

    let patch = server
        .patch("/profile/v1/doctor-channel")
        .add_header(auth_header_name(), doctor_identity(123))
        .json(&json!({ "channel": ["chat"] }))
        .await;
    patch.assert_status_ok();
    assert_eq!(patch.json::<Value>(), json!({ "__type": "Success" }));

    let after: Value = server
        .get("/profile/v1/doctor-configuration")
        .add_header(auth_header_name(), doctor_identity(123))
        .await
        .json();
    assert_eq!(after["channel"], json!(["chat"]));
    // language untouched
    assert_eq!(after["language"], json!(["th", "en"]));
}

#[tokio::test]
async fn patch_language_persists() {
    let (_container, pool) = setup().await;
    seed_doctor(&pool, 123, "voice,chat,video", "th,en", 25, 500.0).await;
    let server = create_test_server(pool);

    let patch = server
        .patch("/profile/v1/doctor-language")
        .add_header(auth_header_name(), doctor_identity(123))
        .json(&json!({ "language": ["en"] }))
        .await;
    patch.assert_status_ok();
    assert_eq!(patch.json::<Value>(), json!({ "__type": "Success" }));

    let after: Value = server
        .get("/profile/v1/doctor-configuration")
        .add_header(auth_header_name(), doctor_identity(123))
        .await
        .json();
    assert_eq!(after["language"], json!(["en"]));
    // channel untouched
    assert_eq!(after["channel"], json!(["voice", "chat", "video"]));
}

#[tokio::test]
async fn patch_channel_creates_config_when_missing() {
    let (_container, pool) = setup().await;
    // Doctor exists (has a profile row) but has no consultation_config row yet.
    seed_doctor_profile_only(&pool, 123).await;
    let server = create_test_server(pool);

    let patch = server
        .patch("/profile/v1/doctor-channel")
        .add_header(auth_header_name(), doctor_identity(123))
        .json(&json!({ "channel": ["chat"] }))
        .await;
    patch.assert_status_ok();
    assert_eq!(patch.json::<Value>(), json!({ "__type": "Success" }));

    let after: Value = server
        .get("/profile/v1/doctor-configuration")
        .add_header(auth_header_name(), doctor_identity(123))
        .await
        .json();
    assert_eq!(after["__type"], "DoctorConfigurationResponse");
    assert_eq!(after["channel"], json!(["chat"]));
    // unset fields fall back to the table defaults
    assert_eq!(after["language"], json!(["th", "en"]));
}

#[tokio::test]
async fn patch_language_creates_config_when_missing() {
    let (_container, pool) = setup().await;
    seed_doctor_profile_only(&pool, 123).await;
    let server = create_test_server(pool);

    let patch = server
        .patch("/profile/v1/doctor-language")
        .add_header(auth_header_name(), doctor_identity(123))
        .json(&json!({ "language": ["en"] }))
        .await;
    patch.assert_status_ok();
    assert_eq!(patch.json::<Value>(), json!({ "__type": "Success" }));

    let after: Value = server
        .get("/profile/v1/doctor-configuration")
        .add_header(auth_header_name(), doctor_identity(123))
        .await
        .json();
    assert_eq!(after["__type"], "DoctorConfigurationResponse");
    assert_eq!(after["language"], json!(["en"]));
    // unset fields fall back to the table defaults
    assert_eq!(after["channel"], json!(["voice", "chat", "video"]));
}

#[tokio::test]
async fn patch_unknown_doctor_returns_not_found() {
    let (_container, pool) = setup().await;
    let server = create_test_server(pool);

    let response = server
        .patch("/profile/v1/doctor-channel")
        .add_header(auth_header_name(), doctor_identity(404))
        .json(&json!({ "channel": ["chat"] }))
        .await;

    response.assert_status_ok();
    assert_eq!(
        response.json::<Value>(),
        json!({ "__type": "DoctorConfigurationNotFound" })
    );
}

#[tokio::test]
async fn empty_channel_returns_400() {
    let (_container, pool) = setup().await;
    seed_doctor(&pool, 123, "voice", "th", 25, 500.0).await;
    let server = create_test_server(pool);

    let response = server
        .patch("/profile/v1/doctor-channel")
        .add_header(auth_header_name(), doctor_identity(123))
        .json(&json!({ "channel": [] }))
        .await;

    assert_eq!(response.status_code(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn empty_language_returns_400() {
    let (_container, pool) = setup().await;
    seed_doctor(&pool, 123, "voice", "th", 25, 500.0).await;
    let server = create_test_server(pool);

    let response = server
        .patch("/profile/v1/doctor-language")
        .add_header(auth_header_name(), doctor_identity(123))
        .json(&json!({ "language": [] }))
        .await;

    assert_eq!(response.status_code(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn invalid_channel_value_returns_422() {
    let (_container, pool) = setup().await;
    seed_doctor(&pool, 123, "voice", "th", 25, 500.0).await;
    let server = create_test_server(pool);

    let response = server
        .patch("/profile/v1/doctor-channel")
        .add_header(auth_header_name(), doctor_identity(123))
        .json(&json!({ "channel": ["telepathy"] }))
        .await;

    // Unknown enum variant fails JSON deserialization before the handler runs.
    assert_eq!(response.status_code(), StatusCode::UNPROCESSABLE_ENTITY);
}
