use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};

use crate::core::auth::DoctorIdentity;
use crate::core::error::AppResult;
use crate::core::RequestId;

use super::services::{
    ConsultationService, EndSessionResult, FaceVerificationRequest, GetSessionInfoResult,
};

#[derive(Debug, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct SessionQuery {
    pub appointment_id: String,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EndSessionBody {
    pub appointment_id: String,
}

#[utoipa::path(
    post,
    path = "/consultation/v1/submit/face-verification",
    tag = "consultation",
    request_body = FaceVerificationRequest,
    responses(
        (status = 200, description = "Face verification submitted"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("TDH-SEC-IAM-USER-IDENTITY" = []))
)]
pub async fn submit_face_verification(
    State(svc): State<Arc<ConsultationService>>,
    identity: DoctorIdentity,
    request_id: RequestId,
    Json(req): Json<FaceVerificationRequest>,
) -> AppResult<impl IntoResponse> {
    svc.submit_face_verification(
        &request_id.0,
        identity.doctor_account_id,
        &req.appointment_id,
        &req.image,
    )
    .await?;
    Ok(StatusCode::OK)
}

#[utoipa::path(
    post,
    path = "/consultation/v1/end-session",
    tag = "consultation",
    request_body = EndSessionBody,
    responses(
        (status = 200, description = "Session ended", body = EndSessionResult),
        (status = 401, description = "Unauthorized"),
    ),
    security(("TDH-SEC-IAM-USER-IDENTITY" = []))
)]
pub async fn end_session(
    State(svc): State<Arc<ConsultationService>>,
    identity: DoctorIdentity,
    request_id: RequestId,
    Json(req): Json<EndSessionBody>,
) -> AppResult<Json<EndSessionResult>> {
    let result = svc
        .end_session(
            &request_id.0,
            identity.doctor_account_id,
            &req.appointment_id,
        )
        .await?;
    Ok(Json(result))
}

#[utoipa::path(
    get,
    path = "/consultation/v1/session-info",
    tag = "consultation",
    params(SessionQuery),
    responses(
        (status = 200, description = "Session information", body = GetSessionInfoResult),
        (status = 401, description = "Unauthorized"),
    ),
    security(("TDH-SEC-IAM-USER-IDENTITY" = []))
)]
pub async fn get_session_info(
    State(svc): State<Arc<ConsultationService>>,
    identity: DoctorIdentity,
    request_id: RequestId,
    Query(params): Query<SessionQuery>,
) -> AppResult<Json<GetSessionInfoResult>> {
    let info = svc
        .get_session_info(
            &request_id.0,
            identity.doctor_account_id,
            &params.appointment_id,
        )
        .await?;
    Ok(Json(info))
}
