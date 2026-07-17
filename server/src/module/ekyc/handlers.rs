use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Serialize;
use tracing::{instrument, warn};
use utoipa::ToSchema;

use crate::core::auth::DoctorIdentity;
use crate::core::error::AppResult;

use super::service::{EkycResult, EkycServiceTrait};

/// Axum state for the eKYC route.
#[derive(Clone)]
pub struct EkycState {
    pub service: Arc<dyn EkycServiceTrait>,
}

/// Wire response (typed-variant convention; 200 OK for all variants).
#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "__type")]
pub enum EkycResponse {
    #[serde(rename_all = "camelCase")]
    EkycAvailable {
        document_image_url: String,
        liveness_image_url: String,
        full_name: String,
        birth_date: String,
        gender: String,
    },
    EkycNotAvailable,
    AppointmentNotFound,
}

/// `GET /appointment/v1/{bookingId}/ekyc`
#[utoipa::path(
    get,
    path = "/appointment/v1/{bookingId}/ekyc",
    tag = "appointment",
    params(
        ("bookingId" = String, Path, description = "Booking ID")
    ),
    responses(
        (status = 200, description = "EkycAvailable | EkycNotAvailable | AppointmentNotFound",
         body = EkycResponse),
        (status = 401, description = "Unauthorized"),
        (status = 502, description = "Upstream (eagle / consultation) failure"),
    ),
    security(("TDH-SEC-IAM-USER-IDENTITY" = []))
)]
#[instrument(
    name = "appointment_ekyc",
    skip(state),
    fields(booking_id = %booking_id, doctor_account_id = %identity.doctor_account_id)
)]
pub async fn get_appointment_ekyc(
    State(state): State<EkycState>,
    identity: DoctorIdentity,
    Path(booking_id): Path<String>,
) -> AppResult<Json<EkycResponse>> {
    let result = state.service.get_ekyc_by_booking_id(&booking_id).await?;

    Ok(Json(match result {
        EkycResult::Found(detail) => EkycResponse::EkycAvailable {
            document_image_url: detail.document_image_url,
            liveness_image_url: detail.liveness_image_url,
            full_name: detail.full_name,
            birth_date: detail.birth_date,
            gender: detail.gender,
        },
        EkycResult::AppointmentNotFound => EkycResponse::AppointmentNotFound,
        EkycResult::EkycNotAvailable => {
            warn!(booking_id = %booking_id, "patient has no eKYC on file");
            EkycResponse::EkycNotAvailable
        }
    }))
}
