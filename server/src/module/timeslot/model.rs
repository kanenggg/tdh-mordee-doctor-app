use garde::Validate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TimeslotStatus {
    Free,
    Reserved,
    Confirmed,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[sqlx(
    type_name = "timeslot_status_enum",
    rename_all = "SCREAMING_SNAKE_CASE"
)]
pub enum TimeslotStatusDb {
    Free,
    Reserved,
    Confirmed,
}

impl From<TimeslotStatusDb> for TimeslotStatus {
    fn from(db: TimeslotStatusDb) -> Self {
        match db {
            TimeslotStatusDb::Free => TimeslotStatus::Free,
            TimeslotStatusDb::Reserved => TimeslotStatus::Reserved,
            TimeslotStatusDb::Confirmed => TimeslotStatus::Confirmed,
        }
    }
}

impl From<TimeslotStatus> for TimeslotStatusDb {
    fn from(status: TimeslotStatus) -> Self {
        match status {
            TimeslotStatus::Free => TimeslotStatusDb::Free,
            TimeslotStatus::Reserved => TimeslotStatusDb::Reserved,
            TimeslotStatus::Confirmed => TimeslotStatusDb::Confirmed,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct Timeslot {
    #[garde(skip)]
    pub timeslot_id: String,
    #[garde(skip)]
    pub doctor_id: i32,
    #[garde(skip)]
    pub start_time: i64,
    #[garde(skip)]
    pub end_time: i64,
    #[garde(skip)]
    pub is_instant: bool,
    #[garde(skip)]
    pub status: TimeslotStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReservationStatus {
    Pending,
    Confirmed,
    Cancelled,
    Expired,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[sqlx(
    type_name = "reservation_status_enum",
    rename_all = "SCREAMING_SNAKE_CASE"
)]
pub enum ReservationStatusDb {
    Pending,
    Confirmed,
    Cancelled,
    Expired,
}

impl From<ReservationStatusDb> for ReservationStatus {
    fn from(db: ReservationStatusDb) -> Self {
        match db {
            ReservationStatusDb::Pending => ReservationStatus::Pending,
            ReservationStatusDb::Confirmed => ReservationStatus::Confirmed,
            ReservationStatusDb::Cancelled => ReservationStatus::Cancelled,
            ReservationStatusDb::Expired => ReservationStatus::Expired,
        }
    }
}

impl From<ReservationStatus> for ReservationStatusDb {
    fn from(status: ReservationStatus) -> Self {
        match status {
            ReservationStatus::Pending => ReservationStatusDb::Pending,
            ReservationStatus::Confirmed => ReservationStatusDb::Confirmed,
            ReservationStatus::Cancelled => ReservationStatusDb::Cancelled,
            ReservationStatus::Expired => ReservationStatusDb::Expired,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct Reservation {
    #[garde(skip)]
    pub id: String,
    #[garde(skip)]
    pub timeslot_id: String,
    #[garde(skip)]
    pub doctor_id: i32,
    #[garde(skip)]
    pub patient_id: i32,
    #[garde(skip)]
    pub status: ReservationStatus,
    #[garde(skip)]
    pub correlation_id: String,
    #[garde(skip)]
    pub booking_id: Option<String>,
    #[garde(skip)]
    pub payment_reference: Option<String>,
    #[garde(skip)]
    pub expires_at: i64,
    #[garde(skip)]
    pub created_at: i64,
    #[garde(skip)]
    pub confirmed_at: Option<i64>,
    #[garde(skip)]
    pub cancelled_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RateLimitType {
    Daily,
    Weekly,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CancelReason {
    UserCancelled,
    PaymentFailed,
    SystemTimeout,
    AdminOverride,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "__type", rename_all = "PascalCase")]
pub enum TimeslotReservedEvent {
    TimeslotReserved {
        reservation_id: String,
        timeslot_id: String,
        doctor_id: i32,
        patient_id: i32,
        expires_at: i64,
        reserved_at: i64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "__type", rename_all = "PascalCase")]
pub enum TimeslotConfirmedEvent {
    TimeslotConfirmed {
        reservation_id: String,
        timeslot_id: String,
        booking_id: String,
        doctor_id: i32,
        patient_id: i32,
        confirmed_at: i64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReleaseReason {
    Expired,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "__type", rename_all = "PascalCase")]
pub enum TimeslotReleasedEvent {
    TimeslotReleased {
        timeslot_id: String,
        doctor_id: i32,
        reservation_id: String,
        released_at: i64,
        reason: ReleaseReason,
    },
}
