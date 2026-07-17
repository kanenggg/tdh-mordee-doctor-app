use jiff::{tz::TimeZone, Timestamp};
use std::collections::HashMap;
use std::sync::Arc;

use tdh_protocol::biz_apm::consultation_event::{ConsultationCancelledEvent, ConsultationEvent};
use tdh_protocol::notification::{
    NotificationPayload, NotificationType, PubsubPushMessage, ScheduledNotificationTask,
};
use tracing::{debug, error, info, warn};

use common::core::dedup_cache::DedupCache;
use common::core::error::{AppError, AppResult};
use common::messaging::pubsub_handler::{PubsubProcessingOutcome, WebhookResponse};
use common::messaging::{CloudTasksService, PubsubPublisher};
use common::notification::{
    NotificationDoc, NotificationRepoTrait, ScheduledNotificationDoc, ScheduledNotificationStatus,
};
use common::patient::PatientService;

use crate::infra::fcm_service::{FcmError, FcmService};

use super::domain::notification_publisher::ConsultationNotificationPublisher;
use super::domain::notification_templates;

const DEDUP_TTL_SECS: u64 = 600;
const FCM_MAX_RETRIES: u32 = 3;

fn build_notification_data(payload: &NotificationPayload) -> HashMap<String, String> {
    let mut data = HashMap::new();
    if let Some(ref custom) = payload.data {
        for (key, value) in custom {
            data.insert(
                key.clone(),
                serde_json::to_string(value).unwrap_or_else(|_| value.to_string()),
            );
        }
    }
    data.insert(
        "type".to_string(),
        format!("{:?}", payload.notification_type),
    );
    data
}

#[derive(Clone)]
pub struct DoctorNotificationService {
    pubsub_publisher: Arc<PubsubPublisher>,
    notification_publisher: Arc<ConsultationNotificationPublisher>,
    notification_repo: Arc<dyn NotificationRepoTrait>,
    cloud_tasks_service: Arc<CloudTasksService>,
    patient_service: Arc<PatientService>,
    doctor_notifications_topic: String,
    dedup_cache: DedupCache,
}

#[derive(Clone)]
pub struct DoctorNotificationDeliveryService {
    notification_repo: Arc<dyn NotificationRepoTrait>,
    fcm_service: Arc<FcmService>,
    cloud_tasks_service: Arc<CloudTasksService>,
    dedup_cache: DedupCache,
}

#[derive(Debug)]
pub enum ScheduledNotificationTaskOutcome {
    Chained(String),
    Sent(String),
}

#[derive(Clone)]
pub struct ScheduledNotificationTaskService {
    cloud_tasks_service: Arc<CloudTasksService>,
    delivery_service: Arc<DoctorNotificationDeliveryService>,
}

impl ScheduledNotificationTaskService {
    pub fn new(
        cloud_tasks_service: Arc<CloudTasksService>,
        delivery_service: Arc<DoctorNotificationDeliveryService>,
    ) -> Self {
        Self {
            cloud_tasks_service,
            delivery_service,
        }
    }

    pub async fn process(
        &self,
        task: ScheduledNotificationTask,
    ) -> AppResult<ScheduledNotificationTaskOutcome> {
        if !self.cloud_tasks_service.should_send_now(&task) {
            let task_name = self.cloud_tasks_service.reschedule_chain(task).await?;
            return Ok(ScheduledNotificationTaskOutcome::Chained(task_name));
        }

        let response = self
            .delivery_service
            .deliver_to_target_doctors(&task.notification)
            .await?;
        Ok(ScheduledNotificationTaskOutcome::Sent(response.message))
    }
}

#[async_trait::async_trait]
pub trait ScheduledNotificationTaskHandler: Send + Sync {
    async fn process_scheduled_notification(
        &self,
        task: ScheduledNotificationTask,
    ) -> AppResult<ScheduledNotificationTaskOutcome>;
}

#[async_trait::async_trait]
pub trait DoctorNotificationDeliveryHandler: Send + Sync {
    async fn deliver_doctor_notification(&self, push: PubsubPushMessage)
        -> PubsubProcessingOutcome;
}

#[async_trait::async_trait]
impl ScheduledNotificationTaskHandler for ScheduledNotificationTaskService {
    async fn process_scheduled_notification(
        &self,
        task: ScheduledNotificationTask,
    ) -> AppResult<ScheduledNotificationTaskOutcome> {
        self.process(task).await
    }
}

#[async_trait::async_trait]
impl DoctorNotificationDeliveryHandler for DoctorNotificationDeliveryService {
    async fn deliver_doctor_notification(
        &self,
        push: PubsubPushMessage,
    ) -> PubsubProcessingOutcome {
        self.handle_push(push).await
    }
}

impl DoctorNotificationService {
    pub fn new(
        pubsub_publisher: Arc<PubsubPublisher>,
        notification_publisher: Arc<ConsultationNotificationPublisher>,
        notification_repo: Arc<dyn NotificationRepoTrait>,
        cloud_tasks_service: Arc<CloudTasksService>,
        patient_service: Arc<PatientService>,
        doctor_notifications_topic: String,
    ) -> Self {
        Self {
            pubsub_publisher,
            notification_publisher,
            notification_repo,
            cloud_tasks_service,
            patient_service,
            doctor_notifications_topic,
            dedup_cache: DedupCache::new(DEDUP_TTL_SECS),
        }
    }

    async fn handle_push(&self, push: PubsubPushMessage) -> AppResult<WebhookResponse> {
        let message_id = push.message.message_id.clone();

        if self.dedup_cache.check_and_mark(&message_id).await {
            info!(
                "Skipping duplicate doctor-notification event: id={}",
                message_id
            );
            return Ok(WebhookResponse {
                message: format!("Duplicate message {} skipped", message_id),
            });
        }

        match self.process_push(push).await {
            Ok(response) => Ok(response),
            Err(e) => {
                warn!(
                    "Doctor-notification processing failed for {}, removing from dedup cache to allow retry: {}",
                    message_id, e
                );
                self.dedup_cache.remove(&message_id).await;
                Err(e)
            }
        }
    }

    async fn process_push(&self, push: PubsubPushMessage) -> AppResult<WebhookResponse> {
        let event = self.decode_event(&push)?;
        self.process_decoded_doctor_notification(event).await
    }

    async fn process_decoded_doctor_notification(
        &self,
        event: ConsultationEvent,
    ) -> AppResult<WebhookResponse> {
        match event {
            ConsultationEvent::ConsultationBooked(booked) => {
                self.notification_publisher
                    .handle_consultation_booked(&booked)
                    .await?;
                Ok(WebhookResponse {
                    message: format!(
                        "Doctor notification sent for booked consultation {}",
                        booked.booking_id
                    ),
                })
            }
            ConsultationEvent::ConsultationCancelled(cancelled) => {
                self.handle_consultation_cancelled(&cancelled).await?;
                Ok(WebhookResponse {
                    message: format!(
                        "Doctor notification sent for cancelled consultation {}",
                        cancelled.booking_id
                    ),
                })
            }
            other => Ok(WebhookResponse {
                message: format!(
                    "Ignored {} for doctor notification action",
                    other.event_type_name()
                ),
            }),
        }
    }

    async fn handle_consultation_cancelled(
        &self,
        event: &ConsultationCancelledEvent,
    ) -> AppResult<()> {
        let pending_notifications = self
            .notification_repo
            .get_pending_scheduled_notifications_by_booking_id(&event.booking_id)
            .await?;

        info!(
            booking_id = %event.booking_id,
            count = pending_notifications.len(),
            "Found pending scheduled notifications to cancel"
        );

        for doc in &pending_notifications {
            if let Err(e) = self
                .cloud_tasks_service
                .cancel_task(&doc.cloud_task_name)
                .await
            {
                warn!(
                    booking_id = %event.booking_id,
                    cloud_task_name = %doc.cloud_task_name,
                    error = %e,
                    "Failed to cancel Cloud Task (may already have been executed)"
                );
            }

            if let Err(e) = self
                .notification_repo
                .update_scheduled_notification_status(
                    &doc.notification_id,
                    ScheduledNotificationStatus::Cancelled,
                )
                .await
            {
                error!(
                    booking_id = %event.booking_id,
                    notification_id = %doc.notification_id,
                    error = %e,
                    "Failed to update scheduled notification status to Cancelled"
                );
            }
        }

        let patient_name = self
            .patient_service
            .get_patient_name(event.patient_identity.account_id)
            .await;

        let (title, body) = notification_templates::cancellation_en(&patient_name);

        let mut data = std::collections::HashMap::new();
        data.insert(
            "bookingId".to_string(),
            serde_json::Value::String(event.booking_id.clone()),
        );

        let payload = NotificationPayload {
            notification_type: NotificationType::Consultation,
            doctor_account_ids: Some(vec![event.doctor_id]),
            title,
            body,
            data: Some(data),
            category: Some("consultation".to_string()),
            scheduled_at: None,
        };

        self.pubsub_publisher
            .publish(&self.doctor_notifications_topic, &payload)
            .await?;

        info!(
            booking_id = %event.booking_id,
            doctor_id = event.doctor_id,
            "Published cancellation notification to doctor"
        );

        Ok(())
    }

    fn decode_event(&self, push: &PubsubPushMessage) -> AppResult<ConsultationEvent> {
        let bytes = self
            .decode_base64(&push.message.data)
            .map_err(|e| AppError::PubsubError(format!("Failed to decode base64: {}", e)))?;

        if let Ok(json_str) = String::from_utf8(bytes.clone()) {
            info!(
                message_id = %push.message.message_id,
                subscription = %push.subscription,
                payload_size_bytes = bytes.len(),
                raw_json_payload = %json_str,
                "Attempting to deserialize consultation event"
            );
        } else {
            warn!(
                message_id = %push.message.message_id,
                payload_size_bytes = bytes.len(),
                "Pub/Sub payload is not valid UTF-8"
            );
        }

        self.deserialize_event(&push.message.message_id, &bytes)
    }

    fn deserialize_event(&self, message_id: &str, bytes: &[u8]) -> AppResult<ConsultationEvent> {
        if let Ok(json_str) = String::from_utf8(bytes.to_vec()) {
            info!(
                message_id = %message_id,
                payload_size_bytes = bytes.len(),
                raw_json_payload = %json_str,
                "Attempting to deserialize consultation event"
            );
        } else {
            warn!(
                message_id = %message_id,
                payload_size_bytes = bytes.len(),
                "Pub/Sub payload is not valid UTF-8"
            );
        }

        let event: ConsultationEvent = serde_json::from_slice(bytes).map_err(|e| {
            let raw_payload = String::from_utf8_lossy(bytes);
            let diagnostic_info =
                if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&raw_payload) {
                    let event_type = json_val
                        .get("__type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("(missing __type)");
                    let booking_type = json_val
                        .get("bookingType")
                        .and_then(|v| v.as_str())
                        .unwrap_or("(missing)");
                    let consultation_channel = json_val
                        .get("consultationChannel")
                        .and_then(|v| v.as_str())
                        .unwrap_or("(missing)");
                    format!(
                        "eventType={}, bookingType={}, consultationChannel={}",
                        event_type, booking_type, consultation_channel
                    )
                } else {
                    "Payload is not valid JSON".to_string()
                };

            error!(
                message_id = %message_id,
                error = %e,
                raw_payload = %raw_payload,
                diagnostic_info = %diagnostic_info,
                "Failed to deserialize consultation event"
            );
            AppError::PubsubError(format!(
                "Failed to deserialize consultation event: {} | Diagnostic: {}",
                e, diagnostic_info
            ))
        })?;

        info!(
            message_id = %message_id,
            event_type = %event.event_type_name(),
            booking_id = %event.booking_id(),
            doctor_id = event.doctor_id(),
            "Successfully deserialized consultation event"
        );

        Ok(event)
    }

    fn decode_base64(&self, data: &str) -> Result<Vec<u8>, AppError> {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD
            .decode(data)
            .map_err(|e| AppError::PubsubError(format!("Invalid base64: {}", e)))
    }
}

impl DoctorNotificationDeliveryService {
    pub fn new(
        notification_repo: Arc<dyn NotificationRepoTrait>,
        fcm_service: Arc<FcmService>,
        cloud_tasks_service: Arc<CloudTasksService>,
    ) -> Self {
        Self {
            notification_repo,
            fcm_service,
            cloud_tasks_service,
            dedup_cache: DedupCache::new(DEDUP_TTL_SECS),
        }
    }

    async fn handle_push(&self, push: PubsubPushMessage) -> PubsubProcessingOutcome {
        let message_id = push.message.message_id.clone();

        if self.dedup_cache.check_and_mark(&message_id).await {
            info!("Skipping duplicate Pub/Sub message: id={}", message_id);
            return PubsubProcessingOutcome::Success {
                message: format!("Duplicate message {} skipped", message_id),
            };
        }

        let data = match self.decode_base64(&push.message.data) {
            Ok(data) => data,
            Err(e) => {
                let reason = e.to_string();
                warn!(
                    message_id = %message_id,
                    reason = %reason,
                    "Permanent base64 decode failure, keeping in dedup cache"
                );
                return PubsubProcessingOutcome::PermanentFailure { reason };
            }
        };

        match self.process_notification_bytes(&message_id, &data).await {
            Ok(response) => PubsubProcessingOutcome::Success {
                message: response.message,
            },
            Err(e) => {
                let outcome = PubsubProcessingOutcome::from_error(e);
                match &outcome {
                    PubsubProcessingOutcome::TransientFailure { error } => {
                        warn!(
                            "Transient failure for message {}, removing from dedup cache to allow retry: {}",
                            message_id, error
                        );
                        self.dedup_cache.remove(&message_id).await;
                    }
                    PubsubProcessingOutcome::PermanentFailure { reason } => {
                        warn!(
                            "Permanent failure for message {}, keeping in dedup cache: {}",
                            message_id, reason
                        );
                    }
                    _ => {}
                }
                outcome
            }
        }
    }

    fn decode_base64(&self, data: &str) -> Result<Vec<u8>, AppError> {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD
            .decode(data)
            .map_err(|e| AppError::PubsubError(format!("Invalid base64: {}", e)))
    }

    async fn process_notification_bytes(
        &self,
        message_id: &str,
        data: &[u8],
    ) -> AppResult<WebhookResponse> {
        let payload: NotificationPayload = serde_json::from_slice(data)
            .map_err(|e| AppError::PubsubError(format!("Failed to deserialize payload: {}", e)))?;

        info!(
            "Processing notification: type='{:?}', title='{}', doctor_ids={:?}, scheduled_at={:?}",
            payload.notification_type,
            payload.title,
            payload.doctor_account_ids,
            payload.scheduled_at
        );

        payload.validate_scheduled_time()?;

        if let Some(scheduled_at) = payload.scheduled_at {
            let now = Timestamp::now();
            if scheduled_at > now {
                return self
                    .handle_scheduled_notification(message_id, payload, scheduled_at)
                    .await;
            }
            info!(
                "Scheduled time {} is in the past, sending immediately",
                scheduled_at
            );
        }

        self.deliver_to_target_doctors(&payload).await
    }

    pub async fn deliver_to_target_doctors(
        &self,
        payload: &NotificationPayload,
    ) -> AppResult<WebhookResponse> {
        let doctor_ids = self.determine_target_doctors(payload).await?;

        if doctor_ids.is_empty() {
            warn!(
                "No target doctors found for notification type='{:?}', title='{}'",
                payload.notification_type, payload.title
            );
            return Ok(WebhookResponse {
                message: "No target doctors".to_string(),
            });
        }

        info!("Sending notification to {} doctors", doctor_ids.len());

        let mut success_count = 0;
        let mut failure_count = 0;

        for doctor_id in doctor_ids {
            match self.process_doctor_notification(&doctor_id, payload).await {
                Ok(_) => {
                    success_count += 1;
                    debug!(
                        "Successfully processed notification for doctor {}",
                        doctor_id
                    );
                }
                Err(e) => {
                    failure_count += 1;
                    error!(
                        "Failed to process notification for doctor {}: {}",
                        doctor_id, e
                    );
                }
            }
        }

        info!(
            "Notification processing complete: {} succeeded, {} failed",
            success_count, failure_count
        );

        Ok(WebhookResponse {
            message: format!(
                "Processed {} doctors: {} succeeded, {} failed",
                success_count + failure_count,
                success_count,
                failure_count
            ),
        })
    }

    async fn handle_scheduled_notification(
        &self,
        message_id: &str,
        payload: NotificationPayload,
        scheduled_at: Timestamp,
    ) -> AppResult<WebhookResponse> {
        let notification_id = uuid::Uuid::new_v4().to_string();

        let task = ScheduledNotificationTask {
            notification: payload.clone(),
            original_schedule_time: scheduled_at,
            chain_count: 0,
        };

        match self
            .cloud_tasks_service
            .schedule_notification(task, scheduled_at)
            .await
        {
            Ok(task_name) => {
                let doc = ScheduledNotificationDoc {
                    notification_id: notification_id.clone(),
                    doctor_account_ids: payload.doctor_account_ids.unwrap_or_default(),
                    notification_type: format!("{:?}", payload.notification_type),
                    title: payload.title,
                    sub_title: payload.body,
                    scheduled_at: scheduled_at.to_zoned(TimeZone::UTC),
                    created_at: Timestamp::now().to_zoned(TimeZone::UTC),
                    chain_count: 0,
                    status: ScheduledNotificationStatus::Pending,
                    cloud_task_name: task_name,
                    data: payload.data,
                    category: payload.category,
                };

                if let Err(e) = self
                    .notification_repo
                    .save_scheduled_notification(&doc)
                    .await
                {
                    warn!(
                        notification_id = %notification_id,
                        "Failed to persist scheduled notification record (task was created): {e}"
                    );
                }

                Ok(WebhookResponse {
                    message: format!("Scheduled for delivery at {}", scheduled_at),
                })
            }
            Err(e) => {
                self.dedup_cache.remove(message_id).await;
                Err(e)
            }
        }
    }

    async fn determine_target_doctors(
        &self,
        payload: &NotificationPayload,
    ) -> AppResult<Vec<String>> {
        match payload.notification_type {
            NotificationType::Appointment
            | NotificationType::Consultation
            | NotificationType::System => {
                if let Some(ref doctor_ids) = payload.doctor_account_ids {
                    let ids: Vec<String> = doctor_ids.iter().map(|id| id.to_string()).collect();
                    return Ok(ids);
                }
                Err(AppError::PubsubError(
                    "doctor_account_ids required for this notification type".to_string(),
                ))
            }
            NotificationType::Broadcast => {
                warn!(
                    "Broadcast notifications not fully implemented - returning empty doctor list"
                );
                Ok(vec![])
            }
        }
    }

    async fn process_doctor_notification(
        &self,
        doctor_id: &str,
        payload: &NotificationPayload,
    ) -> AppResult<()> {
        debug!("Processing notification for doctor {}", doctor_id);

        let tokens = self
            .notification_repo
            .get_tokens(doctor_id)
            .await
            .map_err(|e| {
                error!("Failed to get FCM tokens for doctor {}: {}", doctor_id, e);
                AppError::InternalError(format!("Failed to get FCM tokens: {}", e))
            })?;

        if tokens.is_empty() {
            warn!("No FCM tokens found for doctor {}", doctor_id);
            return Ok(());
        }

        debug!("Found {} FCM tokens for doctor {}", tokens.len(), doctor_id);

        let notification_data = build_notification_data(payload);
        let mut send_tasks = Vec::new();
        for token_doc in tokens {
            let fcm_service = self.fcm_service.clone();
            let title = payload.title.clone();
            let body = payload.body.clone();
            let data = notification_data.clone();
            let doctor_id_str = doctor_id.to_string();
            let device_id = token_doc.device_id.clone();

            send_tasks.push(tokio::spawn(async move {
                (
                    device_id,
                    fcm_service
                        .send_notification_with_retry(
                            &token_doc.fcm_token,
                            &title,
                            &body,
                            data,
                            FCM_MAX_RETRIES,
                        )
                        .await,
                    doctor_id_str,
                )
            }));
        }

        let mut success_count = 0;
        let mut invalid_tokens = Vec::new();

        for task in send_tasks {
            let (device_id, result, _doctor_id_str) = task.await.map_err(|e| {
                AppError::InternalError(format!("Failed to join FCM send task: {}", e))
            })?;

            match result {
                Ok(_) => {
                    success_count += 1;
                    debug!(
                        "Successfully sent FCM notification to doctor {} device {}",
                        doctor_id, device_id
                    );
                }
                Err(FcmError::InvalidToken(msg)) => {
                    warn!(
                        "Invalid FCM token for doctor {} device {}: {}",
                        doctor_id, device_id, msg
                    );
                    invalid_tokens.push(device_id);
                }
                Err(e) => {
                    error!(
                        "Failed to send FCM notification to doctor {} device {}: {}",
                        doctor_id, device_id, e
                    );
                }
            }
        }

        for device_id in &invalid_tokens {
            if let Err(e) = self
                .notification_repo
                .delete_token(doctor_id, device_id)
                .await
            {
                error!(
                    "Failed to delete invalid token for doctor {} device {}: {}",
                    doctor_id, device_id, e
                );
            }
        }

        self.save_notification_record(doctor_id, payload).await?;

        if success_count == 0 {
            warn!(
                "Notification saved for doctor {} but FCM delivery failed for all devices",
                doctor_id
            );
        }

        let invalid_count = invalid_tokens.len();
        debug!(
            "Notification processing complete for doctor {}: {} sent, {} deleted",
            doctor_id, success_count, invalid_count
        );

        Ok(())
    }

    async fn save_notification_record(
        &self,
        doctor_id: &str,
        payload: &NotificationPayload,
    ) -> AppResult<()> {
        let notification_id: String = uuid::Uuid::new_v4().to_string();
        let now = Timestamp::now().to_zoned(TimeZone::UTC);

        let notification_doc = NotificationDoc::Alert {
            notification_id: notification_id.clone(),
            is_read: false,
            title: payload.title.clone(),
            sub_title: payload.body.clone(),
            sent_at: now,
        };

        self.notification_repo
            .create_notification(doctor_id, &notification_doc)
            .await?;

        debug!(
            "Saved notification record {} for doctor {}",
            notification_id, doctor_id
        );

        Ok(())
    }
}

#[async_trait::async_trait]
pub trait DoctorNotificationSendHandler: Send + Sync {
    async fn send_doctor_notification(&self, push: PubsubPushMessage) -> PubsubProcessingOutcome;
}

#[async_trait::async_trait]
impl DoctorNotificationSendHandler for DoctorNotificationService {
    async fn send_doctor_notification(&self, push: PubsubPushMessage) -> PubsubProcessingOutcome {
        match self.handle_push(push).await {
            Ok(response) => PubsubProcessingOutcome::Success {
                message: response.message,
            },
            Err(error) => PubsubProcessingOutcome::from_error(error),
        }
    }
}
