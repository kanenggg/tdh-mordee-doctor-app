use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::info;
use utoipa::ToSchema;

use crate::core::error::{AppError, AppResult};
use crate::module::timeslot::repo::TimeslotRepo;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OverlappingTimeslot {
    pub timeslot_id: String,
    pub start_time: i64,
    pub end_time: i64,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReservedTimeslot {
    pub reservation_id: String,
    pub timeslot_id: String,
    pub start_time: i64,
    pub end_time: i64,
}

#[async_trait]
pub trait FollowUpReservationRepo: Send + Sync {
    async fn find_overlapping_timeslots(
        &self,
        doctor_id: i32,
        start_time: i64,
        end_time: i64,
    ) -> AppResult<Vec<OverlappingTimeslot>>;

    async fn reserve_follow_up(
        &self,
        doctor_id: i32,
        start_time: i64,
        end_time: i64,
    ) -> AppResult<ReservedTimeslot>;

    async fn get_reserved_follow_up(
        &self,
        appointment_id: &str,
    ) -> AppResult<Option<ReservedTimeslot>>;
}

pub struct FollowUpReservationRepoImpl {
    timeslot_repo: std::sync::Arc<dyn TimeslotRepo>,
}

impl FollowUpReservationRepoImpl {
    pub fn new(timeslot_repo: std::sync::Arc<dyn TimeslotRepo>) -> Self {
        Self { timeslot_repo }
    }
}

#[async_trait]
impl FollowUpReservationRepo for FollowUpReservationRepoImpl {
    async fn find_overlapping_timeslots(
        &self,
        doctor_id: i32,
        start_time: i64,
        end_time: i64,
    ) -> AppResult<Vec<OverlappingTimeslot>> {
        let timeslots = self
            .timeslot_repo
            .find_overlapping_timeslots(doctor_id, start_time, end_time)
            .await?;

        Ok(timeslots
            .into_iter()
            .map(|ts| OverlappingTimeslot {
                timeslot_id: ts.timeslot_id,
                start_time: ts.start_time,
                end_time: ts.end_time,
                status: format!("{:?}", ts.status),
            })
            .collect())
    }

    async fn reserve_follow_up(
        &self,
        doctor_id: i32,
        start_time: i64,
        end_time: i64,
    ) -> AppResult<ReservedTimeslot> {
        let available = self
            .timeslot_repo
            .find_available_timeslots(doctor_id, start_time, end_time)
            .await?;

        let timeslot = available
            .into_iter()
            .next()
            .ok_or_else(|| AppError::BadRequest("No available timeslot found".to_string()))?;

        info!(
            timeslot_id = %timeslot.timeslot_id,
            doctor_id,
            start_time,
            end_time,
            "Found available timeslot for follow-up reservation (stub)"
        );

        Ok(ReservedTimeslot {
            reservation_id: format!("stub-reservation-{}", timeslot.timeslot_id),
            timeslot_id: timeslot.timeslot_id,
            start_time: timeslot.start_time,
            end_time: timeslot.end_time,
        })
    }

    async fn get_reserved_follow_up(
        &self,
        _appointment_id: &str,
    ) -> AppResult<Option<ReservedTimeslot>> {
        info!("get_reserved_follow_up called (stub)");
        Ok(None)
    }
}
