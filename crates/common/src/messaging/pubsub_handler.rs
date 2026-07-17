use crate::core::error::AppError;

/// Response for Pub/Sub push/pull handlers.
#[derive(Debug)]
pub struct WebhookResponse {
    pub message: String,
}

/// Outcome of processing a Pub/Sub message.
///
/// Push handlers translate this to HTTP status codes. Pull subscribers translate
/// this to ACK/NACK settlement.
#[derive(Debug)]
pub enum PubsubProcessingOutcome {
    Success { message: String },
    PermanentFailure { reason: String },
    TransientFailure { error: AppError },
}

impl PubsubProcessingOutcome {
    pub fn from_error(error: AppError) -> Self {
        match &error {
            AppError::PubsubError(_)
            | AppError::ValidationError(_)
            | AppError::BadRequest(_)
            | AppError::InvalidToken(_)
            | AppError::InvalidScheduleTime(_) => PubsubProcessingOutcome::PermanentFailure {
                reason: error.to_string(),
            },
            AppError::InternalError(_)
            | AppError::FirestoreError(_)
            | AppError::ReqwestError(_)
            | AppError::FcmError(_)
            | AppError::PubsubPublishError(_)
            | AppError::CloudTasksError(_)
            | AppError::DatabaseError(_)
            | AppError::Unauthorized
            | AppError::Forbidden
            | AppError::UpstreamError(_) => PubsubProcessingOutcome::TransientFailure { error },
        }
    }
}
