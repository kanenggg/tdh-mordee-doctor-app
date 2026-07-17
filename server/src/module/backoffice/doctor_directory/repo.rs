use crate::core::error::AppResult;
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, serde::Serialize, utoipa::ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ApprovedDoctorSummary {
    pub doctor_account_id: i32,
    pub doctor_profile_id: i32,
    #[sqlx(json)]
    pub first_name: JsonValue,
    #[sqlx(json)]
    pub last_name: JsonValue,
    pub department_id: i32,
    #[sqlx(json)]
    pub department: JsonValue,
    #[sqlx(json)]
    pub specialty: JsonValue,
    pub profile_image_url: String,
    pub approved_at: i64,
}

#[derive(Debug, serde::Serialize, utoipa::ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ApprovedDoctorDetail {
    pub doctor_id: Uuid,
    pub doctor_account_id: i32,
    pub doctor_profile_id: i32,
    #[sqlx(json)]
    pub first_name: JsonValue,
    #[sqlx(json)]
    pub last_name: JsonValue,
    #[sqlx(json)]
    pub profession: JsonValue,
    #[sqlx(json)]
    pub academic_position: JsonValue,
    pub department_id: i32,
    #[sqlx(json)]
    pub department: JsonValue,
    #[sqlx(json)]
    pub specialty: JsonValue,
    #[sqlx(json)]
    pub additional_specialties: JsonValue,
    pub special_interests: Vec<String>,
    #[sqlx(json)]
    pub work_place: JsonValue,
    #[sqlx(json)]
    pub additional_workplace: JsonValue,
    pub profile_image_url: String,
    #[sqlx(json)]
    pub supported_languages: JsonValue,
    #[sqlx(json)]
    pub channel_types: JsonValue,
    pub duration_minutes: i32,
    pub doctor_fee_amount: String,
    pub doctor_fee_currency: String,
    pub approved_at: i64,
}

#[derive(Clone)]
pub struct ApprovedDoctorDirectoryRepo {
    pool: PgPool,
}

impl ApprovedDoctorDirectoryRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list(&self, page: u32, limit: u32) -> AppResult<Vec<ApprovedDoctorSummary>> {
        let offset = page.saturating_sub(1) * limit;
        Ok(sqlx::query_as(
            r#"
            SELECT
                dp.doctor_account_id,
                dp.doctor_profile_id,
                dp.first_name,
                dp.last_name,
                dp.department_id,
                COALESCE(dept.name, '{}'::jsonb) AS department,
                dp.specialty,
                dp.profile_image_url,
                EXTRACT(EPOCH FROM dp.created_at)::bigint AS approved_at
            FROM doctor_profile dp
            LEFT JOIN department dept ON dept.department_id = dp.department_id
            WHERE dp.is_active = true
            ORDER BY dp.created_at DESC, dp.doctor_account_id ASC
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(i64::from(limit))
        .bind(i64::from(offset))
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn get(&self, doctor_account_id: i32) -> AppResult<Option<ApprovedDoctorDetail>> {
        Ok(sqlx::query_as(
            r#"
            SELECT
                dp.doctor_id,
                dp.doctor_account_id,
                dp.doctor_profile_id,
                dp.first_name,
                dp.last_name,
                dp.profession,
                dp.academic_position,
                dp.department_id,
                COALESCE(dept.name, '{}'::jsonb) AS department,
                dp.specialty,
                dp.additional_specialties,
                dp.special_interest AS special_interests,
                dp.work_place,
                dp.additional_workplace,
                dp.profile_image_url,
                COALESCE(to_jsonb(config.supported_languages), '[]'::jsonb) AS supported_languages,
                COALESCE(to_jsonb(config.channel_types), '[]'::jsonb) AS channel_types,
                COALESCE(config.duration_minutes, 15) AS duration_minutes,
                COALESCE(config.doctor_fee_amount::text, '0.00') AS doctor_fee_amount,
                'THB'::text AS doctor_fee_currency,
                EXTRACT(EPOCH FROM dp.created_at)::bigint AS approved_at
            FROM doctor_profile dp
            LEFT JOIN department dept ON dept.department_id = dp.department_id
            LEFT JOIN doctor_consultation_config config ON config.doctor_id = dp.doctor_id
            WHERE dp.doctor_account_id = $1
              AND dp.is_active = true
            "#,
        )
        .bind(doctor_account_id)
        .fetch_optional(&self.pool)
        .await?)
    }
}
