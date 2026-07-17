use async_trait::async_trait;
use jiff::Zoned;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::Instant;
use tracing::{debug, instrument, warn};
use utoipa::ToSchema;

use crate::core::error::{AppError, AppResult};
use crate::notification::fcm_token::FcmTokenDoc;
use crate::repo::firestore_repo::{FirestoreRepo, FirestoreRepoTrait, QueryFilter, QueryOp};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "__type")]
pub enum NotificationDoc {
    #[serde(rename = "Alert")]
    Alert {
        #[serde(rename = "notificationId")]
        notification_id: String,
        #[serde(rename = "isRead")]
        is_read: bool,
        title: String,
        #[serde(rename = "subTitle")]
        sub_title: String,
        /// Stored as Firestore native Timestamp by the Scala server; use Zoned to
        /// round-trip correctly with both the Scala-written documents and new Rust writes.
        #[serde(rename = "sentAt")]
        sent_at: Zoned,
    },
    #[serde(rename = "Announcement")]
    Announcement {
        #[serde(rename = "notificationId")]
        notification_id: String,
        #[serde(rename = "isRead")]
        is_read: bool,
        title: String,
        #[serde(rename = "subTitle")]
        sub_title: String,
        #[serde(rename = "sentAt")]
        sent_at: Zoned,
        #[serde(rename = "contentUrl")]
        content_url: String,
        #[serde(rename = "iconUrl")]
        icon_url: String,
        category: String,
    },
}

impl NotificationDoc {
    pub fn notification_id(&self) -> &str {
        match self {
            NotificationDoc::Alert {
                notification_id, ..
            } => notification_id,
            NotificationDoc::Announcement {
                notification_id, ..
            } => notification_id,
        }
    }

    pub fn set_notification_id(&mut self, id: String) {
        match self {
            NotificationDoc::Alert {
                notification_id, ..
            } => *notification_id = id,
            NotificationDoc::Announcement {
                notification_id, ..
            } => *notification_id = id,
        }
    }

    pub fn set_is_read(&mut self, value: bool) {
        match self {
            NotificationDoc::Alert { is_read, .. } => *is_read = value,
            NotificationDoc::Announcement { is_read, .. } => *is_read = value,
        }
    }

    pub fn sent_at(&self) -> &Zoned {
        match self {
            NotificationDoc::Alert { sent_at, .. } => sent_at,
            NotificationDoc::Announcement { sent_at, .. } => sent_at,
        }
    }
}

/// Status of a scheduled notification
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ScheduledNotificationStatus {
    Pending,
    Processing,
    Sent,
    Failed,
    Cancelled,
}

/// Scheduled notification stored in Firestore for tracking and cancellation
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledNotificationDoc {
    pub notification_id: String,
    pub doctor_account_ids: Vec<i32>,
    pub notification_type: String,
    pub title: String,
    pub sub_title: String,
    pub scheduled_at: Zoned,
    pub created_at: Zoned,
    pub chain_count: u32,
    pub status: ScheduledNotificationStatus,
    pub cloud_task_name: String,
    #[serde(default)]
    pub data: Option<HashMap<String, serde_json::Value>>,
    #[serde(default)]
    pub category: Option<String>,
}

/// Per-doctor read marker for a broadcast announcement.
///
/// Stored in the `notification_reads/{doctorId}/broadcasts/{notificationId}`
/// subcollection. Presence of the document means the doctor has read that
/// broadcast; the broadcast doc's own `isRead` field is ignored.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct BroadcastReadDoc {
    #[serde(rename = "notificationId")]
    notification_id: String,
    #[serde(rename = "readAt")]
    read_at: Zoned,
}

#[async_trait]
pub trait NotificationRepoTrait: Send + Sync {
    async fn get_notifications(
        &self,
        doctor_id: &str,
        notification_type: &str,
        category: Option<&str>,
        page_token: Option<Zoned>,
        limit: u32,
    ) -> AppResult<Vec<NotificationDoc>>;
    async fn mark_as_read(&self, doctor_id: &str, notification_id: &str) -> AppResult<()>;
    async fn mark_all_as_read(&self, doctor_id: &str, notification_type: &str) -> AppResult<()>;
    async fn mark_all_as_unread(&self, doctor_id: &str, notification_type: &str) -> AppResult<()>;
    async fn create_notification(
        &self,
        doctor_id: &str,
        notification: &NotificationDoc,
    ) -> AppResult<()>;

    // Scheduled notification management
    async fn save_scheduled_notification(&self, doc: &ScheduledNotificationDoc) -> AppResult<()>;
    async fn get_scheduled_notification(
        &self,
        notification_id: &str,
    ) -> AppResult<Option<ScheduledNotificationDoc>>;
    async fn update_scheduled_notification_status(
        &self,
        notification_id: &str,
        status: ScheduledNotificationStatus,
    ) -> AppResult<()>;

    async fn get_pending_scheduled_notifications_by_booking_id(
        &self,
        booking_id: &str,
    ) -> AppResult<Vec<ScheduledNotificationDoc>>;

    // FCM token management — separate responsibility, grouped here for single impl convenience
    async fn save_token(
        &self,
        doctor_id: &str,
        device_id: &str,
        token: &FcmTokenDoc,
    ) -> AppResult<()>;
    async fn get_tokens(&self, doctor_id: &str) -> AppResult<Vec<FcmTokenDoc>>;
    async fn delete_token(&self, doctor_id: &str, device_id: &str) -> AppResult<()>;
}

/// Sentinel `doctorId` used for broadcast announcements that every doctor sees.
/// Real doctor account ids are always positive, so `0` is unambiguous.
pub const BROADCAST_DOCTOR_ID: i32 = 0;

/// Subcollection holding the broadcast documents under a per-doctor read parent.
const BROADCAST_READS_SUBCOLLECTION: &str = "broadcasts";

#[derive(Clone)]
pub struct NotificationRepo {
    firestore: FirestoreRepo,
    collection: String,
    fcm_tokens_collection: String,
    scheduled_collection: String,
    reads_collection: String,
}

impl NotificationRepo {
    pub fn new(
        firestore: FirestoreRepo,
        collection: String,
        fcm_tokens_collection: String,
    ) -> Self {
        Self {
            firestore,
            scheduled_collection: "scheduled_notifications".to_string(),
            reads_collection: "notification_reads".to_string(),
            collection,
            fcm_tokens_collection,
        }
    }

    fn parse_doctor_id(doctor_id: &str) -> i32 {
        doctor_id.parse::<i32>().unwrap_or(0)
    }

    fn is_missing_index_error(err: &AppError) -> bool {
        match err {
            AppError::FirestoreError(msg) => {
                msg.contains("FailedPrecondition") && msg.contains("requires an index")
            }
            _ => false,
        }
    }

    fn matches_notification_filters(
        doc: &NotificationDoc,
        notification_type: &str,
        category: Option<&str>,
    ) -> bool {
        match doc {
            NotificationDoc::Alert { .. } => notification_type == "Alert",
            NotificationDoc::Announcement { category: cat, .. } => {
                notification_type == "Announcement" && category.is_none_or(|wanted| wanted == cat)
            }
        }
    }

    fn mark_all_matches(doc: &NotificationDoc, notification_type: &str, is_read: bool) -> bool {
        match doc {
            NotificationDoc::Alert { is_read: r, .. } => {
                notification_type == "Alert" && *r == is_read
            }
            NotificationDoc::Announcement { is_read: r, .. } => {
                notification_type == "Announcement" && *r == is_read
            }
        }
    }

    async fn fetch_doctor_notifications(&self, doctor_id: &str) -> AppResult<Vec<NotificationDoc>> {
        let filters = vec![QueryFilter {
            field: "doctorId".to_string(),
            op: QueryOp::Eq,
            value: serde_json::json!(Self::parse_doctor_id(doctor_id)),
        }];

        self.firestore
            .query_collection::<NotificationDoc>(&self.collection, filters, None, None, None)
            .await
    }

    /// Fetch the set of broadcast notification ids the doctor has marked as read.
    async fn fetch_broadcast_read_ids(&self, doctor_id: &str) -> AppResult<HashSet<String>> {
        let reads: Vec<BroadcastReadDoc> = self
            .firestore
            .query_subcollection::<BroadcastReadDoc>(
                &self.reads_collection,
                doctor_id,
                BROADCAST_READS_SUBCOLLECTION,
                vec![],
                None,
                None,
                None,
            )
            .await?;

        Ok(reads.into_iter().map(|r| r.notification_id).collect())
    }

    /// Query broadcast announcements (doctorId == BROADCAST_DOCTOR_ID), overlaying
    /// the requesting doctor's per-doctor read state onto each returned doc.
    async fn fetch_broadcast_announcements(
        &self,
        doctor_id: &str,
        category: Option<&str>,
        page_token: Option<Zoned>,
        limit: u32,
    ) -> AppResult<Vec<NotificationDoc>> {
        let mut docs = self
            .query_announcements_for_doctor(BROADCAST_DOCTOR_ID, category, page_token, limit)
            .await?;

        let read_ids = self.fetch_broadcast_read_ids(doctor_id).await?;
        for doc in &mut docs {
            doc.set_is_read(read_ids.contains(doc.notification_id()));
        }
        Ok(docs)
    }

    /// Query announcements for an explicit `doctorId` value, with the same
    /// filters/index/fallback behavior used by the personal read path.
    async fn query_announcements_for_doctor(
        &self,
        doctor_id_num: i32,
        category: Option<&str>,
        page_token: Option<Zoned>,
        limit: u32,
    ) -> AppResult<Vec<NotificationDoc>> {
        let mut filters = vec![
            QueryFilter {
                field: "doctorId".to_string(),
                op: QueryOp::Eq,
                value: serde_json::json!(doctor_id_num),
            },
            QueryFilter {
                field: "__type".to_string(),
                op: QueryOp::Eq,
                value: serde_json::json!("Announcement"),
            },
        ];
        if let Some(cat) = category {
            filters.push(QueryFilter {
                field: "category".to_string(),
                op: QueryOp::Eq,
                value: serde_json::json!(cat),
            });
        }
        if let Some(ref token) = page_token {
            filters.push(QueryFilter {
                field: "sentAt".to_string(),
                op: QueryOp::Lt,
                value: serde_json::json!(token),
            });
        }

        match self
            .firestore
            .query_collection::<NotificationDoc>(
                &self.collection,
                filters,
                Some("sentAt"),
                Some(limit),
                None,
            )
            .await
        {
            Ok(docs) => Ok(docs),
            Err(err) if Self::is_missing_index_error(&err) => {
                let doctor_id = doctor_id_num.to_string();
                warn!(
                    doctor_id,
                    "Missing Firestore index for announcement query, using fallback scan"
                );
                self.fallback_get_notifications(
                    &doctor_id,
                    "Announcement",
                    category,
                    page_token,
                    limit,
                )
                .await
            }
            Err(err) => Err(err),
        }
    }

    /// Merge personal + broadcast announcements: newest first, stable by id, capped at `limit`.
    fn merge_announcements(
        mut personal: Vec<NotificationDoc>,
        broadcast: Vec<NotificationDoc>,
        limit: u32,
    ) -> Vec<NotificationDoc> {
        personal.extend(broadcast);
        personal.sort_by(|a, b| {
            b.sent_at()
                .cmp(a.sent_at())
                .then_with(|| a.notification_id().cmp(b.notification_id()))
        });
        personal.truncate(limit as usize);
        personal
    }

    /// Determine whether a notification id refers to a broadcast announcement
    /// (stored with the sentinel `doctorId`).
    async fn is_broadcast_notification(&self, notification_id: &str) -> AppResult<bool> {
        let doc = self
            .firestore
            .get_doc::<serde_json::Value>(&self.collection, notification_id)
            .await?;
        Ok(doc
            .as_ref()
            .and_then(|v| v.get("doctorId"))
            .and_then(|v| v.as_i64())
            == Some(BROADCAST_DOCTOR_ID as i64))
    }

    /// Mark every broadcast announcement as read for one doctor.
    async fn mark_all_broadcasts_read(&self, doctor_id: &str) -> AppResult<()> {
        let broadcasts = self
            .query_announcements_for_doctor(BROADCAST_DOCTOR_ID, None, None, 1000)
            .await?;
        for doc in broadcasts {
            let read = BroadcastReadDoc {
                notification_id: doc.notification_id().to_string(),
                read_at: Zoned::now(),
            };
            self.firestore
                .set_subcollection_doc(
                    &self.reads_collection,
                    doctor_id,
                    BROADCAST_READS_SUBCOLLECTION,
                    doc.notification_id(),
                    &read,
                )
                .await?;
        }
        Ok(())
    }

    /// Clear all broadcast read markers for one doctor (mark every broadcast unread).
    async fn mark_all_broadcasts_unread(&self, doctor_id: &str) -> AppResult<()> {
        let read_ids = self.fetch_broadcast_read_ids(doctor_id).await?;
        for id in read_ids {
            self.firestore
                .delete_subcollection_doc(
                    &self.reads_collection,
                    doctor_id,
                    BROADCAST_READS_SUBCOLLECTION,
                    &id,
                )
                .await?;
        }
        Ok(())
    }

    async fn fallback_get_notifications(
        &self,
        doctor_id: &str,
        notification_type: &str,
        category: Option<&str>,
        page_token: Option<Zoned>,
        limit: u32,
    ) -> AppResult<Vec<NotificationDoc>> {
        let mut docs = self.fetch_doctor_notifications(doctor_id).await?;

        docs.retain(|doc| Self::matches_notification_filters(doc, notification_type, category));
        if let Some(token) = page_token {
            docs.retain(|doc| doc.sent_at() < &token);
        }

        docs.sort_by(|a, b| {
            b.sent_at()
                .cmp(a.sent_at())
                .then_with(|| a.notification_id().cmp(b.notification_id()))
        });
        docs.truncate(limit as usize);

        Ok(docs)
    }

    async fn fallback_mark_all(
        &self,
        doctor_id: &str,
        notification_type: &str,
        current_is_read: bool,
        target_is_read: bool,
    ) -> AppResult<()> {
        let mut docs = self.fetch_doctor_notifications(doctor_id).await?;
        docs.retain(|doc| Self::mark_all_matches(doc, notification_type, current_is_read));

        if docs.is_empty() {
            return Ok(());
        }

        let updates: Vec<(String, HashMap<String, serde_json::Value>)> = docs
            .iter()
            .map(|d| {
                let mut fields = HashMap::new();
                fields.insert("isRead".to_string(), serde_json::json!(target_is_read));
                (
                    format!("{}/{}", self.collection, d.notification_id()),
                    fields,
                )
            })
            .collect();

        self.firestore
            .batch_write_updates_partial(updates, vec!["isRead".to_string()])
            .await
    }
}

#[async_trait]
impl NotificationRepoTrait for NotificationRepo {
    #[instrument(
        skip(self),
        fields(
            doctor_id,
            notification_type,
            category = ?category,
            limit,
        ),
        err
    )]
    async fn get_notifications(
        &self,
        doctor_id: &str,
        notification_type: &str,
        category: Option<&str>,
        page_token: Option<Zoned>,
        limit: u32,
    ) -> AppResult<Vec<NotificationDoc>> {
        let start = Instant::now();

        // Announcements include both the doctor's personal announcements and
        // global broadcasts. Broadcasts carry per-doctor read state, so they are
        // fetched and overlaid separately, then merged with the personal results.
        if notification_type == "Announcement" {
            let personal = self
                .query_announcements_for_doctor(
                    Self::parse_doctor_id(doctor_id),
                    category,
                    page_token.clone(),
                    limit,
                )
                .await?;
            let broadcast = self
                .fetch_broadcast_announcements(doctor_id, category, page_token, limit)
                .await?;
            let merged = Self::merge_announcements(personal, broadcast, limit);
            debug!(
                doctor_id,
                category = ?category,
                count = merged.len(),
                latency_ms = start.elapsed().as_millis(),
                "Announcement query (personal + broadcast) completed"
            );
            return Ok(merged);
        }

        let mut filters = vec![
            QueryFilter {
                field: "doctorId".to_string(),
                op: QueryOp::Eq,
                value: serde_json::json!(Self::parse_doctor_id(doctor_id)),
            },
            QueryFilter {
                field: "__type".to_string(),
                op: QueryOp::Eq,
                value: serde_json::json!(notification_type),
            },
        ];
        if let Some(cat) = category {
            filters.push(QueryFilter {
                field: "category".to_string(),
                op: QueryOp::Eq,
                value: serde_json::json!(cat),
            });
        }
        if let Some(ref token) = page_token {
            filters.push(QueryFilter {
                field: "sentAt".to_string(),
                op: QueryOp::Lt,
                value: serde_json::json!(token),
            });
        }
        let result = match self
            .firestore
            .query_collection::<NotificationDoc>(
                &self.collection,
                filters,
                Some("sentAt"),
                Some(limit),
                None,
            )
            .await
        {
            Ok(docs) => {
                let latency = start.elapsed();
                if latency.as_millis() > 500 {
                    warn!(
                        doctor_id,
                        notification_type,
                        category = ?category,
                        latency_ms = latency.as_millis(),
                        "Slow notification query detected (>500ms)"
                    );
                } else {
                    debug!(
                        doctor_id,
                        notification_type,
                        category = ?category,
                        count = docs.len(),
                        latency_ms = latency.as_millis(),
                        "Notification query completed"
                    );
                }
                Ok(docs)
            }
            Err(err) if Self::is_missing_index_error(&err) => {
                warn!(
                    doctor_id,
                    notification_type,
                    category = ?category,
                    "Missing Firestore index for notifications query, using fallback scan"
                );
                self.fallback_get_notifications(
                    doctor_id,
                    notification_type,
                    category,
                    page_token,
                    limit,
                )
                .await
            }
            Err(err) => Err(err),
        };

        result
    }

    async fn mark_as_read(&self, doctor_id: &str, notification_id: &str) -> AppResult<()> {
        // Broadcasts are shared, so read state is tracked per doctor in a
        // separate subcollection instead of on the shared document.
        if self.is_broadcast_notification(notification_id).await? {
            let read = BroadcastReadDoc {
                notification_id: notification_id.to_string(),
                read_at: Zoned::now(),
            };
            return self
                .firestore
                .set_subcollection_doc(
                    &self.reads_collection,
                    doctor_id,
                    BROADCAST_READS_SUBCOLLECTION,
                    notification_id,
                    &read,
                )
                .await;
        }

        let mut fields = HashMap::new();
        fields.insert("isRead".to_string(), serde_json::json!(true));
        self.firestore
            .update_doc_partial(
                &self.collection,
                notification_id,
                fields,
                vec!["isRead".to_string()],
            )
            .await
    }

    async fn mark_all_as_read(&self, doctor_id: &str, notification_type: &str) -> AppResult<()> {
        // Broadcast announcements track read state per doctor.
        if notification_type == "Announcement" {
            self.mark_all_broadcasts_read(doctor_id).await?;
        }

        let filters = vec![
            QueryFilter {
                field: "doctorId".to_string(),
                op: QueryOp::Eq,
                value: serde_json::json!(Self::parse_doctor_id(doctor_id)),
            },
            QueryFilter {
                field: "__type".to_string(),
                op: QueryOp::Eq,
                value: serde_json::json!(notification_type),
            },
            QueryFilter {
                field: "isRead".to_string(),
                op: QueryOp::Eq,
                value: serde_json::json!(false),
            },
        ];
        let docs = match self
            .firestore
            .query_collection::<NotificationDoc>(
                &self.collection,
                filters,
                Some("sentAt"),
                Some(1000),
                None,
            )
            .await
        {
            Ok(docs) => docs,
            Err(err) if Self::is_missing_index_error(&err) => {
                warn!(
                    doctor_id,
                    notification_type,
                    "Missing Firestore index for mark_all_as_read query, using fallback scan"
                );
                return self
                    .fallback_mark_all(doctor_id, notification_type, false, true)
                    .await;
            }
            Err(err) => return Err(err),
        };

        if docs.is_empty() {
            return Ok(());
        }

        let updates: Vec<(String, HashMap<String, serde_json::Value>)> = docs
            .iter()
            .map(|d| {
                let mut fields = HashMap::new();
                fields.insert("isRead".to_string(), serde_json::json!(true));
                (
                    format!("{}/{}", self.collection, d.notification_id()),
                    fields,
                )
            })
            .collect();

        self.firestore
            .batch_write_updates_partial(updates, vec!["isRead".to_string()])
            .await
    }

    async fn mark_all_as_unread(&self, doctor_id: &str, notification_type: &str) -> AppResult<()> {
        // Broadcast announcements track read state per doctor.
        if notification_type == "Announcement" {
            self.mark_all_broadcasts_unread(doctor_id).await?;
        }

        let filters = vec![
            QueryFilter {
                field: "doctorId".to_string(),
                op: QueryOp::Eq,
                value: serde_json::json!(Self::parse_doctor_id(doctor_id)),
            },
            QueryFilter {
                field: "__type".to_string(),
                op: QueryOp::Eq,
                value: serde_json::json!(notification_type),
            },
            QueryFilter {
                field: "isRead".to_string(),
                op: QueryOp::Eq,
                value: serde_json::json!(true),
            },
        ];
        let docs = match self
            .firestore
            .query_collection::<NotificationDoc>(
                &self.collection,
                filters,
                Some("sentAt"),
                Some(1000),
                None,
            )
            .await
        {
            Ok(docs) => docs,
            Err(err) if Self::is_missing_index_error(&err) => {
                warn!(
                    doctor_id,
                    notification_type,
                    "Missing Firestore index for mark_all_as_unread query, using fallback scan"
                );
                return self
                    .fallback_mark_all(doctor_id, notification_type, true, false)
                    .await;
            }
            Err(err) => return Err(err),
        };

        if docs.is_empty() {
            return Ok(());
        }

        let updates: Vec<(String, HashMap<String, serde_json::Value>)> = docs
            .iter()
            .map(|d| {
                let mut fields = HashMap::new();
                fields.insert("isRead".to_string(), serde_json::json!(false));
                (
                    format!("{}/{}", self.collection, d.notification_id()),
                    fields,
                )
            })
            .collect();

        self.firestore
            .batch_write_updates_partial(updates, vec!["isRead".to_string()])
            .await
    }

    async fn create_notification(
        &self,
        doctor_id: &str,
        notification: &NotificationDoc,
    ) -> AppResult<()> {
        let notification_id = notification.notification_id();

        let mut notification_value = serde_json::to_value(notification)
            .map_err(|e| AppError::FirestoreError(e.to_string()))?;

        if let serde_json::Value::Object(map) = &mut notification_value {
            map.insert(
                "doctorId".to_string(),
                serde_json::json!(doctor_id.parse::<i32>().unwrap_or(0)),
            );
        }

        let mut fields = HashMap::new();
        if let serde_json::Value::Object(map) = notification_value {
            for (key, value) in map {
                fields.insert(key, value);
            }
        }

        self.firestore
            .update_doc(&self.collection, notification_id, fields)
            .await
    }

    async fn save_scheduled_notification(&self, doc: &ScheduledNotificationDoc) -> AppResult<()> {
        debug!(
            notification_id = %doc.notification_id,
            "Saving scheduled notification"
        );
        self.firestore
            .set_doc(&self.scheduled_collection, &doc.notification_id, doc)
            .await
    }

    async fn get_scheduled_notification(
        &self,
        notification_id: &str,
    ) -> AppResult<Option<ScheduledNotificationDoc>> {
        self.firestore
            .get_doc::<ScheduledNotificationDoc>(&self.scheduled_collection, notification_id)
            .await
    }

    async fn update_scheduled_notification_status(
        &self,
        notification_id: &str,
        status: ScheduledNotificationStatus,
    ) -> AppResult<()> {
        let mut fields = HashMap::new();
        fields.insert(
            "status".to_string(),
            serde_json::to_value(&status).unwrap_or(serde_json::json!("pending")),
        );
        self.firestore
            .update_doc_partial(
                &self.scheduled_collection,
                notification_id,
                fields,
                vec!["status".to_string()],
            )
            .await
    }

    async fn get_pending_scheduled_notifications_by_booking_id(
        &self,
        booking_id: &str,
    ) -> AppResult<Vec<ScheduledNotificationDoc>> {
        let all_pending: Vec<ScheduledNotificationDoc> = self
            .firestore
            .query_collection(
                &self.scheduled_collection,
                vec![QueryFilter {
                    field: "status".to_string(),
                    op: QueryOp::Eq,
                    value: serde_json::json!("pending"),
                }],
                None,
                None,
                None,
            )
            .await?;

        let filtered = all_pending
            .into_iter()
            .filter(|doc| {
                doc.data
                    .as_ref()
                    .and_then(|d| d.get("bookingId"))
                    .and_then(|v| v.as_str())
                    == Some(booking_id)
            })
            .collect();

        Ok(filtered)
    }

    async fn save_token(
        &self,
        doctor_id: &str,
        device_id: &str,
        token: &FcmTokenDoc,
    ) -> AppResult<()> {
        self.firestore
            .set_subcollection_doc(
                &self.fcm_tokens_collection,
                doctor_id,
                "devices",
                device_id,
                token,
            )
            .await
    }

    async fn get_tokens(&self, doctor_id: &str) -> AppResult<Vec<FcmTokenDoc>> {
        self.firestore
            .query_subcollection::<FcmTokenDoc>(
                &self.fcm_tokens_collection,
                doctor_id,
                "devices",
                vec![],
                None,
                None,
                None,
            )
            .await
    }

    async fn delete_token(&self, doctor_id: &str, device_id: &str) -> AppResult<()> {
        self.firestore
            .delete_subcollection_doc(&self.fcm_tokens_collection, doctor_id, "devices", device_id)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn announcement(id: &str, sent_at: &str, is_read: bool) -> NotificationDoc {
        NotificationDoc::Announcement {
            notification_id: id.to_string(),
            is_read,
            title: "t".to_string(),
            sub_title: "s".to_string(),
            sent_at: sent_at.parse::<Zoned>().expect("valid zoned"),
            content_url: "https://example.com".to_string(),
            icon_url: "https://example.com/i.png".to_string(),
            category: "Marketing".to_string(),
        }
    }

    #[test]
    fn merge_sorts_newest_first_and_truncates() {
        let personal = vec![
            announcement("p_old", "2026-01-01T00:00:00+00:00[UTC]", false),
            announcement("p_new", "2026-03-01T00:00:00+00:00[UTC]", false),
        ];
        let broadcast = vec![announcement(
            "b_mid",
            "2026-02-01T00:00:00+00:00[UTC]",
            true,
        )];

        let merged = NotificationRepo::merge_announcements(personal, broadcast, 10);

        let ids: Vec<&str> = merged.iter().map(|d| d.notification_id()).collect();
        assert_eq!(ids, vec!["p_new", "b_mid", "p_old"]);
    }

    #[test]
    fn merge_truncates_to_limit() {
        let personal = vec![
            announcement("p1", "2026-01-03T00:00:00+00:00[UTC]", false),
            announcement("p2", "2026-01-02T00:00:00+00:00[UTC]", false),
        ];
        let broadcast = vec![announcement("b1", "2026-01-01T00:00:00+00:00[UTC]", false)];

        let merged = NotificationRepo::merge_announcements(personal, broadcast, 2);

        assert_eq!(merged.len(), 2);
        let ids: Vec<&str> = merged.iter().map(|d| d.notification_id()).collect();
        assert_eq!(ids, vec!["p1", "p2"]);
    }

    #[test]
    fn merge_preserves_overlaid_broadcast_read_state() {
        // Broadcast read state is overlaid before merge; merge must keep it.
        let personal = vec![announcement("p1", "2026-01-01T00:00:00+00:00[UTC]", false)];
        let broadcast = vec![announcement(
            "b_read",
            "2026-02-01T00:00:00+00:00[UTC]",
            true,
        )];

        let merged = NotificationRepo::merge_announcements(personal, broadcast, 10);

        let read = merged
            .iter()
            .find(|d| d.notification_id() == "b_read")
            .expect("broadcast present");
        match read {
            NotificationDoc::Announcement { is_read, .. } => assert!(*is_read),
            _ => panic!("expected announcement"),
        }
    }
}
