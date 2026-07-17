use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};

use crate::core::error::AppResult;
use crate::{core::auth::BackofficeIdentity, model::onboarding::OnBoarding};

use super::repo::BackofficeRepo;

// -- Query / Request / Response types --

#[derive(Debug, Deserialize, IntoParams)]
pub struct ListDoctorsQuery {
    pub status: Option<String>,
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateDoctorRequest {
    pub doctor_account_id: i32,
    #[serde(flatten)]
    pub info: OnBoarding,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DoctorListResponse {
    pub data: Vec<OnBoarding>,
    pub page: u32,
    pub limit: u32,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "__type")]
pub enum GetDoctorResponse {
    DoctorProfile(Box<OnBoarding>),
    DoctorNotFound,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "__type")]
pub enum UpdateDoctorResponse {
    Success,
    DoctorNotFound,
}

// -- Handlers --

#[utoipa::path(
    get,
    path = "/backoffice/v1/doctors",
    tag = "backoffice",
    params(ListDoctorsQuery),
    responses(
        (status = 200, description = "List of doctor profiles", body = DoctorListResponse),
        (status = 401, description = "Unauthorized"),
    ),
    security(("TDH-SEC-IAM-USER-IDENTITY" = []))
)]
pub async fn list_doctors(
    State(repo): State<Arc<BackofficeRepo>>,
    _identity: BackofficeIdentity,
    Query(params): Query<ListDoctorsQuery>,
) -> AppResult<impl IntoResponse> {
    let page = params.page.unwrap_or(1);
    let limit = params.limit.unwrap_or(20);
    let doctors = repo
        .list_doctors(params.status.as_deref(), page, limit)
        .await?;
    Ok(Json(DoctorListResponse {
        data: doctors,
        page,
        limit,
    }))
}

#[utoipa::path(
    get,
    path = "/backoffice/v1/doctors/{id}",
    tag = "backoffice",
    params(
        ("id" = i32, Path, description = "Doctor account ID")
    ),
    responses(
        (status = 200, description = "Doctor profile", body = GetDoctorResponse),
        (status = 401, description = "Unauthorized"),
    ),
    security(("TDH-SEC-IAM-USER-IDENTITY" = []))
)]
pub async fn get_doctor(
    State(repo): State<Arc<BackofficeRepo>>,
    _identity: BackofficeIdentity,
    Path(id): Path<i32>,
) -> AppResult<Json<GetDoctorResponse>> {
    match repo.get_doctor(id).await? {
        Some(info) => Ok(Json(GetDoctorResponse::DoctorProfile(Box::new(info)))),
        None => Ok(Json(GetDoctorResponse::DoctorNotFound)),
    }
}

#[utoipa::path(
    post,
    path = "/backoffice/v1/doctors",
    tag = "backoffice",
    request_body = CreateDoctorRequest,
    responses(
        (status = 201, description = "Doctor profile created"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("TDH-SEC-IAM-USER-IDENTITY" = []))
)]
pub async fn create_doctor(
    State(repo): State<Arc<BackofficeRepo>>,
    _identity: BackofficeIdentity,
    Json(req): Json<CreateDoctorRequest>,
) -> AppResult<impl IntoResponse> {
    repo.create_doctor(req.doctor_account_id, &req.info).await?;
    Ok(StatusCode::CREATED)
}

#[utoipa::path(
    put,
    path = "/backoffice/v1/doctors/{id}",
    tag = "backoffice",
    params(
        ("id" = i32, Path, description = "Doctor account ID")
    ),
    request_body = OnBoarding,
    responses(
        (status = 200, description = "Doctor profile updated", body = UpdateDoctorResponse),
        (status = 401, description = "Unauthorized"),
    ),
    security(("TDH-SEC-IAM-USER-IDENTITY" = []))
)]
pub async fn update_doctor(
    State(repo): State<Arc<BackofficeRepo>>,
    _identity: BackofficeIdentity,
    Path(id): Path<i32>,
    Json(info): Json<OnBoarding>,
) -> AppResult<Json<UpdateDoctorResponse>> {
    let existing = repo.get_doctor(id).await?;
    if existing.is_none() {
        return Ok(Json(UpdateDoctorResponse::DoctorNotFound));
    }
    repo.update_doctor(id, &info).await?;
    Ok(Json(UpdateDoctorResponse::Success))
}
