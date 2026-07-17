use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use jiff::{Timestamp, Zoned};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::core::auth::DoctorIdentity;
use crate::core::error::{AppError, AppResult};
use crate::module::notification::fcm_token::{
    FcmTokenDoc, RegisterFcmTokenRequest, RegisterFcmTokenResponse,
};

use super::{repo::NotificationDoc, NotificationState};

#[derive(Debug, Deserialize, IntoParams)]
pub struct NotificationQuery {
    #[serde(rename = "type")]
    pub notification_type: Option<String>,
    pub category: Option<String>,
    #[serde(rename = "pageToken")]
    pub page_token: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct NotificationListResponse {
    pub notifications: Vec<NotificationDoc>,
    #[serde(rename = "nextPageToken")]
    pub next_page_token: Option<String>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct NotificationTypeQuery {
    #[serde(rename = "type")]
    pub notification_type: Option<String>,
}

#[utoipa::path(
    get,
    path = "/notifications/v1",
    tag = "notifications",
    params(NotificationQuery),
    responses(
        (status = 200, description = "List of notifications with cursor pagination", body = NotificationListResponse),
        (status = 401, description = "Unauthorized"),
        (status = 400, description = "Invalid pageToken format"),
    ),
    security(("TDH-SEC-IAM-USER-IDENTITY" = []))
)]
pub async fn get_notifications(
    State(state): State<NotificationState>,
    identity: DoctorIdentity,
    Query(params): Query<NotificationQuery>,
) -> AppResult<impl IntoResponse> {
    let doctor_id = identity.doctor_account_id.to_string();
    let notification_type = params.notification_type.as_deref().unwrap_or("Alert");

    if notification_type != "Alert" && notification_type != "Announcement" {
        return Err(AppError::ValidationError(format!(
            "Invalid notification type '{}', must be 'Alert' or 'Announcement'",
            notification_type
        )));
    }

    let category = params.category.as_deref();
    if let Some(cat) = category {
        if cat.is_empty() || cat.len() > 100 {
            return Err(AppError::ValidationError(
                "Category must be between 1 and 100 characters".to_string(),
            ));
        }
    }

    let limit = params.limit.unwrap_or(20).min(100);

    // Parse page_token if provided
    let page_token = if let Some(token_str) = params.page_token {
        Some(token_str.parse::<Zoned>().map_err(|_| {
            AppError::ValidationError(format!(
                "Invalid pageToken format: {}, expected ISO 8601",
                token_str
            ))
        })?)
    } else {
        None
    };

    // Fetch from repo
    let docs = state
        .repo
        .get_notifications(&doctor_id, notification_type, category, page_token, limit)
        .await?;

    // Calculate next_page_token
    let next_page_token = if docs.len() < limit as usize {
        None
    } else {
        docs.last().map(|last| last.sent_at().to_string())
    };

    Ok(Json(NotificationListResponse {
        notifications: docs,
        next_page_token,
    })
    .into_response())
}

#[utoipa::path(
    post,
    path = "/notifications/v1",
    tag = "notifications",
    request_body = NotificationDoc,
    responses(
        (status = 201, description = "Notification created"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("TDH-SEC-IAM-USER-IDENTITY" = []))
)]
pub async fn create_notification(
    State(state): State<NotificationState>,
    identity: DoctorIdentity,
    Json(mut notification): Json<NotificationDoc>,
) -> AppResult<impl IntoResponse> {
    let doctor_id = identity.doctor_account_id.to_string();
    let notification_id = Uuid::new_v4().to_string();
    notification.set_notification_id(notification_id.clone());
    state
        .repo
        .create_notification(&doctor_id, &notification)
        .await?;

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({ "notificationId": notification_id })),
    ))
}

#[utoipa::path(
    post,
    path = "/notifications/v1/read/{id}",
    tag = "notifications",
    params(
        ("id" = String, Path, description = "Notification ID")
    ),
    responses(
        (status = 200, description = "Marked as read"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("TDH-SEC-IAM-USER-IDENTITY" = []))
)]
pub async fn mark_as_read(
    State(state): State<NotificationState>,
    identity: DoctorIdentity,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    let doctor_id = identity.doctor_account_id.to_string();
    state.repo.mark_as_read(&doctor_id, &id).await?;

    Ok(StatusCode::OK)
}

#[utoipa::path(
    post,
    path = "/notifications/v1/read-all",
    tag = "notifications",
    params(NotificationTypeQuery),
    responses(
        (status = 200, description = "All marked as read"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("TDH-SEC-IAM-USER-IDENTITY" = []))
)]
pub async fn mark_all_as_read(
    State(state): State<NotificationState>,
    identity: DoctorIdentity,
    Query(params): Query<NotificationTypeQuery>,
) -> AppResult<impl IntoResponse> {
    let doctor_id = identity.doctor_account_id.to_string();
    let notification_type = params.notification_type.as_deref().unwrap_or("Alert");
    state
        .repo
        .mark_all_as_read(&doctor_id, notification_type)
        .await?;

    Ok(StatusCode::OK)
}

#[utoipa::path(
    post,
    path = "/notifications/v1/unread-all",
    tag = "notifications",
    params(NotificationTypeQuery),
    responses(
        (status = 200, description = "All marked as unread"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("TDH-SEC-IAM-USER-IDENTITY" = []))
)]
pub async fn mark_all_as_unread(
    State(state): State<NotificationState>,
    identity: DoctorIdentity,
    Query(params): Query<NotificationTypeQuery>,
) -> AppResult<impl IntoResponse> {
    let doctor_id = identity.doctor_account_id.to_string();
    let notification_type = params.notification_type.as_deref().unwrap_or("Alert");
    state
        .repo
        .mark_all_as_unread(&doctor_id, notification_type)
        .await?;

    Ok(StatusCode::OK)
}

#[utoipa::path(
    post,
    path = "/notifications/v1/fcm-token",
    tag = "notifications",
    request_body = RegisterFcmTokenRequest,
    responses(
        (status = 200, description = "FCM token registered successfully", body = RegisterFcmTokenResponse),
        (status = 400, description = "Invalid FCM token or request parameters"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("TDH-SEC-IAM-USER-IDENTITY" = []))
)]
pub async fn register_fcm_token(
    State(state): State<NotificationState>,
    identity: DoctorIdentity,
    Json(req): Json<RegisterFcmTokenRequest>,
) -> AppResult<impl IntoResponse> {
    // Validate FCM token is not empty
    if req.fcm_token.is_empty() {
        return Err(AppError::InvalidToken(
            "FCM token cannot be empty".to_string(),
        ));
    }

    // Validate platform (case-insensitive)
    let platform_lower = req.platform.to_lowercase();
    if platform_lower != "ios" && platform_lower != "android" {
        return Err(AppError::ValidationError(
            "Platform must be either 'ios' or 'android'".to_string(),
        ));
    }

    // Validate device ID
    if req.device_id.is_empty() {
        return Err(AppError::ValidationError(
            "Device ID cannot be empty".to_string(),
        ));
    }

    if req.device_id.len() > 255 {
        return Err(AppError::ValidationError(
            "Device ID cannot exceed 255 characters".to_string(),
        ));
    }

    let doctor_id = identity.doctor_account_id.to_string();
    let now = Timestamp::now();

    let token_doc = FcmTokenDoc {
        fcm_token: req.fcm_token,
        device_id: req.device_id.clone(),
        platform: req.platform,
        app_version: req.app_version,
        registered_at: now.clone(),
        last_used_at: now,
    };

    state
        .repo
        .save_token(&doctor_id, &req.device_id, &token_doc)
        .await?;

    let response = RegisterFcmTokenResponse {
        success: true,
        token_id: req.device_id,
    };

    Ok(Json(response))
}

#[utoipa::path(
    get,
    path = "/notifications/v1/fcm-token",
    tag = "notifications",
    responses(
        (status = 200, description = "List of registered FCM tokens", body = Vec<FcmTokenDoc>),
        (status = 401, description = "Unauthorized"),
    ),
    security(("TDH-SEC-IAM-USER-IDENTITY" = []))
)]
pub async fn get_fcm_tokens(
    State(state): State<NotificationState>,
    identity: DoctorIdentity,
) -> AppResult<impl IntoResponse> {
    let doctor_id = identity.doctor_account_id.to_string();
    let tokens = state.repo.get_tokens(&doctor_id).await?;
    Ok(Json(tokens))
}

#[utoipa::path(
    delete,
    path = "/notifications/v1/fcm-token/{device_id}",
    tag = "notifications",
    params(
        ("device_id" = String, Path, description = "Device ID to delete FCM token for")
    ),
    responses(
        (status = 204, description = "FCM token deleted successfully"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "FCM token not found"),
    ),
    security(("TDH-SEC-IAM-USER-IDENTITY" = []))
)]
pub async fn delete_fcm_token(
    State(state): State<NotificationState>,
    identity: DoctorIdentity,
    Path(device_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    let doctor_id = identity.doctor_account_id.to_string();
    state.repo.delete_token(&doctor_id, &device_id).await?;
    Ok(StatusCode::NO_CONTENT)
}
