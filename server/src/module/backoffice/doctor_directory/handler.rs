use super::repo::{ApprovedDoctorDetail, ApprovedDoctorDirectoryRepo, ApprovedDoctorSummary};
use crate::core::{auth::BackofficeIdentity, error::AppResult};
use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};

pub fn router(repo: Arc<ApprovedDoctorDirectoryRepo>) -> Router {
    Router::new()
        .route("/", get(list_approved_doctors))
        .route("/{doctor_account_id}", get(get_approved_doctor))
        .with_state(repo)
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct ApprovedDoctorListQuery {
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApprovedDoctorListResponse {
    pub data: Vec<ApprovedDoctorSummary>,
    pub page: u32,
    pub limit: u32,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "__type", rename_all = "PascalCase")]
pub enum ApprovedDoctorDetailResponse {
    ApprovedDoctor(ApprovedDoctorDetail),
    ApprovedDoctorNotFound,
}

#[utoipa::path(
    get,
    path = "/internal/v1/doctors",
    tag = "backoffice",
    params(ApprovedDoctorListQuery),
    responses(
        (status = 200, description = "Active approved doctor directory", body = ApprovedDoctorListResponse),
        (status = 401, description = "Unauthorized"),
    ),
    security(("TDH-SEC-IAM-USER-IDENTITY" = []))
)]
pub async fn list_approved_doctors(
    State(repo): State<Arc<ApprovedDoctorDirectoryRepo>>,
    _identity: BackofficeIdentity,
    Query(params): Query<ApprovedDoctorListQuery>,
) -> AppResult<impl IntoResponse> {
    let page = params.page.unwrap_or(1).max(1);
    let limit = params.limit.unwrap_or(20).clamp(1, 100);
    let doctors = repo.list(page, limit).await?;

    Ok(Json(ApprovedDoctorListResponse {
        data: doctors,
        page,
        limit,
    }))
}

#[utoipa::path(
    get,
    path = "/internal/v1/doctors/{doctor_account_id}",
    tag = "backoffice",
    params(
        ("doctor_account_id" = i32, Path, description = "Doctor account ID")
    ),
    responses(
        (status = 200, description = "Active approved doctor or typed not-found", body = ApprovedDoctorDetailResponse),
        (status = 401, description = "Unauthorized"),
    ),
    security(("TDH-SEC-IAM-USER-IDENTITY" = []))
)]
pub async fn get_approved_doctor(
    State(repo): State<Arc<ApprovedDoctorDirectoryRepo>>,
    _identity: BackofficeIdentity,
    Path(doctor_account_id): Path<i32>,
) -> AppResult<Json<ApprovedDoctorDetailResponse>> {
    match repo.get(doctor_account_id).await? {
        Some(doctor) => Ok(Json(ApprovedDoctorDetailResponse::ApprovedDoctor(doctor))),
        None => Ok(Json(ApprovedDoctorDetailResponse::ApprovedDoctorNotFound)),
    }
}
