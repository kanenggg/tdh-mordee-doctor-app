use sqlx::PgPool;

/// Self-contained read of the doctor's own consultation-duration config.
/// Source: PostgreSQL `doctor_consultation_config.duration_minutes`. The table is
/// keyed by `doctor_id` (uuid), so we join `doctor_profile` to resolve it from the
/// caller's `doctor_profile_id`. Returns `None` when the doctor has no config row or
/// `duration_minutes` is unset.
#[async_trait::async_trait]
pub trait ConsultationDurationRepo: Send + Sync {
    async fn get_consultation_duration(
        &self,
        doctor_profile_id: i32,
    ) -> Result<Option<i32>, anyhow::Error>;
}

#[derive(Clone)]
pub struct ConsultationDurationRepoImpl {
    pg_pool: PgPool,
}

impl ConsultationDurationRepoImpl {
    pub fn new(pg_pool: PgPool) -> Self {
        Self { pg_pool }
    }
}

#[async_trait::async_trait]
impl ConsultationDurationRepo for ConsultationDurationRepoImpl {
    async fn get_consultation_duration(
        &self,
        doctor_profile_id: i32,
    ) -> Result<Option<i32>, anyhow::Error> {
        let row: Option<Option<i32>> = sqlx::query_scalar(
            r#"
            SELECT c.duration_minutes
            FROM doctor_consultation_config c
            JOIN doctor_profile p ON p.doctor_id = c.doctor_id
            WHERE p.doctor_profile_id = $1
            "#,
        )
        .bind(doctor_profile_id)
        .fetch_optional(&self.pg_pool)
        .await?;
        Ok(row.flatten())
    }
}
