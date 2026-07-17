use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::post, Json, Router};
use common::messaging::pubsub_handler::PubsubProcessingOutcome;
use serde::Serialize;
use tdh_protocol::notification::PubsubPushMessage;

use super::service::DoctorCalendarUpdateHandler;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiResponse {
    pub status: &'static str,
    pub message: String,
}

pub fn routes(service: Arc<dyn DoctorCalendarUpdateHandler>) -> Router {
    Router::new()
        .route("/consultation/v1/doctor-calendar/update", post(update))
        .route("/consultation/v1/events/status-changed", post(update))
        .with_state(service)
}

async fn update(
    State(service): State<Arc<dyn DoctorCalendarUpdateHandler>>,
    Json(push): Json<PubsubPushMessage>,
) -> impl IntoResponse {
    match service.update_doctor_calendar(push).await {
        PubsubProcessingOutcome::Success { message } => (
            StatusCode::OK,
            Json(ApiResponse {
                status: "processed",
                message,
            }),
        ),
        PubsubProcessingOutcome::PermanentFailure { reason } => (
            StatusCode::OK,
            Json(ApiResponse {
                status: "ignored",
                message: reason,
            }),
        ),
        PubsubProcessingOutcome::TransientFailure { error } => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse {
                status: "retryableError",
                message: error.to_string(),
            }),
        ),
    }
}
