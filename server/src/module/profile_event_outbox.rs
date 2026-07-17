//! The transactional producer boundary for DoctorProfile V2 events.
//!
//! Every profile mutation calls these helpers in the same Postgres transaction
//! that changed the aggregate.  The background service owns delivery only.
use crate::core::error::{AppError, AppResult};
use crate::module::profile::configuration::models::{
    ChannelType, LanguageCode as ConfigurationLanguageCode,
};
use serde_json::Value;
use sqlx::PgConnection;
use tdh_protocol::common::Localized;
use tdh_protocol::consultation::ConsultationChannel;
use tdh_protocol::doctor_profile::{
    AcademicPosition, ConsultationConfig, ConsultationLanguage, DoctorProfileEvent, Profession,
    Specialty, WorkPlace,
};
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DoctorProfileSnapshot {
    pub doctor_id: Uuid,
    pub doctor_account_id: i32,
    pub doctor_profile_id: i32,
    pub department_id: i32,
    pub is_active: bool,
    #[sqlx(json)]
    pub department: Localized,
    #[sqlx(json)]
    pub counseling_areas: Vec<Localized>,
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
    pub approved_at: i64,
}

#[derive(sqlx::FromRow)]
struct ConsultationSnapshotRow {
    channel_types: Vec<ChannelType>,
    supported_languages: Vec<ConfigurationLanguageCode>,
    duration_minutes: Option<i32>,
    doctor_fee_amount: Option<String>,
}

pub async fn profile_for_update(
    conn: &mut PgConnection,
    doctor_account_id: i32,
) -> AppResult<Option<DoctorProfileSnapshot>> {
    Ok(sqlx::query_as(
        r#"SELECT dp.doctor_id, dp.doctor_account_id, dp.doctor_profile_id, dp.department_id,
                  dp.is_active, COALESCE(dept.name, '{"th":"","en":""}'::jsonb) AS department,
                  COALESCE(dept.counseling_areas, '[]'::jsonb) AS counseling_areas,
                  dp.profession,
                  CASE WHEN jsonb_typeof(dp.specialty) = 'array' THEN dp.specialty -> 0 ELSE dp.specialty END AS specialty,
                  dp.work_place, dp.academic_position, dp.first_name, dp.last_name, dp.profile_image_url,
                  extract(epoch FROM dp.created_at)::bigint AS approved_at
           FROM doctor_profile dp
           LEFT JOIN department dept ON dept.department_id = dp.department_id
           WHERE dp.doctor_account_id = $1
           FOR UPDATE OF dp"#,
    )
    .bind(doctor_account_id)
    .fetch_optional(conn)
    .await?)
}

pub async fn active_profile_for_update(
    conn: &mut PgConnection,
    doctor_account_id: i32,
) -> AppResult<Option<DoctorProfileSnapshot>> {
    Ok(profile_for_update(conn, doctor_account_id)
        .await?
        .filter(|profile| profile.is_active))
}

/// Locks the mutable consultation row before reading it. Producers call this
/// after their mutation, so both legacy V1 fields and V2 consultationConfig
/// are built from one post-mutation state.
pub async fn consultation_snapshot_for_update(
    conn: &mut PgConnection,
    doctor_id: Uuid,
) -> AppResult<ConsultationConfig> {
    let row = sqlx::query_as::<_, ConsultationSnapshotRow>(
        r#"SELECT channel_types, supported_languages, duration_minutes,
                  doctor_fee_amount::text AS doctor_fee_amount
           FROM doctor_consultation_config WHERE doctor_id = $1 FOR UPDATE"#,
    )
    .bind(doctor_id)
    .fetch_one(conn)
    .await?;
    let mut channels = row.channel_types;
    channels.sort_by_key(|value| match value {
        ChannelType::Voice => 0,
        ChannelType::Chat => 1,
        ChannelType::Video => 2,
    });
    channels.dedup();
    let mut languages = row
        .supported_languages
        .into_iter()
        .map(|value| match value {
            ConfigurationLanguageCode::Th => ConsultationLanguage::Th,
            ConfigurationLanguageCode::En => ConsultationLanguage::En,
        })
        .collect::<Vec<_>>();
    languages.sort_by_key(|value| match value {
        ConsultationLanguage::Th => 0,
        ConsultationLanguage::En => 1,
    });
    languages.dedup();
    let config = ConsultationConfig {
        channels: channels
            .into_iter()
            .map(|value| match value {
                ChannelType::Voice => ConsultationChannel::Voice,
                ChannelType::Chat => ConsultationChannel::Chat,
                ChannelType::Video => ConsultationChannel::Video,
            })
            .collect(),
        languages,
        // Existing DoctorApp configuration reads have always defaulted omitted
        // legacy duration/fee to 15 minutes and 0.00 THB. Keep that canonical
        // compatibility rule; empty channel/language arrays remain invalid.
        duration_minutes: row.duration_minutes.unwrap_or(15),
        fee_amount: row.doctor_fee_amount.unwrap_or_else(|| "0.00".to_string()),
        currency: "THB".to_string(),
    };
    config.validate().map_err(|message| {
        AppError::BadRequest(format!("incomplete consultation configuration: {message}"))
    })?;
    Ok(config)
}

pub fn approved_event(
    profile: &DoctorProfileSnapshot,
    profile_version: i64,
    consultation_config: ConsultationConfig,
) -> AppResult<DoctorProfileEvent> {
    consultation_config.validate().map_err(|message| {
        AppError::BadRequest(format!("incomplete consultation configuration: {message}"))
    })?;
    Ok(DoctorProfileEvent::DoctorProfileApproved {
        event_id: Uuid::new_v4().to_string(),
        doctor_id: profile.doctor_id,
        doctor_account_id: profile.doctor_account_id,
        doctor_profile_id: profile.doctor_profile_id,
        department_id: profile.department_id,
        department: profile.department.clone(),
        counseling_areas: profile.counseling_areas.clone(),
        is_active: profile.is_active,
        profession: profile.profession.clone(),
        specialty: profile.specialty.clone(),
        work_place: profile.work_place.clone(),
        academic_position: profile.academic_position.clone(),
        first_name: profile.first_name.clone(),
        last_name: profile.last_name.clone(),
        profile_image_url: profile.profile_image_url.clone(),
        approved_at: profile.approved_at,
        occurred_at: jiff::Timestamp::now().as_second(),
        schema_version: Some(2),
        profile_version: Some(profile_version),
        consultation_config: Some(consultation_config),
    })
}

pub fn doctor_status_updated_event(
    profile: &DoctorProfileSnapshot,
    profile_version: i64,
) -> DoctorProfileEvent {
    let occurred_at = jiff::Timestamp::now().as_second();
    DoctorProfileEvent::DoctorProfileStatusUpdated {
        event_id: Uuid::new_v4().to_string(),
        doctor_id: profile.doctor_id.to_string(),
        doctor_account_id: profile.doctor_account_id,
        doctor_profile_id: profile.doctor_profile_id,
        is_active: profile.is_active,
        status_updated_at: occurred_at,
        occurred_at,
        schema_version: Some(3),
        profile_version: Some(profile_version),
    }
}

pub async fn enqueue(
    conn: &mut PgConnection,
    profile: &DoctorProfileSnapshot,
    event: DoctorProfileEvent,
) -> AppResult<()> {
    let version: i64 = event.profile_version().ok_or_else(|| {
        AppError::InternalError("DoctorProfile V2 event is missing profileVersion".to_string())
    })?;
    let payload = serde_json::to_value(&event).map_err(|error| {
        AppError::InternalError(format!("failed to serialize doctor profile event: {error}"))
    })?;
    let event_id = Uuid::parse_str(event.event_id())
        .map_err(|error| AppError::InternalError(format!("invalid generated event id: {error}")))?;
    let occurred_at = match &event {
        DoctorProfileEvent::DoctorProfileApproved { occurred_at, .. }
        | DoctorProfileEvent::DoctorProfileStatusUpdated { occurred_at, .. } => *occurred_at,
    };
    sqlx::query(r#"INSERT INTO doctor_profile_event_outbox
        (event_id, doctor_id, doctor_account_id, event_type, schema_version, profile_version, occurred_at, payload)
        VALUES ($1, $2, $3, $4, $5, $6, to_timestamp($7), $8)"#)
        .bind(event_id).bind(profile.doctor_id).bind(event.doctor_account_id()).bind(event.event_type())
        .bind(event.schema_version()).bind(version).bind(occurred_at).bind(payload)
        .execute(conn).await?;
    Ok(())
}

pub async fn increment_version(conn: &mut PgConnection, doctor_id: Uuid) -> AppResult<i64> {
    Ok(sqlx::query_scalar("UPDATE doctor_profile SET profile_version = profile_version + 1, updated_at = now() WHERE doctor_id = $1 RETURNING profile_version")
        .bind(doctor_id).fetch_one(conn).await?)
}

/// A durable event leased by the request-path compatibility accelerator. The
/// payload is read back from PostgreSQL, never reconstructed after commit.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct LeasedPersistedOutboxEvent {
    pub event_id: Uuid,
    pub doctor_account_id: i32,
    pub event_type: String,
    pub schema_version: i32,
    pub payload: Value,
    pub lease_token: Uuid,
}

pub async fn lease_persisted_event(
    pool: &sqlx::PgPool,
    event_id: Uuid,
    lease_seconds: i64,
) -> AppResult<Option<LeasedPersistedOutboxEvent>> {
    let lease_token = Uuid::new_v4();
    Ok(sqlx::query_as(
        r#"UPDATE doctor_profile_event_outbox o
           SET lease_token = $2, leased_until = now() + ($3 * interval '1 second'), attempts = attempts + 1
           WHERE o.event_id = $1 AND o.published_at IS NULL
             AND (o.leased_until IS NULL OR o.leased_until <= now())
             AND NOT EXISTS (
                 SELECT 1 FROM doctor_profile_event_outbox older
                 WHERE older.doctor_id = o.doctor_id
                   AND older.published_at IS NULL
                   AND older.profile_version < o.profile_version
             )
           RETURNING o.event_id, o.doctor_account_id, o.event_type, o.schema_version, o.payload, o.lease_token"#,
    )
    .bind(event_id)
    .bind(lease_token)
    .bind(lease_seconds)
    .fetch_optional(pool)
    .await?)
}

pub async fn mark_persisted_event_published(
    pool: &sqlx::PgPool,
    event_id: Uuid,
    lease_token: Uuid,
) -> AppResult<()> {
    let result = sqlx::query(
        "UPDATE doctor_profile_event_outbox SET published_at = now(), leased_until = NULL, lease_token = NULL, last_error = NULL WHERE event_id = $1 AND lease_token = $2 AND published_at IS NULL",
    )
    .bind(event_id)
    .bind(lease_token)
    .execute(pool)
    .await?;
    if result.rows_affected() != 1 {
        return Err(AppError::InternalError(format!(
            "stale outbox lease while marking {event_id} published"
        )));
    }
    Ok(())
}

pub async fn release_persisted_event_after_failure(
    pool: &sqlx::PgPool,
    event_id: Uuid,
    lease_token: Uuid,
) -> AppResult<()> {
    let result = sqlx::query(
        "UPDATE doctor_profile_event_outbox SET leased_until = NULL, lease_token = NULL, last_error = 'immediate publish failed' WHERE event_id = $1 AND lease_token = $2 AND published_at IS NULL",
    )
    .bind(event_id)
    .bind(lease_token)
    .execute(pool)
    .await?;
    if result.rows_affected() != 1 {
        return Err(AppError::InternalError(format!(
            "stale outbox lease while releasing {event_id}"
        )));
    }
    Ok(())
}

/// Backfills one V2 snapshot per already-active doctor that has no event for
/// its current version. Safe to rerun: after this commits the outbox contains
/// the new current version, so the next run skips it.
#[derive(Debug, Default, Clone, Copy)]
pub struct ReconciliationResult {
    pub emitted: u64,
    pub skipped: u64,
    pub failures: u64,
}

/// Continues after malformed legacy rows. Such rows are deliberately not
/// published: the caller gets a typed per-doctor error in logs and can repair
/// the profile before re-running reconciliation.
pub async fn reconcile_approved_doctors(pool: &sqlx::PgPool) -> AppResult<ReconciliationResult> {
    let doctor_ids: Vec<i32> = sqlx::query_scalar(
        "SELECT doctor_account_id FROM doctor_profile WHERE is_active = true ORDER BY doctor_account_id",
    )
    .fetch_all(pool)
    .await?;
    let mut result = ReconciliationResult::default();
    for doctor_account_id in doctor_ids {
        match reconcile_one(pool, doctor_account_id).await {
            Ok(true) => result.emitted += 1,
            Ok(false) => result.skipped += 1,
            Err(error) => {
                result.failures += 1;
                tracing::warn!(doctor_account_id, error = %error, "doctor profile outbox reconciliation skipped invalid profile");
            }
        }
    }
    Ok(result)
}

async fn reconcile_one(pool: &sqlx::PgPool, doctor_account_id: i32) -> AppResult<bool> {
    let mut tx = pool.begin().await?;
    let Some(profile) = active_profile_for_update(&mut tx, doctor_account_id).await? else {
        tx.commit().await?;
        return Ok(false);
    };
    let current_version: i64 =
        sqlx::query_scalar("SELECT profile_version FROM doctor_profile WHERE doctor_id = $1")
            .bind(profile.doctor_id)
            .fetch_one(&mut *tx)
            .await?;
    let present: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM doctor_profile_event_outbox WHERE doctor_id = $1 AND profile_version = $2)",
    )
    .bind(profile.doctor_id)
    .bind(current_version)
    .fetch_one(&mut *tx)
    .await?;
    if present {
        tx.commit().await?;
        return Ok(false);
    }
    let config = consultation_snapshot_for_update(&mut tx, profile.doctor_id).await?;
    let version = increment_version(&mut tx, profile.doctor_id).await?;
    let event = approved_event(&profile, version, config)?;
    enqueue(&mut tx, &profile, event).await?;
    tx.commit().await?;
    Ok(true)
}
