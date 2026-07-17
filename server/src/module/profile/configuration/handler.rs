use std::sync::Arc;

use axum::{
    extract::State,
    routing::{get, patch},
    Json, Router,
};

use super::models::{
    GetDoctorConfigurationResponse, UpdateChannelRequest, UpdateConfigurationResponse,
    UpdateLanguageRequest,
};
use super::service::DoctorConfigurationService;
use crate::core::{auth::DoctorIdentity, error::AppResult};

#[derive(Clone)]
pub struct ConfigurationState {
    pub service: Arc<DoctorConfigurationService>,
}

pub fn routes(service: Arc<DoctorConfigurationService>) -> Router {
    let state = ConfigurationState { service };

    Router::new()
        .route("/v1/doctor-configuration", get(get_doctor_configuration))
        .route("/v1/doctor-channel", patch(update_doctor_channel))
        .route("/v1/doctor-language", patch(update_doctor_language))
        .with_state(state)
}

#[utoipa::path(
    get,
    path = "/profile/v1/doctor-configuration",
    tag = "profile",
    responses(
        (status = 200, description = "Doctor configuration or DoctorConfigurationNotFound", body = GetDoctorConfigurationResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("TDH-SEC-IAM-USER-IDENTITY" = []))
)]
pub async fn get_doctor_configuration(
    State(state): State<ConfigurationState>,
    identity: DoctorIdentity,
) -> AppResult<Json<GetDoctorConfigurationResponse>> {
    let response = state
        .service
        .get_configuration(identity.doctor_account_id)
        .await?;

    Ok(Json(response))
}

#[utoipa::path(
    patch,
    path = "/profile/v1/doctor-channel",
    tag = "profile",
    request_body = UpdateChannelRequest,
    responses(
        (status = 200, description = "Channels updated", body = UpdateConfigurationResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("TDH-SEC-IAM-USER-IDENTITY" = []))
)]
pub async fn update_doctor_channel(
    State(state): State<ConfigurationState>,
    identity: DoctorIdentity,
    Json(req): Json<UpdateChannelRequest>,
) -> AppResult<Json<UpdateConfigurationResponse>> {
    let response = state
        .service
        .update_channels(identity.doctor_account_id, req.channel)
        .await?;

    Ok(Json(response))
}

#[utoipa::path(
    patch,
    path = "/profile/v1/doctor-language",
    tag = "profile",
    request_body = UpdateLanguageRequest,
    responses(
        (status = 200, description = "Languages updated", body = UpdateConfigurationResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("TDH-SEC-IAM-USER-IDENTITY" = []))
)]
pub async fn update_doctor_language(
    State(state): State<ConfigurationState>,
    identity: DoctorIdentity,
    Json(req): Json<UpdateLanguageRequest>,
) -> AppResult<Json<UpdateConfigurationResponse>> {
    let response = state
        .service
        .update_languages(identity.doctor_account_id, req.language)
        .await?;

    Ok(Json(response))
}
