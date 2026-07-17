use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};

use crate::core::{auth::BackofficeIdentity, error::AppResult};

const PENDING_STATUS: &str = "PendingApproval";
const REDACTED_FIELDS: [&str; 5] = [
    "citizenId",
    "idCardImageUrl",
    "bookBankImageUrl",
    "medicalLicenseImageUrl",
    "educationLicenseImageUrl",
];

#[derive(Clone)]
pub struct PendingApprovalState {
    repo: Arc<PendingApprovalRepo>,
}

pub fn router(pool: PgPool) -> Router {
    let state = PendingApprovalState {
        repo: Arc::new(PendingApprovalRepo::new(pool)),
    };

    Router::new()
        .route("/", get(list_pending_approvals))
        .route("/{doctor_account_id}", get(get_pending_approval))
        .with_state(state)
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct PendingApprovalListQuery {
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PendingDoctorApprovalListResponse {
    pub data: Vec<PendingDoctorApprovalSummary>,
    pub page: u32,
    pub limit: u32,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PendingDoctorApprovalSummary {
    pub doctor_account_id: i32,
    pub doctor_profile_id: i32,
    pub first_name: JsonValue,
    pub last_name: JsonValue,
    pub profession: JsonValue,
    pub academic_position: JsonValue,
    pub profile_image_url: Option<String>,
    pub status: String,
    pub submitted_at: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "__type", rename_all = "PascalCase")]
pub enum PendingDoctorApprovalDetailResponse {
    PendingDoctorApproval(PendingDoctorApprovalDetail),
    PendingDoctorApprovalNotFound,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PendingDoctorApprovalDetail {
    pub doctor_account_id: i32,
    pub doctor_profile_id: i32,
    pub first_name: JsonValue,
    pub last_name: JsonValue,
    pub profession: JsonValue,
    pub academic_position: JsonValue,
    pub license_number: Option<String>,
    pub primary_medical_school: JsonValue,
    pub specialty: JsonValue,
    pub additional_specialties: JsonValue,
    pub special_interest: Vec<String>,
    pub address: PendingDoctorApprovalAddress,
    pub work_place: JsonValue,
    pub additional_workplace: JsonValue,
    pub profile_image_url: Option<String>,
    pub status: String,
    pub submitted_at: Option<i64>,
    pub redacted_fields: Vec<&'static str>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PendingDoctorApprovalAddress {
    pub address_detail: Option<String>,
    pub sub_district: JsonValue,
    pub district: JsonValue,
    pub province: JsonValue,
    pub postal_code: Option<i32>,
}

#[derive(Debug, sqlx::FromRow)]
struct PendingDoctorApprovalSummaryRow {
    doctor_account_id: i32,
    doctor_profile_id: i32,
    first_name: JsonValue,
    last_name: JsonValue,
    profession: JsonValue,
    academic_position: JsonValue,
    profile_image_url: Option<String>,
    status: String,
    submitted_at: Option<i64>,
}

impl From<PendingDoctorApprovalSummaryRow> for PendingDoctorApprovalSummary {
    fn from(row: PendingDoctorApprovalSummaryRow) -> Self {
        Self {
            doctor_account_id: row.doctor_account_id,
            doctor_profile_id: row.doctor_profile_id,
            first_name: row.first_name,
            last_name: row.last_name,
            profession: row.profession,
            academic_position: row.academic_position,
            profile_image_url: row.profile_image_url,
            status: row.status,
            submitted_at: row.submitted_at,
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
struct PendingDoctorApprovalDetailRow {
    doctor_account_id: i32,
    doctor_profile_id: i32,
    first_name: JsonValue,
    last_name: JsonValue,
    profession: JsonValue,
    academic_position: JsonValue,
    license_number: Option<String>,
    primary_medical_school: JsonValue,
    specialty: JsonValue,
    additional_specialties: JsonValue,
    special_interest: Vec<String>,
    address_detail: Option<String>,
    sub_district: JsonValue,
    district: JsonValue,
    province: JsonValue,
    postal_code: Option<i32>,
    work_place: JsonValue,
    additional_workplace: JsonValue,
    profile_image_url: Option<String>,
    status: String,
    submitted_at: Option<i64>,
}

impl From<PendingDoctorApprovalDetailRow> for PendingDoctorApprovalDetail {
    fn from(row: PendingDoctorApprovalDetailRow) -> Self {
        Self {
            doctor_account_id: row.doctor_account_id,
            doctor_profile_id: row.doctor_profile_id,
            first_name: row.first_name,
            last_name: row.last_name,
            profession: row.profession,
            academic_position: row.academic_position,
            license_number: row.license_number,
            primary_medical_school: row.primary_medical_school,
            specialty: row.specialty,
            additional_specialties: row.additional_specialties,
            special_interest: row.special_interest,
            address: PendingDoctorApprovalAddress {
                address_detail: row.address_detail,
                sub_district: row.sub_district,
                district: row.district,
                province: row.province,
                postal_code: row.postal_code,
            },
            work_place: row.work_place,
            additional_workplace: row.additional_workplace,
            profile_image_url: row.profile_image_url,
            status: row.status,
            submitted_at: row.submitted_at,
            redacted_fields: REDACTED_FIELDS.to_vec(),
        }
    }
}

struct PendingApprovalRepo {
    pool: PgPool,
}

impl PendingApprovalRepo {
    fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn list(&self, page: u32, limit: u32) -> AppResult<Vec<PendingDoctorApprovalSummary>> {
        let offset = page.saturating_sub(1) * limit;
        let rows = sqlx::query_as::<_, PendingDoctorApprovalSummaryRow>(
            r#"
            SELECT
                doctor_account_id,
                doctor_profile_id,
                COALESCE(first_name, '{}'::jsonb) AS first_name,
                COALESCE(last_name, '{}'::jsonb) AS last_name,
                COALESCE(profession, '[]'::jsonb) AS profession,
                COALESCE(academic_position, '[]'::jsonb) AS academic_position,
                profile_image_url,
                status::text AS status,
                EXTRACT(EPOCH FROM created_at)::bigint AS submitted_at
            FROM doctor_profile_draft
            WHERE status = $1::doctor_profile_status_enum
            ORDER BY created_at DESC NULLS LAST, doctor_account_id ASC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(PENDING_STATUS)
        .bind(i64::from(limit))
        .bind(i64::from(offset))
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn get(&self, doctor_account_id: i32) -> AppResult<Option<PendingDoctorApprovalDetail>> {
        let row = sqlx::query_as::<_, PendingDoctorApprovalDetailRow>(
            r#"
            SELECT
                doctor_account_id,
                doctor_profile_id,
                COALESCE(first_name, '{}'::jsonb) AS first_name,
                COALESCE(last_name, '{}'::jsonb) AS last_name,
                COALESCE(profession, '[]'::jsonb) AS profession,
                COALESCE(academic_position, '[]'::jsonb) AS academic_position,
                license_number,
                COALESCE(primary_medical_school, '[]'::jsonb) AS primary_medical_school,
                COALESCE(specialty, '{}'::jsonb) AS specialty,
                COALESCE(additional_specialties, '[]'::jsonb) AS additional_specialties,
                COALESCE(special_interest, '{}'::text[]) AS special_interest,
                address_detail,
                COALESCE(sub_district, '{}'::jsonb) AS sub_district,
                COALESCE(district, '{}'::jsonb) AS district,
                COALESCE(province, '{}'::jsonb) AS province,
                postal_code,
                COALESCE(work_place, '[]'::jsonb) AS work_place,
                COALESCE(additional_workplace, '[]'::jsonb) AS additional_workplace,
                profile_image_url,
                status::text AS status,
                EXTRACT(EPOCH FROM created_at)::bigint AS submitted_at
            FROM doctor_profile_draft
            WHERE doctor_account_id = $1
              AND status = $2::doctor_profile_status_enum
            "#,
        )
        .bind(doctor_account_id)
        .bind(PENDING_STATUS)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(Into::into))
    }
}

#[utoipa::path(
    get,
    path = "/backoffice/v1/onboarding/pending",
    tag = "backoffice",
    params(PendingApprovalListQuery),
    responses(
        (status = 200, description = "Pending doctor approval list", body = PendingDoctorApprovalListResponse),
        (status = 401, description = "Unauthorized"),
    ),
    security(("TDH-SEC-IAM-USER-IDENTITY" = []))
)]
pub async fn list_pending_approvals(
    State(state): State<PendingApprovalState>,
    _identity: BackofficeIdentity,
    Query(params): Query<PendingApprovalListQuery>,
) -> AppResult<impl IntoResponse> {
    let page = params.page.unwrap_or(1).max(1);
    let limit = params.limit.unwrap_or(20).clamp(1, 100);
    let pending = state.repo.list(page, limit).await?;

    Ok(Json(PendingDoctorApprovalListResponse {
        data: pending,
        page,
        limit,
    }))
}

#[utoipa::path(
    get,
    path = "/backoffice/v1/onboarding/pending/{doctor_account_id}",
    tag = "backoffice",
    params(
        ("doctor_account_id" = i32, Path, description = "Doctor account ID")
    ),
    responses(
        (status = 200, description = "Pending doctor approval detail or typed not-found", body = PendingDoctorApprovalDetailResponse),
        (status = 401, description = "Unauthorized"),
    ),
    security(("TDH-SEC-IAM-USER-IDENTITY" = []))
)]
pub async fn get_pending_approval(
    State(state): State<PendingApprovalState>,
    _identity: BackofficeIdentity,
    Path(doctor_account_id): Path<i32>,
) -> AppResult<Json<PendingDoctorApprovalDetailResponse>> {
    match state.repo.get(doctor_account_id).await? {
        Some(detail) => Ok(Json(
            PendingDoctorApprovalDetailResponse::PendingDoctorApproval(detail),
        )),
        None => Ok(Json(
            PendingDoctorApprovalDetailResponse::PendingDoctorApprovalNotFound,
        )),
    }
}
