//! Run with `PUBSUB_EMULATOR_HOST=localhost:8085 cargo test -p server-bg --test
//! doctor_profile_outbox_pubsub_emulator_test -- --ignored` after `just pubsub-emulator`.
use common::config::{PubsubConfig, PubsubSubscriberConfig, PubsubSubscriptions, PubsubTopics};
use common::messaging::PubsubPublisher;
use google_cloud_pubsub::subscription::SubscriptionConfig;
use server_bg::module::doctor_profile_outbox::{
    DoctorProfileOutboxPublisher, LeasedOutboxEvent, PubsubDoctorProfileOutboxPublisher,
};
use std::sync::Arc;
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires a running Pub/Sub emulator; use just pubsub-emulator"]
async fn relay_routes_persisted_events_to_contract_topics() {
    let host = std::env::var("PUBSUB_EMULATOR_HOST")
        .expect("PUBSUB_EMULATOR_HOST must name the running Pub/Sub emulator");
    let approved_topic_name = "doctor-profile";
    let status_topic_name = "doctor-profile-status-updated";
    let config = PubsubConfig {
        gcp_project_id: "test-project".to_string(),
        emulator_host: Some(host),
        topics: PubsubTopics {
            appointments: String::new(),
            consultations: String::new(),
            system: String::new(),
            broadcast: String::new(),
            doctor_notifications: String::new(),
            doctor_profile_approved: approved_topic_name.to_string(),
            doctor_profile_status_updated: status_topic_name.to_string(),
        },
        subscriptions: PubsubSubscriptions {
            notification: String::new(),
            consultation: String::new(),
            doctor_calendar_update: None,
            doctor_notification_send: None,
        },
        subscriber: PubsubSubscriberConfig::default(),
    };
    let publisher = Arc::new(PubsubPublisher::new(&config).await.unwrap());
    let approved_topic = publisher.client().topic(approved_topic_name);
    if !approved_topic.exists(None).await.unwrap() {
        approved_topic.create(None, None).await.unwrap();
    }
    let status_topic = publisher.client().topic(status_topic_name);
    if !status_topic.exists(None).await.unwrap() {
        status_topic.create(None, None).await.unwrap();
    }

    let suffix = Uuid::new_v4().simple().to_string();
    let approved_subscription_name = format!("doctor-profile-approved-{suffix}");
    let approved_subscription = publisher.client().subscription(&approved_subscription_name);
    approved_subscription
        .create(
            approved_topic.fully_qualified_name(),
            SubscriptionConfig::default(),
            None,
        )
        .await
        .unwrap();
    let status_subscription_name = format!("doctor-profile-status-updated-{suffix}");
    let status_subscription = publisher.client().subscription(&status_subscription_name);
    status_subscription
        .create(
            status_topic.fully_qualified_name(),
            SubscriptionConfig::default(),
            None,
        )
        .await
        .unwrap();

    let approved_event_id = Uuid::new_v4();
    let approved_event = LeasedOutboxEvent {
        event_id: approved_event_id,
        doctor_account_id: 2443,
        event_type: "DoctorProfileApproved".to_string(),
        schema_version: 2,
        profile_version: 9,
        payload: serde_json::json!({
            "__type": "DoctorProfileApproved",
            "eventId": approved_event_id,
            "doctorAccountId": 2443,
            "schemaVersion": 2,
            "profileVersion": 9,
            "consultationConfig": {
                "channels": ["voice"],
                "languages": ["th"],
                "durationMinutes": 15,
                "feeAmount": "200.00",
                "currency": "THB"
            }
        }),
        attempts: 1,
        lease_token: Uuid::new_v4(),
    };
    let status_event_id = Uuid::new_v4();
    let status_event = LeasedOutboxEvent {
        event_id: status_event_id,
        doctor_account_id: 2443,
        event_type: "DoctorProfileStatusUpdated".to_string(),
        schema_version: 3,
        profile_version: 10,
        payload: serde_json::json!({
            "__type": "DoctorProfileStatusUpdated",
            "eventId": status_event_id,
            "doctorId": "11111111-1111-1111-1111-111111111111",
            "doctorAccountId": 2443,
            "doctorProfileId": 8891,
            "isActive": false,
            "statusUpdatedAt": 1718668800,
            "occurredAt": 1718668800,
            "schemaVersion": 3,
            "profileVersion": 10
        }),
        attempts: 1,
        lease_token: Uuid::new_v4(),
    };
    let outbox_publisher = PubsubDoctorProfileOutboxPublisher::new(
        publisher,
        approved_topic_name.to_string(),
        status_topic_name.to_string(),
    );
    outbox_publisher.publish(&approved_event).await.unwrap();
    outbox_publisher.publish(&status_event).await.unwrap();

    let approved_messages = approved_subscription.pull(10, None).await.unwrap();
    assert_eq!(approved_messages.len(), 1);
    let approved_message = &approved_messages[0];
    assert_eq!(approved_message.message.ordering_key, "2443");
    assert_eq!(
        approved_message.message.attributes.get("eventType"),
        Some(&"DoctorProfileApproved".to_string())
    );
    assert_eq!(
        approved_message.message.attributes.get("eventId"),
        Some(&approved_event_id.to_string())
    );
    assert_eq!(
        approved_message.message.attributes.get("schemaVersion"),
        Some(&"2".to_string())
    );
    let approved_payload: serde_json::Value =
        serde_json::from_slice(&approved_message.message.data).unwrap();
    assert_eq!(approved_payload["schemaVersion"], 2);
    assert_eq!(approved_payload, approved_event.payload);
    approved_message.ack().await.unwrap();

    let status_messages = status_subscription.pull(10, None).await.unwrap();
    assert_eq!(status_messages.len(), 1);
    let status_message = &status_messages[0];
    assert_eq!(status_message.message.ordering_key, "2443");
    assert_eq!(
        status_message.message.attributes.get("eventType"),
        Some(&"DoctorProfileStatusUpdated".to_string())
    );
    assert_eq!(
        status_message.message.attributes.get("eventId"),
        Some(&status_event_id.to_string())
    );
    assert_eq!(
        status_message.message.attributes.get("schemaVersion"),
        Some(&"3".to_string())
    );
    let status_payload: serde_json::Value =
        serde_json::from_slice(&status_message.message.data).unwrap();
    assert_eq!(status_payload, status_event.payload);
    assert_eq!(status_payload["isActive"], false);
    status_message.ack().await.unwrap();
}
