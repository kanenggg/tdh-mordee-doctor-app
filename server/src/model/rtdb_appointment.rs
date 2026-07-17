use jiff::{tz::TimeZone, Timestamp};
use serde::{Deserialize, Serialize};
use tdh_protocol::biz_apm::consultation_event::ConsultationEvent;
use tdh_protocol::biz_apm::{BookingType, ConsultationChannel};

// use crate::model::AppointmentCardStatus;
// use crate::model::AppointmentTime;
use crate::model::consultation_state::{
    AppointmentCardStatus, AppointmentTime, ConsultationPhase, ConsultationState, SessionStateInfo,
};

/// Unified appointment structure stored in Firebase RTDB at
/// `appointments/{doctor_id}/{date}/{booking_id}`.
/// Merges appointment calendar card fields with consultation state tracking.
///
/// Matches the AppointmentCalendarCard format for existing fields,
/// with optional consultation state fields added.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RtdbAppointment {
    // Existing AppointmentCalendarCard fields (preserve exact naming)
    pub appointment_id: String,
    pub patient_account_id: i32,
    pub status: AppointmentCardStatus,
    pub appointment_date: String,
    pub patient_symptom: String,
    pub appointment_time: AppointmentTime,

    // New consultation state fields (optional, skipped if None)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consultation_state: Option<ConsultationState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consultation_phase: Option<ConsultationPhase>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consultation_channel: Option<ConsultationChannel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub booking_type: Option<BookingType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_info: Option<SessionStateInfo>,
}

impl RtdbAppointment {
    /// Build an initial appointment from a ConsultationEvent (used when no
    /// existing RTDB record is found). Extracts fields common to all event
    /// variants and fills in sensible defaults for session-specific fields.
    ///
    /// Note: Consultation events don't include the full patient profile.
    /// Patient names for notifications are fetched from IAM gatekeeper via
    /// PatientService, not stored in RTDB.
    pub fn from_event(event: &ConsultationEvent) -> Self {
        let booking_id = event.booking_id().to_string();
        let patient_identity = event.patient_identity();

        let (channel, booking_type, date, symptom, time, state, phase) = match event {
            ConsultationEvent::ConsultationBooked(e) => (
                e.consultation_channel,
                e.booking_type,
                date_from_epoch(e.consultation_start_time),
                e.symptoms.clone(),
                AppointmentTime {
                    start_time: time_from_epoch(e.consultation_start_time),
                    end_time: time_from_epoch(
                        e.consultation_start_time + e.consultation_duration_in_second as i64,
                    ),
                },
                Some(ConsultationState::ConsultationBooked),
                Some(ConsultationPhase::PreSession),
            ),
            ConsultationEvent::TimeslotReserved(e) => (
                e.consultation_channel,
                BookingType::Scheduled,
                date_from_epoch(e.reserved_from),
                String::new(),
                AppointmentTime {
                    start_time: time_from_epoch(e.reserved_from),
                    end_time: time_from_epoch(e.reserved_from + e.reservation_duration_sec as i64),
                },
                Some(ConsultationState::TimeslotReserved),
                Some(ConsultationPhase::PreSession),
            ),
            ConsultationEvent::SessionCreated(e) => (
                ConsultationChannel::Video,
                BookingType::Scheduled,
                date_from_epoch(e.consultation_start_time),
                String::new(),
                AppointmentTime {
                    start_time: time_from_epoch(e.consultation_start_time),
                    end_time: time_from_epoch(
                        e.consultation_start_time + e.consultation_duration_in_second as i64,
                    ),
                },
                Some(ConsultationState::SessionCreated),
                Some(ConsultationPhase::Session),
            ),
            _ => (
                ConsultationChannel::Video,
                BookingType::Scheduled,
                Timestamp::now()
                    .to_zoned(TimeZone::UTC)
                    .strftime("%Y-%m-%d")
                    .to_string(),
                String::new(),
                AppointmentTime {
                    start_time: String::new(),
                    end_time: String::new(),
                },
                None,
                None,
            ),
        };

        Self {
            appointment_id: booking_id.clone(),
            patient_account_id: patient_identity.account_id,
            appointment_date: date,
            appointment_time: time,
            status: AppointmentCardStatus::UpComing,
            patient_symptom: symptom,
            consultation_state: state,
            consultation_phase: phase,
            consultation_channel: Some(channel),
            booking_type: Some(booking_type),
            session_info: None,
        }
    }
}

fn date_from_epoch(epoch_secs: i64) -> String {
    Timestamp::from_second(epoch_secs)
        .ok()
        .and_then(|ts| {
            ts.in_tz("Asia/Bangkok")
                .ok()
                .map(|zdt| zdt.strftime("%Y-%m-%d").to_string())
        })
        .unwrap_or_else(|| {
            Timestamp::now()
                .to_zoned(TimeZone::UTC)
                .strftime("%Y-%m-%d")
                .to_string()
        })
}

/// Format epoch seconds into Bangkok-local "HH:MM" string for RTDB appointmentTime.
fn time_from_epoch(epoch_secs: i64) -> String {
    Timestamp::from_second(epoch_secs)
        .ok()
        .and_then(|ts| {
            ts.in_tz("Asia/Bangkok")
                .ok()
                .map(|zdt| zdt.strftime("%H:%M").to_string())
        })
        .unwrap_or_else(|| "--:--".to_string())
}
