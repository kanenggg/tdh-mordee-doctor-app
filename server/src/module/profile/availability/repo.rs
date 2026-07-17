use async_trait::async_trait;
use sqlx::PgPool;

use super::models::ConsultationAvailability;
use crate::core::error::AppResult;

#[async_trait]
pub trait AvailabilityRepo: Send + Sync {
    async fn set_schedule_availability(
        &self,
        doctor_id: i32,
        biz_unit_id: i32,
        available: bool,
    ) -> AppResult<()>;

    async fn set_instant_availability(
        &self,
        doctor_id: i32,
        biz_unit_id: i32,
        available: bool,
    ) -> AppResult<()>;

    async fn get_availability(
        &self,
        doctor_id: i32,
        biz_unit_id: i32,
    ) -> AppResult<ConsultationAvailability>;
}

#[derive(Debug, Clone)]
pub struct AvailabilityRepoPsql {
    pool: PgPool,
}

impl AvailabilityRepoPsql {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(sqlx::FromRow)]
struct AvailabilityRow {
    schedule_available: bool,
    instant_available: bool,
}

#[async_trait]
impl AvailabilityRepo for AvailabilityRepoPsql {
    async fn set_schedule_availability(
        &self,
        doctor_id: i32,
        biz_unit_id: i32,
        available: bool,
    ) -> AppResult<()> {
        sqlx::query("SELECT set_consultation_schedule_availability($1, $2, $3)")
            .bind(doctor_id)
            .bind(biz_unit_id)
            .bind(available)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn set_instant_availability(
        &self,
        doctor_id: i32,
        biz_unit_id: i32,
        available: bool,
    ) -> AppResult<()> {
        sqlx::query("SELECT set_consultation_instant_availability($1, $2, $3)")
            .bind(doctor_id)
            .bind(biz_unit_id)
            .bind(available)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn get_availability(
        &self,
        doctor_id: i32,
        biz_unit_id: i32,
    ) -> AppResult<ConsultationAvailability> {
        let row = sqlx::query_as::<_, AvailabilityRow>(
            "SELECT schedule_available, instant_available FROM get_consultation_availability($1, $2)",
        )
        .bind(doctor_id)
        .bind(biz_unit_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(ConsultationAvailability {
            schedule_available: row.schedule_available,
            instant_available: row.instant_available,
        })
    }
}
