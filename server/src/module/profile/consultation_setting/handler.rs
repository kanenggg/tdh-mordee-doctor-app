use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};

use super::{
    model::{ScheduleAvailableConfig, UpdateScheduleConfigResponse},
    service::ConsultationSettingService,
};
use crate::core::{auth::DoctorIdentity, error::AppResult};

#[derive(Clone)]
pub struct ConsultationSettingState {
    pub service: Arc<ConsultationSettingService>,
}

#[utoipa::path(
    put,
    path = "/profile/v1/consultation-setting/schedule-config/{bizUnit}",
    tag = "profile",
    params(("bizUnit" = i32, Path, description = "Business unit ID")),
    request_body = ScheduleAvailableConfig,
    responses(
        (status = 200, description = "Success or conflict-time-overlap failure", body = UpdateScheduleConfigResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("TDH-SEC-IAM-USER-IDENTITY" = []))
)]
pub async fn update_schedule_config(
    State(state): State<ConsultationSettingState>,
    identity: DoctorIdentity,
    Path(biz_unit_id): Path<i32>,
    Json(req): Json<ScheduleAvailableConfig>,
) -> AppResult<Json<UpdateScheduleConfigResponse>> {
    let response = state
        .service
        .update_schedule_config(identity.doctor_account_id, biz_unit_id, req)
        .await?;

    Ok(Json(response))
}

#[utoipa::path(
    get,
    path = "/profile/v1/consultation-setting/schedule-config/{bizUnit}",
    tag = "profile",
    params(("bizUnit" = i32, Path, description = "Business unit ID")),
    responses(
        (status = 200, description = "Schedule configuration", body = ScheduleAvailableConfig),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("TDH-SEC-IAM-USER-IDENTITY" = []))
)]
pub async fn get_schedule_config(
    State(state): State<ConsultationSettingState>,
    identity: DoctorIdentity,
    Path(biz_unit_id): Path<i32>,
) -> AppResult<Json<ScheduleAvailableConfig>> {
    let config = state
        .service
        .get_schedule_config(identity.doctor_account_id, biz_unit_id)
        .await?;

    Ok(Json(config))
}
