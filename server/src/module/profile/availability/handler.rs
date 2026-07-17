use std::sync::Arc;

use axum::{
    extract::{Query, State},
    Json,
};

use super::{
    models::{AvailabilityResponse, UpdateAvailabilityRequest},
    service::AvailabilityService,
};
use crate::core::{auth::DoctorIdentity, error::AppResult};
use crate::module::profile::common::{BizUnitQuery, SuccessResponse};

#[derive(Clone)]
pub struct AvailabilityState {
    pub service: Arc<AvailabilityService>,
}

#[utoipa::path(
    post,
    path = "/profile/v1/availability/schedule",
    tag = "profile",
    request_body = UpdateAvailabilityRequest,
    responses(
        (status = 200, description = "Schedule availability saved", body = SuccessResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("TDH-SEC-IAM-USER-IDENTITY" = []))
)]
pub async fn update_schedule_availability(
    State(state): State<AvailabilityState>,
    identity: DoctorIdentity,
    Json(req): Json<UpdateAvailabilityRequest>,
) -> AppResult<Json<SuccessResponse>> {
    state
        .service
        .set_schedule_availability(identity.doctor_account_id, req.biz_unit_id, req.available)
        .await?;

    Ok(Json(SuccessResponse::Success))
}

#[utoipa::path(
    post,
    path = "/profile/v1/availability/instant",
    tag = "profile",
    request_body = UpdateAvailabilityRequest,
    responses(
        (status = 200, description = "Instant availability saved", body = SuccessResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("TDH-SEC-IAM-USER-IDENTITY" = []))
)]
pub async fn update_instant_availability(
    State(state): State<AvailabilityState>,
    identity: DoctorIdentity,
    Json(req): Json<UpdateAvailabilityRequest>,
) -> AppResult<Json<SuccessResponse>> {
    state
        .service
        .set_instant_availability(identity.doctor_account_id, req.biz_unit_id, req.available)
        .await?;

    Ok(Json(SuccessResponse::Success))
}

#[utoipa::path(
    get,
    path = "/profile/v1/availability",
    tag = "profile",
    params(BizUnitQuery),
    responses(
        (status = 200, description = "Consultation availability", body = AvailabilityResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("TDH-SEC-IAM-USER-IDENTITY" = []))
)]
pub async fn get_availability(
    State(state): State<AvailabilityState>,
    identity: DoctorIdentity,
    Query(query): Query<BizUnitQuery>,
) -> AppResult<Json<AvailabilityResponse>> {
    let availability = state
        .service
        .get_availability(identity.doctor_account_id, query.biz_unit_id)
        .await?;

    Ok(Json(AvailabilityResponse::success(
        query.biz_unit_id,
        availability.schedule_available,
        availability.instant_available,
    )))
}
