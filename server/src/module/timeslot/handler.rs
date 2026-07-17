use std::sync::Arc;

use axum::{
    extract::{Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use tdh_protocol::timeslot::timeslot::DoctorTimeslot;
use tokio::sync::Mutex;
use utoipa::{IntoParams, ToSchema};

use crate::config::TimeslotConfig;
use crate::core::error::{AppError, AppResult};
use crate::core::user_identity::UserIdentity;
use crate::doctor_actor::repo::DoctorTimeslotRepo;
use crate::module::timeslot::{IdempotencyCache, TimeslotService};

#[derive(Clone)]
pub struct TimeslotState {
    pub service: Arc<TimeslotService>,
    pub idempotency_cache: Arc<Mutex<IdempotencyCache>>,
    pub redis: redis::aio::ConnectionManager,
    pub config: TimeslotConfig,
    pub doctor_timeslot_repo: Arc<dyn DoctorTimeslotRepo>,
    pub consultation_duration_repo:
        std::sync::Arc<dyn crate::module::timeslot::get_my_available_time_slots::repo::ConsultationDurationRepo>,
    pub reserved_timeslots_client:
        std::sync::Arc<dyn crate::module::timeslot::get_my_available_time_slots::gateway::ReservedTimeslotsClientTrait>,
}

#[derive(Debug, Deserialize, IntoParams, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GetAvailableTimeslotsQuery {
    pub doctor_id: i32,
    pub start_time: i64,
    pub end_time: i64,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "__type", rename_all = "PascalCase")]
pub enum GetAvailableTimeslotsResponse {
    AvailableTimeslots {
        timeslots: Vec<crate::module::timeslot::model::Timeslot>,
    },
}

#[utoipa::path(
    get,
    path = "/available",
    tag = "timeslot",
    params(GetAvailableTimeslotsQuery),
    responses(
        (status = 200, description = "Available timeslots", body = GetAvailableTimeslotsResponse),
        (status = 400, description = "Invalid request parameters"),
    )
)]
pub async fn get_available_timeslot(
    State(_state): State<TimeslotState>,
    Query(query): Query<GetAvailableTimeslotsQuery>,
    _identity: UserIdentity,
) -> AppResult<Json<GetAvailableTimeslotsResponse>> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before epoch")
        .as_secs() as i64;

    if query.start_time < now {
        return Err(AppError::BadRequest(
            "start_time must be in the future".to_string(),
        ));
    }

    let max_range_seconds = _state.config.max_query_range_days as i64 * 86400;
    if query.end_time - query.start_time > max_range_seconds {
        return Err(AppError::BadRequest(format!(
            "Date range must not exceed {} days",
            _state.config.max_query_range_days
        )));
    }

    let timeslots = _state
        .service
        .get_available_timeslots(query.doctor_id, query.start_time, query.end_time)
        .await?;

    Ok(Json(GetAvailableTimeslotsResponse::AvailableTimeslots {
        timeslots,
    }))
}

#[derive(Debug, Deserialize, IntoParams, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MyAvailableQuery {
    /// Date-time format (e.g. 2026-04-02)
    pub date: String,
    #[serde(default = "default_timezone")]
    pub time_zone: String,
}

fn default_timezone() -> String {
    "Asia/Bangkok".to_string()
}

/// A single doctor timeslot as returned to the client. This is both the wire
/// type and the OpenAPI schema, so the documentation can't drift from the data.
/// `slotDate`/`startTime`/`endTime` are ISO strings (`YYYY-MM-DD` / `HH:MM:SS`),
/// the civil date/time in the request's `time_zone`.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DoctorTimeslotSchema {
    pub slot_id: i64,
    pub slot_date: String,
    pub start_time: String,
    pub end_time: String,
}

impl From<DoctorTimeslot> for DoctorTimeslotSchema {
    fn from(t: DoctorTimeslot) -> Self {
        Self {
            slot_id: t.slot_id,
            slot_date: t.slot_date.to_string(),
            start_time: t.start_time.to_string(),
            end_time: t.end_time.to_string(),
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "__type", rename_all = "PascalCase")]
pub enum MyAvailableResponse {
    Success {
        timeslots: Vec<DoctorTimeslotSchema>,
    },
    NoScheduleConfig,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_serializes_civil_date_and_time_as_hhmmss_strings() {
        let schema: DoctorTimeslotSchema = DoctorTimeslot {
            slot_id: 1,
            slot_date: "2026-06-18".parse().unwrap(),
            start_time: jiff::civil::Time::new(0, 0, 0, 0).unwrap(),
            end_time: jiff::civil::Time::new(0, 20, 0, 0).unwrap(),
        }
        .into();

        let json = serde_json::to_value(&schema).unwrap();
        assert_eq!(json["slotId"], 1);
        assert_eq!(json["slotDate"], "2026-06-18");
        assert_eq!(json["startTime"], "00:00:00");
        assert_eq!(json["endTime"], "00:20:00");
    }
}
