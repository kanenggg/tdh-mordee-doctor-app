//! Integration tests for FCM token endpoints
//!
//! Tests the FCM token registration, retrieval, and deletion functionality
//! including validation, authentication, and error handling.

use axum::http::StatusCode;
use axum::Router;
use axum_test::TestServer;
use serde_json::json;
use std::sync::Arc;

use server::module::notification::fcm_token::FcmTokenDoc;
use server::module::notification::repo::NotificationRepoTrait;

/// Mock implementation of NotificationRepoTrait for testing
struct MockNotificationRepo {
    tokens: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, Vec<FcmTokenDoc>>>>,
}

impl MockNotificationRepo {
    fn new() -> Self {
        Self {
            tokens: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
        }
    }
}

#[async_trait::async_trait]
impl NotificationRepoTrait for MockNotificationRepo {
    async fn get_notifications(
        &self,
        _doctor_id: &str,
        _notification_type: &str,
        _category: Option<&str>,
        _page_token: Option<jiff::Zoned>,
        _limit: u32,
    ) -> server::core::error::AppResult<Vec<server::module::notification::repo::NotificationDoc>>
    {
        Ok(vec![])
    }

    async fn mark_as_read(
        &self,
        _doctor_id: &str,
        _notification_id: &str,
    ) -> server::core::error::AppResult<()> {
        Ok(())
    }

    async fn mark_all_as_read(
        &self,
        _doctor_id: &str,
        _notification_type: &str,
    ) -> server::core::error::AppResult<()> {
        Ok(())
    }

    async fn mark_all_as_unread(
        &self,
        _doctor_id: &str,
        _notification_type: &str,
    ) -> server::core::error::AppResult<()> {
        Ok(())
    }

    async fn create_notification(
        &self,
        _doctor_id: &str,
        _notification: &server::module::notification::repo::NotificationDoc,
    ) -> server::core::error::AppResult<()> {
        Ok(())
    }

    async fn save_token(
        &self,
        doctor_id: &str,
        device_id: &str,
        token: &FcmTokenDoc,
    ) -> server::core::error::AppResult<()> {
        let mut tokens = self.tokens.lock().unwrap();
        let doctor_tokens = tokens.entry(doctor_id.to_string()).or_insert_with(Vec::new);

        // Remove existing token for same device_id if exists
        doctor_tokens.retain(|t| t.device_id != device_id);
        doctor_tokens.push(token.clone());

        Ok(())
    }

    async fn get_tokens(
        &self,
        doctor_id: &str,
    ) -> server::core::error::AppResult<Vec<FcmTokenDoc>> {
        let tokens = self.tokens.lock().unwrap();
        Ok(tokens.get(doctor_id).cloned().unwrap_or_default())
    }

    async fn delete_token(
        &self,
        doctor_id: &str,
        device_id: &str,
    ) -> server::core::error::AppResult<()> {
        let mut tokens = self.tokens.lock().unwrap();
        if let Some(doctor_tokens) = tokens.get_mut(doctor_id) {
            doctor_tokens.retain(|t| t.device_id != device_id);
        }
        Ok(())
    }

    async fn save_scheduled_notification(
        &self,
        _doc: &server::module::notification::repo::ScheduledNotificationDoc,
    ) -> server::core::error::AppResult<()> {
        Ok(())
    }

    async fn get_scheduled_notification(
        &self,
        _notification_id: &str,
    ) -> server::core::error::AppResult<
        Option<server::module::notification::repo::ScheduledNotificationDoc>,
    > {
        Ok(None)
    }

    async fn update_scheduled_notification_status(
        &self,
        _notification_id: &str,
        _status: server::module::notification::repo::ScheduledNotificationStatus,
    ) -> server::core::error::AppResult<()> {
        Ok(())
    }

    async fn get_pending_scheduled_notifications_by_booking_id(
        &self,
        _booking_id: &str,
    ) -> server::core::error::AppResult<
        Vec<server::module::notification::repo::ScheduledNotificationDoc>,
    > {
        Ok(vec![])
    }
}

fn create_test_router() -> Router {
    let repo: Arc<dyn NotificationRepoTrait> = Arc::new(MockNotificationRepo::new());
    axum::Router::new()
        .route(
            "/fcm-token",
            axum::routing::post(server::module::notification::handlers::register_fcm_token),
        )
        .route(
            "/fcm-token",
            axum::routing::get(server::module::notification::handlers::get_fcm_tokens),
        )
        .route(
            "/fcm-token/{device_id}",
            axum::routing::delete(server::module::notification::handlers::delete_fcm_token),
        )
        .with_state(server::module::notification::NotificationState { repo })
}

/// Creates a valid FCM token request (minimum 150 characters)
fn create_valid_token_request(device_id: &str) -> serde_json::Value {
    json!({
        "fcmToken": format!("{}{}", "a".repeat(150), "_additional_token_data_to_make_valid_length"),
        "deviceId": device_id,
        "platform": "ios",
        "appVersion": "1.0.0"
    })
}

// ============================================================================
// Happy Path Tests
// ============================================================================

#[tokio::test]
async fn test_register_new_fcm_token_success() {
    let server = TestServer::new(create_test_router()).unwrap();

    let response = server
        .post("/fcm-token")
        .add_header("tdh-sec-iam-user-identity", "{\"accountId\":123,\"accountType\":2,\"userProfileId\":100,\"userMainProfileId\":100,\"tenantId\":1}")
        .json(&create_valid_token_request("device-001"))
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);

    let body: serde_json::Value = response.json();
    assert_eq!(body["success"], true);
    assert_eq!(body["tokenId"], "device-001");
}

#[tokio::test]
async fn test_update_existing_token_for_same_device() {
    let server = TestServer::new(create_test_router()).unwrap();

    // Register first token
    let first_token = json!({
        "fcmToken": format!("{}{}", "b".repeat(150), "_first_token"),
        "deviceId": "device-001",
        "platform": "ios",
        "appVersion": "1.0.0"
    });

    let response = server
        .post("/fcm-token")
        .add_header("tdh-sec-iam-user-identity", "{\"accountId\":123,\"accountType\":2,\"userProfileId\":100,\"userMainProfileId\":100,\"tenantId\":1}")
        .json(&first_token)
        .await;
    assert_eq!(response.status_code(), StatusCode::OK);

    // Update with new token for same device
    let second_token = json!({
        "fcmToken": format!("{}{}", "c".repeat(150), "_second_token"),
        "deviceId": "device-001",
        "platform": "android",
        "appVersion": "1.1.0"
    });

    let response = server
        .post("/fcm-token")
        .add_header("tdh-sec-iam-user-identity", "{\"accountId\":123,\"accountType\":2,\"userProfileId\":100,\"userMainProfileId\":100,\"tenantId\":1}")
        .json(&second_token)
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);
    let body: serde_json::Value = response.json();
    assert_eq!(body["success"], true);
}

#[tokio::test]
async fn test_register_multiple_tokens_for_different_devices() {
    let server = TestServer::new(create_test_router()).unwrap();

    // Register token for first device
    let token1 = json!({
        "fcmToken": format!("{}{}", "d".repeat(150), "_device_1"),
        "deviceId": "device-ios-001",
        "platform": "ios",
        "appVersion": "1.0.0"
    });

    let response = server
        .post("/fcm-token")
        .add_header("tdh-sec-iam-user-identity", "{\"accountId\":456,\"accountType\":2,\"userProfileId\":100,\"userMainProfileId\":100,\"tenantId\":1}")
        .json(&token1)
        .await;
    assert_eq!(response.status_code(), StatusCode::OK);

    // Register token for second device
    let token2 = json!({
        "fcmToken": format!("{}{}", "e".repeat(150), "_device_2"),
        "deviceId": "device-android-002",
        "platform": "android",
        "appVersion": "1.0.0"
    });

    let response = server
        .post("/fcm-token")
        .add_header("tdh-sec-iam-user-identity", "{\"accountId\":456,\"accountType\":2,\"userProfileId\":100,\"userMainProfileId\":100,\"tenantId\":1}")
        .json(&token2)
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);

    // Get all tokens and verify count
    let response = server
        .get("/fcm-token")
        .add_header("tdh-sec-iam-user-identity", "{\"accountId\":456,\"accountType\":2,\"userProfileId\":100,\"userMainProfileId\":100,\"tenantId\":1}")
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);
    let body: serde_json::Value = response.json();
    assert_eq!(body.as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn test_get_all_tokens_for_doctor() {
    let server = TestServer::new(create_test_router()).unwrap();

    // Register some tokens first
    let token1 = create_valid_token_request("device-001");
    let token2 = create_valid_token_request("device-002");

    server
        .post("/fcm-token")
        .add_header("tdh-sec-iam-user-identity", "{\"accountId\":789,\"accountType\":2,\"userProfileId\":100,\"userMainProfileId\":100,\"tenantId\":1}")
        .json(&token1)
        .await;

    server
        .post("/fcm-token")
        .add_header("tdh-sec-iam-user-identity", "{\"accountId\":789,\"accountType\":2,\"userProfileId\":100,\"userMainProfileId\":100,\"tenantId\":1}")
        .json(&token2)
        .await;

    // Get all tokens
    let response = server
        .get("/fcm-token")
        .add_header("tdh-sec-iam-user-identity", "{\"accountId\":789,\"accountType\":2,\"userProfileId\":100,\"userMainProfileId\":100,\"tenantId\":1}")
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);

    let body: serde_json::Value = response.json();
    let tokens = body.as_array().unwrap();
    assert_eq!(tokens.len(), 2);
    assert_eq!(tokens[0]["deviceId"], "device-001");
    assert_eq!(tokens[1]["deviceId"], "device-002");
}

#[tokio::test]
async fn test_delete_token_success() {
    let server = TestServer::new(create_test_router()).unwrap();

    // Register a token first
    server
        .post("/fcm-token")
        .add_header("tdh-sec-iam-user-identity", "{\"accountId\":999,\"accountType\":2,\"userProfileId\":100,\"userMainProfileId\":100,\"tenantId\":1}")
        .json(&create_valid_token_request("device-to-delete"))
        .await;

    // Delete the token
    let response = server
        .delete("/fcm-token/device-to-delete")
        .add_header("tdh-sec-iam-user-identity", "{\"accountId\":999,\"accountType\":2,\"userProfileId\":100,\"userMainProfileId\":100,\"tenantId\":1}")
        .await;

    assert_eq!(response.status_code(), StatusCode::NO_CONTENT);
}

// ============================================================================
// Validation Error Tests
// ============================================================================

#[tokio::test]
async fn test_empty_token_returns_error() {
    let server = TestServer::new(create_test_router()).unwrap();

    let request = json!({
        "fcmToken": "",
        "deviceId": "device-001",
        "platform": "ios"
    });

    let response = server
        .post("/fcm-token")
        .add_header("tdh-sec-iam-user-identity", "{\"accountId\":111,\"accountType\":2,\"userProfileId\":100,\"userMainProfileId\":100,\"tenantId\":1}")
        .json(&request)
        .await;

    assert_eq!(response.status_code(), StatusCode::BAD_REQUEST);

    let body: serde_json::Value = response.json();
    assert!(body["error"]
        .as_str()
        .unwrap()
        .contains("FCM token cannot be empty"));
}

#[tokio::test]
async fn test_short_token_succeeds() {
    let server = TestServer::new(create_test_router()).unwrap();

    let request = json!({
        "fcmToken": "a".repeat(50), // Short token is valid (no minimum length requirement)
        "deviceId": "device-001",
        "platform": "ios"
    });

    let response = server
        .post("/fcm-token")
        .add_header("tdh-sec-iam-user-identity", "{\"accountId\":222,\"accountType\":2,\"userProfileId\":100,\"userMainProfileId\":100,\"tenantId\":1}")
        .json(&request)
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);

    let body: serde_json::Value = response.json();
    assert_eq!(body["success"], true);
}

#[tokio::test]
async fn test_realistic_fcm_token_succeeds() {
    let server = TestServer::new(create_test_router()).unwrap();

    // Realistic FCM token length (FCM tokens can vary but are typically 100-300 chars)
    let realistic_token = "dWlDXYwk:APA91bG7XvLJKxJJ8N6SYFN8N4FZQN8KNfJvJJ8N6SYFN8N4FZQN8KNfJvJJ8N6SYFN8N4FZQN8KNfJvJJ8N6SYFN8N4FZQN8KNfJ";

    let request = json!({
        "fcmToken": realistic_token,
        "deviceId": "device-001",
        "platform": "ios"
    });

    let response = server
        .post("/fcm-token")
        .add_header("tdh-sec-iam-user-identity", "{\"accountId\":333,\"accountType\":2,\"userProfileId\":100,\"userMainProfileId\":100,\"tenantId\":1}")
        .json(&request)
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);
}

#[tokio::test]
async fn test_invalid_platform_returns_error() {
    let server = TestServer::new(create_test_router()).unwrap();

    let request = json!({
        "fcmToken": "a".repeat(200),
        "deviceId": "device-001",
        "platform": "webcam" // Invalid platform
    });

    let response = server
        .post("/fcm-token")
        .add_header("tdh-sec-iam-user-identity", "{\"accountId\":444,\"accountType\":2,\"userProfileId\":100,\"userMainProfileId\":100,\"tenantId\":1}")
        .json(&request)
        .await;

    assert_eq!(response.status_code(), StatusCode::BAD_REQUEST);

    let body: serde_json::Value = response.json();
    assert!(body["error"]
        .as_str()
        .unwrap()
        .contains("Platform must be either 'ios' or 'android'"));
}

#[tokio::test]
async fn test_empty_device_id_returns_error() {
    let server = TestServer::new(create_test_router()).unwrap();

    let request = json!({
        "fcmToken": "a".repeat(200),
        "deviceId": "",
        "platform": "ios"
    });

    let response = server
        .post("/fcm-token")
        .add_header("tdh-sec-iam-user-identity", "{\"accountId\":555,\"accountType\":2,\"userProfileId\":100,\"userMainProfileId\":100,\"tenantId\":1}")
        .json(&request)
        .await;

    assert_eq!(response.status_code(), StatusCode::BAD_REQUEST);

    let body: serde_json::Value = response.json();
    assert!(body["error"]
        .as_str()
        .unwrap()
        .contains("Device ID cannot be empty"));
}

#[tokio::test]
async fn test_device_id_exceeds_255_chars_returns_error() {
    let server = TestServer::new(create_test_router()).unwrap();

    let request = json!({
        "fcmToken": "a".repeat(200),
        "deviceId": "d".repeat(256), // One character over limit
        "platform": "ios"
    });

    let response = server
        .post("/fcm-token")
        .add_header("tdh-sec-iam-user-identity", "{\"accountId\":666,\"accountType\":2,\"userProfileId\":100,\"userMainProfileId\":100,\"tenantId\":1}")
        .json(&request)
        .await;

    assert_eq!(response.status_code(), StatusCode::BAD_REQUEST);

    let body: serde_json::Value = response.json();
    assert!(body["error"]
        .as_str()
        .unwrap()
        .contains("Device ID cannot exceed 255 characters"));
}

#[tokio::test]
async fn test_exactly_255_char_device_id_succeeds() {
    let server = TestServer::new(create_test_router()).unwrap();

    let request = json!({
        "fcmToken": "a".repeat(200),
        "deviceId": "d".repeat(255), // Exactly at limit - should succeed
        "platform": "ios"
    });

    let response = server
        .post("/fcm-token")
        .add_header("tdh-sec-iam-user-identity", "{\"accountId\":777,\"accountType\":2,\"userProfileId\":100,\"userMainProfileId\":100,\"tenantId\":1}")
        .json(&request)
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);
}

// ============================================================================
// Authentication Tests
// ============================================================================

#[tokio::test]
async fn test_missing_auth_header_returns_unauthorized() {
    let server = TestServer::new(create_test_router()).unwrap();

    let response = server
        .post("/fcm-token")
        .json(&create_valid_token_request("device-001"))
        .await;

    assert_eq!(response.status_code(), StatusCode::UNAUTHORIZED);

    let body: serde_json::Value = response.json();
    assert!(body["error"].as_str().unwrap().contains("Unauthorized"));
}

#[tokio::test]
async fn test_invalid_auth_header_returns_unauthorized() {
    let server = TestServer::new(create_test_router()).unwrap();

    let response = server
        .post("/fcm-token")
        .add_header("tdh-sec-iam-user-identity", "invalid-json")
        .json(&create_valid_token_request("device-001"))
        .await;

    assert_eq!(response.status_code(), StatusCode::UNAUTHORIZED);

    let body: serde_json::Value = response.json();
    assert!(body["error"].as_str().unwrap().contains("Unauthorized"));
}

#[tokio::test]
async fn test_malformed_auth_header_returns_unauthorized() {
    let server = TestServer::new(create_test_router()).unwrap();

    let response = server
        .post("/fcm-token")
        .add_header("tdh-sec-iam-user-identity", "{missing_closing_bracket")
        .json(&create_valid_token_request("device-001"))
        .await;

    assert_eq!(response.status_code(), StatusCode::UNAUTHORIZED);

    let body: serde_json::Value = response.json();
    assert!(body["error"].as_str().unwrap().contains("Unauthorized"));
}

#[tokio::test]
async fn test_get_tokens_without_auth_returns_unauthorized() {
    let server = TestServer::new(create_test_router()).unwrap();

    let response = server.get("/fcm-token").await;

    assert_eq!(response.status_code(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_delete_token_without_auth_returns_unauthorized() {
    let server = TestServer::new(create_test_router()).unwrap();

    let response = server.delete("/fcm-token/device-001").await;

    assert_eq!(response.status_code(), StatusCode::UNAUTHORIZED);
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[tokio::test]
async fn test_get_tokens_when_none_exist_returns_empty_array() {
    let server = TestServer::new(create_test_router()).unwrap();

    let response = server
        .get("/fcm-token")
        .add_header("tdh-sec-iam-user-identity", "{\"accountId\":888,\"accountType\":2,\"userProfileId\":100,\"userMainProfileId\":100,\"tenantId\":1}")
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);

    let body: serde_json::Value = response.json();
    let tokens = body.as_array().unwrap();
    assert_eq!(tokens.len(), 0);
}

#[tokio::test]
async fn test_delete_non_existent_token_succeeds() {
    let server = TestServer::new(create_test_router()).unwrap();

    // Deleting a token that doesn't exist should still return 204 NO_CONTENT
    let response = server
        .delete("/fcm-token/non-existent-device")
        .add_header("tdh-sec-iam-user-identity", "{\"accountId\":999,\"accountType\":2,\"userProfileId\":100,\"userMainProfileId\":100,\"tenantId\":1}")
        .await;

    assert_eq!(response.status_code(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn test_case_insensitive_platform_validation_ios() {
    let server = TestServer::new(create_test_router()).unwrap();

    let request = json!({
        "fcmToken": "a".repeat(200),
        "deviceId": "device-001",
        "platform": "IOS" // Uppercase should work
    });

    let response = server
        .post("/fcm-token")
        .add_header("tdh-sec-iam-user-identity", "{\"accountId\":101,\"accountType\":2,\"userProfileId\":100,\"userMainProfileId\":100,\"tenantId\":1}")
        .json(&request)
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);
}

#[tokio::test]
async fn test_case_insensitive_platform_validation_android() {
    let server = TestServer::new(create_test_router()).unwrap();

    let request = json!({
        "fcmToken": "a".repeat(200),
        "deviceId": "device-002",
        "platform": "ANDROID" // Uppercase should work
    });

    let response = server
        .post("/fcm-token")
        .add_header("tdh-sec-iam-user-identity", "{\"accountId\":102,\"accountType\":2,\"userProfileId\":100,\"userMainProfileId\":100,\"tenantId\":1}")
        .json(&request)
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);
}

#[tokio::test]
async fn test_mixed_case_platform_validation() {
    let server = TestServer::new(create_test_router()).unwrap();

    let request = json!({
        "fcmToken": "a".repeat(200),
        "deviceId": "device-003",
        "platform": "iOs" // Mixed case should work
    });

    let response = server
        .post("/fcm-token")
        .add_header("tdh-sec-iam-user-identity", "{\"accountId\":103,\"accountType\":2,\"userProfileId\":100,\"userMainProfileId\":100,\"tenantId\":1}")
        .json(&request)
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);
}

#[tokio::test]
async fn test_optional_app_version_field() {
    let server = TestServer::new(create_test_router()).unwrap();

    // Request without appVersion (optional field)
    let request = json!({
        "fcmToken": "a".repeat(200),
        "deviceId": "device-004",
        "platform": "ios"
        // appVersion omitted - should still succeed
    });

    let response = server
        .post("/fcm-token")
        .add_header("tdh-sec-iam-user-identity", "{\"accountId\":104,\"accountType\":2,\"userProfileId\":100,\"userMainProfileId\":100,\"tenantId\":1}")
        .json(&request)
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);

    let body: serde_json::Value = response.json();
    assert_eq!(body["success"], true);
}

#[tokio::test]
async fn test_different_doctors_separate_tokens() {
    let server = TestServer::new(create_test_router()).unwrap();

    // Doctor 1 registers a token
    server
        .post("/fcm-token")
        .add_header("tdh-sec-iam-user-identity", "{\"accountId\":201,\"accountType\":2,\"userProfileId\":100,\"userMainProfileId\":100,\"tenantId\":1}")
        .json(&create_valid_token_request("doctor1-device"))
        .await;

    // Doctor 2 registers a token
    server
        .post("/fcm-token")
        .add_header("tdh-sec-iam-user-identity", "{\"accountId\":202,\"accountType\":2,\"userProfileId\":100,\"userMainProfileId\":100,\"tenantId\":1}")
        .json(&create_valid_token_request("doctor2-device"))
        .await;

    // Doctor 1 should only see their own token
    let response = server
        .get("/fcm-token")
        .add_header("tdh-sec-iam-user-identity", "{\"accountId\":201,\"accountType\":2,\"userProfileId\":100,\"userMainProfileId\":100,\"tenantId\":1}")
        .await;

    let body: serde_json::Value = response.json();
    let tokens = body.as_array().unwrap();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0]["deviceId"], "doctor1-device");

    // Doctor 2 should only see their own token
    let response = server
        .get("/fcm-token")
        .add_header("tdh-sec-iam-user-identity", "{\"accountId\":202,\"accountType\":2,\"userProfileId\":100,\"userMainProfileId\":100,\"tenantId\":1}")
        .await;

    let body: serde_json::Value = response.json();
    let tokens = body.as_array().unwrap();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0]["deviceId"], "doctor2-device");
}

#[tokio::test]
async fn test_delete_token_with_url_encoded_device_id() {
    let server = TestServer::new(create_test_router()).unwrap();

    let device_id = "device-with-special-chars-123";

    // Register token
    server
        .post("/fcm-token")
        .add_header("tdh-sec-iam-user-identity", "{\"accountId\":303,\"accountType\":2,\"userProfileId\":100,\"userMainProfileId\":100,\"tenantId\":1}")
        .json(&create_valid_token_request(device_id))
        .await;

    // Delete the token
    let response = server
        .delete(&format!("/fcm-token/{}", device_id))
        .add_header("tdh-sec-iam-user-identity", "{\"accountId\":303,\"accountType\":2,\"userProfileId\":100,\"userMainProfileId\":100,\"tenantId\":1}")
        .await;

    assert_eq!(response.status_code(), StatusCode::NO_CONTENT);
}
