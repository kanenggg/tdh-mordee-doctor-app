use axum::http::{HeaderName, StatusCode};
use axum::Router;
use axum_test::TestServer;
use serde_json::{json, Value};
use sqlx::PgPool;

mod common;
use common::setup_postgres;

fn auth_header_name() -> HeaderName {
    HeaderName::from_static("tdh-sec-iam-user-identity")
}

fn doctor_identity(account_id: i32) -> String {
    serde_json::json!({
        "accountId": account_id,
        "accountType": 2,
        "userProfileId": 456,
        "userMainProfileId": 456,
        "tenantId": 1
    })
    .to_string()
}

fn patient_identity(account_id: i32) -> String {
    serde_json::json!({
        "accountId": account_id,
        "accountType": 1,
        "userProfileId": 456,
        "userMainProfileId": 456,
        "tenantId": 1
    })
    .to_string()
}

fn create_test_server(pool: PgPool) -> TestServer {
    let kms = std::sync::Arc::new(common::MockKms);
    let app = Router::new().nest("/profile", server::module::profile::router(pool, kms));

    TestServer::new(app).unwrap()
}

/// A date inside the forward window `[today, today + 90]` (anchored to Bangkok,
/// matching the service). Computed relative to today so the round-trip survives
/// the GET window filter / PUT past-strip regardless of when the test runs.
fn in_window_date() -> String {
    use jiff::{tz::TimeZone, Timestamp, ToSpan};
    let tz = TimeZone::get("Asia/Bangkok").unwrap();
    let today = Timestamp::now().to_zoned(tz).date();
    (today + 30.days()).strftime("%Y-%m-%d").to_string()
}

fn valid_schedule_config() -> Value {
    json!({
        "specificDate": [
            {
                "date": in_window_date(),
                "periods": [
                    { "startTime": 540, "endTime": 720 },
                    { "startTime": 780, "endTime": 1020 }
                ]
            }
        ],
        "timezone": "Asia/Bangkok",
        "daysOfWeek": {
            "1": [{ "startTime": 540, "endTime": 1020 }],
            "3": [{ "startTime": 600, "endTime": 900 }]
        }
    })
}

fn default_schedule_config() -> Value {
    json!({
        "specificDate": [],
        "timezone": "Asia/Bangkok",
        "daysOfWeek": {}
    })
}

#[tokio::test]
async fn missing_auth_returns_401() {
    let (_container, pool) = setup_postgres().await;
    let server = create_test_server(pool);

    let response = server.get("/profile/v1/availability").await;

    assert_eq!(response.status_code(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn non_doctor_auth_returns_403() {
    let (_container, pool) = setup_postgres().await;
    let server = create_test_server(pool);

    let response = server
        .get("/profile/v1/availability")
        .add_header(auth_header_name(), patient_identity(999))
        .add_query_param("bizUnitId", "1")
        .await;

    assert_eq!(response.status_code(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn availability_defaults_to_false() {
    let (_container, pool) = setup_postgres().await;
    let server = create_test_server(pool);

    let response = server
        .get("/profile/v1/availability")
        .add_header(auth_header_name(), doctor_identity(123))
        .add_query_param("bizUnitId", "1")
        .await;

    response.assert_status_ok();
    let body: Value = response.json();

    assert_eq!(body["__type"], "Success");
    assert_eq!(body["bizUnitId"], 1);
    assert_eq!(body["scheduleAvailable"], false);
    assert_eq!(body["instantAvailable"], false);
}

#[tokio::test]
async fn get_endpoints_and_db_functions_return_defaults_when_records_are_missing() {
    let (_container, pool) = setup_postgres().await;
    let server = create_test_server(pool.clone());

    let schedule_response = server
        .get("/profile/v1/consultation-setting/schedule-config/1")
        .add_header(auth_header_name(), doctor_identity(123))
        .await;

    schedule_response.assert_status_ok();
    assert_eq!(schedule_response.json::<Value>(), default_schedule_config());

    let availability_response = server
        .get("/profile/v1/availability")
        .add_header(auth_header_name(), doctor_identity(123))
        .add_query_param("bizUnitId", "1")
        .await;

    availability_response.assert_status_ok();
    let availability_body: Value = availability_response.json();
    assert_eq!(availability_body["scheduleAvailable"], false);
    assert_eq!(availability_body["instantAvailable"], false);

    let (db_config,): (Option<Value>,) =
        sqlx::query_as("SELECT get_consultation_schedule_config($1, $2)")
            .bind(123)
            .bind(1)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(db_config, Some(default_schedule_config()));

    let (schedule_available, instant_available): (bool, bool) = sqlx::query_as(
        "SELECT schedule_available, instant_available FROM get_consultation_availability($1, $2)",
    )
    .bind(123)
    .bind(1)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!schedule_available);
    assert!(!instant_available);
}

#[tokio::test]
async fn setup_postgres_runs_all_migrations() {
    let (_container, pool) = setup_postgres().await;

    let (exists,): (bool,) =
        sqlx::query_as("SELECT to_regclass('public.consultation_summarization') IS NOT NULL")
            .fetch_one(&pool)
            .await
            .unwrap();

    assert!(exists);
}

#[tokio::test]
async fn schedule_and_instant_toggles_are_independent() {
    let (_container, pool) = setup_postgres().await;
    let server = create_test_server(pool);

    let schedule_response = server
        .post("/profile/v1/availability/schedule")
        .add_header(auth_header_name(), doctor_identity(123))
        .json(&json!({ "available": true, "bizUnitId": 1 }))
        .await;
    schedule_response.assert_status_ok();

    let after_schedule: Value = server
        .get("/profile/v1/availability")
        .add_header(auth_header_name(), doctor_identity(123))
        .add_query_param("bizUnitId", "1")
        .await
        .json();
    assert_eq!(after_schedule["scheduleAvailable"], true);
    assert_eq!(after_schedule["instantAvailable"], false);

    let instant_response = server
        .post("/profile/v1/availability/instant")
        .add_header(auth_header_name(), doctor_identity(123))
        .json(&json!({ "available": true, "bizUnitId": 1 }))
        .await;
    instant_response.assert_status_ok();

    let after_instant: Value = server
        .get("/profile/v1/availability")
        .add_header(auth_header_name(), doctor_identity(123))
        .add_query_param("bizUnitId", "1")
        .await
        .json();
    assert_eq!(after_instant["scheduleAvailable"], true);
    assert_eq!(after_instant["instantAvailable"], true);

    let disable_schedule = server
        .post("/profile/v1/availability/schedule")
        .add_header(auth_header_name(), doctor_identity(123))
        .json(&json!({ "available": false, "bizUnitId": 1 }))
        .await;
    disable_schedule.assert_status_ok();

    let after_disable_schedule: Value = server
        .get("/profile/v1/availability")
        .add_header(auth_header_name(), doctor_identity(123))
        .add_query_param("bizUnitId", "1")
        .await
        .json();
    assert_eq!(after_disable_schedule["scheduleAvailable"], false);
    assert_eq!(after_disable_schedule["instantAvailable"], true);
}

#[tokio::test]
async fn settings_are_isolated_by_biz_unit() {
    let (_container, pool) = setup_postgres().await;
    let server = create_test_server(pool);

    server
        .post("/profile/v1/availability/schedule")
        .add_header(auth_header_name(), doctor_identity(123))
        .json(&json!({ "available": true, "bizUnitId": 1 }))
        .await
        .assert_status_ok();

    server
        .post("/profile/v1/availability/instant")
        .add_header(auth_header_name(), doctor_identity(123))
        .json(&json!({ "available": true, "bizUnitId": 2 }))
        .await
        .assert_status_ok();

    let biz_unit_1: Value = server
        .get("/profile/v1/availability")
        .add_header(auth_header_name(), doctor_identity(123))
        .add_query_param("bizUnitId", "1")
        .await
        .json();
    assert_eq!(biz_unit_1["scheduleAvailable"], true);
    assert_eq!(biz_unit_1["instantAvailable"], false);

    let biz_unit_2: Value = server
        .get("/profile/v1/availability")
        .add_header(auth_header_name(), doctor_identity(123))
        .add_query_param("bizUnitId", "2")
        .await
        .json();
    assert_eq!(biz_unit_2["scheduleAvailable"], false);
    assert_eq!(biz_unit_2["instantAvailable"], true);
}

#[tokio::test]
async fn schedule_config_round_trips() {
    let (_container, pool) = setup_postgres().await;
    let server = create_test_server(pool);
    let config = valid_schedule_config();

    server
        .put("/profile/v1/consultation-setting/schedule-config/1")
        .add_header(auth_header_name(), doctor_identity(123))
        .json(&config)
        .await
        .assert_status_ok();

    let response = server
        .get("/profile/v1/consultation-setting/schedule-config/1")
        .add_header(auth_header_name(), doctor_identity(123))
        .await;

    response.assert_status_ok();
    let body: Value = response.json();
    assert_eq!(body, config);
}

#[tokio::test]
async fn overlapping_schedule_config_returns_conflict_days() {
    let (_container, pool) = setup_postgres().await;
    let server = create_test_server(pool);

    let response = server
        .put("/profile/v1/consultation-setting/schedule-config/1")
        .add_header(auth_header_name(), doctor_identity(123))
        .json(&json!({
            "specificDate": [],
            "daysOfWeek": {
                "1": [
                    { "startTime": 540, "endTime": 720 },
                    { "startTime": 660, "endTime": 780 }
                ],
                "2": [
                    { "startTime": 540, "endTime": 720 },
                    { "startTime": 720, "endTime": 780 }
                ],
                "3": [
                    { "startTime": 600, "endTime": 900 },
                    { "startTime": 840, "endTime": 1020 }
                ]
            }
        }))
        .await;

    response.assert_status_ok();
    assert_eq!(
        response.json::<Value>(),
        json!({
            "__type": "Failure.ConflictTimeOverlap",
            "days": [1, 3]
        })
    );

    let saved_config = server
        .get("/profile/v1/consultation-setting/schedule-config/1")
        .add_header(auth_header_name(), doctor_identity(123))
        .await;

    saved_config.assert_status_ok();
    assert_eq!(saved_config.json::<Value>(), default_schedule_config());
}

#[tokio::test]
async fn invalid_input_returns_400() {
    let (_container, pool) = setup_postgres().await;
    let server = create_test_server(pool);

    let invalid_biz_unit = server
        .get("/profile/v1/availability")
        .add_header(auth_header_name(), doctor_identity(123))
        .add_query_param("bizUnitId", "0")
        .await;
    assert_eq!(invalid_biz_unit.status_code(), StatusCode::BAD_REQUEST);

    let invalid_schedule_biz_unit = server
        .get("/profile/v1/consultation-setting/schedule-config/0")
        .add_header(auth_header_name(), doctor_identity(123))
        .await;
    assert_eq!(
        invalid_schedule_biz_unit.status_code(),
        StatusCode::BAD_REQUEST
    );

    let invalid_day_of_week = server
        .put("/profile/v1/consultation-setting/schedule-config/1")
        .add_header(auth_header_name(), doctor_identity(123))
        .json(&json!({
            "specificDate": [],
            "daysOfWeek": { "0": [{ "startTime": 540, "endTime": 720 }] }
        }))
        .await;
    assert_eq!(invalid_day_of_week.status_code(), StatusCode::BAD_REQUEST);

    let invalid_date = server
        .put("/profile/v1/consultation-setting/schedule-config/1")
        .add_header(auth_header_name(), doctor_identity(123))
        .json(&json!({
            "specificDate": [{ "date": "20260520", "periods": [{ "startTime": 540, "endTime": 720 }] }],
            "daysOfWeek": {}
        }))
        .await;
    assert_eq!(invalid_date.status_code(), StatusCode::BAD_REQUEST);

    let invalid_period = server
        .put("/profile/v1/consultation-setting/schedule-config/1")
        .add_header(auth_header_name(), doctor_identity(123))
        .json(&json!({
            "specificDate": [{ "date": "2026-05-20", "periods": [{ "startTime": 720, "endTime": 540 }] }],
            "daysOfWeek": {}
        }))
        .await;
    assert_eq!(invalid_period.status_code(), StatusCode::BAD_REQUEST);
}
