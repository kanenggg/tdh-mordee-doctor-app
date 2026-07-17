use super::service::{DoctorManagementService, UpdateConsultationConfigInfo};
use crate::core::auth::BackofficeIdentity;
use crate::core::error::AppResult;
use crate::core::logging::RequestId;
use crate::module::backoffice::consultation_configuration::DoctorConsultationConfig;
use crate::module::profile::configuration::models::UpdateConfigurationResponse;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Deserialize;
use std::sync::Arc;
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateConsultationConfigurationRequest {
    pub doctor_account_id: i32,
    pub consultation_config: DoctorConsultationConfig,
}

impl From<UpdateConsultationConfigurationRequest> for UpdateConsultationConfigInfo {
    fn from(req: UpdateConsultationConfigurationRequest) -> Self {
        Self {
            doctor_account_id: req.doctor_account_id,
            consultation_config: req.consultation_config.into(),
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDoctorActiveStatusRequest {
    pub doctor_account_id: i32,
    pub is_active: bool,
}

#[derive(Clone)]
pub struct DoctorManagementState {
    pub service: Arc<DoctorManagementService>,
}

#[utoipa::path(
    patch,
    path = "/internal/v1/consultation-configuration",
    tag = "backoffice",
    request_body = UpdateConsultationConfigurationRequest,
    responses(
        (status = 200, description = "Doctor consultation configuration updated", body = UpdateConfigurationResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
    )
)]
pub async fn update_consultation_configuration(
    State(state): State<DoctorManagementState>,
    identity: BackofficeIdentity,
    request_id: RequestId,
    Json(req): Json<UpdateConsultationConfigurationRequest>,
) -> AppResult<Json<UpdateConfigurationResponse>> {
    let response = state
        .service
        .update_consultation_config(&request_id.0, identity.account_id, req.into())
        .await?;

    Ok(Json(response))
}

#[utoipa::path(
    patch,
    path = "/internal/v1/doctor-active-status",
    tag = "backoffice",
    request_body = UpdateDoctorActiveStatusRequest,
    responses(
        (status = 200, description = "Doctor status updated (idempotent)"),
    )
)]
pub async fn update_doctor_active_status(
    State(state): State<DoctorManagementState>,
    request_id: RequestId,
    Json(req): Json<UpdateDoctorActiveStatusRequest>,
) -> AppResult<impl IntoResponse> {
    state
        .service
        .update_doctor_active_status(&request_id.0, req.doctor_account_id, req.is_active)
        .await?;
    Ok(StatusCode::OK)
}
