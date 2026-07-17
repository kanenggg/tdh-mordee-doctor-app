//! Integration test: approve_doctor_profile_draft returns approved core fields.
use axum::extract::Request;
use axum::middleware::{self, Next};
use axum::response::Response;
use axum_test::TestServer;
use server::core::RequestId;
use sqlx::Row;
use std::collections::BTreeSet;
use std::sync::Arc;
mod common;
use common::setup_postgres;

async fn insert_request_id(mut request: Request, next: Next) -> Response {
    request
        .extensions_mut()
        .insert(RequestId("test-request-id".to_string()));
    next.run(request).await
}

fn approve_body(
    doctor_account_id: i32,
    department_id: i32,
    clinics: Vec<i32>,
) -> serde_json::Value {
    serde_json::json!({
        "doctorAccountId": doctor_account_id,
        "departmentId": department_id,
        "clinics": clinics,
        "consultationConfig": {
            "supportedLanguages": ["th", "en"],
            "channelTypes": ["voice", "chat"],
            "durationMinutes": 15,
            "doctorFeeAmount": 200.0
        }
    })
}

fn backoffice_identity(account_id: i32) -> String {
    serde_json::json!({
        "accountId": account_id,
        "accountType": 4,
        "userProfileId": 456,
        "userMainProfileId": 456,
        "tenantId": 1
    })
    .to_string()
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

fn assert_compact_consultation_config(payload: &serde_json::Value) {
    assert!(payload["consultationConfig"].is_object());
    for duplicate in [
        "doctorFee",
        "doctorFeeCurrency",
        "languages",
        "durationMinutes",
        "channels",
    ] {
        assert!(payload.get(duplicate).is_none(), "unexpected {duplicate}");
    }
}

#[tokio::test]
async fn approve_function_uses_one_defaulted_signature() {
    let (_pg, pool) = setup_postgres().await;

    let (function_count, default_argument_count): (i64, Option<i16>) = sqlx::query_as(
        r#"SELECT count(*), max(p.pronargdefaults)
           FROM pg_proc p
           JOIN pg_namespace n ON n.oid = p.pronamespace
           WHERE n.nspname = 'public'
             AND p.proname = 'approve_doctor_profile_draft'"#,
    )
    .fetch_one(&pool)
    .await
    .expect("inspect approve function signatures");

    assert_eq!(function_count, 1);
    assert_eq!(default_argument_count, Some(5));
}

#[tokio::test]
async fn approve_returns_core_profile_fields() {
    let (_pg, pool) = setup_postgres().await;

    // Seed a PendingApproval draft. citizen_id must be non-null because the
    // doctor_profile table (target of the approval upsert) has a NOT NULL
    // constraint on citizen_id.
    sqlx::query(
        r#"INSERT INTO department (department_id, name, counseling_areas)
           VALUES (
               17,
               '{"th":"อายุรกรรม","en":"Internal Medicine"}'::jsonb,
               '[{"th":"หัวใจ","en":"Heart"}]'::jsonb
           )
           ON CONFLICT (department_id) DO UPDATE
           SET name = EXCLUDED.name,
               counseling_areas = EXCLUDED.counseling_areas"#,
    )
    .execute(&pool)
    .await
    .expect("seed department");

    sqlx::query(
        r#"INSERT INTO doctor_profile_draft
           (doctor_account_id, doctor_profile_id, citizen_id, profession, academic_position,
            first_name, last_name, license_number, primary_medical_school, specialty,
            additional_specialties, special_interest, address_detail, sub_district, district,
            province, postal_code, work_place, additional_workplace, profile_image_url,
            id_card_image_url, book_bank_image_url, medical_license_image_url,
            education_license_image_url, status)
           VALUES (5001, 9001, '1234567890123', '{"id":1,"name":{"th":"แพทย์","en":"Doctor"},"abbr":{"th":"พญ.","en":"Dr."}}'::jsonb,
            '{"id":2,"name":{"th":"อาจารย์","en":"Lecturer"},"abbr":{"th":"อ.","en":"Lect."}}'::jsonb,
            '{"th":"สมชาย","en":"Somchai"}'::jsonb, '{"th":"ใจดี","en":"Jaidee"}'::jsonb,
            'LIC1', '[]'::jsonb,
            '[{"id":10,"name":{"th":"หัวใจ","en":"Cardiology"},"subspecialty":{"id":11,"name":{"th":"หัวใจ","en":"Cardiology"},"medicalSchool":{"id":12,"name":{"th":"มหาวิทยาลัยแพทย์","en":"Medical University"}}},"medicalSchool":{"id":12,"name":{"th":"มหาวิทยาลัยแพทย์","en":"Medical University"}}}]'::jsonb,
            '[]'::jsonb, ARRAY[]::text[], '', '{}'::jsonb,
            '{}'::jsonb, '{}'::jsonb, 10110, '[{"id":20,"name":{"th":"โรงพยาบาล","en":"Hospital"}}]'::jsonb, '[]'::jsonb, 'http://img/x.jpg',
            '', '', '', ARRAY[]::text[], 'PendingApproval')"#,
    )
    .execute(&pool)
    .await
    .expect("seed draft");

    let row = sqlx::query("SELECT * FROM approve_doctor_profile_draft($1,$2,$3)")
        .bind(5001_i32)
        .bind(0_i32)
        .bind(17_i32)
        .fetch_one(&pool)
        .await
        .expect("approve returns a row");

    let doctor_account_id: i32 = row.get("doctor_account_id");
    let department_id: i32 = row.get("department_id");
    let is_active: bool = row.get("is_active");
    let profile_image_url: String = row.get("profile_image_url");
    let _doctor_id: uuid::Uuid = row.get("doctor_id");
    let _profession: serde_json::Value = row.get("profession");
    let department: serde_json::Value = row.get("department");
    let counseling_areas: serde_json::Value = row.get("counseling_areas");
    let specialty: serde_json::Value = row.get("specialty");
    let work_place: serde_json::Value = row.get("work_place");
    let doctor_fee: i32 = row.get("doctor_fee");
    let doctor_fee_currency: String = row.get("doctor_fee_currency");
    let languages: serde_json::Value = row.get("languages");
    let duration_minutes: i32 = row.get("duration_minutes");
    let channels: serde_json::Value = row.get("channels");

    assert_eq!(doctor_account_id, 5001);
    assert_eq!(department_id, 17);
    assert_eq!(department["en"], "Internal Medicine");
    assert_eq!(counseling_areas[0]["en"], "Heart");
    assert_eq!(specialty[0]["name"]["en"], "Cardiology");
    assert_eq!(work_place[0]["name"]["en"], "Hospital");
    assert!(is_active);
    assert_eq!(profile_image_url, "http://img/x.jpg");
    assert_eq!(doctor_fee, 0);
    assert_eq!(doctor_fee_currency, "THB");
    assert_eq!(languages, serde_json::json!([]));
    assert_eq!(duration_minutes, 15);
    assert_eq!(channels, serde_json::json!([]));
}

#[tokio::test]
async fn approve_inserts_an_outbox_event_even_when_direct_publish_is_unavailable() {
    let (_pg, pool) = setup_postgres().await;

    // Seed a PendingApproval draft (same valid shape as the DB-level test above,
    // but with a distinct doctor_account_id to avoid cross-test coupling).
    sqlx::query(
        r#"INSERT INTO doctor_profile_draft
           (doctor_account_id, doctor_profile_id, citizen_id, profession, academic_position,
            first_name, last_name, license_number, primary_medical_school, specialty,
            additional_specialties, special_interest, address_detail, sub_district, district,
            province, postal_code, work_place, additional_workplace, profile_image_url,
            id_card_image_url, book_bank_image_url, medical_license_image_url,
            education_license_image_url, status)
           VALUES (6002, 9002, '1234567890123', '{"id":1,"name":{"th":"แพทย์","en":"Doctor"},"abbr":{"th":"พญ.","en":"Dr."}}'::jsonb,
            '{"id":2,"name":{"th":"อาจารย์","en":"Lecturer"},"abbr":{"th":"อ.","en":"Lect."}}'::jsonb,
            '{"th":"สมชาย","en":"Somchai"}'::jsonb, '{"th":"ใจดี","en":"Jaidee"}'::jsonb,
            'LIC2', '[]'::jsonb,
            '[{"id":10,"name":{"th":"หัวใจ","en":"Cardiology"},"subspecialty":{"id":11,"name":{"th":"หัวใจ","en":"Cardiology"},"medicalSchool":{"id":12,"name":{"th":"มหาวิทยาลัยแพทย์","en":"Medical University"}}},"medicalSchool":{"id":12,"name":{"th":"มหาวิทยาลัยแพทย์","en":"Medical University"}}}]'::jsonb,
            '[]'::jsonb, ARRAY[]::text[], '', '{}'::jsonb,
            '{}'::jsonb, '{}'::jsonb, 10110, '[{"id":20,"name":{"th":"โรงพยาบาล","en":"Hospital"}}]'::jsonb, '[]'::jsonb, 'http://img/x.jpg',
            '', '', '', ARRAY[]::text[], 'PendingApproval')"#,
    )
    .execute(&pool)
    .await
    .expect("seed draft");

    // Bind a TCP listener that accepts connections but never speaks gRPC, then
    // immediately drops each socket. This lets `PubsubPublisher::new` establish
    // its channel (so construction succeeds, skipping the GCP auth path because
    // `emulator_host` is non-empty), while the actual publish RPC fails fast.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let emulator_addr = listener.local_addr().unwrap().to_string();
    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((socket, _)) => drop(socket),
                Err(_) => break,
            }
        }
    });

    let cfg = server::config::PubsubConfig {
        gcp_project_id: "test-project".to_string(),
        emulator_host: Some(emulator_addr),
        topics: server::config::PubsubTopics {
            appointments: "appointments".to_string(),
            consultations: "consultations".to_string(),
            system: "system".to_string(),
            broadcast: "broadcast".to_string(),
            doctor_notifications: "doctor-notifications".to_string(),
            doctor_profile_approved: "doctor-profile".to_string(),
            doctor_profile_status_updated: "doctor-profile-status-updated".to_string(),
        },
        subscriptions: server::config::PubsubSubscriptions {
            notification: "test-notification-sub".to_string(),
            consultation: "test-consultation-sub".to_string(),
        },
        subscriber: Default::default(),
    };
    let publisher = Arc::new(
        server::module::webhook::PubsubPublisher::new(&cfg)
            .await
            .expect("publisher builds"),
    );

    let app = server::module::backoffice::onboarding::router(
        pool.clone(),
        publisher,
        "doctor-profile".to_string(),
    )
    .layer(middleware::from_fn(insert_request_id));

    let server = TestServer::new(app).unwrap();
    let resp = server
        .post("/approve")
        .json(&approve_body(6002, 17, vec![]))
        .await;

    // 200 OK despite the publish failure (best-effort path).
    resp.assert_status_ok();

    // The approval really happened: an active doctor_profile row exists.
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM doctor_profile WHERE doctor_account_id = $1 AND is_active = true",
    )
    .bind(6002_i32)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1);

    let outbox: (i64, i32, serde_json::Value) = sqlx::query_as(
        "SELECT profile_version, schema_version, payload FROM doctor_profile_event_outbox WHERE doctor_account_id = $1",
    )
    .bind(6002_i32)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(outbox.0, 1);
    assert_eq!(outbox.1, 2);
    assert_eq!(outbox.2["__type"], "DoctorProfileApproved");
    assert_eq!(outbox.2["consultationConfig"]["feeAmount"], "200.00");
    assert_compact_consultation_config(&outbox.2);
}

// Build a backoffice onboarding app whose publisher connects but whose publish RPC fails.
// Returns (app, pool) — the caller seeds rows on `pool` before issuing requests.
async fn app_with_failing_publisher(pool: sqlx::PgPool) -> axum::Router {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let emulator_addr = listener.local_addr().unwrap().to_string();
    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((socket, _)) => drop(socket),
                Err(_) => break,
            }
        }
    });

    let cfg = server::config::PubsubConfig {
        gcp_project_id: "test-project".to_string(),
        emulator_host: Some(emulator_addr),
        topics: server::config::PubsubTopics {
            appointments: "appointments".to_string(),
            consultations: "consultations".to_string(),
            system: "system".to_string(),
            broadcast: "broadcast".to_string(),
            doctor_notifications: "doctor-notifications".to_string(),
            doctor_profile_approved: "doctor-profile".to_string(),
            doctor_profile_status_updated: "doctor-profile-status-updated".to_string(),
        },
        subscriptions: server::config::PubsubSubscriptions {
            notification: "test-notification-sub".to_string(),
            consultation: "test-consultation-sub".to_string(),
        },
        subscriber: Default::default(),
    };
    let publisher = Arc::new(
        server::module::webhook::PubsubPublisher::new(&cfg)
            .await
            .expect("publisher builds"),
    );

    let approved_immediate_delivery = Arc::new(
        server::module::backoffice::onboarding::outbox_delivery::ImmediateDoctorProfileDelivery::new(
            pool.clone(),
            publisher.clone(),
            "doctor-profile".to_string(),
        ),
    );
    let status_updated_immediate_delivery = Arc::new(
        server::module::backoffice::onboarding::outbox_delivery::ImmediateDoctorProfileDelivery::new(
            pool.clone(),
            publisher,
            "doctor-profile-status-updated".to_string(),
        ),
    );
    let onboarding = server::module::backoffice::onboarding::routers(
        pool.clone(),
        Some(approved_immediate_delivery.clone()),
    );

    onboarding
        .internal
        .merge(onboarding.backoffice)
        .merge(server::module::backoffice::doctor_management::router(
            pool,
            Some(approved_immediate_delivery),
            Some(status_updated_immediate_delivery),
        ))
        .layer(middleware::from_fn(insert_request_id))
}

/// Inserts one PendingApproval draft row for the given account id.
async fn seed_pending_draft(pool: &sqlx::PgPool, account_id: i32, profile_id: i32) {
    sqlx::query(
        r#"INSERT INTO doctor_profile_draft
           (doctor_account_id, doctor_profile_id, citizen_id, profession, academic_position,
            first_name, last_name, license_number, primary_medical_school, specialty,
            additional_specialties, special_interest, address_detail, sub_district, district,
            province, postal_code, work_place, additional_workplace, profile_image_url,
            id_card_image_url, book_bank_image_url, medical_license_image_url,
            education_license_image_url, status)
           VALUES ($1, $2, '1234567890123', '{"id":1,"name":{"th":"แพทย์","en":"Doctor"},"abbr":{"th":"พญ.","en":"Dr."}}'::jsonb,
            '{"id":2,"name":{"th":"อาจารย์","en":"Lecturer"},"abbr":{"th":"อ.","en":"Lect."}}'::jsonb,
            '{"th":"ก","en":"A"}'::jsonb, '{"th":"ข","en":"B"}'::jsonb,
            'LIC', '[]'::jsonb,
            '{"id":10,"name":{"th":"หัวใจ","en":"Cardiology"},"subspecialty":{"id":11,"name":{"th":"หัวใจ","en":"Cardiology"},"medicalSchool":{"id":12,"name":{"th":"มหาวิทยาลัยแพทย์","en":"Medical University"}}},"medicalSchool":{"id":12,"name":{"th":"มหาวิทยาลัยแพทย์","en":"Medical University"}}}'::jsonb,
            '[]'::jsonb, ARRAY[]::text[], '', '{}'::jsonb,
            '{}'::jsonb, '{}'::jsonb, 10110, '[{"id":20,"name":{"th":"โรงพยาบาล","en":"Hospital"}}]'::jsonb, '[]'::jsonb, 'http://img/x.jpg',
            '', '', '', ARRAY[]::text[], 'PendingApproval')"#,
    )
    .bind(account_id)
    .bind(profile_id)
    .execute(pool)
    .await
    .expect("seed pending draft");
}

#[tokio::test]
async fn update_doctor_active_status_is_durable_versioned_and_idempotent() {
    let (_pg, pool) = setup_postgres().await;
    seed_pending_draft(&pool, 12099, 9700).await;
    let app = app_with_failing_publisher(pool.clone()).await;
    let server = TestServer::new(app).unwrap();

    server
        .post("/approve")
        .json(&approve_body(12099, 17, vec![]))
        .await
        .assert_status_ok();

    let resp = server
        .patch("/doctor-active-status")
        .json(&serde_json::json!({
            "doctorAccountId": 12099,
            "isActive": false
        }))
        .await;

    resp.assert_status_ok();
    let is_active: bool =
        sqlx::query_scalar("SELECT is_active FROM doctor_profile WHERE doctor_account_id = $1")
            .bind(12099_i32)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!is_active);
    let event: (i64, String, i32, serde_json::Value) = sqlx::query_as(
        "SELECT profile_version, event_type, schema_version, payload FROM doctor_profile_event_outbox WHERE doctor_account_id = $1 ORDER BY profile_version DESC LIMIT 1",
    )
    .bind(12099_i32)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(event.0, 2);
    assert_eq!(event.1, "DoctorProfileStatusUpdated");
    assert_eq!(event.2, 3);
    assert_eq!(event.3["__type"], "DoctorProfileStatusUpdated");
    assert_eq!(event.3["profileVersion"], 2);
    assert_eq!(event.3["isActive"], false);
    assert!(event.3["eventId"].as_str().is_some());
    assert!(event.3["occurredAt"].as_i64().is_some());
    assert!(event.3["statusUpdatedAt"].as_i64().is_some());
    assert!(event.3.get("deactivatedAt").is_none());
    assert!(event.3.get("reason").is_none());

    // Setting the same state again cannot create a new event or version.
    server
        .patch("/doctor-active-status")
        .json(&serde_json::json!({ "doctorAccountId": 12099, "isActive": false }))
        .await
        .assert_status_ok();
    let count_after_deactivation_retry: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM doctor_profile_event_outbox WHERE doctor_account_id = $1",
    )
    .bind(12099_i32)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count_after_deactivation_retry, 2);

    server
        .patch("/doctor-active-status")
        .json(&serde_json::json!({ "doctorAccountId": 12099, "isActive": true }))
        .await
        .assert_status_ok();

    let is_active: bool =
        sqlx::query_scalar("SELECT is_active FROM doctor_profile WHERE doctor_account_id = $1")
            .bind(12099_i32)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(is_active);
    let activation_event: (i64, String, i32, serde_json::Value) = sqlx::query_as(
        "SELECT profile_version, event_type, schema_version, payload FROM doctor_profile_event_outbox WHERE doctor_account_id = $1 ORDER BY profile_version DESC LIMIT 1",
    )
    .bind(12099_i32)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(activation_event.0, 3);
    assert_eq!(activation_event.1, "DoctorProfileStatusUpdated");
    assert_eq!(activation_event.2, 3);
    assert_eq!(activation_event.3["__type"], "DoctorProfileStatusUpdated");
    assert_eq!(activation_event.3["profileVersion"], 3);
    assert_eq!(activation_event.3["isActive"], true);
    assert!(activation_event.3["statusUpdatedAt"].as_i64().is_some());
    assert!(activation_event.3.get("deactivatedAt").is_none());
    assert!(activation_event.3.get("consultationConfig").is_none());
    let deactivation_keys = event.3.as_object().unwrap().keys().collect::<BTreeSet<_>>();
    let activation_keys = activation_event
        .3
        .as_object()
        .unwrap()
        .keys()
        .collect::<BTreeSet<_>>();
    assert_eq!(deactivation_keys, activation_keys);

    server
        .patch("/doctor-active-status")
        .json(&serde_json::json!({ "doctorAccountId": 12099, "isActive": true }))
        .await
        .assert_status_ok();
    let final_state: (i64, i64) = sqlx::query_as(
        "SELECT profile_version, (SELECT count(*) FROM doctor_profile_event_outbox WHERE doctor_account_id = $1) FROM doctor_profile WHERE doctor_account_id = $1",
    )
    .bind(12099_i32)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(final_state, (3, 3));
}

#[tokio::test]
async fn approve_unknown_account_returns_400_and_writes_no_audit() {
    let (_pg, pool) = setup_postgres().await;
    let app = app_with_failing_publisher(pool.clone()).await;
    let server = TestServer::new(app).unwrap();

    // No draft seeded for account 7777, and no active profile exists.
    let resp = server
        .post("/approve")
        .json(&approve_body(7777, 17, vec![]))
        .await;

    assert_eq!(resp.status_code(), axum::http::StatusCode::BAD_REQUEST);

    let audit_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM doctor_profile_transaction WHERE doctor_account_id = $1",
    )
    .bind(7777_i32)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit_count, 0, "no forged audit row for an invalid approve");
}

#[tokio::test]
async fn reapprove_is_idempotent_no_duplicate_audit() {
    let (_pg, pool) = setup_postgres().await;
    seed_pending_draft(&pool, 8888, 9100).await;
    let app = app_with_failing_publisher(pool.clone()).await;
    let server = TestServer::new(app).unwrap();

    let body = approve_body(8888, 17, vec![]);

    let first = server.post("/approve").json(&body).await;
    first.assert_status_ok();
    let second = server.post("/approve").json(&body).await;
    second.assert_status_ok();

    // Exactly one active profile and exactly one audit row despite two approve calls.
    let profile_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM doctor_profile WHERE doctor_account_id = $1 AND is_active = true",
    )
    .bind(8888_i32)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(profile_count, 1);

    let audit_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM doctor_profile_transaction WHERE doctor_account_id = $1",
    )
    .bind(8888_i32)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        audit_count, 1,
        "re-approve must not write a second audit row"
    );
}

#[tokio::test]
async fn approve_saves_clinics() {
    let (_pg, pool) = setup_postgres().await;
    seed_pending_draft(&pool, 9999, 9200).await;
    let app = app_with_failing_publisher(pool.clone()).await;
    let server = TestServer::new(app).unwrap();

    let resp = server
        .post("/approve")
        .json(&approve_body(9999, 17, vec![1, 2]))
        .await;

    resp.assert_status_ok();

    let clinic_ids: Vec<i32> = sqlx::query_scalar(
        r#"
        SELECT dc.clinic_id
        FROM doctor_clinic dc
        JOIN doctor_profile dp ON dp.doctor_id = dc.doctor_id
        WHERE dp.doctor_account_id = $1
        ORDER BY dc.clinic_id
        "#,
    )
    .bind(9999_i32)
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(clinic_ids, vec![1, 2]);

    let config = sqlx::query(
        r#"
        SELECT
            c.channel_types::text[] AS channel_types,
            c.supported_languages::text[] AS supported_languages,
            c.duration_minutes,
            c.doctor_fee_amount::float8 AS doctor_fee_amount
        FROM doctor_consultation_config c
        JOIN doctor_profile dp ON dp.doctor_id = c.doctor_id
        WHERE dp.doctor_account_id = $1
        "#,
    )
    .bind(9999_i32)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(
        config.get::<Vec<String>, _>("channel_types"),
        vec!["voice".to_string(), "chat".to_string()]
    );
    assert_eq!(
        config.get::<Vec<String>, _>("supported_languages"),
        vec!["th".to_string(), "en".to_string()]
    );
    assert_eq!(config.get::<i32, _>("duration_minutes"), 15);
    assert_eq!(config.get::<f64, _>("doctor_fee_amount"), 200.0);
}

#[tokio::test]
async fn approve_upserts_clinics_without_deleting_existing_assignments() {
    let (_pg, pool) = setup_postgres().await;
    seed_pending_draft(&pool, 10002, 9400).await;
    let app = app_with_failing_publisher(pool.clone()).await;
    let server = TestServer::new(app).unwrap();

    server
        .post("/approve")
        .json(&approve_body(10002, 17, vec![1, 2]))
        .await
        .assert_status_ok();

    let second = serde_json::json!({
        "doctorAccountId": 10002,
        "departmentId": 17,
        "clinics": [1],
        "consultationConfig": {
            "supportedLanguages": ["th", "en"],
            "channelTypes": ["voice", "chat"],
            "durationMinutes": 15,
            "doctorFeeAmount": 200.0
        }
    });

    server
        .post("/approve")
        .json(&second)
        .await
        .assert_status_ok();

    let clinic_ids: Vec<i32> = sqlx::query_scalar(
        r#"
        SELECT dc.clinic_id
        FROM doctor_clinic dc
        JOIN doctor_profile dp ON dp.doctor_id = dc.doctor_id
        WHERE dp.doctor_account_id = $1
        ORDER BY dc.clinic_id
        "#,
    )
    .bind(10002_i32)
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(clinic_ids, vec![1, 2]);
}

#[tokio::test]
async fn approve_rejects_invalid_consultation_config_before_approval() {
    let (_pg, pool) = setup_postgres().await;
    seed_pending_draft(&pool, 10001, 9300).await;
    let app = app_with_failing_publisher(pool.clone()).await;
    let server = TestServer::new(app).unwrap();

    let body = serde_json::json!({
        "doctorAccountId": 10001,
        "departmentId": 17,
        "clinics": [1],
        "consultationConfig": {
            "supportedLanguages": ["th"],
            "channelTypes": ["voice"],
            "durationMinutes": 30,
            "doctorFeeAmount": 200.0
        }
    });

    let resp = server.post("/approve").json(&body).await;

    assert_eq!(resp.status_code(), axum::http::StatusCode::BAD_REQUEST);

    let active_profile_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM doctor_profile WHERE doctor_account_id = $1 AND is_active = true",
    )
    .bind(10001_i32)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(active_profile_count, 0);
}

#[tokio::test]
async fn update_consultation_configuration_persists_full_config() {
    let (_pg, pool) = setup_postgres().await;
    seed_pending_draft(&pool, 11001, 9500).await;
    let app = app_with_failing_publisher(pool.clone()).await;
    let server = TestServer::new(app).unwrap();

    server
        .post("/approve")
        .json(&approve_body(11001, 17, vec![1]))
        .await
        .assert_status_ok();

    let resp = server
        .patch("/consultation-configuration")
        .add_header("tdh-sec-iam-user-identity", backoffice_identity(7001))
        .json(&serde_json::json!({
            "doctorAccountId": 11001,
            "consultationConfig": {
                "supportedLanguages": ["th", "en"],
                "channelTypes": ["voice", "chat"],
                "durationMinutes": 15,
                "doctorFeeAmount": 650.5
            }
        }))
        .await;

    resp.assert_status_ok();
    assert_eq!(
        resp.json::<serde_json::Value>(),
        serde_json::json!({ "__type": "Success" })
    );

    let config = sqlx::query(
        r#"
        SELECT
            c.channel_types::text[] AS channel_types,
            c.supported_languages::text[] AS supported_languages,
            c.duration_minutes,
            c.doctor_fee_amount::float8 AS doctor_fee_amount
        FROM doctor_consultation_config c
        JOIN doctor_profile dp ON dp.doctor_id = c.doctor_id
        WHERE dp.doctor_account_id = $1
        "#,
    )
    .bind(11001_i32)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(
        config.get::<Vec<String>, _>("channel_types"),
        vec!["voice".to_string(), "chat".to_string()]
    );
    assert_eq!(
        config.get::<Vec<String>, _>("supported_languages"),
        vec!["th".to_string(), "en".to_string()]
    );
    assert_eq!(config.get::<i32, _>("duration_minutes"), 15);
    assert_eq!(config.get::<f64, _>("doctor_fee_amount"), 650.5);

    let outbox: (i64, serde_json::Value) = sqlx::query_as(
        "SELECT profile_version, payload FROM doctor_profile_event_outbox WHERE doctor_account_id = $1 ORDER BY profile_version DESC LIMIT 1",
    )
    .bind(11001_i32)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(outbox.0, 2);
    assert_eq!(outbox.1["__type"], "DoctorProfileApproved");
    assert_eq!(outbox.1["consultationConfig"]["durationMinutes"], 15);
    assert_eq!(outbox.1["consultationConfig"]["feeAmount"], "650.50");
    assert_compact_consultation_config(&outbox.1);

    let changed_fee_transaction = sqlx::query(
        r#"
        SELECT
            t.doctor_fee_amount::float8 AS doctor_fee_amount,
            t.previous_fee_amount::float8 AS previous_fee_amount,
            t.action_by
        FROM doctor_fee_transaction t
        JOIN doctor_profile p ON p.doctor_id = t.doctor_id
        WHERE p.doctor_account_id = $1
          AND t.doctor_fee_amount = 650.5
        "#,
    )
    .bind(11001_i32)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(
        changed_fee_transaction.get::<f64, _>("doctor_fee_amount"),
        650.5
    );
    assert_eq!(
        changed_fee_transaction.get::<f64, _>("previous_fee_amount"),
        200.0
    );
    assert_eq!(changed_fee_transaction.get::<i32, _>("action_by"), 7001);
}

#[tokio::test]
async fn pending_events_include_types_in_version_order() {
    let (_pg, pool) = setup_postgres().await;
    seed_pending_draft(&pool, 11004, 9503).await;
    let server = TestServer::new(app_with_failing_publisher(pool.clone()).await).unwrap();

    server
        .post("/approve")
        .json(&approve_body(11004, 17, vec![]))
        .await
        .assert_status_ok();

    server
        .patch("/doctor-active-status")
        .json(&serde_json::json!({
            "doctorAccountId": 11004,
            "isActive": false
        }))
        .await
        .assert_status_ok();

    let expected: Vec<uuid::Uuid> = sqlx::query_scalar(
        "SELECT event_id FROM doctor_profile_event_outbox WHERE doctor_account_id = $1 ORDER BY profile_version",
    )
    .bind(11004_i32)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(expected.len(), 2);

    let attempts: Vec<i32> = sqlx::query_scalar(
        "SELECT attempts FROM doctor_profile_event_outbox WHERE doctor_account_id = $1 ORDER BY profile_version",
    )
    .bind(11004_i32)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(attempts, vec![2, 0]);

    let repo = server::module::backoffice::doctor_management::repo::DoctorManagementRepo::new(pool);
    let pending = repo.pending_events_through(expected[1]).await.unwrap();

    assert_eq!(
        pending
            .iter()
            .map(|event| event.event_id)
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(
        pending
            .iter()
            .map(|event| event.event_type.as_str())
            .collect::<Vec<_>>(),
        vec!["DoctorProfileApproved", "DoctorProfileStatusUpdated"]
    );
}

#[tokio::test]
async fn update_consultation_configuration_skips_fee_transaction_when_fee_unchanged() {
    let (_pg, pool) = setup_postgres().await;
    seed_pending_draft(&pool, 11002, 9501).await;
    let app = app_with_failing_publisher(pool.clone()).await;
    let server = TestServer::new(app).unwrap();

    server
        .post("/approve")
        .json(&approve_body(11002, 17, vec![1]))
        .await
        .assert_status_ok();

    let before_count: i64 = sqlx::query_scalar(
        r#"
        SELECT count(*)
        FROM doctor_fee_transaction t
        JOIN doctor_profile p ON p.doctor_id = t.doctor_id
        WHERE p.doctor_account_id = $1
        "#,
    )
    .bind(11002_i32)
    .fetch_one(&pool)
    .await
    .unwrap();

    let resp = server
        .patch("/consultation-configuration")
        .add_header("tdh-sec-iam-user-identity", backoffice_identity(7001))
        .json(&serde_json::json!({
            "doctorAccountId": 11002,
            "consultationConfig": {
                "supportedLanguages": ["en"],
                "channelTypes": ["video"],
                "durationMinutes": 15,
                "doctorFeeAmount": 200.0
            }
        }))
        .await;

    resp.assert_status_ok();

    let after_count: i64 = sqlx::query_scalar(
        r#"
        SELECT count(*)
        FROM doctor_fee_transaction t
        JOIN doctor_profile p ON p.doctor_id = t.doctor_id
        WHERE p.doctor_account_id = $1
        "#,
    )
    .bind(11002_i32)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(after_count, before_count);
}

#[tokio::test]
async fn update_consultation_configuration_preserves_omitted_duration_and_fee() {
    let (_pg, pool) = setup_postgres().await;
    seed_pending_draft(&pool, 11003, 9502).await;
    let server = TestServer::new(app_with_failing_publisher(pool.clone()).await).unwrap();
    server
        .post("/approve")
        .json(&approve_body(11003, 17, vec![]))
        .await
        .assert_status_ok();

    server
        .patch("/consultation-configuration")
        .add_header("tdh-sec-iam-user-identity", backoffice_identity(7001))
        .json(&serde_json::json!({
            "doctorAccountId": 11003,
            "consultationConfig": { "supportedLanguages": ["en"], "channelTypes": ["video"] }
        }))
        .await
        .assert_status_ok();

    let payload: serde_json::Value = sqlx::query_scalar(
        "SELECT payload FROM doctor_profile_event_outbox WHERE doctor_account_id = $1 AND profile_version = 2",
    )
    .bind(11003_i32)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(payload["consultationConfig"]["durationMinutes"], 15);
    assert_eq!(payload["consultationConfig"]["feeAmount"], "200.00");
    assert_eq!(payload["consultationConfig"]["currency"], "THB");
    assert_compact_consultation_config(&payload);
}

#[tokio::test]
async fn concurrent_approval_produces_one_profile_audit_and_outbox_snapshot() {
    let (_pg, pool) = setup_postgres().await;
    seed_pending_draft(&pool, 13001, 9800).await;
    let repo = server::module::backoffice::onboarding::BackofficeOnboardingRepo::new(pool.clone());
    let config = server::module::profile::configuration::models::DoctorConfiguration {
        channel: vec![server::module::profile::configuration::models::ChannelType::Voice],
        language: vec![server::module::profile::configuration::models::LanguageCode::Th],
        duration: Some(15),
        fee: server::module::profile::configuration::models::Fee {
            amount: Some(200.0),
            currency: "THB".into(),
        },
    };
    let left = tokio::spawn({
        let repo = repo.clone();
        let config = config.clone();
        async move {
            repo.approve_with_setup(13001, 1, 17, &config, &[])
                .await
                .unwrap()
        }
    });
    let right = tokio::spawn(async move {
        repo.approve_with_setup(13001, 2, 17, &config, &[])
            .await
            .unwrap()
    });
    let (left, right) = tokio::join!(left, right);
    assert!(left.unwrap().is_some());
    assert!(right.unwrap().is_some());
    let counts: (i64, i64, i64) = sqlx::query_as(
        r#"
        SELECT
          (SELECT count(*) FROM doctor_profile WHERE doctor_account_id = 13001 AND is_active),
          (SELECT count(*) FROM doctor_profile_transaction WHERE doctor_account_id = 13001),
          (SELECT count(*) FROM doctor_profile_event_outbox WHERE doctor_account_id = 13001)
    "#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(counts, (1, 1, 1));
}

#[tokio::test]
async fn doctor_channel_and_language_mutations_each_enqueue_complete_snapshot() {
    let (_pg, pool) = setup_postgres().await;
    seed_pending_draft(&pool, 14001, 9900).await;
    let onboarding = TestServer::new(app_with_failing_publisher(pool.clone()).await).unwrap();
    onboarding
        .post("/approve")
        .json(&approve_body(14001, 17, vec![]))
        .await
        .assert_status_ok();

    let profile =
        TestServer::new(server::module::profile::configuration::router(pool.clone())).unwrap();
    profile
        .patch("/v1/doctor-channel")
        .add_header("tdh-sec-iam-user-identity", doctor_identity(14001))
        .json(&serde_json::json!({ "channel": ["video"] }))
        .await
        .assert_status_ok();
    profile
        .patch("/v1/doctor-language")
        .add_header("tdh-sec-iam-user-identity", doctor_identity(14001))
        .json(&serde_json::json!({ "language": ["en"] }))
        .await
        .assert_status_ok();

    let snapshots: Vec<serde_json::Value> = sqlx::query_scalar(
        "SELECT payload FROM doctor_profile_event_outbox WHERE doctor_account_id = $1 ORDER BY profile_version",
    )
    .bind(14001_i32)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(snapshots.len(), 3);
    assert_eq!(snapshots[1]["profileVersion"], 2);
    assert_eq!(snapshots[2]["profileVersion"], 3);
    assert_eq!(
        snapshots[2]["consultationConfig"]["channels"],
        serde_json::json!(["video"])
    );
    assert_eq!(
        snapshots[2]["consultationConfig"]["languages"],
        serde_json::json!(["en"])
    );
    assert_eq!(snapshots[2]["consultationConfig"]["feeAmount"], "200.00");
    for snapshot in &snapshots {
        assert_compact_consultation_config(snapshot);
    }
}

#[tokio::test]
async fn reconciliation_continues_after_an_incomplete_legacy_profile() {
    let (_pg, pool) = setup_postgres().await;
    for (account, profile) in [(15001, 9910), (15002, 9911)] {
        seed_pending_draft(&pool, account, profile).await;
        let config = server::module::profile::configuration::models::DoctorConfiguration {
            channel: vec![server::module::profile::configuration::models::ChannelType::Voice],
            language: vec![server::module::profile::configuration::models::LanguageCode::Th],
            duration: Some(15),
            fee: server::module::profile::configuration::models::Fee {
                amount: Some(200.0),
                currency: "THB".into(),
            },
        };
        server::module::backoffice::onboarding::BackofficeOnboardingRepo::new(pool.clone())
            .approve_with_setup(account, 1, 17, &config, &[])
            .await
            .unwrap();
    }
    sqlx::query(
        "DELETE FROM doctor_profile_event_outbox WHERE doctor_account_id IN (15001, 15002)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE doctor_consultation_config cfg SET channel_types = '{}'::channel_type_enum[] FROM doctor_profile dp WHERE cfg.doctor_id = dp.doctor_id AND dp.doctor_account_id = 15002",
    )
    .execute(&pool).await.unwrap();

    let result = server::module::profile_event_outbox::reconcile_approved_doctors(&pool)
        .await
        .unwrap();
    assert_eq!(result.emitted, 1);
    assert_eq!(result.failures, 1);
    let emitted_accounts: Vec<i32> = sqlx::query_scalar(
        "SELECT doctor_account_id FROM doctor_profile_event_outbox ORDER BY doctor_account_id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(emitted_accounts, vec![15001]);
}
