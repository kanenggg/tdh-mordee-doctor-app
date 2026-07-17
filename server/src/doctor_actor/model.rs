use jiff::{civil::Date, civil::Time, Timestamp};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReservationSource {
    Booking,
    FollowUp,
}

// Generated timeslot result
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedTimeslot {
    pub date: Date,
    pub time_ranges: Vec<TimeRange>,
}

// Time range for on-the-fly generation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeRange {
    pub start_time: Time,
    pub end_time: Time,
}

// Routine schedule pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutineSchedule {
    pub day_of_week: u8,
    pub times: Vec<TimeRange>,
}

// Ad-hoc schedule override
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdHocSchedule {
    pub date: Date,
    pub times: Vec<TimeRange>,
}

// Doctor's schedule configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorScheduleConfig {
    pub routine: Vec<RoutineSchedule>,
    pub ad_hoc: Vec<AdHocSchedule>,
    pub slot_duration: i32,
}

// Re-export from timeslot models
pub use crate::module::timeslot::model::{
    RateLimitType, ReleaseReason, ReservationStatus, TimeslotConfirmedEvent, TimeslotReleasedEvent,
    TimeslotReservedEvent,
};

// Reserve result variant
#[derive(Debug, Clone)]
pub enum ReserveResult {
    Success {
        reservation_id: i64,
        expires_at: i64,
    },
    Conflict,
    NoScheduleConfig,
    RateLimitExceeded {
        limit_type: RateLimitType,
        current_count: i32,
        max_allowed: i32,
        retry_after_seconds: i32,
    },
}

#[derive(Debug, Clone)]
pub struct DoctorReservation {
    pub reservation_id: i64,
    pub reserved_from: Timestamp,
    pub reserved_until: Timestamp,
}
