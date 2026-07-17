use async_trait::async_trait;
use sqlx::PgPool;

use super::models::{ChannelType, DoctorConfiguration, Fee, LanguageCode};
use crate::core::error::AppResult;
use crate::module::profile_event_outbox::{
    active_profile_for_update, approved_event, consultation_snapshot_for_update, enqueue,
    increment_version,
};

#[async_trait]
pub trait DoctorConfigurationRepo: Send + Sync {
    /// Returns the doctor's configuration, or `None` if no profile row exists.
    async fn get_configuration(
        &self,
        doctor_account_id: i32,
    ) -> AppResult<Option<DoctorConfiguration>>;

    /// Upserts the doctor's supported channels: updates the existing config row,
    /// or creates one (other fields defaulted) if the doctor has none yet.
    /// Returns `false` only when the doctor has no `doctor_profile` row.
    async fn update_channels(
        &self,
        doctor_account_id: i32,
        channels: &[ChannelType],
    ) -> AppResult<bool>;

    /// Upserts the doctor's supported languages: updates the existing config row,
    /// or creates one (other fields defaulted) if the doctor has none yet.
    /// Returns `false` only when the doctor has no `doctor_profile` row.
    async fn update_languages(
        &self,
        doctor_account_id: i32,
        languages: &[LanguageCode],
    ) -> AppResult<bool>;
}

#[derive(Debug, Clone)]
pub struct DoctorConfigurationRepoPsql {
    pool: PgPool,
}

impl DoctorConfigurationRepoPsql {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

/// Currency for the consultation fee. The `doctor_consultation_config` table
/// does not store a currency, so it is fixed to Thai Baht.
const FEE_CURRENCY: &str = "THB";

#[derive(sqlx::FromRow)]
struct ConfigurationRow {
    channel_types: Vec<ChannelType>,
    supported_languages: Vec<LanguageCode>,
    duration_minutes: Option<i32>,
    doctor_fee_amount: Option<f64>,
}

#[async_trait]
impl DoctorConfigurationRepo for DoctorConfigurationRepoPsql {
    async fn get_configuration(
        &self,
        doctor_account_id: i32,
    ) -> AppResult<Option<DoctorConfiguration>> {
        let row = sqlx::query_as::<_, ConfigurationRow>(
            r#"
            SELECT
                channel_types,
                supported_languages,
                duration_minutes,
                doctor_fee_amount::float8 AS doctor_fee_amount
            FROM doctor_consultation_config
            WHERE doctor_id = (SELECT doctor_id FROM doctor_profile WHERE doctor_account_id = $1)
            "#,
        )
        .bind(doctor_account_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| DoctorConfiguration {
            channel: r.channel_types,
            fee: Fee {
                amount: r.doctor_fee_amount,
                currency: FEE_CURRENCY.to_string(),
            },
            duration: r.duration_minutes,
            language: r.supported_languages,
        }))
    }

    async fn update_channels(
        &self,
        doctor_account_id: i32,
        channels: &[ChannelType],
    ) -> AppResult<bool> {
        let mut tx = self.pool.begin().await?;
        let Some(profile) = active_profile_for_update(&mut tx, doctor_account_id).await? else {
            tx.commit().await?;
            return Ok(false);
        };
        let result = sqlx::query(
            r#"
            INSERT INTO doctor_consultation_config (doctor_id, channel_types)
            SELECT doctor_id, $2 FROM doctor_profile WHERE doctor_account_id = $1
            ON CONFLICT (doctor_id) DO UPDATE
                SET channel_types = EXCLUDED.channel_types, updated_at = now()
            "#,
        )
        .bind(doctor_account_id)
        .bind(channels)
        .execute(&mut *tx)
        .await?;
        if result.rows_affected() == 0 {
            tx.commit().await?;
            return Ok(false);
        }
        let version = increment_version(&mut tx, profile.doctor_id).await?;
        let snapshot = consultation_snapshot_for_update(&mut tx, profile.doctor_id).await?;
        let event = approved_event(&profile, version, snapshot)?;
        enqueue(&mut tx, &profile, event).await?;
        tx.commit().await?;
        Ok(true)
    }

    async fn update_languages(
        &self,
        doctor_account_id: i32,
        languages: &[LanguageCode],
    ) -> AppResult<bool> {
        let mut tx = self.pool.begin().await?;
        let Some(profile) = active_profile_for_update(&mut tx, doctor_account_id).await? else {
            tx.commit().await?;
            return Ok(false);
        };
        let result = sqlx::query(
            r#"
            INSERT INTO doctor_consultation_config (doctor_id, supported_languages)
            SELECT doctor_id, $2 FROM doctor_profile WHERE doctor_account_id = $1
            ON CONFLICT (doctor_id) DO UPDATE
                SET supported_languages = EXCLUDED.supported_languages, updated_at = now()
            "#,
        )
        .bind(doctor_account_id)
        .bind(languages)
        .execute(&mut *tx)
        .await?;
        if result.rows_affected() == 0 {
            tx.commit().await?;
            return Ok(false);
        }
        let version = increment_version(&mut tx, profile.doctor_id).await?;
        let snapshot = consultation_snapshot_for_update(&mut tx, profile.doctor_id).await?;
        let event = approved_event(&profile, version, snapshot)?;
        enqueue(&mut tx, &profile, event).await?;
        tx.commit().await?;
        Ok(true)
    }
}
