//! Integration tests for notification endpoints
//!
//! Tests the notification and announcement functionality including:
//! - Alert notification CRUD (create, get)
//! - Announcement records (create, get with category filter)
//! - Cursor-based pagination (pageToken/limit query params)
//! - Mark as read (single notification)
//! - Mark all as read (by notification type)
//! - Authentication enforcement
//! - Edge cases (empty results, defaults, boundary pagination)

use axum::http::StatusCode;
use axum::Router;
use axum_test::TestServer;
use jiff::{Span, Zoned};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;

use server::core::error::{AppError, AppResult};
use server::module::notification::fcm_token::FcmTokenDoc;
use server::module::notification::repo::{NotificationDoc, NotificationRepoTrait};

fn url_encode(s: &str) -> String {
    s.replace('+', "%2B")
        .replace(':', "%3A")
        .replace('[', "%5B")
        .replace(']', "%5D")
}

// ============================================================================
// Auth header helper
// ============================================================================

const AUTH_HEADER: &str = "tdh-sec-iam-user-identity";

fn doctor_identity(account_id: i32) -> String {
    serde_json::json!({
        "accountId": account_id,
        "accountType": 2,
        "userProfileId": 100,
        "userMainProfileId": 100,
        "tenantId": 1
    })
    .to_string()
}

// ============================================================================
// Mock NotificationRepo with in-memory storage for notifications
// ============================================================================

struct MockNotificationRepo {
    notifications: Arc<std::sync::Mutex<HashMap<String, Vec<NotificationDoc>>>>,
}

impl MockNotificationRepo {
    fn new() -> Self {
        Self {
            notifications: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }

    /// Pre-seed notifications for a doctor
    fn with_notifications(doctor_id: &str, docs: Vec<NotificationDoc>) -> Self {
        let mut map = HashMap::new();
        map.insert(doctor_id.to_string(), docs);
        Self {
            notifications: Arc::new(std::sync::Mutex::new(map)),
        }
    }
}

#[async_trait::async_trait]
impl NotificationRepoTrait for MockNotificationRepo {
    async fn get_notifications(
        &self,
        doctor_id: &str,
        notification_type: &str,
        category: Option<&str>,
        page_token: Option<Zoned>,
        limit: u32,
    ) -> AppResult<Vec<NotificationDoc>> {
        let store = self.notifications.lock().unwrap();
        let all = store.get(doctor_id).cloned().unwrap_or_default();

        // Filter by __type tag and category, then sort by sentAt descending
        let mut filtered: Vec<NotificationDoc> = all
            .into_iter()
            .filter(|doc| match doc {
                NotificationDoc::Alert { .. } => notification_type == "Alert",
                NotificationDoc::Announcement { category: cat, .. } => {
                    notification_type == "Announcement" && category.map_or(true, |c| c == cat)
                }
            })
            .collect();

        // Sort by sentAt descending (newest first)
        filtered.sort_by(|a, b| b.sent_at().cmp(a.sent_at()));

        // Apply cursor filter: sentAt < pageToken
        let filtered: Vec<NotificationDoc> = if let Some(token) = page_token {
            filtered
                .into_iter()
                .filter(|doc| doc.sent_at() < &token)
                .collect()
        } else {
            filtered
        };

        // Apply limit only (no offset for cursor-based pagination)
        let paged: Vec<NotificationDoc> = filtered.into_iter().take(limit as usize).collect();

        Ok(paged)
    }

    async fn mark_as_read(&self, doctor_id: &str, notification_id: &str) -> AppResult<()> {
        let mut store = self.notifications.lock().unwrap();
        if let Some(docs) = store.get_mut(doctor_id) {
            for doc in docs.iter_mut() {
                if doc.notification_id() == notification_id {
                    match doc {
                        NotificationDoc::Alert { is_read, .. } => *is_read = true,
                        NotificationDoc::Announcement { is_read, .. } => *is_read = true,
                    }
                    return Ok(());
                }
            }
        }
        Ok(())
    }

    async fn mark_all_as_read(&self, doctor_id: &str, notification_type: &str) -> AppResult<()> {
        let mut store = self.notifications.lock().unwrap();
        if let Some(docs) = store.get_mut(doctor_id) {
            for doc in docs.iter_mut() {
                // Match real repo: only update unread docs of the matching type
                let (matches_type, already_read) = match doc {
                    NotificationDoc::Alert { is_read, .. } => {
                        (notification_type == "Alert", *is_read)
                    }
                    NotificationDoc::Announcement { is_read, .. } => {
                        (notification_type == "Announcement", *is_read)
                    }
                };
                if matches_type && !already_read {
                    match doc {
                        NotificationDoc::Alert { is_read, .. } => *is_read = true,
                        NotificationDoc::Announcement { is_read, .. } => *is_read = true,
                    }
                }
            }
        }
        Ok(())
    }

    async fn mark_all_as_unread(&self, doctor_id: &str, notification_type: &str) -> AppResult<()> {
        let mut store = self.notifications.lock().unwrap();
        if let Some(docs) = store.get_mut(doctor_id) {
            for doc in docs.iter_mut() {
                // Mark all read docs of the matching type as unread
                let matches_type = match doc {
                    NotificationDoc::Alert { is_read, .. } => {
                        notification_type == "Alert" && *is_read
                    }
                    NotificationDoc::Announcement { is_read, .. } => {
                        notification_type == "Announcement" && *is_read
                    }
                };
                if matches_type {
                    match doc {
                        NotificationDoc::Alert { is_read, .. } => *is_read = false,
                        NotificationDoc::Announcement { is_read, .. } => *is_read = false,
                    }
                }
            }
        }
        Ok(())
    }

    async fn create_notification(
        &self,
        doctor_id: &str,
        notification: &NotificationDoc,
    ) -> AppResult<()> {
        let mut store = self.notifications.lock().unwrap();
        let docs = store.entry(doctor_id.to_string()).or_insert_with(Vec::new);
        docs.push(notification.clone());
        Ok(())
    }

    // FCM token methods — not under test here, stubs only
    async fn save_token(&self, _: &str, _: &str, _: &FcmTokenDoc) -> AppResult<()> {
        Ok(())
    }
    async fn get_tokens(&self, _: &str) -> AppResult<Vec<FcmTokenDoc>> {
        Ok(vec![])
    }
    async fn delete_token(&self, _: &str, _: &str) -> AppResult<()> {
        Ok(())
    }

    async fn save_scheduled_notification(
        &self,
        _doc: &server::module::notification::repo::ScheduledNotificationDoc,
    ) -> AppResult<()> {
        Ok(())
    }

    async fn get_scheduled_notification(
        &self,
        _notification_id: &str,
    ) -> AppResult<Option<server::module::notification::repo::ScheduledNotificationDoc>> {
        Ok(None)
    }

    async fn update_scheduled_notification_status(
        &self,
        _notification_id: &str,
        _status: server::module::notification::repo::ScheduledNotificationStatus,
    ) -> AppResult<()> {
        Ok(())
    }

    async fn get_pending_scheduled_notifications_by_booking_id(
        &self,
        _booking_id: &str,
    ) -> AppResult<Vec<server::module::notification::repo::ScheduledNotificationDoc>> {
        Ok(vec![])
    }
}

// ============================================================================
// Error-returning mock for testing repo failures
// ============================================================================

struct ErrorNotificationRepo;

#[async_trait::async_trait]
impl NotificationRepoTrait for ErrorNotificationRepo {
    async fn get_notifications(
        &self,
        _: &str,
        _: &str,
        _: Option<&str>,
        _: Option<Zoned>,
        _: u32,
    ) -> AppResult<Vec<NotificationDoc>> {
        Err(AppError::FirestoreError(
            "Firestore unavailable".to_string(),
        ))
    }

    async fn mark_as_read(&self, _: &str, _: &str) -> AppResult<()> {
        Err(AppError::FirestoreError(
            "Firestore unavailable".to_string(),
        ))
    }
    async fn mark_all_as_read(&self, _: &str, _: &str) -> AppResult<()> {
        Err(AppError::FirestoreError(
            "Firestore unavailable".to_string(),
        ))
    }
    async fn mark_all_as_unread(&self, _: &str, _: &str) -> AppResult<()> {
        Err(AppError::FirestoreError(
            "Firestore unavailable".to_string(),
        ))
    }
    async fn create_notification(&self, _: &str, _: &NotificationDoc) -> AppResult<()> {
        Err(AppError::FirestoreError(
            "Firestore unavailable".to_string(),
        ))
    }
    async fn save_token(&self, _: &str, _: &str, _: &FcmTokenDoc) -> AppResult<()> {
        Ok(())
    }
    async fn get_tokens(&self, _: &str) -> AppResult<Vec<FcmTokenDoc>> {
        Ok(vec![])
    }
    async fn delete_token(&self, _: &str, _: &str) -> AppResult<()> {
        Ok(())
    }

    async fn save_scheduled_notification(
        &self,
        _doc: &server::module::notification::repo::ScheduledNotificationDoc,
    ) -> AppResult<()> {
        Ok(())
    }

    async fn get_scheduled_notification(
        &self,
        _notification_id: &str,
    ) -> AppResult<Option<server::module::notification::repo::ScheduledNotificationDoc>> {
        Ok(None)
    }

    async fn update_scheduled_notification_status(
        &self,
        _notification_id: &str,
        _status: server::module::notification::repo::ScheduledNotificationStatus,
    ) -> AppResult<()> {
        Ok(())
    }

    async fn get_pending_scheduled_notifications_by_booking_id(
        &self,
        _booking_id: &str,
    ) -> AppResult<Vec<server::module::notification::repo::ScheduledNotificationDoc>> {
        Ok(vec![])
    }
}

// ============================================================================
// Router factory helpers
// ============================================================================

fn create_notification_router_with(repo: Arc<dyn NotificationRepoTrait>) -> Router {
    Router::new()
        .route(
            "/notification",
            axum::routing::get(server::module::notification::handlers::get_notifications)
                .post(server::module::notification::handlers::create_notification),
        )
        .route(
            "/notification/read-all",
            axum::routing::post(server::module::notification::handlers::mark_all_as_read),
        )
        .route(
            "/notification/unread-all",
            axum::routing::post(server::module::notification::handlers::mark_all_as_unread),
        )
        .route(
            "/notification/{id}/read",
            axum::routing::post(server::module::notification::handlers::mark_as_read),
        )
        .with_state(server::module::notification::NotificationState { repo })
}

fn create_notification_router() -> Router {
    create_notification_router_with(Arc::new(MockNotificationRepo::new()))
}

fn make_alert(id: &str, title: &str, is_read: bool) -> NotificationDoc {
    NotificationDoc::Alert {
        notification_id: id.to_string(),
        is_read,
        title: title.to_string(),
        sub_title: format!("Subtitle for {}", title),
        sent_at: Zoned::now(),
    }
}

fn make_alert_with_time(id: &str, title: &str, is_read: bool, sent_at: Zoned) -> NotificationDoc {
    NotificationDoc::Alert {
        notification_id: id.to_string(),
        is_read,
        title: title.to_string(),
        sub_title: format!("Subtitle for {}", title),
        sent_at,
    }
}

fn make_announcement(id: &str, title: &str, category: &str, is_read: bool) -> NotificationDoc {
    NotificationDoc::Announcement {
        notification_id: id.to_string(),
        is_read,
        title: title.to_string(),
        sub_title: format!("Subtitle for {}", title),
        sent_at: Zoned::now(),
        content_url: format!("https://example.com/content/{}", id),
        icon_url: format!("https://example.com/icon/{}", id),
        category: category.to_string(),
    }
}

fn make_announcement_with_time(
    id: &str,
    title: &str,
    category: &str,
    is_read: bool,
    sent_at: Zoned,
) -> NotificationDoc {
    NotificationDoc::Announcement {
        notification_id: id.to_string(),
        is_read,
        title: title.to_string(),
        sub_title: format!("Subtitle for {}", title),
        sent_at,
        content_url: format!("https://example.com/content/{}", id),
        icon_url: format!("https://example.com/icon/{}", id),
        category: category.to_string(),
    }
}

// ============================================================================
// GET /notification — Alert notifications
// ============================================================================

#[tokio::test]
async fn test_get_alert_notifications_returns_alerts_only() {
    let repo = MockNotificationRepo::with_notifications(
        "42",
        vec![
            make_alert("a1", "Alert One", false),
            make_announcement("n1", "Ann One", "promo", false),
            make_alert("a2", "Alert Two", true),
        ],
    );
    let server = TestServer::new(create_notification_router_with(Arc::new(repo))).unwrap();

    let res = server
        .get("/notification?type=Alert")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;

    assert_eq!(res.status_code(), StatusCode::OK);
    let body: serde_json::Value = res.json();
    let notifications = &body["notifications"];
    assert_eq!(notifications.as_array().unwrap().len(), 2);
    assert!(notifications
        .as_array()
        .unwrap()
        .iter()
        .all(|n| n["__type"] == "Alert"));
}

#[tokio::test]
async fn test_get_notifications_defaults_to_alert_type() {
    let repo = MockNotificationRepo::with_notifications(
        "42",
        vec![
            make_alert("a1", "Alert", false),
            make_announcement("n1", "Ann", "promo", false),
        ],
    );
    let server = TestServer::new(create_notification_router_with(Arc::new(repo))).unwrap();

    // No `type` param — handler defaults to "Alert"
    let res = server
        .get("/notification")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;

    assert_eq!(res.status_code(), StatusCode::OK);
    let body: serde_json::Value = res.json();
    let notifications = &body["notifications"];
    assert_eq!(notifications.as_array().unwrap().len(), 1);
    assert!(notifications
        .as_array()
        .unwrap()
        .iter()
        .all(|n| n["__type"] == "Alert"));
}

#[tokio::test]
async fn test_get_notifications_empty_result() {
    let server = TestServer::new(create_notification_router()).unwrap();

    let res = server
        .get("/notification")
        .add_header(AUTH_HEADER, &doctor_identity(99))
        .await;

    assert_eq!(res.status_code(), StatusCode::OK);
    let body: serde_json::Value = res.json();
    let notifications = &body["notifications"];
    assert!(notifications.as_array().unwrap().is_empty());
    assert!(body["nextPageToken"].is_null());
}

// ============================================================================
// GET /notification — Announcement records with category
// ============================================================================

#[tokio::test]
async fn test_get_announcements_returns_announcements_only() {
    let repo = MockNotificationRepo::with_notifications(
        "42",
        vec![
            make_alert("a1", "Alert", false),
            make_announcement("n1", "Promo 1", "promo", false),
            make_announcement("n2", "News 1", "news", false),
        ],
    );
    let server = TestServer::new(create_notification_router_with(Arc::new(repo))).unwrap();

    let res = server
        .get("/notification?type=Announcement")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;

    assert_eq!(res.status_code(), StatusCode::OK);
    let body: serde_json::Value = res.json();
    let notifications = &body["notifications"];
    assert_eq!(notifications.as_array().unwrap().len(), 2);
    assert!(notifications
        .as_array()
        .unwrap()
        .iter()
        .all(|n| n["__type"] == "Announcement"));
}

#[tokio::test]
async fn test_get_announcements_filtered_by_category() {
    let repo = MockNotificationRepo::with_notifications(
        "42",
        vec![
            make_announcement("n1", "Promo 1", "promo", false),
            make_announcement("n2", "News 1", "news", false),
            make_announcement("n3", "Promo 2", "promo", true),
        ],
    );
    let server = TestServer::new(create_notification_router_with(Arc::new(repo))).unwrap();

    let res = server
        .get("/notification?type=Announcement&category=promo")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;

    assert_eq!(res.status_code(), StatusCode::OK);
    let body: serde_json::Value = res.json();
    let notifications = &body["notifications"];
    assert_eq!(notifications.as_array().unwrap().len(), 2);
    assert!(notifications
        .as_array()
        .unwrap()
        .iter()
        .all(|n| n["category"] == "promo"));
}

#[tokio::test]
async fn test_get_announcements_category_with_no_matches() {
    let repo = MockNotificationRepo::with_notifications(
        "42",
        vec![make_announcement("n1", "Promo", "promo", false)],
    );
    let server = TestServer::new(create_notification_router_with(Arc::new(repo))).unwrap();

    let res = server
        .get("/notification?type=Announcement&category=nonexistent")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;

    assert_eq!(res.status_code(), StatusCode::OK);
    let body: serde_json::Value = res.json();
    let notifications = &body["notifications"];
    assert!(notifications.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_announcement_response_contains_content_and_icon_urls() {
    let repo = MockNotificationRepo::with_notifications(
        "42",
        vec![make_announcement("n1", "Title", "promo", false)],
    );
    let server = TestServer::new(create_notification_router_with(Arc::new(repo))).unwrap();

    let res = server
        .get("/notification?type=Announcement")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;

    assert_eq!(res.status_code(), StatusCode::OK);
    let body: serde_json::Value = res.json();
    let notifications = &body["notifications"];
    assert_eq!(notifications.as_array().unwrap().len(), 1);
    assert!(notifications[0]["contentUrl"]
        .as_str()
        .unwrap()
        .starts_with("https://"));
    assert!(notifications[0]["iconUrl"]
        .as_str()
        .unwrap()
        .starts_with("https://"));
    assert_eq!(notifications[0]["category"], "promo");
}

// ============================================================================
// GET /notification — Cursor-based Paging
// ============================================================================

#[tokio::test]
async fn test_cursor_paging_defaults_limit20() {
    // Seed 25 alerts — default limit=20 should return first 20 with nextPageToken
    let alerts: Vec<NotificationDoc> = (1..=25)
        .map(|i| make_alert(&format!("a{}", i), &format!("Alert {}", i), false))
        .collect();
    let repo = MockNotificationRepo::with_notifications("42", alerts);
    let server = TestServer::new(create_notification_router_with(Arc::new(repo))).unwrap();

    let res = server
        .get("/notification?type=Alert")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;

    assert_eq!(res.status_code(), StatusCode::OK);
    let body: serde_json::Value = res.json();
    let notifications = &body["notifications"];
    assert_eq!(notifications.as_array().unwrap().len(), 20);
    // Should have nextPageToken since we returned exactly limit=20
    assert!(!body["nextPageToken"].is_null());
}

#[tokio::test]
async fn test_cursor_paging_second_page() {
    // Create 25 alerts with distinct timestamps (newest first)
    let base_time = Zoned::now();
    let alerts: Vec<NotificationDoc> = (0..25)
        .map(|i| {
            let time = base_time
                .checked_sub(Span::new().seconds(i as i64))
                .unwrap_or_else(|_| Zoned::now());
            make_alert_with_time(&format!("a{}", i), &format!("Alert {}", i), false, time)
        })
        .collect();
    let repo = MockNotificationRepo::with_notifications("42", alerts);
    let server = TestServer::new(create_notification_router_with(Arc::new(repo))).unwrap();

    // First page - get first 3 and the token
    let res = server
        .get("/notification?type=Alert&limit=3")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;

    assert_eq!(res.status_code(), StatusCode::OK);
    let body1: serde_json::Value = res.json();
    let notifications1 = &body1["notifications"];
    assert_eq!(notifications1.as_array().unwrap().len(), 3);
    let page_token = body1["nextPageToken"]
        .as_str()
        .expect("Should have nextPageToken");

    // Second page - use the token (URL-encoded)
    let res = server
        .get(&format!(
            "/notification?type=Alert&limit=3&pageToken={}",
            url_encode(page_token)
        ))
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;

    assert_eq!(res.status_code(), StatusCode::OK);
    let body2: serde_json::Value = res.json();
    let notifications2 = &body2["notifications"];
    assert_eq!(notifications2.as_array().unwrap().len(), 3);
    assert!(!body2["nextPageToken"].is_null());
}

#[tokio::test]
async fn test_cursor_paging_last_page_no_next_token() {
    // Create exactly 3 alerts
    let alerts: Vec<NotificationDoc> = (0..3)
        .map(|i| {
            let time = Zoned::now()
                .checked_sub(Span::new().seconds(i as i64))
                .unwrap_or_else(|_| Zoned::now());
            make_alert_with_time(&format!("a{}", i), &format!("Alert {}", i), false, time)
        })
        .collect();
    let repo = MockNotificationRepo::with_notifications("42", alerts);
    let server = TestServer::new(create_notification_router_with(Arc::new(repo))).unwrap();

    let res = server
        .get("/notification?type=Alert&limit=5")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;

    assert_eq!(res.status_code(), StatusCode::OK);
    let body: serde_json::Value = res.json();
    let notifications = &body["notifications"];
    // Only 3 records exist, less than limit=5
    assert_eq!(notifications.as_array().unwrap().len(), 3);
    // No nextPageToken on last page
    assert!(body["nextPageToken"].is_null());
}

#[tokio::test]
async fn test_cursor_paging_custom_limit() {
    let alerts: Vec<NotificationDoc> = (1..=10)
        .map(|i| make_alert(&format!("a{}", i), &format!("Alert {}", i), false))
        .collect();
    let repo = MockNotificationRepo::with_notifications("42", alerts);
    let server = TestServer::new(create_notification_router_with(Arc::new(repo))).unwrap();

    let res = server
        .get("/notification?type=Alert&limit=3")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;

    assert_eq!(res.status_code(), StatusCode::OK);
    let body: serde_json::Value = res.json();
    let notifications = &body["notifications"];
    assert_eq!(notifications.as_array().unwrap().len(), 3);
    assert!(!body["nextPageToken"].is_null());
}

#[tokio::test]
async fn test_cursor_paging_beyond_total_returns_empty() {
    let alerts: Vec<NotificationDoc> = (0..5)
        .map(|i| {
            let time = Zoned::now()
                .checked_sub(Span::new().seconds(i as i64))
                .unwrap_or_else(|_| Zoned::now());
            make_alert_with_time(&format!("a{}", i), &format!("Alert {}", i), false, time)
        })
        .collect();
    let repo = MockNotificationRepo::with_notifications("42", alerts);
    let server = TestServer::new(create_notification_router_with(Arc::new(repo))).unwrap();

    // Get first page to get a token
    let res = server
        .get("/notification?type=Alert&limit=2")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;
    let body1: serde_json::Value = res.json();
    let _page_token = body1["nextPageToken"].as_str().unwrap();

    // Use a very old timestamp to simulate paging beyond available data
    let old_token = Zoned::now()
        .checked_sub(Span::new().days(365))
        .unwrap_or_else(|_| Zoned::now())
        .to_string();
    let res = server
        .get(&format!(
            "/notification?type=Alert&limit=10&pageToken={}",
            url_encode(&old_token)
        ))
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;

    assert_eq!(res.status_code(), StatusCode::OK);
    let body: serde_json::Value = res.json();
    let notifications = &body["notifications"];
    assert!(notifications.as_array().unwrap().is_empty());
    assert!(body["nextPageToken"].is_null());
}

#[tokio::test]
async fn test_cursor_paging_limit_1() {
    let alerts: Vec<NotificationDoc> = (0..5)
        .map(|i| {
            let time = Zoned::now()
                .checked_sub(Span::new().seconds(i as i64))
                .unwrap_or_else(|_| Zoned::now());
            make_alert_with_time(&format!("a{}", i), &format!("Alert {}", i), false, time)
        })
        .collect();
    let repo = MockNotificationRepo::with_notifications("42", alerts);
    let server = TestServer::new(create_notification_router_with(Arc::new(repo))).unwrap();

    let res = server
        .get("/notification?type=Alert&limit=1")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;

    assert_eq!(res.status_code(), StatusCode::OK);
    let body: serde_json::Value = res.json();
    let notifications = &body["notifications"];
    assert_eq!(notifications.as_array().unwrap().len(), 1);
    assert!(!body["nextPageToken"].is_null());
}

#[tokio::test]
async fn test_cursor_paging_invalid_token_format_returns_400() {
    let repo = MockNotificationRepo::with_notifications("42", vec![]);
    let server = TestServer::new(create_notification_router_with(Arc::new(repo))).unwrap();

    let res = server
        .get("/notification?type=Alert&limit=10&pageToken=invalid-token")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;

    assert_eq!(res.status_code(), StatusCode::BAD_REQUEST);
}

// ============================================================================
// POST /notification — Create notification
// ============================================================================

#[tokio::test]
async fn test_create_alert_notification() {
    let server = TestServer::new(create_notification_router()).unwrap();

    let body = json!({
        "__type": "Alert",
        "notificationId": "",
        "isRead": false,
        "title": "New Appointment",
        "subTitle": "Patient John requests appointment",
        "sentAt": "2025-01-15T10:30:00+00:00[UTC]"
    });

    let res = server
        .post("/notification")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .json(&body)
        .await;

    assert_eq!(res.status_code(), StatusCode::CREATED);
    let resp: serde_json::Value = res.json();
    // Handler generates a new UUID for notificationId
    assert!(!resp["notificationId"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn test_create_announcement_notification() {
    let server = TestServer::new(create_notification_router()).unwrap();

    let body = json!({
        "__type": "Announcement",
        "notificationId": "",
        "isRead": false,
        "title": "System Maintenance",
        "subTitle": "Scheduled maintenance on Saturday",
        "sentAt": "2025-01-20T08:00:00+00:00[UTC]",
        "contentUrl": "https://example.com/maintenance",
        "iconUrl": "https://example.com/icon.png",
        "category": "system"
    });

    let res = server
        .post("/notification")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .json(&body)
        .await;

    assert_eq!(res.status_code(), StatusCode::CREATED);
    let resp: serde_json::Value = res.json();
    assert!(!resp["notificationId"].as_str().unwrap().is_empty());
}

// ============================================================================
// POST /notification/{id}/read — Mark single as read
// ============================================================================

#[tokio::test]
async fn test_mark_single_notification_as_read() {
    let repo = Arc::new(MockNotificationRepo::with_notifications(
        "42",
        vec![make_alert("notif-1", "Alert 1", false)],
    ));
    let server = TestServer::new(create_notification_router_with(
        Arc::clone(&repo) as Arc<dyn NotificationRepoTrait>
    ))
    .unwrap();

    let res = server
        .post("/notification/notif-1/read")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;

    assert_eq!(res.status_code(), StatusCode::OK);

    // Verify via GET that the notification is now marked as read
    let res = server
        .get("/notification?type=Alert")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;
    let body: serde_json::Value = res.json();
    let notifications = &body["notifications"];
    assert_eq!(notifications.as_array().unwrap().len(), 1);
    assert_eq!(notifications[0]["isRead"], true);
}

#[tokio::test]
async fn test_mark_nonexistent_notification_as_read_still_succeeds() {
    let repo = MockNotificationRepo::with_notifications(
        "42",
        vec![make_alert("notif-1", "Alert 1", false)],
    );
    let server = TestServer::new(create_notification_router_with(Arc::new(repo))).unwrap();

    // Non-existent ID — repo returns Ok(()) anyway
    let res = server
        .post("/notification/does-not-exist/read")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;

    assert_eq!(res.status_code(), StatusCode::OK);
}

#[tokio::test]
async fn test_mark_already_read_notification_is_idempotent() {
    let repo = MockNotificationRepo::with_notifications(
        "42",
        vec![make_alert("notif-1", "Alert 1", true)], // already read
    );
    let server = TestServer::new(create_notification_router_with(Arc::new(repo))).unwrap();

    let res = server
        .post("/notification/notif-1/read")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;

    assert_eq!(res.status_code(), StatusCode::OK);
}

// ============================================================================
// POST /notification/read-all — Mark all as read
// ============================================================================

#[tokio::test]
async fn test_mark_all_alerts_as_read() {
    let repo = Arc::new(MockNotificationRepo::with_notifications(
        "42",
        vec![
            make_alert("a1", "Alert 1", false),
            make_alert("a2", "Alert 2", false),
            make_announcement("n1", "Ann 1", "promo", false),
        ],
    ));
    let server = TestServer::new(create_notification_router_with(
        Arc::clone(&repo) as Arc<dyn NotificationRepoTrait>
    ))
    .unwrap();

    // Default type=Alert
    let res = server
        .post("/notification/read-all")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;

    assert_eq!(res.status_code(), StatusCode::OK);

    // Verify alerts are read via GET
    let res = server
        .get("/notification?type=Alert")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;
    let body: serde_json::Value = res.json();
    let notifications = &body["notifications"];
    assert_eq!(notifications.as_array().unwrap().len(), 2);
    assert!(notifications
        .as_array()
        .unwrap()
        .iter()
        .all(|n| n["isRead"] == true));

    // Verify announcement is NOT read via GET
    let res = server
        .get("/notification?type=Announcement")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;
    let body: serde_json::Value = res.json();
    let notifications = &body["notifications"];
    assert_eq!(notifications.as_array().unwrap().len(), 1);
    assert_eq!(notifications[0]["isRead"], false);
}

#[tokio::test]
async fn test_mark_all_announcements_as_read() {
    let repo = Arc::new(MockNotificationRepo::with_notifications(
        "42",
        vec![
            make_alert("a1", "Alert 1", false),
            make_announcement("n1", "Ann 1", "promo", false),
            make_announcement("n2", "Ann 2", "news", false),
        ],
    ));
    let server = TestServer::new(create_notification_router_with(
        Arc::clone(&repo) as Arc<dyn NotificationRepoTrait>
    ))
    .unwrap();

    let res = server
        .post("/notification/read-all?type=Announcement")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;

    assert_eq!(res.status_code(), StatusCode::OK);

    // Verify alert is NOT read via GET
    let res = server
        .get("/notification?type=Alert")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;
    let body: serde_json::Value = res.json();
    let notifications = &body["notifications"];
    assert_eq!(notifications.as_array().unwrap().len(), 1);
    assert_eq!(notifications[0]["isRead"], false);

    // Verify announcements are read via GET
    let res = server
        .get("/notification?type=Announcement")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;
    let body: serde_json::Value = res.json();
    let notifications = &body["notifications"];
    assert_eq!(notifications.as_array().unwrap().len(), 2);
    assert!(notifications
        .as_array()
        .unwrap()
        .iter()
        .all(|n| n["isRead"] == true));
}

#[tokio::test]
async fn test_mark_all_as_read_when_already_all_read() {
    let repo = MockNotificationRepo::with_notifications(
        "42",
        vec![
            make_alert("a1", "Alert 1", true),
            make_alert("a2", "Alert 2", true),
        ],
    );
    let server = TestServer::new(create_notification_router_with(Arc::new(repo))).unwrap();

    let res = server
        .post("/notification/read-all")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;

    assert_eq!(res.status_code(), StatusCode::OK);
}

#[tokio::test]
async fn test_mark_all_as_read_when_no_notifications() {
    let server = TestServer::new(create_notification_router()).unwrap();

    let res = server
        .post("/notification/read-all")
        .add_header(AUTH_HEADER, &doctor_identity(99))
        .await;

    assert_eq!(res.status_code(), StatusCode::OK);
}

// ============================================================================
// Mark all as unread tests
// ============================================================================

#[tokio::test]
async fn test_mark_all_alerts_as_unread() {
    let repo = Arc::new(MockNotificationRepo::with_notifications(
        "42",
        vec![
            make_alert("a1", "Alert 1", true),
            make_alert("a2", "Alert 2", true),
            make_announcement("n1", "Ann 1", "Pharmacy", true),
        ],
    ));
    let server = TestServer::new(create_notification_router_with(
        Arc::clone(&repo) as Arc<dyn NotificationRepoTrait>
    ))
    .unwrap();

    // Default type=Alert
    let res = server
        .post("/notification/unread-all")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;

    assert_eq!(res.status_code(), StatusCode::OK);

    // Verify alerts are unread via GET
    let res = server
        .get("/notification?type=Alert")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;

    assert_eq!(res.status_code(), StatusCode::OK);
    let body: serde_json::Value = res.json();
    let notifications = body["notifications"].as_array().unwrap();

    // Both alerts should be unread
    assert_eq!(notifications.len(), 2);
    for notif in notifications {
        assert_eq!(notif["isRead"], false);
    }
}

#[tokio::test]
async fn test_mark_all_announcements_as_unread() {
    let repo = Arc::new(MockNotificationRepo::with_notifications(
        "42",
        vec![
            make_alert("a1", "Alert 1", true),
            make_announcement("n1", "Ann 1", "Pharmacy", true),
            make_announcement("n2", "Ann 2", "Regulation", true),
        ],
    ));
    let server = TestServer::new(create_notification_router_with(
        Arc::clone(&repo) as Arc<dyn NotificationRepoTrait>
    ))
    .unwrap();

    let res = server
        .post("/notification/unread-all?type=Announcement")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;

    assert_eq!(res.status_code(), StatusCode::OK);

    // Verify alert is NOT unread via GET (it stays read)
    let res = server
        .get("/notification?type=Alert")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;

    assert_eq!(res.status_code(), StatusCode::OK);
    let body: serde_json::Value = res.json();
    let notifications = body["notifications"].as_array().unwrap();

    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0]["notificationId"], "a1");
    assert_eq!(notifications[0]["isRead"], true);

    // Verify announcements are unread via GET
    let res = server
        .get("/notification?type=Announcement")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;

    assert_eq!(res.status_code(), StatusCode::OK);
    let body: serde_json::Value = res.json();
    let notifications = body["notifications"].as_array().unwrap();

    assert_eq!(notifications.len(), 2);
    for notif in notifications {
        assert_eq!(notif["isRead"], false);
    }
}

#[tokio::test]
async fn test_mark_all_as_unread_when_already_all_unread() {
    let repo = MockNotificationRepo::with_notifications(
        "42",
        vec![
            make_alert("a1", "Alert 1", false),
            make_alert("a2", "Alert 2", false),
        ],
    );
    let server = TestServer::new(create_notification_router_with(Arc::new(repo))).unwrap();

    let res = server
        .post("/notification/unread-all")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;

    assert_eq!(res.status_code(), StatusCode::OK);
}

#[tokio::test]
async fn test_mark_all_as_unread_when_no_notifications() {
    let server = TestServer::new(create_notification_router()).unwrap();

    let res = server
        .post("/notification/unread-all")
        .add_header(AUTH_HEADER, &doctor_identity(99))
        .await;

    assert_eq!(res.status_code(), StatusCode::OK);
}

#[tokio::test]
async fn test_mark_all_as_unread_without_auth_returns_401() {
    let server = TestServer::new(create_notification_router()).unwrap();

    let res = server.post("/notification/unread-all").await;
    assert_eq!(res.status_code(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_mark_all_as_unread_repo_error_returns_500() {
    let repo: Arc<dyn NotificationRepoTrait> = Arc::new(ErrorNotificationRepo);
    let server = TestServer::new(create_notification_router_with(repo)).unwrap();

    let res = server
        .post("/notification/unread-all")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;

    assert_eq!(res.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
}

// ============================================================================
// Authentication tests
// ============================================================================

#[tokio::test]
async fn test_get_notifications_without_auth_returns_401() {
    let server = TestServer::new(create_notification_router()).unwrap();

    let res = server.get("/notification").await;
    assert_eq!(res.status_code(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_create_notification_without_auth_returns_401() {
    let server = TestServer::new(create_notification_router()).unwrap();

    let body = json!({
        "__type": "Alert",
        "notificationId": "",
        "isRead": false,
        "title": "Test",
        "subTitle": "Test",
        "sentAt": "2025-01-01T00:00:00+00:00[UTC]"
    });

    let res = server.post("/notification").json(&body).await;
    assert_eq!(res.status_code(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_mark_as_read_without_auth_returns_401() {
    let server = TestServer::new(create_notification_router()).unwrap();

    let res = server.post("/notification/some-id/read").await;
    assert_eq!(res.status_code(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_mark_all_as_read_without_auth_returns_401() {
    let server = TestServer::new(create_notification_router()).unwrap();

    let res = server.post("/notification/read-all").await;
    assert_eq!(res.status_code(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_invalid_auth_header_returns_401() {
    let server = TestServer::new(create_notification_router()).unwrap();

    let res = server
        .get("/notification")
        .add_header(AUTH_HEADER, "not-json")
        .await;

    assert_eq!(res.status_code(), StatusCode::UNAUTHORIZED);
}

// ============================================================================
// Error handling — repo failures
// ============================================================================

#[tokio::test]
async fn test_get_notifications_repo_error_returns_500() {
    let repo: Arc<dyn NotificationRepoTrait> = Arc::new(ErrorNotificationRepo);
    let server = TestServer::new(create_notification_router_with(repo)).unwrap();

    let res = server
        .get("/notification")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;

    assert_eq!(res.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_mark_as_read_repo_error_returns_500() {
    let repo: Arc<dyn NotificationRepoTrait> = Arc::new(ErrorNotificationRepo);
    let server = TestServer::new(create_notification_router_with(repo)).unwrap();

    let res = server
        .post("/notification/id/read")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;

    assert_eq!(res.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_mark_all_as_read_repo_error_returns_500() {
    let repo: Arc<dyn NotificationRepoTrait> = Arc::new(ErrorNotificationRepo);
    let server = TestServer::new(create_notification_router_with(repo)).unwrap();

    let res = server
        .post("/notification/read-all")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;

    assert_eq!(res.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_create_notification_repo_error_returns_500() {
    let repo: Arc<dyn NotificationRepoTrait> = Arc::new(ErrorNotificationRepo);
    let server = TestServer::new(create_notification_router_with(repo)).unwrap();

    let body = json!({
        "__type": "Alert",
        "notificationId": "",
        "isRead": false,
        "title": "Test",
        "subTitle": "Test",
        "sentAt": "2025-01-01T00:00:00+00:00[UTC]"
    });

    let res = server
        .post("/notification")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .json(&body)
        .await;

    assert_eq!(res.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
}

// ============================================================================
// Doctor isolation
// ============================================================================

#[tokio::test]
async fn test_doctors_see_only_their_own_notifications() {
    let mut map = HashMap::new();
    map.insert(
        "10".to_string(),
        vec![make_alert("a1", "Doctor 10 alert", false)],
    );
    map.insert(
        "20".to_string(),
        vec![make_alert("a2", "Doctor 20 alert", false)],
    );
    let repo = MockNotificationRepo {
        notifications: Arc::new(std::sync::Mutex::new(map)),
    };
    let server = TestServer::new(create_notification_router_with(Arc::new(repo))).unwrap();

    // Doctor 10 sees only their alert
    let res = server
        .get("/notification?type=Alert")
        .add_header(AUTH_HEADER, &doctor_identity(10))
        .await;
    let body: serde_json::Value = res.json();
    let notifications = &body["notifications"];
    assert_eq!(notifications.as_array().unwrap().len(), 1);
    assert_eq!(notifications[0]["title"], "Doctor 10 alert");

    // Doctor 20 sees only their alert
    let res = server
        .get("/notification?type=Alert")
        .add_header(AUTH_HEADER, &doctor_identity(20))
        .await;
    let body: serde_json::Value = res.json();
    let notifications = &body["notifications"];
    assert_eq!(notifications.as_array().unwrap().len(), 1);
    assert_eq!(notifications[0]["title"], "Doctor 20 alert");
}

// ============================================================================
// Announcement cursor paging with category filter
// ============================================================================

#[tokio::test]
async fn test_announcement_cursor_paging_with_category_filter() {
    let announcements: Vec<NotificationDoc> = (1..=8)
        .map(|i| {
            let time = Zoned::now()
                .checked_sub(Span::new().seconds(i as i64))
                .unwrap_or_else(|_| Zoned::now());
            make_announcement_with_time(
                &format!("n{}", i),
                &format!("Promo {}", i),
                "promo",
                false,
                time,
            )
        })
        .collect();
    let repo = MockNotificationRepo::with_notifications("42", announcements);
    let server = TestServer::new(create_notification_router_with(Arc::new(repo))).unwrap();

    // First page, limit 3
    let res = server
        .get("/notification?type=Announcement&category=promo&limit=3")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;
    assert_eq!(res.status_code(), StatusCode::OK);
    let body1: serde_json::Value = res.json();
    let notifications1 = &body1["notifications"];
    assert_eq!(notifications1.as_array().unwrap().len(), 3);
    let page_token = body1["nextPageToken"]
        .as_str()
        .expect("Should have nextPageToken");

    // Second page using the token
    let res = server
        .get(&format!(
            "/notification?type=Announcement&category=promo&limit=3&pageToken={}",
            url_encode(page_token)
        ))
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;
    assert_eq!(res.status_code(), StatusCode::OK);
    let body2: serde_json::Value = res.json();
    let notifications2 = &body2["notifications"];
    assert_eq!(notifications2.as_array().unwrap().len(), 3);

    // Third page — should get remaining 2
    let page_token = body2["nextPageToken"]
        .as_str()
        .expect("Should have nextPageToken");
    let res = server
        .get(&format!(
            "/notification?type=Announcement&category=promo&limit=3&pageToken={}",
            url_encode(page_token)
        ))
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;
    assert_eq!(res.status_code(), StatusCode::OK);
    let body3: serde_json::Value = res.json();
    let notifications3 = &body3["notifications"];
    assert_eq!(notifications3.as_array().unwrap().len(), 2);
    // Last page has no nextPageToken
    assert!(body3["nextPageToken"].is_null());
}

// ============================================================================
// Field preservation — mark_as_read / mark_all_as_read must not lose data
// ============================================================================

#[tokio::test]
async fn test_mark_as_read_preserves_all_fields() {
    let repo = Arc::new(MockNotificationRepo::with_notifications(
        "42",
        vec![
            make_alert("a1", "Important Alert", false),
            make_announcement("n1", "Big News", "promo", false),
        ],
    ));
    let server = TestServer::new(create_notification_router_with(
        Arc::clone(&repo) as Arc<dyn NotificationRepoTrait>
    ))
    .unwrap();

    // Mark the alert as read
    let res = server
        .post("/notification/a1/read")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;
    assert_eq!(res.status_code(), StatusCode::OK);

    // Verify the alert still has all its fields (not just isRead)
    let res = server
        .get("/notification?type=Alert")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;
    let body: serde_json::Value = res.json();
    let notifications = body["notifications"].as_array().unwrap();
    assert_eq!(notifications.len(), 1);
    let notif = &notifications[0];
    assert_eq!(notif["__type"], "Alert");
    assert_eq!(notif["notificationId"], "a1");
    assert_eq!(notif["title"], "Important Alert");
    assert!(notif["subTitle"].as_str().is_some());
    assert!(notif["sentAt"].as_str().is_some());
    assert_eq!(notif["isRead"], true);

    // Mark the announcement as read
    let res = server
        .post("/notification/n1/read")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;
    assert_eq!(res.status_code(), StatusCode::OK);

    // Verify announcement still has all its fields including contentUrl, iconUrl, category
    let res = server
        .get("/notification?type=Announcement")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;
    let body: serde_json::Value = res.json();
    let notifications = body["notifications"].as_array().unwrap();
    assert_eq!(notifications.len(), 1);
    let notif = &notifications[0];
    assert_eq!(notif["__type"], "Announcement");
    assert_eq!(notif["notificationId"], "n1");
    assert_eq!(notif["title"], "Big News");
    assert!(notif["subTitle"].as_str().is_some());
    assert!(notif["sentAt"].as_str().is_some());
    assert_eq!(notif["contentUrl"], "https://example.com/content/n1");
    assert_eq!(notif["iconUrl"], "https://example.com/icon/n1");
    assert_eq!(notif["category"], "promo");
    assert_eq!(notif["isRead"], true);
}

#[tokio::test]
async fn test_mark_all_as_read_preserves_all_fields() {
    let repo = Arc::new(MockNotificationRepo::with_notifications(
        "42",
        vec![
            make_alert("a1", "Alert One", false),
            make_alert("a2", "Alert Two", false),
            make_announcement("n1", "Ann One", "news", false),
        ],
    ));
    let server = TestServer::new(create_notification_router_with(
        Arc::clone(&repo) as Arc<dyn NotificationRepoTrait>
    ))
    .unwrap();

    // Mark all alerts as read
    let res = server
        .post("/notification/read-all?type=Alert")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;
    assert_eq!(res.status_code(), StatusCode::OK);

    // Verify all alert fields are preserved
    let res = server
        .get("/notification?type=Alert")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;
    let body: serde_json::Value = res.json();
    let notifications = body["notifications"].as_array().unwrap();
    assert_eq!(notifications.len(), 2);
    for notif in notifications {
        assert_eq!(notif["__type"], "Alert");
        assert!(notif["notificationId"].as_str().is_some());
        assert!(notif["title"].as_str().is_some());
        assert!(notif["subTitle"].as_str().is_some());
        assert!(notif["sentAt"].as_str().is_some());
        assert_eq!(notif["isRead"], true);
    }

    // Verify announcement is untouched
    let res = server
        .get("/notification?type=Announcement")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;
    let body: serde_json::Value = res.json();
    let notifications = body["notifications"].as_array().unwrap();
    assert_eq!(notifications.len(), 1);
    let notif = &notifications[0];
    assert_eq!(notif["__type"], "Announcement");
    assert_eq!(notif["title"], "Ann One");
    assert_eq!(notif["category"], "news");
    assert_eq!(notif["isRead"], false);
}

// ============================================================================
// Mixed scenarios
// ============================================================================

#[tokio::test]
async fn test_create_then_get_alert_roundtrip() {
    let repo = Arc::new(MockNotificationRepo::new());
    let server = TestServer::new(create_notification_router_with(
        Arc::clone(&repo) as Arc<dyn NotificationRepoTrait>
    ))
    .unwrap();

    // Create
    let body = json!({
        "__type": "Alert",
        "notificationId": "",
        "isRead": false,
        "title": "Roundtrip Alert",
        "subTitle": "Testing roundtrip",
        "sentAt": "2025-06-01T12:00:00+00:00[UTC]"
    });
    let res = server
        .post("/notification")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .json(&body)
        .await;
    assert_eq!(res.status_code(), StatusCode::CREATED);

    // Get
    let res = server
        .get("/notification?type=Alert")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;
    assert_eq!(res.status_code(), StatusCode::OK);
    let body: serde_json::Value = res.json();
    let notifications = &body["notifications"];
    assert_eq!(notifications.as_array().unwrap().len(), 1);
    assert_eq!(notifications[0]["title"], "Roundtrip Alert");
    assert_eq!(notifications[0]["isRead"], false);
}

#[tokio::test]
async fn test_create_then_mark_read_then_verify() {
    let repo = Arc::new(MockNotificationRepo::new());
    let server = TestServer::new(create_notification_router_with(
        Arc::clone(&repo) as Arc<dyn NotificationRepoTrait>
    ))
    .unwrap();

    // Create
    let body = json!({
        "__type": "Alert",
        "notificationId": "",
        "isRead": false,
        "title": "To Be Read",
        "subTitle": "Will be marked read",
        "sentAt": "2025-06-01T12:00:00+00:00[UTC]"
    });
    let res = server
        .post("/notification")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .json(&body)
        .await;
    assert_eq!(res.status_code(), StatusCode::CREATED);
    let created: serde_json::Value = res.json();
    let notif_id = created["notificationId"].as_str().unwrap();

    // Mark as read
    let res = server
        .post(&format!("/notification/{}/read", notif_id))
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;
    assert_eq!(res.status_code(), StatusCode::OK);

    // Verify via GET that it's now read
    let res = server
        .get("/notification?type=Alert")
        .add_header(AUTH_HEADER, &doctor_identity(42))
        .await;
    assert_eq!(res.status_code(), StatusCode::OK);
    let body: serde_json::Value = res.json();
    let notifications = &body["notifications"];
    assert_eq!(notifications.as_array().unwrap().len(), 1);
    assert_eq!(notifications[0]["isRead"], true);
    assert_eq!(notifications[0]["title"], "To Be Read");
}
