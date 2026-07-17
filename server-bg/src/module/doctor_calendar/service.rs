use std::sync::Arc;
use std::time::Instant;

use jiff::Timestamp;
use tdh_protocol::biz_apm::consultation_event::{ConsultationEvent, TerminationCode};
use tdh_protocol::notification::PubsubPushMessage;
use tracing::{error, info, warn};

use common::core::dedup_cache::DedupCache;
use common::core::error::{AppError, AppResult};
use common::messaging::pubsub_handler::{PubsubProcessingOutcome, WebhookResponse};

use super::domain::models::{
    AppointmentCardStatus, ConsultationPhase, ConsultationState, ConsultationStateDoc,
    RtdbAppointment,
};
use super::domain::repository::RtdbConsultationEventRepoTrait;
use super::domain::state_machine::ConsultationStateMachine;

const DEDUP_TTL_SECS: u64 = 600;

#[derive(Clone)]
pub struct DoctorCalendarService {
    event_repo: Arc<dyn RtdbConsultationEventRepoTrait>,
    dedup_cache: DedupCache,
}

impl DoctorCalendarService {
    pub fn new(event_repo: Arc<dyn RtdbConsultationEventRepoTrait>) -> Self {
        Self {
            event_repo,
            dedup_cache: DedupCache::new(DEDUP_TTL_SECS),
        }
    }

    async fn handle_push(&self, push: PubsubPushMessage) -> AppResult<WebhookResponse> {
        let message_id = push.message.message_id.clone();

        info!(
            "Received consultation event: id={}, subscription={}, payload={}",
            message_id, push.subscription, push.message.data
        );

        if self.dedup_cache.check_and_mark(&message_id).await {
            info!("Skipping duplicate consultation event: id={}", message_id);
            return Ok(WebhookResponse {
                message: format!("Duplicate message {} skipped", message_id),
            });
        }

        match self.process_push(push).await {
            Ok(response) => Ok(response),
            Err(e) => {
                warn!(
                    "Processing failed for consultation event {}, removing from dedup cache to allow retry: {}",
                    message_id, e
                );
                self.dedup_cache.remove(&message_id).await;
                Err(e)
            }
        }
    }

    async fn process_push(&self, push: PubsubPushMessage) -> AppResult<WebhookResponse> {
        let event = self.decode_event(&push)?;
        self.process_decoded_event(event).await
    }

    async fn process_decoded_event(&self, event: ConsultationEvent) -> AppResult<WebhookResponse> {
        let booking_id = event.booking_id().to_string();
        let doctor_id = event.doctor_id();
        let date = extract_date_from_event(&event);

        if matches!(event, ConsultationEvent::ReservationCancelled(_)) {
            if self
                .event_repo
                .delete_appointment_state(&booking_id, doctor_id, &date)
                .await?
            {
                return Ok(WebhookResponse {
                    message: format!("Reserved booking {} removed", booking_id),
                });
            }

            warn!(
                booking_id = %booking_id,
                "Skipping reservation cancellation: no reserved booking found"
            );
            return Ok(WebhookResponse {
                message: format!("Reserved booking {} not found", booking_id),
            });
        }

        let step_start = Instant::now();
        let current_appointment = self
            .event_repo
            .get_appointment_state(&booking_id, doctor_id, &date)
            .await?;
        info!(elapsed_ms = step_start.elapsed().as_millis(), booking_id = %booking_id, "get_appointment_state completed");
        let current_appointment = current_appointment.unwrap_or_else(|| {
            let mut appt = RtdbAppointment::from_event(&event);
            appt.consultation_state = None;
            appt.consultation_phase = None;
            appt
        });

        let next_state = if let Some(current_state) = current_appointment.consultation_state {
            ConsultationStateMachine::validate_transition(&current_state, &event)?
        } else {
            match ConsultationStateMachine::get_initial_state(&event) {
                Ok(state) => state,
                Err(_) => {
                    warn!(
                        booking_id = %booking_id,
                        event_type = %event.event_type_name(),
                        "Skipping event: no existing consultation state and event is not a valid initial event, acknowledging to prevent poison message"
                    );
                    return Ok(WebhookResponse {
                        message: format!(
                            "Skipped non-initial event {} for booking {} with no existing state",
                            event.event_type_name(),
                            booking_id
                        ),
                    });
                }
            }
        };

        let event_key = event_idempotency_key(&event, &next_state);
        if current_appointment
            .applied_event_keys
            .contains_key(&event_key)
        {
            info!(
                booking_id = %booking_id,
                event_key = %event_key,
                "Skipping already-applied consultation event"
            );
            return Ok(WebhookResponse {
                message: format!(
                    "Event {} already applied for booking {}",
                    event.event_type_name(),
                    booking_id
                ),
            });
        }

        let is_state_noop = current_appointment
            .consultation_state
            .as_ref()
            .is_some_and(|current_state| current_state == &next_state);

        let state_doc = ConsultationStateDoc {
            booking_id: booking_id.clone(),
            doctor_id,
            patient_identity: event.patient_identity(),
            current_state: current_appointment
                .consultation_state
                .unwrap_or(ConsultationState::None),
            current_phase: current_appointment
                .consultation_phase
                .unwrap_or(ConsultationPhase::PreSession),
            created_at: Timestamp::now(),
            updated_at: Timestamp::now(),
            session_info: current_appointment.session_info.clone(),
        };
        ConsultationStateMachine::validate_event_rules(&state_doc, &event)?;

        let next_phase = next_state.phase();
        let mut applied_event_keys = current_appointment.applied_event_keys.clone();
        applied_event_keys.insert(event_key.clone(), Timestamp::now().to_string());
        let updated_appointment = RtdbAppointment {
            consultation_state: Some(next_state),
            consultation_phase: Some(next_phase),
            status: appointment_status_for_event(&event, &next_state),
            session_info: ConsultationStateMachine::update_session_info(
                current_appointment.session_info,
                &event,
            ),
            consultation_channel: if let ConsultationEvent::ConsultationBooked(booked) = &event {
                Some(booked.consultation_channel)
            } else {
                current_appointment.consultation_channel
            },
            booking_type: if let ConsultationEvent::ConsultationBooked(booked) = &event {
                Some(booked.booking_type)
            } else {
                current_appointment.booking_type
            },
            last_applied_event_key: Some(event_key),
            applied_event_keys,
            ..current_appointment
        };

        let step_start = Instant::now();
        self.event_repo
            .upsert_appointment_state(&booking_id, doctor_id, &date, &updated_appointment)
            .await?;
        info!(elapsed_ms = step_start.elapsed().as_millis(), booking_id = %booking_id, "upsert_appointment_state completed");

        if is_state_noop {
            info!(
                booking_id = %booking_id,
                event = %event.event_type_name(),
                "Calendar update is an idempotent no-op consultation event"
            );
        }

        info!(
            "Consultation event processed: booking_id={}, event={}, state={:?}",
            booking_id,
            event.event_type_name(),
            next_state
        );

        Ok(WebhookResponse {
            message: format!("Event {} processed successfully", event.event_type_name()),
        })
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

#[async_trait::async_trait]
pub trait DoctorCalendarUpdateHandler: Send + Sync {
    async fn update_doctor_calendar(&self, push: PubsubPushMessage) -> PubsubProcessingOutcome;
}

#[async_trait::async_trait]
impl DoctorCalendarUpdateHandler for DoctorCalendarService {
    async fn update_doctor_calendar(&self, push: PubsubPushMessage) -> PubsubProcessingOutcome {
        match self.handle_push(push).await {
            Ok(response) => PubsubProcessingOutcome::Success {
                message: response.message,
            },
            Err(error) => PubsubProcessingOutcome::from_error(error),
        }
    }
}

fn extract_date_from_event(event: &ConsultationEvent) -> String {
    let epoch = match event {
        ConsultationEvent::ConsultationBooked(e) => e.consultation_start_time,
        ConsultationEvent::TimeslotReserved(e) => e.reserved_from,
        ConsultationEvent::SessionCreated(e) => e.consultation_start_time,
        ConsultationEvent::ReservationCancelled(e) => e.cancelled_at,
        ConsultationEvent::ReservationExpired(e) => e.cancelled_at,
        ConsultationEvent::ConsultationCancelled(e) => e.cancelled_at,
        ConsultationEvent::PatientJoined(e) => e.joined_at,
        ConsultationEvent::DoctorJoined(e) => e.joined_at,
        ConsultationEvent::AllParticipantJoined(e) => e.patient_joined_at,
        ConsultationEvent::PatientDisconnected(e) => e.disconnected_at,
        ConsultationEvent::DoctorDisconnected(e) => e.disconnected_at,
        ConsultationEvent::SessionTerminated(e) => e.terminated_at,
        ConsultationEvent::ConsultationSummarized(e) => e.summarized_at,
        ConsultationEvent::FollowUpRequired(e) => e.created_at,
        ConsultationEvent::FollowUpRequestExpired(e) => e.created_at,
        ConsultationEvent::PatientAcceptedFollowUp(e) => e.created_at,
        ConsultationEvent::FollowUpCancelled(e) => e.created_at,
    };
    Timestamp::from_second(epoch)
        .unwrap_or_else(|_| Timestamp::now())
        .strftime("%Y-%m-%d")
        .to_string()
}

fn event_idempotency_key(event: &ConsultationEvent, next_state: &ConsultationState) -> String {
    format!(
        "{}:{}:{:?}",
        event.booking_id(),
        event.event_type_name(),
        next_state
    )
}

fn appointment_status_for_event(
    event: &ConsultationEvent,
    next_state: &ConsultationState,
) -> AppointmentCardStatus {
    match event {
        ConsultationEvent::SessionTerminated(e) => match &e.termination_code {
            TerminationCode::SuccessfulSession { .. } => AppointmentCardStatus::PendingRecord,
            TerminationCode::PatientAbsent { .. }
            | TerminationCode::DoctorAbsent { .. }
            | TerminationCode::BothPartiesAbsent => AppointmentCardStatus::Missed,
            TerminationCode::TechnicalError { .. } => AppointmentCardStatus::Fail,
        },
        ConsultationEvent::ConsultationSummarized(_) => AppointmentCardStatus::Completed,
        _ => next_state.to_appointment_status(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use axum_test::TestServer;
    use base64::Engine;
    use serde_json::json;
    use std::collections::HashMap;
    use std::sync::Mutex;
    use tdh_protocol::biz_apm::{
        consultation_event::*, BookingType, ConsultationChannel, PatientIdentity,
    };

    #[derive(Default)]
    struct MemoryRepo {
        rows: Mutex<HashMap<String, RtdbAppointment>>,
    }

    #[async_trait]
    impl RtdbConsultationEventRepoTrait for MemoryRepo {
        async fn get_appointment_state(
            &self,
            booking_id: &str,
            doctor_id: i32,
            date: &str,
        ) -> AppResult<Option<RtdbAppointment>> {
            Ok(self
                .rows
                .lock()
                .unwrap()
                .get(&key(booking_id, doctor_id, date))
                .cloned())
        }

        async fn upsert_appointment_state(
            &self,
            booking_id: &str,
            doctor_id: i32,
            date: &str,
            appointment: &RtdbAppointment,
        ) -> AppResult<()> {
            self.rows
                .lock()
                .unwrap()
                .insert(key(booking_id, doctor_id, date), appointment.clone());
            Ok(())
        }

        async fn delete_appointment_state(
            &self,
            booking_id: &str,
            doctor_id: i32,
            date: &str,
        ) -> AppResult<bool> {
            let mut rows = self.rows.lock().unwrap();
            if rows.remove(&key(booking_id, doctor_id, date)).is_some() {
                return Ok(true);
            }

            let existing_key = rows.iter().find_map(|(key, appointment)| {
                (appointment.appointment_id == booking_id).then(|| key.clone())
            });
            Ok(existing_key.and_then(|key| rows.remove(&key)).is_some())
        }
    }

    fn key(booking_id: &str, doctor_id: i32, date: &str) -> String {
        format!("{doctor_id}/{date}/{booking_id}")
    }

    fn patient() -> PatientIdentity {
        PatientIdentity {
            account_id: 456,
            user_profile_id: 789,
            tenant_id: 1,
            oidc_user_id: None,
        }
    }

    fn reserved_event(booking_id: &str) -> ConsultationEvent {
        ConsultationEvent::TimeslotReserved(TimeslotReservedEvent {
            booking_id: booking_id.to_string(),
            patient_identity: patient(),
            doctor_id: 123,
            biz_unit_id: 1,
            reserved_from: 1_787_965_200,
            reservation_duration_sec: 1800,
            consultation_channel: ConsultationChannel::Video,
            reserved_at: 1_787_965_100,
        })
    }

    fn booked_event(booking_id: &str) -> ConsultationEvent {
        ConsultationEvent::ConsultationBooked(ConsultationBookedEvent {
            booking_id: booking_id.to_string(),
            patient_identity: patient(),
            doctor_id: 123,
            biz_unit_id: 1,
            payment_module_id: 1,
            booking_type: BookingType::Scheduled,
            consultation_start_time: 1_787_965_200,
            consultation_duration_in_second: 1800,
            consultation_channel: ConsultationChannel::Video,
            booked_at: 1_787_965_100,
            symptoms: "test".to_string(),
            consultation_fee: 100.0,
        })
    }

    fn session_created_event(booking_id: &str) -> ConsultationEvent {
        ConsultationEvent::SessionCreated(SessionCreatedEvent {
            booking_id: booking_id.to_string(),
            patient_identity: patient(),
            doctor_id: 123,
            session_provider: "daily".to_string(),
            consultation_start_time: 1_787_965_200,
            consultation_duration_in_second: 1800,
            created_at: 1_787_965_100,
        })
    }

    async fn process(service: &DoctorCalendarService, event: ConsultationEvent) {
        service.process_decoded_event(event).await.unwrap();
    }

    async fn appointment(repo: &MemoryRepo, booking_id: &str) -> Option<RtdbAppointment> {
        repo.rows
            .lock()
            .unwrap()
            .values()
            .find(|appointment| appointment.appointment_id == booking_id)
            .cloned()
    }

    fn pubsub_push(message_id: &str, event: serde_json::Value) -> serde_json::Value {
        let data = base64::engine::general_purpose::STANDARD.encode(event.to_string());
        json!({
            "message": {
                "data": data,
                "messageId": message_id,
                "publishTime": "2026-06-24T00:00:00Z",
                "attributes": {}
            },
            "subscription": "doctor-calendar-update-sub"
        })
    }

    #[tokio::test]
    async fn timeslot_reserved_adds_reserved_appointment_record() {
        let repo = Arc::new(MemoryRepo::default());
        let service = DoctorCalendarService::new(repo.clone());

        process(&service, reserved_event("reserve-1")).await;

        let appointment = appointment(&repo, "reserve-1").await.unwrap();
        assert_eq!(
            serde_json::to_value(appointment.status).unwrap()["__type"],
            "Reserved"
        );
        assert!(matches!(
            appointment.consultation_state,
            Some(ConsultationState::TimeslotReserved)
        ));
    }

    #[tokio::test]
    async fn doctor_calendar_http_example_consultation_booked_is_processed() {
        let repo = Arc::new(MemoryRepo::default());
        let service = Arc::new(DoctorCalendarService::new(repo.clone()));
        let server =
            TestServer::new(crate::module::doctor_calendar::handler::routes(service)).unwrap();

        let response = server
            .post("/consultation/v1/doctor-calendar/update")
            .json(&pubsub_push(
                "consultationbooked-http-example",
                json!({
                    "__type": "ConsultationBooked",
                    "bookingId": "bg-cal-success-1",
                    "patientIdentity": {
                        "accountId": 456,
                        "userProfileId": 789,
                        "tenantId": 1,
                        "oidcUserId": "test-patient"
                    },
                    "doctorId": 123,
                    "bizUnitId": 1,
                    "paymentModuleId": 1,
                    "bookingType": {
                        "__type": "scheduled"
                    },
                    "consultationStartTime": 1787965200,
                    "consultationDurationInSecond": 1800,
                    "consultationChannel": "video",
                    "bookedAt": 1787965100,
                    "symptoms": "HTTP calendar test",
                    "consultationFee": 100
                }),
            ))
            .await;

        response.assert_status_ok();
        let body = response.json::<serde_json::Value>();
        assert_eq!(body["status"], "processed");
        assert_eq!(
            body["message"],
            "Event ConsultationBooked processed successfully"
        );

        let appointment = appointment(&repo, "bg-cal-success-1").await.unwrap();
        assert_eq!(
            serde_json::to_value(appointment.status).unwrap()["__type"],
            "UpComing"
        );
        assert!(matches!(
            appointment.consultation_state,
            Some(ConsultationState::ConsultationBooked)
        ));
    }

    #[tokio::test]
    async fn reservation_cancelled_removes_reserved_booking() {
        let repo = Arc::new(MemoryRepo::default());
        let service = DoctorCalendarService::new(repo.clone());

        process(&service, reserved_event("cancel-1")).await;
        process(
            &service,
            ConsultationEvent::ReservationCancelled(ReservationCancelledEvent {
                booking_id: "cancel-1".to_string(),
                patient_identity: patient(),
                doctor_id: 123,
                biz_unit_id: 1,
                cancelled_at: 1_787_878_900,
            }),
        )
        .await;

        assert!(appointment(&repo, "cancel-1").await.is_none());
    }

    #[tokio::test]
    async fn session_terminated_maps_status_from_termination_code() {
        let cases = [
            (
                TerminationCode::SuccessfulSession {
                    patient_joined_at: 1_787_965_240,
                    doctor_joined_at: 1_787_965_250,
                },
                "PendingRecord",
                "success-1",
            ),
            (
                TerminationCode::PatientAbsent {
                    doctor_joined_at: 1_787_965_250,
                },
                "Missed",
                "patient-absent-1",
            ),
            (
                TerminationCode::DoctorAbsent {
                    patient_joined_at: 1_787_965_250,
                },
                "Missed",
                "doctor-absent-1",
            ),
            (
                TerminationCode::BothPartiesAbsent,
                "Missed",
                "both-absent-1",
            ),
            (
                TerminationCode::TechnicalError {
                    error_message: "network".to_string(),
                },
                "Fail",
                "technical-error-1",
            ),
        ];

        for (termination_code, expected_status, booking_id) in cases {
            let repo = Arc::new(MemoryRepo::default());
            let service = DoctorCalendarService::new(repo.clone());

            process(&service, booked_event(booking_id)).await;
            process(&service, session_created_event(booking_id)).await;
            process(
                &service,
                ConsultationEvent::SessionTerminated(SessionTerminatedEvent {
                    booking_id: booking_id.to_string(),
                    patient_identity: patient(),
                    doctor_id: 123,
                    termination_code,
                    terminated_by: SessionParticipant::System,
                    terminated_at: 1_787_966_000,
                }),
            )
            .await;

            let appointment = appointment(&repo, booking_id).await.unwrap();
            assert_eq!(
                serde_json::to_value(appointment.status).unwrap()["__type"],
                expected_status
            );
        }
    }

    #[tokio::test]
    async fn consultation_summarized_updates_completed_status() {
        let repo = Arc::new(MemoryRepo::default());
        let service = DoctorCalendarService::new(repo.clone());

        process(&service, booked_event("summary-1")).await;
        process(&service, session_created_event("summary-1")).await;
        process(
            &service,
            ConsultationEvent::SessionTerminated(SessionTerminatedEvent {
                booking_id: "summary-1".to_string(),
                patient_identity: patient(),
                doctor_id: 123,
                termination_code: TerminationCode::SuccessfulSession {
                    patient_joined_at: 1_787_965_240,
                    doctor_joined_at: 1_787_965_250,
                },
                terminated_by: SessionParticipant::Doctor,
                terminated_at: 1_787_966_000,
            }),
        )
        .await;
        process(
            &service,
            ConsultationEvent::ConsultationSummarized(ConsultationSummarizedEvent {
                booking_id: "summary-1".to_string(),
                patient_identity: patient(),
                doctor_id: 123,
                doctor_note: "done".to_string(),
                prescription_info: PrescriptionInfo {
                    prescription_refcode: "rx-1".to_string(),
                    medicine_items: vec![],
                    expire_at: 1_787_970_000,
                },
                summarized_at: 1_787_966_100,
            }),
        )
        .await;

        let appointment = appointment(&repo, "summary-1").await.unwrap();
        assert_eq!(
            serde_json::to_value(appointment.status).unwrap()["__type"],
            "Completed"
        );
    }
}
