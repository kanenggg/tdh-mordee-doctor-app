//! Full-route Redis-backed wiremock e2e for GET /timeslot/v1/me/available-timeslots.
//!
//! Drives the REAL `get_my_available_timeslots` handler through a real `TimeslotState`:
//! - ConsultationService stubbed by wiremock (`/internal/v1/appointment/reserved-timeslots`)
//! - A live Postgres testcontainer backs the consultation-duration repo
//! - A live Redis testcontainer backs the remaining Redis-using fields
//! - PubsubPublisher is constructed against a real Pub/Sub emulator testcontainer (gRPC handshake required; never published to)
//!
//! Proves the end-to-end path: duration config -> gateway call -> reserved-slot removal -> response.

use std::sync::Arc;

use axum_test::TestServer;
use deadpool_redis::{Config as RedisConfig, Runtime};
use serde_json::Value;
use testcontainers::core::{ContainerPort, IntoContainerPort, WaitFor};
use testcontainers::{runners::AsyncRunner, ContainerAsync, GenericImage, ImageExt};
use testcontainers_modules::redis::{Redis, REDIS_PORT};
use tokio::sync::Mutex;

use server::config::{
    PubsubConfig, PubsubSubscriberConfig, PubsubSubscriptions, PubsubTopics, TimeslotConfig,
};
use server::module::timeslot::get_my_available_time_slots::gateway::ReservedTimeslotsClient;
use server::module::timeslot::get_my_available_time_slots::handler::get_my_available_timeslots;
use server::module::timeslot::get_my_available_time_slots::repo::ConsultationDurationRepoImpl;
use server::module::timeslot::handler::TimeslotState;
use server::module::timeslot::{IdempotencyCache, RateLimiter, TimeslotRepoImpl, TimeslotService};
use server::module::webhook::PubsubPublisher;
use sqlx::PgPool;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

mod common;
use common::setup_postgres;

async fn setup_redis() -> (ContainerAsync<Redis>, String) {
    let container = Redis::default().start().await.unwrap();
    let port = container.get_host_port_ipv4(REDIS_PORT).await.unwrap();
    let url = format!("redis://127.0.0.1:{}", port);
    (container, url)
}

/// Start a real GCP Pub/Sub emulator. `google-cloud-pubsub 0.30`'s `Client::new`
/// establishes the gRPC channel eagerly even in emulator mode, so `PubsubPublisher::new`
/// needs a live endpoint to handshake against. The publisher is never published to by
/// this route — the emulator only exists so the unused `service` field can be constructed.
async fn setup_pubsub_emulator() -> (ContainerAsync<GenericImage>, String) {
    let container = GenericImage::new("gcr.io/google.com/cloudsdktool/cloud-sdk", "emulators")
        .with_wait_for(WaitFor::message_on_stderr("Server started, listening on"))
        .with_exposed_port(8085.tcp())
        .with_entrypoint("gcloud")
        .with_cmd([
            "beta",
            "emulators",
            "pubsub",
            "start",
            "--host-port=0.0.0.0:8085",
            "--project=test-project",
        ])
        .start()
        .await
        .unwrap();
    let port = container
        .get_host_port_ipv4(ContainerPort::Tcp(8085))
        .await
        .unwrap();
    (container, format!("127.0.0.1:{}", port))
}

/// Build a minimal `PubsubConfig` pointing at the live emulator host.
fn pubsub_config(emulator_host: String) -> PubsubConfig {
    PubsubConfig {
        gcp_project_id: "test-project".to_string(),
        emulator_host: Some(emulator_host),
        topics: PubsubTopics {
            appointments: "appointments".to_string(),
            consultations: "consultations".to_string(),
            system: "system".to_string(),
            broadcast: "broadcast".to_string(),
            doctor_notifications: "doctor-notifications".to_string(),
            doctor_profile_approved: "doctor-profile".to_string(),
            doctor_profile_status_updated: "doctor-profile-status-updated".to_string(),
        },
        subscriptions: PubsubSubscriptions {
            notification: "notification-sub".to_string(),
            consultation: "consultation-sub".to_string(),
        },
        subscriber: PubsubSubscriberConfig::default(),
    }
}

/// Doctor identity JSON for the `tdh-sec-iam-user-identity` header:
/// accountType=2 (doctor), accountId=555, userProfileId=900, userMainProfileId=901,
/// tenantId=1, oidcUserId=null, legacyData=null.
fn doctor_identity_header() -> String {
    serde_json::json!({
        "accountId": 555,
        "accountType": 2,
        "userProfileId": 900,
        "userMainProfileId": 901,
        "tenantId": 1,
        "oidcUserId": null,
        "legacyData": null
    })
    .to_string()
}

/// Builds the real `TimeslotState` (mirrors `module::timeslot::router`). The
/// consultation-duration repo is Postgres-backed; `reserved_uri` is the upstream
/// ConsultationService base (a wiremock URI, or unused when no config is seeded).
async fn build_state(
    redis_url: &str,
    pubsub_host: String,
    pg: PgPool,
    reserved_uri: String,
) -> TimeslotState {
    let redis_pool = RedisConfig::from_url(redis_url.to_string())
        .create_pool(Some(Runtime::Tokio1))
        .unwrap();
    let client = redis::Client::open(redis_url.to_string()).unwrap();
    let redis_manager = redis::aio::ConnectionManager::new(client).await.unwrap();
    let idempotency_cache = IdempotencyCache::new(redis_url).await.unwrap();

    let doctor_timeslot_repo: Arc<dyn server::doctor_actor::repo::DoctorTimeslotRepo> = Arc::new(
        server::doctor_actor::repo::DoctorTimeslotRepoImpl::new(pg.clone(), redis_pool.clone()),
    );
    let repo = Arc::new(TimeslotRepoImpl::new(pg.clone(), redis_pool.clone()));
    let rate_limiter = RateLimiter::new(pg.clone(), 100, 500);
    let pubsub = Arc::new(
        PubsubPublisher::new(&pubsub_config(pubsub_host))
            .await
            .unwrap(),
    );
    let service = Arc::new(TimeslotService::new(repo, rate_limiter, pubsub));

    let consultation_duration_repo: Arc<
        dyn server::module::timeslot::get_my_available_time_slots::repo::ConsultationDurationRepo,
    > = Arc::new(ConsultationDurationRepoImpl::new(pg.clone()));

    let reserved_timeslots_client: Arc<
        dyn server::module::timeslot::get_my_available_time_slots::gateway::ReservedTimeslotsClientTrait,
    > = Arc::new(ReservedTimeslotsClient::new(reserved_uri));

    TimeslotState {
        service,
        idempotency_cache: Arc::new(Mutex::new(idempotency_cache)),
        redis: redis_manager,
        config: TimeslotConfig::default(),
        doctor_timeslot_repo,
        consultation_duration_repo,
        reserved_timeslots_client,
    }
}

/// Mounts the route mirroring production composition (nested under `/timeslot/v1`).
fn mount(state: TimeslotState) -> TestServer {
    let sub = axum::Router::new()
        .route(
            "/me/available-timeslots",
            axum::routing::get(get_my_available_timeslots),
        )
        .with_state(state);
    let app = axum::Router::new().nest("/timeslot/v1", sub);
    TestServer::new(app).unwrap()
}

#[tokio::test]
async fn my_available_timeslots_full_route_removes_reserved_slot() {
    // --- Live Redis testcontainer ---
    let (_redis_container, redis_url) = setup_redis().await;

    // --- Live Pub/Sub emulator testcontainer (only so the unused `service` field constructs) ---
    let (_pubsub_container, pubsub_host) = setup_pubsub_emulator().await;

    // --- Live Postgres testcontainer backing the consultation-duration repo ---
    let (_pg_container, pg) = setup_postgres().await;

    // Seed a doctor_profile (profile_id 901, matching the header's userMainProfileId) and
    // its consultation-duration config keyed by the generated doctor_id:
    // duration_minutes = 15 => block = gap_rule(15) = 20.
    let (doctor_id,): (String,) = sqlx::query_as(
        r#"
        INSERT INTO doctor_profile (
            doctor_id, doctor_account_id, doctor_profile_id, citizen_id, department_id,
            license_number, address_detail, sub_district, district, province, postal_code,
            profile_image_url, id_card_image_url, book_bank_image_url, medical_license_image_url
        ) VALUES (
            gen_random_uuid(), 555, 901, '1234567890123', 1,
            'LIC-1', 'addr', '{}'::jsonb, '{}'::jsonb, '{}'::jsonb, 10000,
            'p.png', 'id.png', 'bank.png', 'lic.png'
        )
        RETURNING doctor_id::text
        "#,
    )
    .fetch_one(&pg)
    .await
    .unwrap();

    sqlx::query("INSERT INTO doctor_consultation_config (doctor_id, duration_minutes) VALUES ($1::uuid, $2)")
        .bind(doctor_id)
        .bind(15_i32)
        .execute(&pg)
        .await
        .unwrap();

    // --- Wiremock: stub ConsultationService reserved-timeslots ---
    // 1781715600 = 2026-06-18 00:00:00 +07:00; 1781716800 = 00:20:00 +07:00 (the first 20-min slot).
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/internal/v1/appointment/reserved-timeslots"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "reservedTimeslots": [
                { "bookingId": "BK1", "startTime": 1781715600, "endTime": 1781716800 }
            ]
        })))
        .mount(&mock_server)
        .await;

    // --- Build the real TimeslotState + mount the route ---
    let state = build_state(&redis_url, pubsub_host, pg, mock_server.uri()).await;
    let server = mount(state);

    // --- Drive the request ---
    let resp = server
        .get("/timeslot/v1/me/available-timeslots")
        .add_header("tdh-sec-iam-user-identity", doctor_identity_header())
        .add_query_param("date", "2026-06-18")
        .add_query_param("time_zone", "Asia/Bangkok")
        .await;

    resp.assert_status_ok();
    let body: Value = resp.json();

    assert_eq!(
        body["__type"], "Success",
        "expected MyAvailableResponse::Success, got: {body}"
    );
    let timeslots = body["timeslots"].as_array().unwrap();

    // 24h with 20-min blocks = 72 slots; the single reserved 00:00-00:20 slot is removed => 71.
    assert_eq!(
        timeslots.len(),
        71,
        "expected 71 slots (72 full-day minus reserved 00:00-00:20), got {}",
        timeslots.len()
    );

    // The reserved slot must be gone: no returned slot starts at 00:00:00.
    let start_times: Vec<&str> = timeslots
        .iter()
        .map(|t| t["startTime"].as_str().unwrap())
        .collect();
    assert!(
        !start_times.contains(&"00:00:00"),
        "reserved 00:00:00 slot should be removed, found it in: {start_times:?}"
    );
}

/// A doctor with no consultation-duration config → `NoScheduleConfig` (not a mock grid).
/// Nothing is seeded, so the duration lookup returns None and the handler returns early
/// before ever calling ConsultationService.
#[tokio::test]
async fn my_available_timeslots_no_config_returns_no_schedule_config() {
    let (_redis_container, redis_url) = setup_redis().await;
    let (_pubsub_container, pubsub_host) = setup_pubsub_emulator().await;
    let (_pg_container, pg) = setup_postgres().await;

    // No reserved-timeslots gateway is reached on this path, so the URI is unused.
    let state = build_state(
        &redis_url,
        pubsub_host,
        pg,
        "http://unused.invalid".to_string(),
    )
    .await;
    let server = mount(state);

    let resp = server
        .get("/timeslot/v1/me/available-timeslots")
        .add_header("tdh-sec-iam-user-identity", doctor_identity_header())
        .add_query_param("date", "2026-06-18")
        .add_query_param("time_zone", "Asia/Bangkok")
        .await;

    resp.assert_status_ok();
    let body: Value = resp.json();
    assert_eq!(
        body["__type"], "NoScheduleConfig",
        "expected MyAvailableResponse::NoScheduleConfig, got: {body}"
    );
}
