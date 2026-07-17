use super::service::{ApproveDoctorInfo, BackofficeOnboardingService};
use crate::core::error::AppResult;
use crate::core::logging::RequestId;
use crate::module::backoffice::consultation_configuration::DoctorConsultationConfig;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Deserialize;
use std::sync::Arc;
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApproveRequest {
    pub doctor_account_id: i32,
    pub department_id: i32,
    pub clinics: Vec<i32>,
    pub consultation_config: DoctorConsultationConfig,
}

impl From<ApproveRequest> for ApproveDoctorInfo {
    fn from(req: ApproveRequest) -> Self {
        Self {
            doctor_account_id: req.doctor_account_id,
            department_id: req.department_id,
            clinics: req.clinics,
            consultation_config: req.consultation_config.into(),
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RejectRequest {
    pub doctor_account_id: i32,
    pub reason: Option<String>,
}

#[derive(Clone)]
pub struct BackofficeOnboardingState {
    pub service: Arc<BackofficeOnboardingService>,
}

// -- Handlers --

#[utoipa::path(
    post,
    path = "/internal/v1/onboarding/approve",
    tag = "backoffice",
    request_body = ApproveRequest,
    responses(
        (status = 200, description = "Doctor approved and profile created"),
        (status = 400, description = "Onboarding info not found"),
    )
)]
pub async fn approve_doctor(
    State(state): State<BackofficeOnboardingState>,
    request_id: RequestId,
    Json(req): Json<ApproveRequest>,
) -> AppResult<impl IntoResponse> {
    state
        .service
        .approve_doctor(&request_id.0, req.into())
        .await?;
    Ok(StatusCode::OK)
}

#[utoipa::path(
    post,
    path = "/internal/v1/onboarding/reject",
    tag = "backoffice",
    request_body = RejectRequest,
    responses(
        (status = 200, description = "Doctor rejected"),
        (status = 401, description = "Unauthorized"),
    ),
)]
pub async fn reject_doctor(
    State(state): State<BackofficeOnboardingState>,
    request_id: RequestId,
    Json(req): Json<RejectRequest>,
) -> AppResult<impl IntoResponse> {
    state
        .service
        .reject_doctor(
            &request_id.0,
            req.doctor_account_id,
            req.reason.unwrap_or_default(),
        )
        .await?;
    Ok(StatusCode::OK)
}
