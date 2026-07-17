//! Tests for consultation notification templates and patient name fallback.

use server_bg::module::doctor_notification::domain::notification_templates;

// ============================================================================
// Template Tests
// ============================================================================

#[test]
fn test_scheduled_immediate_generates_english() {
    let (title_en, body_en) =
        notification_templates::scheduled_immediate_en("John", "10:00", "10:30");
    assert_eq!(title_en, "New Appointment!");
    assert!(body_en.contains("John"));
    assert!(body_en.contains("10:00 - 10:30"));
}

#[test]
fn test_scheduled_reminder_generates_english() {
    let (title_en, body_en) = notification_templates::scheduled_reminder_en("John", 15);
    assert!(title_en.contains("15 minutes"));
    assert!(body_en.contains("John"));
}

#[test]
fn test_scheduled_now_generates_english() {
    let (title_en, body_en) = notification_templates::scheduled_now_en("John");
    assert_eq!(title_en, "Time for your appointment!");
    assert!(body_en.contains("John"));
    assert!(body_en.contains("Click here"));
}

#[test]
fn test_instant_immediate_generates_english() {
    let (title_en, body_en) = notification_templates::instant_immediate_en("John");
    assert_eq!(title_en, "New Instant Appointment!");
    assert!(body_en.contains("Instant"));
    assert!(body_en.contains("John"));
}

#[test]
fn test_format_time_range_produces_valid_strings() {
    // 2024-02-28 10:00:00 UTC => 17:00 BKK, +1800s => 17:30 BKK
    let (start, end) = notification_templates::format_time_range(1709110800, 1800);
    assert_eq!(start.len(), 5); // HH:MM
    assert_eq!(end.len(), 5);
    assert_ne!(start, end);
}

// ============================================================================
// Patient Name Fallback Tests
// ============================================================================

#[tokio::test]
async fn test_patient_service_fallback_on_unreachable_server() {
    use common::patient::PatientService;

    // Point to a non-existent server so the HTTP call will fail quickly
    let svc = PatientService::new("http://127.0.0.1:1".to_string());
    let name = svc.get_patient_name(999).await;
    assert_eq!(name, "Patient #999");
}

// ============================================================================
// Notification Payload Structure Tests
// ============================================================================

#[test]
fn test_scheduled_booking_should_produce_three_notification_types() {
    // Verify the template functions exist for all 3 scheduled notification types
    let patient = "Test Patient";
    let (start, end) = notification_templates::format_time_range(1709110800, 1800);

    let (t1, b1) = notification_templates::scheduled_immediate_en(patient, &start, &end);
    assert!(!t1.is_empty());
    assert!(!b1.is_empty());

    let (t2, b2) = notification_templates::scheduled_reminder_en(patient, 15);
    assert!(!t2.is_empty());
    assert!(!b2.is_empty());

    let (t3, b3) = notification_templates::scheduled_now_en(patient);
    assert!(!t3.is_empty());
    assert!(!b3.is_empty());
}

#[test]
fn test_instant_booking_should_produce_one_notification_type() {
    let patient = "Test Patient";

    let (title, body) = notification_templates::instant_immediate_en(patient);
    assert!(!title.is_empty());
    assert!(!body.is_empty());
    assert!(title.contains("Instant"));
}
