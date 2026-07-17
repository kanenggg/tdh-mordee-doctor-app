use crate::core::error::AppResult;
use crate::module::profile::configuration::models::DoctorConfiguration;
use crate::module::profile_event_outbox::{
    active_profile_for_update, approved_event, consultation_snapshot_for_update,
    doctor_status_updated_event, enqueue, increment_version, profile_for_update,
};
use sqlx::PgPool;
use tdh_protocol::doctor_profile::DoctorProfileEvent;
use uuid::Uuid;

pub struct DoctorManagementMutation {
    pub event: Option<DoctorProfileEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PendingDoctorProfileEvent {
    pub event_id: Uuid,
    pub event_type: String,
}

#[derive(Clone)]
pub struct DoctorManagementRepo {
    pool: PgPool,
}

impl DoctorManagementRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn pending_events_through(
        &self,
        event_id: Uuid,
    ) -> AppResult<Vec<PendingDoctorProfileEvent>> {
        Ok(sqlx::query_as(
            r#"
            SELECT pending.event_id, pending.event_type
            FROM doctor_profile_event_outbox target
            JOIN doctor_profile_event_outbox pending
              ON pending.doctor_id = target.doctor_id
             AND pending.profile_version <= target.profile_version
            WHERE target.event_id = $1
              AND pending.published_at IS NULL
            ORDER BY pending.profile_version, pending.created_at, pending.event_id
            "#,
        )
        .bind(event_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn update_consultation_config(
        &self,
        doctor_account_id: i32,
        action_by: i32,
        consultation_config: &DoctorConfiguration,
    ) -> AppResult<Option<DoctorManagementMutation>> {
        let mut tx = self.pool.begin().await?;
        let Some(snapshot) = active_profile_for_update(&mut tx, doctor_account_id).await? else {
            tx.commit().await?;
            return Ok(None);
        };
        let existing = consultation_snapshot_for_update(&mut tx, snapshot.doctor_id).await?;
        let mut merged = consultation_config.clone();
        if merged.duration.is_none() {
            merged.duration = Some(existing.duration_minutes);
        }
        if merged.fee.amount.is_none() {
            merged.fee.amount = existing.fee_amount.parse::<f64>().ok();
        }
        let updated: bool = sqlx::query_scalar(
            r#"
            SELECT update_consultation_configuration($1, $2, $3, $4, $5::numeric, $6)
            "#,
        )
        .bind(doctor_account_id)
        .bind(&merged.channel)
        .bind(&merged.language)
        .bind(merged.duration)
        .bind(merged.fee.amount)
        .bind(action_by)
        .fetch_one(&mut *tx)
        .await?;
        if !updated {
            tx.commit().await?;
            return Ok(None);
        }
        let config = consultation_snapshot_for_update(&mut tx, snapshot.doctor_id).await?;
        let version = increment_version(&mut tx, snapshot.doctor_id).await?;
        let event = approved_event(&snapshot, version, config)?;
        enqueue(&mut tx, &snapshot, event.clone()).await?;
        tx.commit().await?;
        Ok(Some(DoctorManagementMutation { event: Some(event) }))
    }

    pub async fn update_doctor_active_status(
        &self,
        doctor_account_id: i32,
        is_active: bool,
    ) -> AppResult<Option<DoctorProfileEvent>> {
        let mut tx = self.pool.begin().await?;
        let Some(mut profile) = profile_for_update(&mut tx, doctor_account_id).await? else {
            tx.commit().await?;
            return Ok(None);
        };
        if profile.is_active == is_active {
            tx.commit().await?;
            return Ok(None);
        }
        sqlx::query(
            "UPDATE doctor_profile SET is_active = $2, updated_at = now() WHERE doctor_id = $1",
        )
        .bind(profile.doctor_id)
        .bind(is_active)
        .execute(&mut *tx)
        .await?;

        profile.is_active = is_active;
        let version = increment_version(&mut tx, profile.doctor_id).await?;
        let event = doctor_status_updated_event(&profile, version);
        enqueue(&mut tx, &profile, event.clone()).await?;
        tx.commit().await?;
        Ok(Some(event))
    }
}
