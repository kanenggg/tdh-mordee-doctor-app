use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Unauthorized")]
    Unauthorized,

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Invalid FCM token: {0}")]
    InvalidToken(String),

    #[error("Internal error: {0}")]
    InternalError(String),

    #[error("Firestore error: {0}")]
    FirestoreError(String),

    #[error("Pub/Sub error: {0}")]
    PubsubError(String),

    #[error("Pub/Sub publish error: {0}")]
    PubsubPublishError(String),

    #[error("Cloud Tasks error: {0}")]
    CloudTasksError(String),

    #[error("Invalid schedule time: {0}")]
    InvalidScheduleTime(String),

    #[error("FCM error: {0}")]
    FcmError(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Forbidden")]
    Forbidden,

    #[error("Upstream service unavailable: {0}")]
    UpstreamError(String),

    #[error(transparent)]
    ReqwestError(#[from] reqwest::Error),
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        AppError::InternalError(e.to_string())
    }
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        AppError::DatabaseError(e.to_string())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, self.to_string()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::ValidationError(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::InvalidToken(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::InternalError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
            AppError::FirestoreError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
            AppError::PubsubError(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::PubsubPublishError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
            AppError::CloudTasksError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
            AppError::InvalidScheduleTime(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::FcmError(msg) => (StatusCode::SERVICE_UNAVAILABLE, msg.clone()),
            AppError::DatabaseError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
            AppError::Forbidden => (StatusCode::FORBIDDEN, self.to_string()),
            AppError::UpstreamError(_) => (StatusCode::BAD_GATEWAY, self.to_string()),
            AppError::ReqwestError(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        };

        (status, Json(json!({ "error": message }))).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;
