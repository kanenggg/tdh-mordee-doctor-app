//! Domain models for the consultation module.
//!
//! Includes consultation state machine types and the Firebase RTDB appointment
//! record that tracks live consultation state.

use jiff::{tz::TimeZone, Timestamp};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tdh_protocol::biz_apm::consultation_event::ConsultationEvent;
use tdh_protocol::biz_apm::PatientIdentity;
use tdh_protocol::biz_apm::{BookingType, ConsultationChannel};
use utoipa::ToSchema;

// ─────────────────────────────────────────────────────────────────────────────
// Shared RTDB types
// ─────────────────────────────────────────────────────────────────────────────

/// Simple start/end time pair for appointments (formatted as "HH:MM" strings).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AppointmentTime {
    pub start_time: String,
    pub end_time: String,
}

/// Lightweight appointment card status for Firebase RTDB.
/// Matches Scala `AppointmentCardStatus` — serialises as `{"__type": "UpComing"}`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "__type")]
pub enum AppointmentCardStatus {
    #[serde(rename = "Reserved")]
    Reserved,
    #[serde(rename = "UpComing")]
    UpComing,
    #[serde(rename = "Cancelled")]
    Cancelled,
    #[serde(rename = "Missed")]
    Missed,
    #[serde(rename = "Fail")]
    Fail,
    #[serde(rename = "Completed")]
    Completed,
    #[serde(rename = "PendingRecord")]
    PendingRecord,
}

// ─────────────────────────────────────────────────────────────────────────────
// Consultation state machine types
// ─────────────────────────────────────────────────────────────────────────────

/// Consultation phase (high-level lifecycle stage)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ConsultationPhase {
    PreSession,
    Session,
    PostSession,
    Completed,
}

/// Detailed consultation state for state machine validation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum ConsultationState {
    // No state (initial)
    None,

    // Pre-Session states
    TimeslotReserved,
    ConsultationBooked,
    ConsultationCancelled,
    ReservationCancelled,
    ReservationExpired,

    // Session states
    SessionCreated,
    PatientJoined,
    DoctorJoined,
    AllParticipantJoined,
    PatientDisconnected,
    DoctorDisconnected,
    SessionTerminated,

    // Post-Session states
    ConsultationSummarized,
    FollowUpRequired,
    FollowUpRequestExpired,
    PatientAcceptedFollowUp,
    FollowUpCancelled,
}

impl ConsultationState {
    /// Check if this is a terminal state (no further transitions allowed)
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            ConsultationState::ConsultationCancelled
                | ConsultationState::ReservationCancelled
                | ConsultationState::ReservationExpired
                | ConsultationState::FollowUpCancelled
                | ConsultationState::FollowUpRequestExpired
        )
    }

    /// Check if this is a session state
    pub fn is_session(&self) -> bool {
        matches!(
            self,
            ConsultationState::SessionCreated
                | ConsultationState::PatientJoined
                | ConsultationState::DoctorJoined
                | ConsultationState::AllParticipantJoined
                | ConsultationState::PatientDisconnected
                | ConsultationState::DoctorDisconnected
                | ConsultationState::SessionTerminated
        )
    }

    /// Get the phase for this state
    pub fn phase(&self) -> ConsultationPhase {
        match self {
            ConsultationState::None
            | ConsultationState::TimeslotReserved
            | ConsultationState::ConsultationBooked
            | ConsultationState::ConsultationCancelled
            | ConsultationState::ReservationCancelled
            | ConsultationState::ReservationExpired => ConsultationPhase::PreSession,

            ConsultationState::SessionCreated
            | ConsultationState::PatientJoined
            | ConsultationState::DoctorJoined
            | ConsultationState::AllParticipantJoined
            | ConsultationState::PatientDisconnected
            | ConsultationState::DoctorDisconnected
            | ConsultationState::SessionTerminated => ConsultationPhase::Session,

            ConsultationState::ConsultationSummarized
            | ConsultationState::FollowUpRequired
            | ConsultationState::FollowUpRequestExpired
            | ConsultationState::PatientAcceptedFollowUp
            | ConsultationState::FollowUpCancelled => ConsultationPhase::PostSession,
        }
    }

    /// Map consultation state to the RTDB appointment card status
    pub fn to_appointment_status(&self) -> AppointmentCardStatus {
        match self {
            ConsultationState::None
            | ConsultationState::ConsultationBooked
            | ConsultationState::SessionCreated
            | ConsultationState::PatientJoined
            | ConsultationState::DoctorJoined
            | ConsultationState::AllParticipantJoined
            | ConsultationState::PatientDisconnected
            | ConsultationState::DoctorDisconnected => AppointmentCardStatus::UpComing,

            ConsultationState::TimeslotReserved => AppointmentCardStatus::Reserved,

            ConsultationState::ConsultationCancelled | ConsultationState::ReservationCancelled => {
                AppointmentCardStatus::Cancelled
            }

            ConsultationState::ReservationExpired => AppointmentCardStatus::Missed,

            ConsultationState::SessionTerminated => AppointmentCardStatus::PendingRecord,

            ConsultationState::ConsultationSummarized
            | ConsultationState::FollowUpRequired
            | ConsultationState::FollowUpRequestExpired
            | ConsultationState::PatientAcceptedFollowUp
            | ConsultationState::FollowUpCancelled => AppointmentCardStatus::Completed,
        }
    }

    /// Check if a transition from this state to another is valid
    pub fn can_transition_to(&self, next: &ConsultationState) -> bool {
        // Terminal states cannot transition
        if self.is_terminal() {
            return false;
        }

        match self {
            ConsultationState::None => {
                matches!(
                    next,
                    ConsultationState::TimeslotReserved | ConsultationState::ConsultationBooked
                )
            }
            ConsultationState::TimeslotReserved => {
                matches!(
                    next,
                    ConsultationState::ReservationCancelled
                        | ConsultationState::ReservationExpired
                        | ConsultationState::ConsultationBooked
                )
            }
            ConsultationState::ConsultationBooked => {
                matches!(
                    next,
                    ConsultationState::ConsultationCancelled | ConsultationState::SessionCreated
                )
            }
            ConsultationState::SessionCreated => {
                matches!(
                    next,
                    ConsultationState::PatientJoined
                        | ConsultationState::DoctorJoined
                        | ConsultationState::SessionTerminated
                )
            }
            ConsultationState::PatientJoined => {
                matches!(
                    next,
                    ConsultationState::DoctorJoined | ConsultationState::PatientDisconnected
                )
            }
            ConsultationState::DoctorJoined => {
                matches!(
                    next,
                    ConsultationState::PatientJoined
                        | ConsultationState::AllParticipantJoined
                        | ConsultationState::DoctorDisconnected
                )
            }
            ConsultationState::AllParticipantJoined => {
                matches!(
                    next,
                    ConsultationState::PatientDisconnected
                        | ConsultationState::DoctorDisconnected
                        | ConsultationState::SessionTerminated
                )
            }
            ConsultationState::PatientDisconnected => {
                matches!(
                    next,
                    ConsultationState::DoctorDisconnected | ConsultationState::SessionTerminated
                )
            }
            ConsultationState::DoctorDisconnected => {
                matches!(
                    next,
                    ConsultationState::PatientDisconnected | ConsultationState::SessionTerminated
                )
            }
            ConsultationState::SessionTerminated => {
                matches!(next, ConsultationState::ConsultationSummarized)
            }
            ConsultationState::ConsultationSummarized => {
                matches!(
                    next,
                    ConsultationState::FollowUpRequired
                        | ConsultationState::FollowUpRequestExpired
                        | ConsultationState::PatientAcceptedFollowUp
                )
            }
            ConsultationState::FollowUpRequired => {
                matches!(
                    next,
                    ConsultationState::FollowUpRequestExpired
                        | ConsultationState::PatientAcceptedFollowUp
                        | ConsultationState::FollowUpCancelled
                )
            }
            ConsultationState::PatientAcceptedFollowUp => {
                matches!(next, ConsultationState::FollowUpCancelled)
            }
            ConsultationState::ConsultationCancelled
            | ConsultationState::ReservationCancelled
            | ConsultationState::ReservationExpired
            | ConsultationState::FollowUpRequestExpired
            | ConsultationState::FollowUpCancelled => false,
        }
    }
}

/// Information about the current session state
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionStateInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consultation_start_time: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consultation_duration_in_second: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patient_joined_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doctor_joined_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patient_disconnected_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doctor_disconnected_at: Option<i64>,
}

/// Consultation state document stored in Firestore
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConsultationStateDoc {
    pub booking_id: String,
    pub doctor_id: i32,
    pub patient_identity: PatientIdentity,
    pub current_state: ConsultationState,
    pub current_phase: ConsultationPhase,
    #[schema(value_type = String, format = DateTime)]
    pub created_at: Timestamp,
    #[schema(value_type = String, format = DateTime)]
    pub updated_at: Timestamp,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_info: Option<SessionStateInfo>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Firebase RTDB appointment record
// ─────────────────────────────────────────────────────────────────────────────

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_applied_event_key: Option<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub applied_event_keys: HashMap<String, String>,
}

impl RtdbAppointment {
    /// Build an initial appointment from a ConsultationEvent (used when no
    /// existing RTDB record is found). Extracts fields common to all event
    /// variants and fills in sensible defaults for session-specific fields.
    pub fn from_event(event: &ConsultationEvent) -> Self {
        let booking_id = event.booking_id().to_string();
        let patient_identity = event.patient_identity();

        let (channel, booking_type, date, time, state, phase) = match event {
            ConsultationEvent::ConsultationBooked(e) => (
                e.consultation_channel,
                e.booking_type,
                date_from_epoch(e.consultation_start_time),
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
            status: state
                .map(|state| state.to_appointment_status())
                .unwrap_or(AppointmentCardStatus::UpComing),
            consultation_state: state,
            consultation_phase: phase,
            consultation_channel: Some(channel),
            booking_type: Some(booking_type),
            session_info: None,
            last_applied_event_key: None,
            applied_event_keys: HashMap::new(),
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
