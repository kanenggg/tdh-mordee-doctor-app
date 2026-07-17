//! Notification message templates for consultation events (Thai/English)

use jiff::Timestamp;

/// Format epoch seconds into a Bangkok-local "HH:MM" string.
fn format_bkk_time(epoch_secs: i64) -> String {
    Timestamp::from_second(epoch_secs)
        .ok()
        .and_then(|ts| {
            ts.in_tz("Asia/Bangkok")
                .ok()
                .map(|zdt| zdt.strftime("%H:%M").to_string())
        })
        .unwrap_or_else(|| "--:--".to_string())
}

/// Return `(start_display, end_display)` in Bangkok time.
pub fn format_time_range(start_epoch: i64, duration_secs: i64) -> (String, String) {
    let start = format_bkk_time(start_epoch);
    let end = format_bkk_time(start_epoch + duration_secs);
    (start, end)
}

// ── Scheduled: Immediate ────────────────────────────────────────────

pub fn scheduled_immediate_en(patient_name: &str, start: &str, end: &str) -> (String, String) {
    let title = "New Appointment!".to_string();
    let body = format!(
        "You have new Appointment with {} ({} - {})",
        patient_name, start, end
    );
    (title, body)
}

// ── Scheduled: T-15 reminder ────────────────────────────────────────

pub fn scheduled_reminder_en(patient_name: &str, minutes: i64) -> (String, String) {
    let title = format!("Your appointment will start in {} minutes", minutes);
    let body = format!(
        "Your appointment with {} will start in {} minutes",
        patient_name, minutes
    );
    (title, body)
}

// ── Scheduled: T-0 (now) ───────────────────────────────────────────

pub fn scheduled_now_en(patient_name: &str) -> (String, String) {
    let title = "Time for your appointment!".to_string();
    let body = format!(
        "Time for your appointment with {}. Click here to join the session.",
        patient_name
    );
    (title, body)
}

// ── Instant: Immediate ─────────────────────────────────────────────

pub fn instant_immediate_en(patient_name: &str) -> (String, String) {
    let title = "New Instant Appointment!".to_string();
    let body = format!("You have new Instant Appointment with {}", patient_name);
    (title, body)
}

// ── Cancellation ──────────────────────────────────────────────────

pub fn cancellation_en(patient_name: &str) -> (String, String) {
    let title = "Consultation Cancelled".to_string();
    let body = format!("Patient {} has cancelled their consultation.", patient_name);
    (title, body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_time_range() {
        // 2024-02-28 10:00 UTC => 17:00 Bangkok
        let (start, end) = format_time_range(1709110800, 1800);
        assert!(!start.is_empty());
        assert!(!end.is_empty());
    }

    #[test]
    fn test_scheduled_immediate_templates() {
        let (title_en, body_en) = scheduled_immediate_en("John", "10:00", "10:30");
        assert!(title_en.contains("New Appointment"));
        assert!(body_en.contains("John"));
    }

    #[test]
    fn test_instant_immediate_templates() {
        let (title_en, body_en) = instant_immediate_en("John");
        assert!(title_en.contains("Instant"));
        assert!(body_en.contains("John"));
    }
}
