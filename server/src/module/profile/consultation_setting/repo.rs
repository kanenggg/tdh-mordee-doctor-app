use async_trait::async_trait;
use sqlx::PgPool;

use super::model::ScheduleAvailableConfig;
use crate::core::error::{AppError, AppResult};

#[async_trait]
pub trait ConsultationSettingRepo: Send + Sync {
    async fn get_schedule_config(
        &self,
        doctor_id: i32,
        biz_unit_id: i32,
    ) -> AppResult<Option<ScheduleAvailableConfig>>;

    async fn save_schedule_config(
        &self,
        doctor_id: i32,
        biz_unit_id: i32,
        config: &ScheduleAvailableConfig,
    ) -> AppResult<()>;
}

#[derive(Debug, Clone)]
pub struct ConsultationSettingRepoPsql {
    pool: PgPool,
}

impl ConsultationSettingRepoPsql {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ConsultationSettingRepo for ConsultationSettingRepoPsql {
    async fn get_schedule_config(
        &self,
        doctor_id: i32,
        biz_unit_id: i32,
    ) -> AppResult<Option<ScheduleAvailableConfig>> {
        let (value,): (Option<serde_json::Value>,) =
            sqlx::query_as("SELECT get_consultation_schedule_config($1, $2)")
                .bind(doctor_id)
                .bind(biz_unit_id)
                .fetch_one(&self.pool)
                .await?;

        value
            .map(serde_json::from_value)
            .transpose()
            .map_err(|e| AppError::InternalError(format!("invalid schedule config JSON: {e}")))
    }

    async fn save_schedule_config(
        &self,
        doctor_id: i32,
        biz_unit_id: i32,
        config: &ScheduleAvailableConfig,
    ) -> AppResult<()> {
        let value = serde_json::to_value(config)
            .map_err(|e| AppError::InternalError(format!("failed to encode config: {e}")))?;

        sqlx::query("SELECT save_consultation_schedule_config($1, $2, $3)")
            .bind(doctor_id)
            .bind(biz_unit_id)
            .bind(value)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}
