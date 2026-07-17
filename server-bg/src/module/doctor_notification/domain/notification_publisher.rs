//! Publishes doctor-facing notifications when consultation events occur.
//!
//! For **Scheduled** bookings three notifications are created:
//!   1. Immediate — "New Appointment" (published right away via Pub/Sub)
//!   2. T-15 min   — reminder (scheduled via Cloud Tasks)
//!   3. T-0        — "Time for your appointment" (scheduled via Cloud Tasks)
//!
//! For **Instant** bookings only the immediate notification is published.

use std::collections::HashMap;
use std::sync::Arc;

use jiff::{tz::TimeZone, SignedDuration, Timestamp};
use tdh_protocol::biz_apm::consultation_event::ConsultationBookedEvent;
use tdh_protocol::biz_apm::BookingType;
use tdh_protocol::notification::ScheduledNotificationTask;
use tracing::{error, info};
use uuid::Uuid;

use common::core::error::AppResult;
use common::messaging::{CloudTasksService, PubsubPublisher};
use common::notification::{
    NotificationRepoTrait, ScheduledNotificationDoc, ScheduledNotificationStatus,
};
use common::patient::PatientService;
// Use tdh-protocol::serde_compat for protocol types
use tdh_protocol::notification::{NotificationPayload, NotificationType};

use super::notification_templates;

const REMINDER_MINUTES: i64 = 15;

#[derive(Clone)]
pub struct ConsultationNotificationPublisher {
    pubsub_publisher: Arc<PubsubPublisher>,
    patient_service: Arc<PatientService>,
    cloud_tasks_service: Arc<CloudTasksService>,
    notification_repo: Arc<dyn NotificationRepoTrait>,
    doctor_notifications_topic: String,
}

impl ConsultationNotificationPublisher {
    pub fn new(
        pubsub_publisher: Arc<PubsubPublisher>,
        patient_service: Arc<PatientService>,
        cloud_tasks_service: Arc<CloudTasksService>,
        notification_repo: Arc<dyn NotificationRepoTrait>,
        doctor_notifications_topic: String,
    ) -> Self {
        Self {
            pubsub_publisher,
            patient_service,
            cloud_tasks_service,
            notification_repo,
            doctor_notifications_topic,
        }
    }

    /// React to a ConsultationBooked event by sending the appropriate
    /// notification(s) to the doctor.
    pub async fn handle_consultation_booked(
        &self,
        event: &ConsultationBookedEvent,
    ) -> AppResult<()> {
        let patient_name = self
            .patient_service
            .get_patient_name(event.patient_identity.account_id)
            .await;

        match event.booking_type {
            BookingType::Instant => {
                self.publish_instant_notifications(event, &patient_name)
                    .await?;
            }
            BookingType::Scheduled => {
                self.publish_scheduled_notifications(event, &patient_name)
                    .await?;
            }
        }

        Ok(())
    }

    // ── Instant ─────────────────────────────────────────────────────

    async fn publish_instant_notifications(
        &self,
        event: &ConsultationBookedEvent,
        patient_name: &str,
    ) -> AppResult<()> {
        let (title, body) = notification_templates::instant_immediate_en(patient_name);
        let payload = self.build_payload(event, title, body, None);

        info!(
            booking_id = %event.booking_id,
            doctor_id = event.doctor_id,
            "Publishing instant booking immediate notification"
        );
        self.pubsub_publisher
            .publish(&self.doctor_notifications_topic, &payload)
            .await?;

        Ok(())
    }

    // ── Scheduled ───────────────────────────────────────────────────

    async fn publish_scheduled_notifications(
        &self,
        event: &ConsultationBookedEvent,
        patient_name: &str,
    ) -> AppResult<()> {
        let (start_display, end_display) = notification_templates::format_time_range(
            event.consultation_start_time,
            event.consultation_duration_in_second as i64,
        );

        // 1. Immediate notification
        let (title, body) = notification_templates::scheduled_immediate_en(
            patient_name,
            &start_display,
            &end_display,
        );
        let payload = self.build_payload(event, title, body, None);

        info!(
            booking_id = %event.booking_id,
            doctor_id = event.doctor_id,
            "Publishing scheduled booking immediate notification"
        );
        self.pubsub_publisher
            .publish(&self.doctor_notifications_topic, &payload)
            .await?;

        let start_dt = Timestamp::from_second(event.consultation_start_time)
            .unwrap_or_else(|_| Timestamp::now());

        // 2. T-15 reminder (only if start time is more than 15 min from now)
        let reminder_time = start_dt
            .checked_sub(SignedDuration::from_secs(REMINDER_MINUTES * 60))
            .unwrap_or(start_dt);
        if reminder_time > Timestamp::now() {
            let (title, body) =
                notification_templates::scheduled_reminder_en(patient_name, REMINDER_MINUTES);
            let payload =
                self.build_payload(event, title.clone(), body.clone(), Some(reminder_time));

            info!(
                booking_id = %event.booking_id,
                schedule_time = %reminder_time,
                "Scheduling T-15 reminder via Cloud Tasks"
            );
            let task = ScheduledNotificationTask {
                notification: payload.clone(),
                original_schedule_time: reminder_time,
                chain_count: 0,
            };
            let task_name = self
                .cloud_tasks_service
                .schedule_notification(task, reminder_time)
                .await?;

            self.save_scheduled_notification_doc(
                &event.booking_id,
                event.doctor_id,
                &title,
                &body,
                reminder_time,
                &task_name,
                &payload,
            )
            .await;
        }

        // 3. T-0 notification (only if start time is in the future)
        if start_dt > Timestamp::now() {
            let (title, body) = notification_templates::scheduled_now_en(patient_name);
            let payload = self.build_payload(event, title.clone(), body.clone(), Some(start_dt));

            info!(
                booking_id = %event.booking_id,
                schedule_time = %start_dt,
                "Scheduling T-0 notification via Cloud Tasks"
            );
            let task = ScheduledNotificationTask {
                notification: payload.clone(),
                original_schedule_time: start_dt,
                chain_count: 0,
            };
            let task_name = self
                .cloud_tasks_service
                .schedule_notification(task, start_dt)
                .await?;

            self.save_scheduled_notification_doc(
                &event.booking_id,
                event.doctor_id,
                &title,
                &body,
                start_dt,
                &task_name,
                &payload,
            )
            .await;
        }

        Ok(())
    }

    // ── Persistence ────────────────────────────────────────────────

    async fn save_scheduled_notification_doc(
        &self,
        booking_id: &str,
        doctor_id: i32,
        title: &str,
        body: &str,
        scheduled_at: Timestamp,
        cloud_task_name: &str,
        payload: &NotificationPayload,
    ) {
        let notification_id = Uuid::new_v4().to_string();
        let doc = ScheduledNotificationDoc {
            notification_id: notification_id.clone(),
            doctor_account_ids: vec![doctor_id],
            notification_type: "consultation".to_string(),
            title: title.to_string(),
            sub_title: body.to_string(),
            scheduled_at: scheduled_at.to_zoned(TimeZone::UTC),
            created_at: Timestamp::now().to_zoned(TimeZone::UTC),
            chain_count: 0,
            status: ScheduledNotificationStatus::Pending,
            cloud_task_name: cloud_task_name.to_string(),
            data: payload.data.clone(),
            category: payload.category.clone(),
        };
        if let Err(e) = self
            .notification_repo
            .save_scheduled_notification(&doc)
            .await
        {
            error!(
                booking_id = %booking_id,
                notification_id = %notification_id,
                error = %e,
                "Failed to persist scheduled notification to Firestore"
            );
        }
    }

    // ── Helpers ──────────────────────────────────────────────────────

    fn build_payload(
        &self,
        event: &ConsultationBookedEvent,
        title: String,
        body: String,
        scheduled_at: Option<Timestamp>,
    ) -> NotificationPayload {
        let mut data = HashMap::new();
        data.insert(
            "bookingId".to_string(),
            serde_json::Value::String(event.booking_id.clone()),
        );

        NotificationPayload {
            notification_type: NotificationType::Consultation,
            doctor_account_ids: Some(vec![event.doctor_id]),
            title,
            body,
            data: Some(data),
            category: Some("consultation".to_string()),
            scheduled_at,
        }
    }
}
