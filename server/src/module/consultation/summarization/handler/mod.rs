use std::sync::Arc;

use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};

use crate::core::error::AppResult;
use crate::core::logging::RequestId;
use crate::core::user_identity::UserIdentity;

pub mod dto;

pub use dto::{
    GetDraftResponse, GetSummarizationResponse, SaveDraftRequest, SaveDraftResult, SubmitRequest,
    SubmitResponse, SubmitSummaryNote,
};

use super::service::SummarizationService;

// ─── State ────────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct SummarizationState {
    pub service: Arc<SummarizationService>,
}

// ─── Handlers ─────────────────────────────────────────────────────────────────

/// `GET /consultation/v1/summarization/:appointment_id`
#[utoipa::path(
    get,
    path = "/consultation/v1/summarization/{appointment_id}",
    tag = "consultation",
    params(
        ("appointment_id" = String, Path, description = "Appointment ID")
    ),
    responses(
        (status = 200, description = "PendingRecord | SummarizationRecord (Draft/Submitted)",
         body = GetSummarizationResponse),
        (status = 401, description = "Unauthorized"),
    ),
    security(("TDH-SEC-IAM-USER-IDENTITY" = []))
)]
pub async fn get_summary_note(
    State(state): State<SummarizationState>,
    user_identity: UserIdentity,
    Path(appointment_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    let response = state
        .service
        .get_draft(&user_identity, &appointment_id)
        .await?;
    Ok(Json(response))
}

/// `POST /consultation/v1/summarization/draft`
#[utoipa::path(
    post,
    path = "/consultation/v1/summarization/draft",
    tag = "consultation",
    request_body = SaveDraftRequest,
    responses(
        (status = 200, description = "Success | AlreadySubmitted | Unauthorized", body = SaveDraftResult),
        (status = 401, description = "Unauthorized"),
    ),
    security(("TDH-SEC-IAM-USER-IDENTITY" = []))
)]
pub async fn save_draft(
    State(state): State<SummarizationState>,
    user_identity: UserIdentity,
    Json(body): Json<SaveDraftRequest>,
) -> AppResult<impl IntoResponse> {
    let response = state.service.save_draft(&user_identity, body).await?;
    Ok(Json(response))
}

/// `POST /consultation/v1/summarization/submit`
#[utoipa::path(
    post,
    path = "/consultation/v1/summarization/submit",
    tag = "consultation",
    request_body = SubmitRequest,
    responses(
        (status = 200, description = "Success | AlreadySubmitted | Unauthorized | PrescriptionServiceError | ConsultationServiceError",
         body = SubmitResponse),
        (status = 401, description = "Unauthorized"),
    ),
    security(("TDH-SEC-IAM-USER-IDENTITY" = []))
)]
pub async fn submit(
    State(state): State<SummarizationState>,
    request_id: RequestId,
    user_identity: UserIdentity,
    Json(body): Json<SubmitRequest>,
) -> AppResult<impl IntoResponse> {
    let response = state
        .service
        .save_and_submit(&request_id.0, &user_identity, body)
        .await?;
    Ok(Json(response))
}
