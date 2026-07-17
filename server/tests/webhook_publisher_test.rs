//! Unit and emulator coverage for the generic Pub/Sub publisher.
//! DoctorProfile wire delivery is covered by the durable outbox relay tests in server-bg.
//! Integration tests for the Pub/Sub publisher service.
//!
//! Tests that don't require a live Pub/Sub emulator run by default.
//! Tests requiring the emulator are marked with `#[ignore]` and can be run with:
//!
//! ```bash
//! # Start the emulator first:
//! gcloud beta emulators pubsub start --project=test-project
//!
//! # Then run:
//! PUBSUB_EMULATOR_HOST=localhost:8085 cargo test webhook_publisher -- --ignored
//! ```

use serde::Serialize;
use server::config::{PubsubConfig, PubsubTopics};
use server::core::error::AppError;
use std::collections::HashMap;
#[test]
fn test_pubsub_config_deserializes_from_toml() {
    let toml_str = r#"
        gcp_project_id = "my-project"
        emulator_host = "localhost:8085"

        [topics]
        appointments = "appointments"
        consultations = "consultations"
        system = "system"
        broadcast = "broadcast"
        doctor_notifications = "doctor-notifications"
        doctor_profile_approved = "doctor-profile"
        doctor_profile_status_updated = "doctor-profile.status-updated"

        [subscriptions]
        notification = "notification-sub"
        consultation = "consultation-sub"
    "#;

    let config: PubsubConfig = toml::from_str(toml_str).unwrap();
    assert_eq!(config.gcp_project_id, "my-project");
    assert_eq!(config.emulator_host.as_deref(), Some("localhost:8085"));
    assert_eq!(config.topics.appointments, "appointments");
    assert_eq!(config.topics.consultations, "consultations");
    assert_eq!(config.topics.system, "system");
    assert_eq!(config.topics.broadcast, "broadcast");
    assert_eq!(config.topics.doctor_notifications, "doctor-notifications");
    assert_eq!(config.topics.doctor_profile_approved, "doctor-profile");
    assert_eq!(
        config.topics.doctor_profile_status_updated,
        "doctor-profile.status-updated"
    );
    assert_eq!(config.subscriptions.notification, "notification-sub");
    assert_eq!(config.subscriptions.consultation, "consultation-sub");
}

#[test]
fn test_pubsub_config_empty_emulator_host() {
    let toml_str = r#"
        gcp_project_id = "my-project"
        emulator_host = ""

        [topics]
        appointments = "appt"
        consultations = "consult"
        system = "sys"
        broadcast = "bcast"
        doctor_notifications = "doc-notif"
        doctor_profile_approved = "doctor-profile"
        doctor_profile_status_updated = "doctor-profile.status-updated"

        [subscriptions]
        notification = "notif-sub"
        consultation = "consult-sub"
    "#;

    let config: PubsubConfig = toml::from_str(toml_str).unwrap();
    let use_emulator = config.emulator_host.as_ref().is_some_and(|h| !h.is_empty());
    assert!(!use_emulator);
}

#[test]
fn test_pubsub_config_missing_emulator_host_defaults_to_none() {
    let toml_str = r#"
        gcp_project_id = "my-project"

        [topics]
        appointments = "appt"
        consultations = "consult"
        system = "sys"
        broadcast = "bcast"
        doctor_notifications = "doc-notif"
        doctor_profile_approved = "doctor-profile"
        doctor_profile_status_updated = "doctor-profile.status-updated"

        [subscriptions]
        notification = "notif-sub"
        consultation = "consult-sub"
    "#;

    let config: PubsubConfig = toml::from_str(toml_str).unwrap();
    assert!(config.emulator_host.is_none());
}

// ============================================================================
// Serialization Tests (payload formatting)
// ============================================================================

#[derive(Serialize)]
struct TestPayload {
    event_type: String,
    doctor_id: i32,
    message: String,
}

#[test]
fn test_payload_serializes_to_json() {
    let payload = TestPayload {
        event_type: "appointment_created".to_string(),
        doctor_id: 42,
        message: "New appointment scheduled".to_string(),
    };

    let json = serde_json::to_vec(&payload).unwrap();
    let parsed: serde_json::Value = serde_json::from_slice(&json).unwrap();

    assert_eq!(parsed["event_type"], "appointment_created");
    assert_eq!(parsed["doctor_id"], 42);
    assert_eq!(parsed["message"], "New appointment scheduled");
}

#[derive(Serialize)]
struct NestedPayload {
    metadata: HashMap<String, String>,
    items: Vec<String>,
}

#[test]
fn test_nested_payload_serializes_correctly() {
    let mut metadata = HashMap::new();
    metadata.insert("source".to_string(), "doctor-app".to_string());

    let payload = NestedPayload {
        metadata,
        items: vec!["a".to_string(), "b".to_string()],
    };

    let json = serde_json::to_vec(&payload).unwrap();
    let parsed: serde_json::Value = serde_json::from_slice(&json).unwrap();

    assert_eq!(parsed["metadata"]["source"], "doctor-app");
    assert_eq!(parsed["items"].as_array().unwrap().len(), 2);
}

#[test]
fn test_unicode_payload_serializes_correctly() {
    let payload = TestPayload {
        event_type: "system".to_string(),
        doctor_id: 1,
        message: "สวัสดี - こんにちは - 🏥".to_string(),
    };

    let json = serde_json::to_vec(&payload).unwrap();
    let roundtrip: serde_json::Value = serde_json::from_slice(&json).unwrap();
    assert_eq!(roundtrip["message"], "สวัสดี - こんにちは - 🏥");
}

// ============================================================================
// Error Variant Tests
// ============================================================================

#[test]
fn test_pubsub_publish_error_variant_display() {
    let err = AppError::PubsubPublishError("connection refused".to_string());
    assert_eq!(err.to_string(), "Pub/Sub publish error: connection refused");
}

#[test]
fn test_pubsub_publish_error_is_500() {
    use axum::response::IntoResponse;
    let err = AppError::PubsubPublishError("topic not found".to_string());
    let response = err.into_response();
    assert_eq!(
        response.status(),
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    );
}

// ============================================================================
// Topic Config Tests
// ============================================================================

#[test]
fn test_topics_config_fields() {
    let topics = PubsubTopics {
        appointments: "custom-appointments-topic".to_string(),
        consultations: "custom-consultations-topic".to_string(),
        system: "custom-system-topic".to_string(),
        broadcast: "custom-broadcast-topic".to_string(),
        doctor_notifications: "custom-doctor-notifications-topic".to_string(),
        doctor_profile_approved: "doctor-profile".to_string(),
        doctor_profile_status_updated: "doctor-profile-status-updated".to_string(),
    };

    assert_eq!(topics.appointments, "custom-appointments-topic");
    assert_eq!(topics.consultations, "custom-consultations-topic");
    assert_eq!(topics.system, "custom-system-topic");
    assert_eq!(topics.broadcast, "custom-broadcast-topic");
    assert_eq!(
        topics.doctor_notifications,
        "custom-doctor-notifications-topic"
    );
}

// ============================================================================
// Emulator Integration Tests (require running pubsub emulator)
// ============================================================================

#[tokio::test]
#[ignore]
async fn test_publisher_new_with_emulator() {
    let config = PubsubConfig {
        gcp_project_id: "test-project".to_string(),
        emulator_host: Some("localhost:8085".to_string()),
        topics: PubsubTopics {
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

    let result = server::module::webhook::PubsubPublisher::new(&config).await;
    assert!(result.is_ok(), "Publisher should connect to emulator");
}

#[tokio::test]
#[ignore]
async fn test_publish_and_receive_via_emulator() {
    let config = PubsubConfig {
        gcp_project_id: "test-project".to_string(),
        emulator_host: Some("localhost:8085".to_string()),
        topics: PubsubTopics {
            appointments: "test-appointments".to_string(),
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

    let publisher = server::module::webhook::PubsubPublisher::new(&config)
        .await
        .expect("Publisher should connect to emulator");

    let payload = TestPayload {
        event_type: "test".to_string(),
        doctor_id: 99,
        message: "Integration test message".to_string(),
    };

    let result = publisher
        .publish(&config.topics.appointments, &payload)
        .await;
    assert!(
        result.is_ok(),
        "Should publish to emulator topic: {:?}",
        result.err()
    );

    let message_id = result.unwrap();
    assert!(!message_id.is_empty(), "Message ID should not be empty");
}

#[tokio::test]
#[ignore]
async fn test_publish_with_ordering_key_via_emulator() {
    let config = PubsubConfig {
        gcp_project_id: "test-project".to_string(),
        emulator_host: Some("localhost:8085".to_string()),
        topics: PubsubTopics {
            appointments: "test-appointments".to_string(),
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

    let publisher = server::module::webhook::PubsubPublisher::new(&config)
        .await
        .expect("Publisher should connect to emulator");

    let payload = serde_json::json!({
        "appointment_id": "appt-123",
        "status": "confirmed",
    });

    let mut attrs = HashMap::new();
    attrs.insert("source".to_string(), "doctor-app".to_string());

    let result = publisher
        .publish_with_options(
            &config.topics.appointments,
            &payload,
            Some("appt-123"),
            Some(attrs),
        )
        .await;

    assert!(
        result.is_ok(),
        "Should publish with ordering key: {:?}",
        result.err()
    );
}
