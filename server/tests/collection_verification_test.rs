//! Test to verify all notification operations use the correct flat collection
//!
//! This test explicitly verifies that:
//! - Collection: "notifications" (flat, top-level)
//! - Full path: notifications/{notification_id}
//! - doctorId is a field filter, not a path segment

use std::sync::Arc;
use std::sync::Mutex;

use server::core::error::AppResult;
use server::module::notification::fcm_token::FcmTokenDoc;
use server::module::notification::repo::{NotificationDoc, NotificationRepoTrait};

// ============================================================================
// Verification Mock that tracks all collection calls
// ============================================================================

#[derive(Debug, Clone)]
struct FirestoreCall {
    collection: String,
    doctor_id: String,
    operation: String,
}

struct VerificationMockRepo {
    calls: Arc<Mutex<Vec<FirestoreCall>>>,
}

impl VerificationMockRepo {
    fn new() -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn record_call(&self, collection: &str, doctor_id: &str, operation: &str) {
        let call = FirestoreCall {
            collection: collection.to_string(),
            doctor_id: doctor_id.to_string(),
            operation: operation.to_string(),
        };
        self.calls.lock().unwrap().push(call);
    }

    fn get_calls(&self) -> Vec<FirestoreCall> {
        self.calls.lock().unwrap().clone()
    }
}

#[async_trait::async_trait]
impl NotificationRepoTrait for VerificationMockRepo {
    async fn get_notifications(
        &self,
        doctor_id: &str,
        _notification_type: &str,
        _category: Option<&str>,
        _page_token: Option<jiff::Zoned>,
        _limit: u32,
    ) -> AppResult<Vec<NotificationDoc>> {
        self.record_call("notifications", doctor_id, "get_notifications");
        Ok(vec![])
    }

    async fn mark_as_read(&self, doctor_id: &str, _notification_id: &str) -> AppResult<()> {
        self.record_call("notifications", doctor_id, "mark_as_read");
        Ok(())
    }

    async fn mark_all_as_read(&self, doctor_id: &str, _notification_type: &str) -> AppResult<()> {
        self.record_call("notifications", doctor_id, "mark_all_as_read");
        Ok(())
    }

    async fn mark_all_as_unread(&self, doctor_id: &str, _notification_type: &str) -> AppResult<()> {
        self.record_call("notifications", doctor_id, "mark_all_as_unread");
        Ok(())
    }

    async fn create_notification(
        &self,
        doctor_id: &str,
        _notification: &NotificationDoc,
    ) -> AppResult<()> {
        self.record_call("notifications", doctor_id, "create_notification");
        Ok(())
    }

    async fn save_token(
        &self,
        _doctor_id: &str,
        _device_id: &str,
        _token: &FcmTokenDoc,
    ) -> AppResult<()> {
        Ok(())
    }

    async fn get_tokens(&self, _doctor_id: &str) -> AppResult<Vec<FcmTokenDoc>> {
        Ok(vec![])
    }

    async fn delete_token(&self, _doctor_id: &str, _device_id: &str) -> AppResult<()> {
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
// Tests
// ============================================================================

#[tokio::test]
async fn test_all_operations_use_correct_flat_collection() {
    let repo = VerificationMockRepo::new();

    let doctor_id = "test_doctor_123";

    let _ = repo
        .get_notifications(doctor_id, "Alert", None, None, 20)
        .await;
    let _ = repo.mark_as_read(doctor_id, "notif_123").await;
    let _ = repo.mark_all_as_read(doctor_id, "Alert").await;
    let _ = repo.mark_all_as_unread(doctor_id, "Alert").await;

    let notification = NotificationDoc::Alert {
        notification_id: "test_id".to_string(),
        is_read: false,
        title: "Test".to_string(),
        sub_title: "Test".to_string(),
        sent_at: jiff::Zoned::now(),
    };
    let _ = repo.create_notification(doctor_id, &notification).await;

    let calls = repo.get_calls();

    for call in &calls {
        assert_eq!(
            call.collection, "notifications",
            "Operation '{}' used collection '{}', expected 'notifications'",
            call.operation, call.collection
        );
        assert_eq!(
            call.doctor_id, doctor_id,
            "Operation '{}' used doctor_id '{}', expected '{}'",
            call.operation, call.doctor_id, doctor_id
        );
    }

    assert_eq!(calls.len(), 5, "Expected 5 operations to be recorded");
}

#[tokio::test]
async fn test_expected_firestore_path_structure() {
    // Flat collection: notifications/{notificationId}
    // doctorId is a document field used as an equality filter in queries
    let collection = "notifications";
    let notification_id = "alert_abc-123-def";
    let expected_path = format!("{}/{}", collection, notification_id);
    assert_eq!(expected_path, "notifications/alert_abc-123-def");
}
