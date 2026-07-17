use std::sync::Arc;

use jiff::{civil::Date, tz::TimeZone, Timestamp};

use crate::core::error::{AppError, AppResult};
use crate::module::profile::common::validate_biz_unit_id;

use super::model::{ScheduleAvailableConfig, UpdateScheduleConfigResponse};
use super::validate::ScheduleConfigValidationError;
use super::window::{drop_past_specific_dates, retain_forward_window};
use super::ConsultationSettingRepo;

/// Current calendar date in Bangkok. The forward window and past-strip are both
/// anchored here regardless of the config's descriptive `timezone` field.
fn bangkok_today() -> Date {
    let tz = TimeZone::get("Asia/Bangkok").expect("Asia/Bangkok is a valid IANA timezone");
    Timestamp::now().to_zoned(tz).date()
}

pub struct ConsultationSettingService {
    repo: Arc<dyn ConsultationSettingRepo>,
}

impl ConsultationSettingService {
    pub fn new(repo: Arc<dyn ConsultationSettingRepo>) -> Self {
        Self { repo }
    }

    pub async fn get_schedule_config(
        &self,
        doctor_id: i32,
        biz_unit_id: i32,
    ) -> AppResult<ScheduleAvailableConfig> {
        validate_biz_unit_id(biz_unit_id)?;

        let mut config = self
            .repo
            .get_schedule_config(doctor_id, biz_unit_id)
            .await?
            .unwrap_or_default();

        retain_forward_window(&mut config, bangkok_today());

        Ok(config)
    }

    pub async fn update_schedule_config(
        &self,
        doctor_id: i32,
        biz_unit_id: i32,
        mut req: ScheduleAvailableConfig,
    ) -> AppResult<UpdateScheduleConfigResponse> {
        validate_biz_unit_id(biz_unit_id)?;

        match req.validate() {
            Ok(()) => {}
            Err(ScheduleConfigValidationError::Invalid(message)) => {
                return Err(AppError::BadRequest(message));
            }
            Err(ScheduleConfigValidationError::ConflictTimeOverlap { days }) => {
                return Ok(UpdateScheduleConfigResponse::ConflictTimeOverlap { days });
            }
        }

        // Forward-looking config: never persist past-dated overrides so the
        // stored blob matches what a read returns.
        drop_past_specific_dates(&mut req, bangkok_today());

        self.repo
            .save_schedule_config(doctor_id, biz_unit_id, &req)
            .await?;

        Ok(UpdateScheduleConfigResponse::Success)
    }
}
