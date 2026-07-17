use crate::core::logging::RequestId;
use crate::core::user_identity::UserIdentity;
use axum::{extract::State, response::IntoResponse, Json};
use serde::Serialize;
use utoipa::ToSchema;

use crate::core::error::AppResult;
use crate::model::onboarding::{OnBoarding, OnBoardingRequestPatch};

use super::OnboardingState;

#[derive(Serialize, ToSchema)]
#[serde(tag = "__type")]
pub enum GetOnboardingResponse {
    #[serde(rename = "OnBoardingResponse")]
    Found(Box<OnBoarding>),
    #[serde(rename = "OnBoardingNotFound")]
    NotFound,
}

#[derive(Serialize, ToSchema)]
#[serde(tag = "__type", rename_all = "PascalCase")]
pub enum OnboardingStatusResponse {
    Draft,
    PendingApproval,
    CancelledByUser, // in the domain enum; never produced by this DB-backed read,
    // included so the From mapping below is total.
    Approved,
    Rejected {
        status_reason: String,
    },
    Deactivated {
        status_reason: String,
    },
    #[serde(rename = "OnBoardingNotFound")]
    NotFound,
}

impl From<Option<crate::model::onboarding::OnBoardingStatus>> for OnboardingStatusResponse {
    fn from(status: Option<crate::model::onboarding::OnBoardingStatus>) -> Self {
        use crate::model::onboarding::OnBoardingStatus as S;
        match status {
            Some(S::Draft) => Self::Draft,
            Some(S::PendingApproval) => Self::PendingApproval,
            Some(S::CancelledByUser) => Self::CancelledByUser,
            Some(S::Approved) => Self::Approved,
            Some(S::Rejected { reason }) => Self::Rejected {
                status_reason: reason,
            },
            Some(S::Deactivated { reason }) => Self::Deactivated {
                status_reason: reason,
            },
            None => Self::NotFound,
        }
    }
}

#[derive(Serialize, ToSchema)]
#[serde(tag = "__type")]
pub enum SaveAsDraftResponse {
    Success,
}

#[derive(Serialize, ToSchema)]
#[serde(tag = "__type")]
pub enum SubmitFormResponse {
    Success,
}

#[utoipa::path(
    get,
    path = "/onboarding/v1",
    tag = "onboarding",
    responses(
        (status = 200, description = "OnBoardingResponse or OnBoardingNotFound", body = GetOnboardingResponse),
        (status = 401, description = "Unauthorized"),
    ),
    security(("TDH-SEC-IAM-USER-IDENTITY" = []))
)]
pub async fn get_doctor_profile_draft(
    State(state): State<OnboardingState>,
    request_id: RequestId,
    identity: UserIdentity,
) -> AppResult<impl IntoResponse> {
    let info = state
        .service
        .get_doctor_profile_draft(&request_id.0, identity.account_id)
        .await?;
    match info {
        Some(i) => Ok(Json(GetOnboardingResponse::Found(Box::new(i))).into_response()),
        None => Ok(Json(GetOnboardingResponse::NotFound).into_response()),
    }
}

#[utoipa::path(
    get,
    path = "/onboarding/v1/status",
    tag = "onboarding",
    responses(
        (status = 200, description = "Onboarding status or OnBoardingNotFound", body = OnboardingStatusResponse),
        (status = 401, description = "Unauthorized"),
    ),
    security(("TDH-SEC-IAM-USER-IDENTITY" = []))
)]
pub async fn get_onboarding_status(
    State(state): State<OnboardingState>,
    request_id: RequestId,
    identity: UserIdentity,
) -> AppResult<impl IntoResponse> {
    let status = state
        .service
        .get_onboarding_status(&request_id.0, identity.account_id)
        .await?;
    Ok(Json(OnboardingStatusResponse::from(status)))
}

#[utoipa::path(
    post,
    path = "/onboarding/v1",
    tag = "onboarding",
    request_body = OnBoardingRequestPatch,
    responses(
        (status = 200, description = "Onboarding info saved", body = SaveAsDraftResponse),
        (status = 401, description = "Unauthorized"),
    ),
    security(("TDH-SEC-IAM-USER-IDENTITY" = []))
)]
pub async fn save_doctor_profile_draft(
    State(state): State<OnboardingState>,
    request_id: RequestId,
    identity: UserIdentity,
    Json(input): Json<OnBoardingRequestPatch>,
) -> AppResult<impl IntoResponse> {
    state
        .service
        .save_doctor_profile_draft(
            &request_id.0,
            identity.account_id,
            identity.user_profile_id,
            input,
        )
        .await?;
    Ok(Json(SaveAsDraftResponse::Success).into_response())
}

#[utoipa::path(
    post,
    path = "/onboarding/v1/submit",
    tag = "onboarding",
    request_body = OnBoardingRequestPatch,
    responses(
        (status = 200, description = "Submitted for approval"),
        (status = 400, description = "Validation failed"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("TDH-SEC-IAM-USER-IDENTITY" = []))
)]
pub async fn submit_doctor_profile_draft(
    State(state): State<OnboardingState>,
    request_id: RequestId,
    identity: UserIdentity,
    Json(input): Json<OnBoardingRequestPatch>,
) -> AppResult<impl IntoResponse> {
    state
        .service
        .submit_doctor_profile_draft(
            &request_id.0,
            identity.account_id,
            identity.user_profile_id,
            input,
        )
        .await?;
    Ok(Json(SubmitFormResponse::Success).into_response())
}
