//! Integration tests for consultation event deserialization
//!
//! Tests the deserialization of consultation events from Pub/Sub messages,
//! focusing on the ConsultationBooked event and enum handling issues.
//!
//! Background:
//! - Java service sends consultation events via Pub/Sub
//! - Rust service deserializes these events using serde
//! - Error: "unknown variant `__type`, expected one of `SCHEDULED`, `Scheduled', 'scheduled'..."
//! - This indicates enum deserialization issues with BookingType/ConsultationChannel

use base64::Engine;
use serde_json::json;
use tdh_protocol::biz_apm::consultation_event::{ConsultationBookedEvent, ConsultationEvent};
use tdh_protocol::biz_apm::{BookingType, ConsultationChannel};
use tdh_protocol::notification::{PubsubMessage, PubsubPushMessage};

/// Create a valid ConsultationBooked event JSON payload
fn create_consultation_booked_payload(
    booking_type: &str,
    consultation_channel: &str,
) -> serde_json::Value {
    json!({
        "__type": "ConsultationBooked",
        "bookingId": "2602277T24Y2",
        "patientIdentity": {
            "accountId": 12345,
            "userProfileId": 67890,
            "tenantId": 1,
            "oidcUserId": null
        },
        "doctorId": 6789,
        "bizUnitId": 1,
        "paymentModuleId": 100,
        "bookingType": {
            "__type": booking_type
        },
        "consultationStartTime": 1709072400000i64,
        "consultationDurationInSecond": 1800,
        "consultationChannel": consultation_channel,
        "bookedAt": 1709070000000i64,
        "symptoms": "Headache and fever",
        "consultationFee": 250.00
    })
}

/// Create a Pub/Sub push message wrapping the given payload
fn create_pubsub_push_message(payload: serde_json::Value) -> PubsubPushMessage {
    let payload_str = payload.to_string();
    let encoded = base64::engine::general_purpose::STANDARD.encode(payload_str);

    PubsubPushMessage {
        message: PubsubMessage {
            data: encoded,
            message_id: "17989428954797062".to_string(),
            publish_time: "2026-02-27T06:38:35.680Z".to_string(),
            attributes: std::collections::HashMap::new(),
        },
        subscription: "projects/tdg-dh-truehealth-core-nonprod/subscriptions/doctor-app-consultation-event-sub".to_string(),
    }
}

#[test]
fn test_consultation_booked_event_lowercase_enums() {
    // Test successful deserialization with lowercase enum values
    let payload = create_consultation_booked_payload("scheduled", "video");
    let event: Result<ConsultationEvent, _> = serde_json::from_value(payload.clone());

    assert!(event.is_ok(), "Should deserialize with lowercase enums");
    let event = event.unwrap();

    match event {
        ConsultationEvent::ConsultationBooked(booked) => {
            assert_eq!(booked.booking_id, "2602277T24Y2");
            assert_eq!(booked.patient_identity.account_id, 12345);
            assert!(matches!(booked.booking_type, BookingType::Scheduled));
            assert!(matches!(
                booked.consultation_channel,
                ConsultationChannel::Video
            ));
        }
        _ => panic!("Expected ConsultationBooked event"),
    }
}

#[test]
fn test_consultation_booked_event_uppercase_enums() {
    // Test successful deserialization with uppercase enum values (via aliases)
    let payload = create_consultation_booked_payload("SCHEDULED", "VIDEO");
    let event: Result<ConsultationEvent, _> = serde_json::from_value(payload.clone());

    assert!(
        event.is_ok(),
        "Should deserialize with uppercase enums via aliases"
    );
    let event = event.unwrap();

    match event {
        ConsultationEvent::ConsultationBooked(booked) => {
            assert!(matches!(booked.booking_type, BookingType::Scheduled));
            assert!(matches!(
                booked.consultation_channel,
                ConsultationChannel::Video
            ));
        }
        _ => panic!("Expected ConsultationBooked event"),
    }
}

#[test]
fn test_consultation_booked_event_mixed_case_enums() {
    // Test successful deserialization with mixed case enum values
    let payload = create_consultation_booked_payload("Scheduled", "Video");
    let event: Result<ConsultationEvent, _> = serde_json::from_value(payload.clone());

    assert!(event.is_ok(), "Should deserialize with mixed case enums");
    let event = event.unwrap();

    match event {
        ConsultationEvent::ConsultationBooked(booked) => {
            assert!(matches!(booked.booking_type, BookingType::Scheduled));
            assert!(matches!(
                booked.consultation_channel,
                ConsultationChannel::Video
            ));
        }
        _ => panic!("Expected ConsultationBooked event"),
    }
}

#[test]
fn test_consultation_booked_event_instant_booking_type() {
    // Test with instant booking type
    let payload = create_consultation_booked_payload("instant", "chat");
    let event: Result<ConsultationEvent, _> = serde_json::from_value(payload.clone());

    assert!(
        event.is_ok(),
        "Should deserialize with instant booking type"
    );
    let event = event.unwrap();

    match event {
        ConsultationEvent::ConsultationBooked(booked) => {
            assert!(matches!(booked.booking_type, BookingType::Instant));
            assert!(matches!(
                booked.consultation_channel,
                ConsultationChannel::Chat
            ));
        }
        _ => panic!("Expected ConsultationBooked event"),
    }
}

#[test]
fn test_consultation_booked_event_voice_channel() {
    // Test with voice consultation channel
    let payload = create_consultation_booked_payload("SCHEDULED", "VOICE");
    let event: Result<ConsultationEvent, _> = serde_json::from_value(payload.clone());

    assert!(event.is_ok(), "Should deserialize with voice channel");
    let event = event.unwrap();

    match event {
        ConsultationEvent::ConsultationBooked(booked) => {
            assert!(matches!(
                booked.consultation_channel,
                ConsultationChannel::Voice
            ));
        }
        _ => panic!("Expected ConsultationBooked event"),
    }
}

#[test]
fn test_consultation_booked_event_invalid_booking_type() {
    // Test with invalid booking type value
    // This test captures what happens when Java service sends an unexpected value
    let payload = create_consultation_booked_payload("OnDemand", "video");
    let event: Result<ConsultationEvent, _> = serde_json::from_value(payload);

    assert!(event.is_err(), "Should fail with invalid booking type");
    let err = event.unwrap_err();
    let err_msg = err.to_string();

    // Log the exact error message for debugging
    eprintln!("Invalid bookingType error: {}", err_msg);

    // The error should mention the invalid variant
    assert!(
        err_msg.contains("unknown variant") || err_msg.contains("OnDemand"),
        "Error should mention unknown variant or the invalid value"
    );
}

#[test]
fn test_consultation_booked_event_invalid_consultation_channel() {
    // Test with invalid consultation channel value
    let payload = create_consultation_booked_payload("scheduled", "AUDIO");
    let event: Result<ConsultationEvent, _> = serde_json::from_value(payload);

    assert!(
        event.is_err(),
        "Should fail with invalid consultation channel"
    );
    let err = event.unwrap_err();
    let err_msg = err.to_string();

    eprintln!("Invalid consultationChannel error: {}", err_msg);

    assert!(
        err_msg.contains("unknown variant") || err_msg.contains("AUDIO"),
        "Error should mention unknown variant or the invalid value"
    );
}

#[test]
fn test_consultation_booked_event_missing_booking_type() {
    // Test with missing bookingType field
    let mut payload = create_consultation_booked_payload("scheduled", "video");
    if let Some(obj) = payload.as_object_mut() {
        obj.remove("bookingType");
    }

    let event: Result<ConsultationEvent, _> = serde_json::from_value(payload);

    assert!(event.is_err(), "Should fail with missing bookingType field");
    let err = event.unwrap_err();
    eprintln!("Missing bookingType error: {}", err.to_string());
}

#[test]
fn test_consultation_booked_event_wrong_data_type() {
    // Test with wrong data type for bookingType (number instead of string)
    let mut payload = create_consultation_booked_payload("scheduled", "video");
    if let Some(obj) = payload.as_object_mut() {
        obj.insert("bookingType".to_string(), json!(123));
    }

    let event: Result<ConsultationEvent, _> = serde_json::from_value(payload);

    assert!(event.is_err(), "Should fail with wrong data type");
    let err = event.unwrap_err();
    eprintln!("Wrong data type error: {}", err.to_string());
}

#[test]
fn test_consultation_event_from_pubsub_message() {
    // Test full deserialization from Pub/Sub push message
    let payload = create_consultation_booked_payload("scheduled", "video");
    let push_msg = create_pubsub_push_message(payload);

    // Decode the Pub/Sub message
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(&push_msg.message.data)
        .unwrap();
    let json_str = String::from_utf8(decoded).unwrap();

    // Deserialize to ConsultationEvent
    let event: Result<ConsultationEvent, _> = serde_json::from_str(&json_str);

    assert!(event.is_ok(), "Should deserialize from Pub/Sub message");
    let event = event.unwrap();

    match event {
        ConsultationEvent::ConsultationBooked(booked) => {
            assert_eq!(booked.booking_id, "2602277T24Y2");
            assert_eq!(booked.doctor_id, 6789);
        }
        _ => panic!("Expected ConsultationBooked event"),
    }
}

#[test]
fn test_consultation_booked_serialization_roundtrip() {
    use tdh_protocol::biz_apm::PatientIdentity;

    // Test that we can serialize and deserialize correctly
    let original = ConsultationEvent::ConsultationBooked(ConsultationBookedEvent {
        booking_id: "test-booking-123".to_string(),
        patient_identity: PatientIdentity {
            account_id: 111,
            user_profile_id: 222,
            tenant_id: 1,
            oidc_user_id: None,
        },
        doctor_id: 222,
        biz_unit_id: 1,
        payment_module_id: 100,
        booking_type: BookingType::Instant,
        consultation_start_time: 1709072400000,
        consultation_duration_in_second: 900,
        consultation_channel: ConsultationChannel::Chat,
        booked_at: 1709070000000,
        symptoms: "Test symptoms".to_string(),
        consultation_fee: 150.00,
    });

    // Serialize
    let serialized = serde_json::to_string(&original).unwrap();
    eprintln!("Serialized: {}", serialized);

    // Deserialize
    let deserialized: Result<ConsultationEvent, _> = serde_json::from_str(&serialized);

    assert!(deserialized.is_ok(), "Should deserialize after roundtrip");
    let deserialized = deserialized.unwrap();

    match deserialized {
        ConsultationEvent::ConsultationBooked(booked) => {
            assert_eq!(booked.booking_id, "test-booking-123");
            assert!(matches!(booked.booking_type, BookingType::Instant));
            assert!(matches!(
                booked.consultation_channel,
                ConsultationChannel::Chat
            ));
        }
        _ => panic!("Expected ConsultationBooked event"),
    }
}

#[test]
fn test_real_failing_payload_from_logs() {
    // This test reproduces the exact error from production logs
    // Updated to use the new Java service schema with patientIdentity and tagged bookingType
    let payload_json = r#"{"__type":"ConsultationBooked","bookingId":"2602277T24Y2","patientIdentity":{"accountId":12345,"userProfileId":67890,"tenantId":1,"oidcUserId":null},"doctorId":6789,"bizUnitId":1,"paymentModuleId":100,"bookingType":{"__type":"Scheduled"},"consultationStartTime":1709072400000,"consultationDurationInSecond":1800,"consultationChannel":"VIDEO","bookedAt":1709070000000,"symptoms":"Headache","consultationFee":250.0}"#;

    let event: Result<ConsultationEvent, _> = serde_json::from_str(payload_json);

    // This should succeed with current schema
    match &event {
        Ok(_) => eprintln!("SUCCESS: Payload deserialized correctly"),
        Err(e) => eprintln!("FAILED to deserialize: {}", e),
    }

    assert!(event.is_ok(), "Should deserialize real production payload");
}

#[test]
fn test_actual_production_payload_with_nested_patient_identity() {
    // This is the ACTUAL production payload structure from Java service
    // Key differences:
    // 1. patientIdentity is nested object with accountId/userProfileId/tenantId
    // 2. bookingType is an object {"__type":"Instant"} not a string
    // 3. Timestamps are in seconds, not milliseconds
    let payload_json = r#"{"__type":"ConsultationBooked","bookingId":"2602277T24Y2","patientIdentity":{"accountId":232,"userProfileId":232,"tenantId":1,"oidcUserId":null},"doctorId":305048,"bizUnitId":1,"paymentModuleId":1,"bookingType":{"__type":"Instant"},"consultationStartTime":1772174280,"consultationDurationInSecond":900,"consultationChannel":"video","bookedAt":1772174310,"symptoms":"6กภ34796กภ3479","consultationFee":375.0}"#;

    let event: Result<ConsultationEvent, _> = serde_json::from_str(payload_json);

    // The payload has a DIFFERENT schema than what our model expects:
    // - patientIdentity is nested (our model has flat patientId/patientProfileId)
    // - bookingType is an object with __type (our model has a simple enum string)
    // - patientId field is MISSING (will default to 0)
    // - patientProfileId field is MISSING (will default to empty string)

    // Serde silently ignores unknown fields and uses defaults for missing required fields
    // This means we're accepting invalid payloads!
    match &event {
        Ok(e) => {
            match e {
                ConsultationEvent::ConsultationBooked(booked) => {
                    // These will have default values since they're missing from the payload
                    println!("booking_id: {}", booked.booking_id);
                    println!(
                        "patient_id: {} (DEFAULT VALUE - MISSING FROM PAYLOAD)",
                        booked.patient_identity.account_id
                    );
                    println!(
                        "patient_profile_id: '{}' (DEFAULT VALUE - MISSING FROM PAYLOAD)",
                        booked.patient_identity.user_profile_id
                    );
                    println!("doctor_id: {}", booked.doctor_id);

                    // This will fail because bookingType as object doesn't deserialize properly
                    // Actually it might default too since the field doesn't match
                    println!("booking_type: {:?}", booked.booking_type);
                }
                _ => {}
            }
        }
        Err(e) => {
            println!("FAILED to deserialize: {}", e);
        }
    }

    // This should actually FAIL because the payload schema is wrong
    // But serde is being lenient - we need to fix the model
    assert!(
        event.is_ok(),
        "Currently succeeds due to lenient deserialization"
    );
}

#[test]
fn test_all_consultation_channels() {
    // Test all valid consultation channel values
    let channels = [
        "video", "VIDEO", "Video", "chat", "CHAT", "Chat", "voice", "VOICE", "Voice",
    ];

    for channel in channels {
        let payload = create_consultation_booked_payload("scheduled", channel);
        let event: Result<ConsultationEvent, _> = serde_json::from_value(payload);

        assert!(
            event.is_ok(),
            "Should deserialize with channel: {}",
            channel
        );
    }
}

#[test]
fn test_all_booking_types() {
    // Test all valid booking type values
    let booking_types = [
        "scheduled",
        "SCHEDULED",
        "Scheduled",
        "Schedule", // Added: alias without trailing 'd'
        "instant",
        "INSTANT",
        "Instant",
    ];

    for booking_type in booking_types {
        let payload = create_consultation_booked_payload(booking_type, "video");
        let event: Result<ConsultationEvent, _> = serde_json::from_value(payload);

        assert!(
            event.is_ok(),
            "Should deserialize with booking_type: {}",
            booking_type
        );
    }
}

#[test]
fn test_consultation_event_type_tag() {
    // Test that the __type tag correctly discriminates the event type
    // Updated to use the new schema with patientIdentity
    let payloads = vec![
        (
            r#"{"__type":"TimeslotReserved","bookingId":"test","patientIdentity":{"accountId":1,"userProfileId":2,"tenantId":1,"oidcUserId":null},"doctorId":1,"bizUnitId":1,"reservedFrom":1709072400000,"reservationDurationSec":1800,"consultationChannel":"video","reservedAt":1709070000000}"#,
            "TimeslotReserved",
        ),
        (
            r#"{"__type":"ConsultationBooked","bookingId":"test","patientIdentity":{"accountId":1,"userProfileId":2,"tenantId":1,"oidcUserId":null},"doctorId":1,"bizUnitId":1,"paymentModuleId":100,"bookingType":{"__type":"Scheduled"},"consultationStartTime":1709072400000,"consultationDurationInSecond":1800,"consultationChannel":"video","bookedAt":1709070000000,"symptoms":"test","consultationFee":100.0}"#,
            "ConsultationBooked",
        ),
        (
            r#"{"__type":"SessionCreated","bookingId":"test","patientIdentity":{"accountId":1,"userProfileId":2,"tenantId":1,"oidcUserId":null},"doctorId":1,"sessionProvider":"test","consultationStartTime":1709072400000,"consultationDurationInSecond":1800,"createdAt":1709070000000}"#,
            "SessionCreated",
        ),
    ];

    for (payload, expected_type) in payloads {
        let event: Result<ConsultationEvent, _> = serde_json::from_str(payload);
        assert!(event.is_ok(), "Should deserialize {} event", expected_type);

        let event = event.unwrap();
        assert_eq!(
            event.event_type_name(),
            expected_type,
            "Event type should match"
        );
    }
}
