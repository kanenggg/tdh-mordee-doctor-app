use std::sync::Arc;

use super::{models::ConsultationAvailability, AvailabilityRepo};
use crate::core::error::AppResult;
use crate::module::profile::common::validate_biz_unit_id;

pub struct AvailabilityService {
    repo: Arc<dyn AvailabilityRepo>,
}

impl AvailabilityService {
    pub fn new(repo: Arc<dyn AvailabilityRepo>) -> Self {
        Self { repo }
    }

    pub async fn set_schedule_availability(
        &self,
        doctor_id: i32,
        biz_unit_id: i32,
        available: bool,
    ) -> AppResult<()> {
        validate_biz_unit_id(biz_unit_id)?;
        self.repo
            .set_schedule_availability(doctor_id, biz_unit_id, available)
            .await
    }

    pub async fn set_instant_availability(
        &self,
        doctor_id: i32,
        biz_unit_id: i32,
        available: bool,
    ) -> AppResult<()> {
        validate_biz_unit_id(biz_unit_id)?;
        self.repo
            .set_instant_availability(doctor_id, biz_unit_id, available)
            .await
    }

    pub async fn get_availability(
        &self,
        doctor_id: i32,
        biz_unit_id: i32,
    ) -> AppResult<ConsultationAvailability> {
        validate_biz_unit_id(biz_unit_id)?;
        self.repo.get_availability(doctor_id, biz_unit_id).await
    }
}
