use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use common::messaging::pubsub_handler::PubsubProcessingOutcome;
use serde::Serialize;
use tdh_protocol::notification::{PubsubPushMessage, ScheduledNotificationTask};

use super::{
    service::{
        DoctorNotificationDeliveryHandler, DoctorNotificationSendHandler,
        ScheduledNotificationTaskHandler, ScheduledNotificationTaskOutcome,
    },
    DELIVERY_PATH, SCHEDULED_NOTIFICATION_TASK_PATH, SEND_PATH, TASK_HEALTH_PATH,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiResponse {
    pub status: &'static str,
    pub message: String,
}

#[derive(Clone)]
pub struct ScheduledNotificationState {
    handler: Option<Arc<dyn ScheduledNotificationTaskHandler>>,
}

impl ScheduledNotificationState {
    fn new(handler: Option<Arc<dyn ScheduledNotificationTaskHandler>>) -> Self {
        Self { handler }
    }
}

#[derive(Clone)]
pub struct DeliveryState {
    handler: Option<Arc<dyn DoctorNotificationDeliveryHandler>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledNotificationTaskResponse {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

pub fn routes(service: Arc<dyn DoctorNotificationSendHandler>) -> Router {
    Router::new()
        .route(SEND_PATH, post(send))
        .with_state(service)
}

pub fn delivery_routes(handler: Option<Arc<dyn DoctorNotificationDeliveryHandler>>) -> Router {
    Router::new()
        .route(DELIVERY_PATH, post(deliver))
        .with_state(DeliveryState { handler })
}

pub fn scheduled_notification_routes(
    handler: Option<Arc<dyn ScheduledNotificationTaskHandler>>,
) -> Router {
    Router::new()
        .route(
            SCHEDULED_NOTIFICATION_TASK_PATH,
            post(process_scheduled_notification),
        )
        .route(TASK_HEALTH_PATH, get(health))
        .with_state(ScheduledNotificationState::new(handler))
}

async fn deliver(
    State(state): State<DeliveryState>,
    Json(push): Json<PubsubPushMessage>,
) -> impl IntoResponse {
    let Some(handler) = state.handler else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiResponse {
                status: "unavailable",
                message: "Delivery handler is not configured".to_string(),
            }),
        );
    };

    match handler.deliver_doctor_notification(push).await {
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

async fn send(
    State(service): State<Arc<dyn DoctorNotificationSendHandler>>,
    Json(push): Json<PubsubPushMessage>,
) -> impl IntoResponse {
    match service.send_doctor_notification(push).await {
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

async fn process_scheduled_notification(
    State(state): State<ScheduledNotificationState>,
    Json(task): Json<ScheduledNotificationTask>,
) -> impl IntoResponse {
    let Some(handler) = state.handler else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ScheduledNotificationTaskResponse {
                status: "unavailable".to_string(),
                message: Some("Task handler is not configured".to_string()),
            }),
        );
    };

    match handler.process_scheduled_notification(task).await {
        Ok(ScheduledNotificationTaskOutcome::Chained(task_name)) => (
            StatusCode::OK,
            Json(ScheduledNotificationTaskResponse {
                status: "chained".to_string(),
                message: Some(format!("Rescheduled as {}", task_name)),
            }),
        ),
        Ok(ScheduledNotificationTaskOutcome::Sent(message)) => (
            StatusCode::OK,
            Json(ScheduledNotificationTaskResponse {
                status: "sent".to_string(),
                message: Some(message),
            }),
        ),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ScheduledNotificationTaskResponse {
                status: "retryableError".to_string(),
                message: Some(error.to_string()),
            }),
        ),
    }
}

async fn health() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(ApiResponse {
            status: "ok",
            message: "task handler is healthy".to_string(),
        }),
    )
}
