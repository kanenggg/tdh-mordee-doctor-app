use crate::core::error::AppResult;
use crate::module::profile::configuration::models::DoctorConfiguration;
use crate::module::profile_event_outbox::{
    active_profile_for_update, approved_event, consultation_snapshot_for_update, enqueue,
    increment_version,
};
use sqlx::PgPool;
use tdh_protocol::common::Localized;
use tdh_protocol::consultation::ConsultationChannel;
use tdh_protocol::doctor_profile::{
    AcademicPosition, DoctorProfileEvent, LanguageCode, Profession, Specialty, WorkPlace,
};

const APPROVED_DOCTOR_PROFILE_COLUMNS: &str = r#"
    doctor_id,
    doctor_account_id,
    doctor_profile_id,
    department_id,
    department,
    counseling_areas,
    is_active,
    profession,
    CASE
        WHEN jsonb_typeof(specialty) = 'array' THEN specialty -> 0
        ELSE specialty
    END AS specialty,
    work_place,
    academic_position,
    first_name,
    last_name,
    profile_image_url,
    doctor_fee,
    doctor_fee_currency,
    languages,
    duration_minutes,
    channels,
    approved_at,
    newly_approved
"#;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ApprovedDoctorProfile {
    pub doctor_id: uuid::Uuid,
    pub doctor_account_id: i32,
    pub doctor_profile_id: i32,
    pub department_id: i32,
    #[sqlx(json)]
    pub department: Localized,
    #[sqlx(json)]
    pub counseling_areas: Vec<Localized>,
    pub is_active: bool,
    #[sqlx(json)]
    pub profession: Profession,
    #[sqlx(json)]
    pub specialty: Specialty,
    #[sqlx(json)]
    pub work_place: Vec<WorkPlace>,
    #[sqlx(json)]
    pub academic_position: AcademicPosition,
    #[sqlx(json)]
    pub first_name: Localized,
    #[sqlx(json)]
    pub last_name: Localized,
    pub profile_image_url: String,
    pub doctor_fee: i32,
    pub doctor_fee_currency: String,
    #[sqlx(json)]
    pub languages: Vec<LanguageCode>,
    pub duration_minutes: i32,
    #[sqlx(json)]
    pub channels: Vec<ConsultationChannel>,
    pub approved_at: i64,
    pub newly_approved: bool,
}

pub struct ProfileMutation {
    pub event: Option<DoctorProfileEvent>,
}

#[derive(Clone)]
pub struct BackofficeOnboardingRepo {
    pool: PgPool,
}

impl BackofficeOnboardingRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn approve_onboarding(
        &self,
        doctor_account_id: i32,
        action_by: i32,
        department_id: i32,
    ) -> AppResult<Option<ApprovedDoctorProfile>> {
        let query = format!(
            "SELECT {APPROVED_DOCTOR_PROFILE_COLUMNS} FROM approve_doctor_profile_draft($1,$2,$3)"
        );
        Ok(sqlx::query_as::<_, ApprovedDoctorProfile>(&query)
            .persistent(false)
            .bind(doctor_account_id)
            .bind(action_by)
            .bind(department_id)
            .fetch_optional(&self.pool)
            .await?)
    }

    pub async fn approve_with_setup(
        &self,
        doctor_account_id: i32,
        action_by: i32,
        department_id: i32,
        consultation_config: &DoctorConfiguration,
        clinic_ids: &[i32],
    ) -> AppResult<Option<ProfileMutation>> {
        let mut tx = self.pool.begin().await?;
        // Serialise concurrent approval attempts before calling the legacy SQL function.
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(i64::from(doctor_account_id))
            .execute(&mut *tx)
            .await?;
        let query = format!(
            "SELECT {APPROVED_DOCTOR_PROFILE_COLUMNS} FROM approve_doctor_profile_draft($1, $2, $3, $4, $5, $6, $7::numeric, $8)"
        );
        let profile = sqlx::query_as::<_, ApprovedDoctorProfile>(&query)
            .persistent(false)
            .bind(doctor_account_id)
            .bind(action_by)
            .bind(department_id)
            .bind(&consultation_config.channel)
            .bind(&consultation_config.language)
            .bind(consultation_config.duration)
            .bind(consultation_config.fee.amount)
            .bind(clinic_ids)
            .fetch_optional(&mut *tx)
            .await?;

        let Some(profile) = profile else {
            tx.commit().await?;
            return Ok(None);
        };
        if !profile.newly_approved {
            tx.commit().await?;
            return Ok(Some(ProfileMutation { event: None }));
        }

        let snapshot = active_profile_for_update(&mut tx, doctor_account_id)
            .await?
            .expect("newly approved profile is active");
        let config = consultation_snapshot_for_update(&mut tx, snapshot.doctor_id).await?;
        let version = increment_version(&mut tx, snapshot.doctor_id).await?;
        let event = approved_event(&snapshot, version, config)?;
        enqueue(&mut tx, &snapshot, event.clone()).await?;
        tx.commit().await?;
        Ok(Some(ProfileMutation { event: Some(event) }))
    }

    pub async fn reject_onboarding(
        &self,
        doctor_id: i32,
        reason: String,
        action_by: i32,
    ) -> AppResult<()> {
        sqlx::query("SELECT * FROM reject_doctor_profile_draft($1,$2,$3)")
            .bind(doctor_id)
            .bind(reason)
            .bind(action_by)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
