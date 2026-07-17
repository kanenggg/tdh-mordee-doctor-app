use async_trait::async_trait;
use sqlx::PgPool;

use crate::core::{error::AppResult, UserIdentity};

use super::models::SummarizationStatus;

pub mod follow_up;

pub use follow_up::{
    FollowUpReservationRepo, FollowUpReservationRepoImpl, OverlappingTimeslot, ReservedTimeslot,
};

// ─── Raw DB row ───────────────────────────────────────────────────────────────

/// Internal sqlx row type for database queries.
/// Maps to `consultation_summarization` table.
#[derive(sqlx::FromRow)]
struct SummarizationRow {
    appointment_id: String,
    doctor_account_id: i32,
    status: SummarizationStatus,
    summary_note_encrypted: Option<String>,
}

// ─── Repo Data Transfer Type ──────────────────────────────────────────────────

/// Data transfer type between repo and service layer.
/// Contains encrypted data that service will decrypt.
#[derive(Debug)]
pub struct SummarizationRawRecord {
    pub appointment_id: String,
    pub doctor_account_id: i32,
    pub status: SummarizationStatus,
    /// Encrypted JSON containing SummaryNote, Prescription, FollowUpInfo
    pub summary_note_encrypted: Option<String>,
}

impl From<SummarizationRow> for SummarizationRawRecord {
    fn from(row: SummarizationRow) -> Self {
        Self {
            appointment_id: row.appointment_id,
            doctor_account_id: row.doctor_account_id,
            status: row.status,
            summary_note_encrypted: row.summary_note_encrypted,
        }
    }
}

// ─── Trait ────────────────────────────────────────────────────────────────────

#[async_trait]
pub trait SummarizationRepo: Send + Sync {
    /// Returns `None` if no row exists yet (appointment is in `PendingRecord` state, no draft started).
    async fn get_summary(
        &self,
        user_id: &UserIdentity,
        appointment_id: &str,
    ) -> AppResult<Option<SummarizationRawRecord>>;

    /// Upsert a draft record. Status is stamped `Draft`.
    async fn save_draft(
        &self,
        appointment_id: &str,
        doctor_account_id: i32,
        doctor_profile_id: i32,
        encrypted_payload: &str,
    ) -> AppResult<()>;

    /// Save and update status to `Submitted`.
    async fn save_and_submit(
        &self,
        appointment_id: &str,
        doctor_account_id: i32,
        doctor_profile_id: i32,
        encrypted_payload: &str,
    ) -> AppResult<()>;
}

// ─── Implementation ────────────────────────────────────────────────────────────────────

pub struct SummarizationRepoPsql {
    pool: PgPool,
}

impl SummarizationRepoPsql {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SummarizationRepo for SummarizationRepoPsql {
    async fn get_summary(
        &self,
        user_id: &UserIdentity,
        appointment_id: &str,
    ) -> AppResult<Option<SummarizationRawRecord>> {
        let row = sqlx::query_as::<_, SummarizationRow>(
            r#"
            SELECT appointment_id, doctor_account_id, status, summary_note_encrypted
            FROM consultation_summarization
            WHERE appointment_id = $1
            "#,
        )
        .bind(appointment_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(SummarizationRawRecord::from))
    }

    async fn save_draft(
        &self,
        appointment_id: &str,
        doctor_account_id: i32,
        doctor_profile_id: i32,
        encrypted_payload: &str,
    ) -> AppResult<()> {
        sqlx::query(
            r#"
            INSERT INTO consultation_summarization (appointment_id, doctor_account_id, doctor_profile_id, status, summary_note_encrypted)
            VALUES ($1, $2, $3, 'Draft', $4)
            ON CONFLICT (appointment_id)
            DO UPDATE SET
                summary_note_encrypted = EXCLUDED.summary_note_encrypted,
                updated_at = NOW()
            WHERE consultation_summarization.status = 'Draft'
            "#,
        )
        .bind(appointment_id)
        .bind(doctor_account_id)
        .bind(doctor_profile_id)
        .bind(encrypted_payload)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn save_and_submit(
        &self,
        appointment_id: &str,
        doctor_account_id: i32,
        doctor_profile_id: i32,
        encrypted_payload: &str,
    ) -> AppResult<()> {
        sqlx::query(
            r#"
            INSERT INTO consultation_summarization (appointment_id, doctor_account_id, doctor_profile_id, status, summary_note_encrypted)
            VALUES ($1, $2, $3, 'Submitted', $4)
            ON CONFLICT (appointment_id)
            DO UPDATE SET
                summary_note_encrypted = EXCLUDED.summary_note_encrypted,
                status = 'Submitted',
                updated_at = NOW()
            "#,
        )
        .bind(appointment_id)
        .bind(doctor_account_id)
        .bind(doctor_profile_id)
        .bind(encrypted_payload)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
