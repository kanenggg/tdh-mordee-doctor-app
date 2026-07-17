//! State machine validation for consultation events
//!
//! This module implements the state transition logic to ensure
//! consultation events follow the correct lifecycle.

use tdh_protocol::biz_apm::consultation_event::ConsultationEvent;

use super::models::{ConsultationState, ConsultationStateDoc, SessionStateInfo};
use common::core::error::{AppError, AppResult};
// Use tdh-protocol::serde_compat for protocol types

/// State machine for validating consultation event transitions
pub struct ConsultationStateMachine;

impl ConsultationStateMachine {
    /// Get the initial state for a consultation when receiving the first event.
    /// Returns an error if the event type is not valid as an initial event.
    pub fn get_initial_state(event: &ConsultationEvent) -> AppResult<ConsultationState> {
        match event {
            ConsultationEvent::TimeslotReserved(_) => Ok(ConsultationState::TimeslotReserved),
            ConsultationEvent::ConsultationBooked(_) => Ok(ConsultationState::ConsultationBooked),
            ConsultationEvent::SessionCreated(_) => Ok(ConsultationState::SessionCreated),
            _ => Err(AppError::InternalError(
                "Invalid first event for consultation".to_string(),
            )),
        }
    }

    /// Validate if an event can be applied given the current state
    ///
    /// Returns the next state if the transition is valid, or an error otherwise.
    pub fn validate_transition(
        current_state: &ConsultationState,
        event: &ConsultationEvent,
    ) -> AppResult<ConsultationState> {
        let next_state = Self::get_next_state(event);

        // Allow no-op transitions (idempotent event processing)
        // If the state would not change, treat it as a valid no-op
        if current_state == &next_state {
            tracing::debug!(
                "No-op state transition: {:?} -> {:?} (event: {})",
                current_state,
                next_state,
                event.event_type_name()
            );
            return Ok(next_state);
        }

        // Check if transition is valid
        if !current_state.can_transition_to(&next_state) {
            return Err(AppError::ValidationError(format!(
                "Invalid state transition: {:?} -> {:?} (event: {})",
                current_state,
                next_state,
                event.event_type_name()
            )));
        }

        Ok(next_state)
    }

    /// Get the next state after applying an event
    pub fn get_next_state(event: &ConsultationEvent) -> ConsultationState {
        match event {
            ConsultationEvent::TimeslotReserved(_) => ConsultationState::TimeslotReserved,
            ConsultationEvent::ReservationCancelled(_) => ConsultationState::ReservationCancelled,
            ConsultationEvent::ReservationExpired(_) => ConsultationState::ReservationExpired,
            ConsultationEvent::ConsultationBooked(_) => ConsultationState::ConsultationBooked,
            ConsultationEvent::ConsultationCancelled(_) => ConsultationState::ConsultationCancelled,
            ConsultationEvent::SessionCreated(_) => ConsultationState::SessionCreated,
            ConsultationEvent::PatientJoined(_) => ConsultationState::PatientJoined,
            ConsultationEvent::DoctorJoined(_) => ConsultationState::DoctorJoined,
            ConsultationEvent::AllParticipantJoined(_) => ConsultationState::AllParticipantJoined,
            ConsultationEvent::PatientDisconnected(_) => ConsultationState::PatientDisconnected,
            ConsultationEvent::DoctorDisconnected(_) => ConsultationState::DoctorDisconnected,
            ConsultationEvent::SessionTerminated(_) => ConsultationState::SessionTerminated,
            ConsultationEvent::ConsultationSummarized(_) => {
                ConsultationState::ConsultationSummarized
            }
            ConsultationEvent::FollowUpRequired(_) => ConsultationState::FollowUpRequired,
            ConsultationEvent::FollowUpRequestExpired(_) => {
                ConsultationState::FollowUpRequestExpired
            }
            ConsultationEvent::PatientAcceptedFollowUp(_) => {
                ConsultationState::PatientAcceptedFollowUp
            }
            ConsultationEvent::FollowUpCancelled(_) => ConsultationState::FollowUpCancelled,
        }
    }

    /// Update session info based on the event
    pub fn update_session_info(
        session_info: Option<SessionStateInfo>,
        event: &ConsultationEvent,
    ) -> Option<SessionStateInfo> {
        match event {
            ConsultationEvent::SessionCreated(e) => Some(SessionStateInfo {
                session_provider: Some(e.session_provider.clone()),
                consultation_start_time: Some(e.consultation_start_time),
                consultation_duration_in_second: Some(e.consultation_duration_in_second),
                patient_joined_at: None,
                doctor_joined_at: None,
                patient_disconnected_at: None,
                doctor_disconnected_at: None,
            }),
            ConsultationEvent::PatientJoined(e) => {
                let mut info = session_info.unwrap_or(SessionStateInfo {
                    session_provider: None,
                    consultation_start_time: None,
                    consultation_duration_in_second: None,
                    patient_joined_at: None,
                    doctor_joined_at: None,
                    patient_disconnected_at: None,
                    doctor_disconnected_at: None,
                });
                info.patient_joined_at = Some(e.joined_at);
                Some(info)
            }
            ConsultationEvent::DoctorJoined(e) => {
                let mut info = session_info.unwrap_or(SessionStateInfo {
                    session_provider: None,
                    consultation_start_time: None,
                    consultation_duration_in_second: None,
                    patient_joined_at: None,
                    doctor_joined_at: None,
                    patient_disconnected_at: None,
                    doctor_disconnected_at: None,
                });
                info.doctor_joined_at = Some(e.joined_at);
                Some(info)
            }
            ConsultationEvent::PatientDisconnected(e) => {
                let mut info = session_info.unwrap_or(SessionStateInfo {
                    session_provider: None,
                    consultation_start_time: None,
                    consultation_duration_in_second: None,
                    patient_joined_at: None,
                    doctor_joined_at: None,
                    patient_disconnected_at: None,
                    doctor_disconnected_at: None,
                });
                info.patient_disconnected_at = Some(e.disconnected_at);
                Some(info)
            }
            ConsultationEvent::DoctorDisconnected(e) => {
                let mut info = session_info.unwrap_or(SessionStateInfo {
                    session_provider: None,
                    consultation_start_time: None,
                    consultation_duration_in_second: None,
                    patient_joined_at: None,
                    doctor_joined_at: None,
                    patient_disconnected_at: None,
                    doctor_disconnected_at: None,
                });
                info.doctor_disconnected_at = Some(e.disconnected_at);
                Some(info)
            }
            _ => session_info,
        }
    }

    /// Validate event-specific business rules
    pub fn validate_event_rules(
        current_state: &ConsultationStateDoc,
        event: &ConsultationEvent,
    ) -> AppResult<()> {
        match event {
            // SessionCreated must have consultation booked first
            ConsultationEvent::SessionCreated(_) => {
                if !matches!(
                    current_state.current_state,
                    ConsultationState::ConsultationBooked
                ) {
                    return Err(AppError::ValidationError(
                        "SessionCreated requires ConsultationBooked state".to_string(),
                    ));
                }
            }

            // PatientJoined must have session created
            ConsultationEvent::PatientJoined(_) => {
                if !matches!(
                    current_state.current_state,
                    ConsultationState::SessionCreated | ConsultationState::DoctorJoined
                ) {
                    return Err(AppError::ValidationError(
                        "PatientJoined requires SessionCreated or DoctorJoined state".to_string(),
                    ));
                }
            }

            // DoctorJoined must have session created
            ConsultationEvent::DoctorJoined(_) => {
                if !matches!(
                    current_state.current_state,
                    ConsultationState::SessionCreated | ConsultationState::PatientJoined
                ) {
                    return Err(AppError::ValidationError(
                        "DoctorJoined requires SessionCreated or PatientJoined state".to_string(),
                    ));
                }
            }

            // AllParticipantJoined requires both to have joined
            ConsultationEvent::AllParticipantJoined(_) => {
                if !matches!(
                    current_state.current_state,
                    ConsultationState::PatientJoined | ConsultationState::DoctorJoined
                ) {
                    return Err(AppError::ValidationError(
                        "AllParticipantJoined requires both participants to have joined"
                            .to_string(),
                    ));
                }

                // Verify both joined in session info
                if let Some(ref info) = current_state.session_info {
                    if info.patient_joined_at.is_none() || info.doctor_joined_at.is_none() {
                        return Err(AppError::ValidationError(
                            "AllParticipantJoined requires both patient and doctor join times"
                                .to_string(),
                        ));
                    }
                }
            }

            // SessionTerminated validation
            ConsultationEvent::SessionTerminated(_) => {
                // Can terminate from various session states
                if !current_state.current_state.is_session() {
                    return Err(AppError::ValidationError(
                        "SessionTerminated requires a session state".to_string(),
                    ));
                }
            }

            // ConsultationSummarized must be after session terminated
            ConsultationEvent::ConsultationSummarized(_) => {
                if current_state.current_state != ConsultationState::SessionTerminated {
                    return Err(AppError::ValidationError(
                        "ConsultationSummarized requires SessionTerminated state".to_string(),
                    ));
                }
            }

            // Post-session events require ConsultationSummarized
            ConsultationEvent::FollowUpRequired(_) => {
                if current_state.current_state != ConsultationState::ConsultationSummarized {
                    return Err(AppError::ValidationError(
                        "FollowUpRequired requires ConsultationSummarized state".to_string(),
                    ));
                }
            }

            ConsultationEvent::PatientAcceptedFollowUp(_) => {
                if !matches!(
                    current_state.current_state,
                    ConsultationState::FollowUpRequired | ConsultationState::FollowUpRequestExpired
                ) {
                    return Err(AppError::ValidationError(
                        "PatientAcceptedFollowUp requires FollowUpRequired or FollowUpRequestExpired state".to_string(),
                    ));
                }
            }

            _ => {}
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use crate::module::doctor_calendar::domain::models::ConsultationPhase;

    use super::*;
    use jiff::Timestamp;
    use tdh_protocol::biz_apm::{
        consultation_event::*, BookingType, ConsultationChannel, PatientIdentity,
    };

    fn make_test_state() -> ConsultationStateDoc {
        ConsultationStateDoc {
            booking_id: "test-booking".to_string(),
            doctor_id: 123,
            patient_identity: PatientIdentity {
                account_id: 456,
                user_profile_id: 789,
                tenant_id: 1,
                oidc_user_id: None,
            },
            current_state: ConsultationState::None,
            current_phase: ConsultationPhase::PreSession,
            created_at: Timestamp::now(),
            updated_at: Timestamp::now(),
            session_info: None,
        }
    }

    #[test]
    fn test_valid_none_to_booked_transition() {
        let state = make_test_state();
        let event = ConsultationEvent::ConsultationBooked(ConsultationBookedEvent {
            booking_id: "test-booking".to_string(),
            patient_identity: state.patient_identity.clone(),
            doctor_id: 123,
            biz_unit_id: 1,
            payment_module_id: 1,
            booking_type: BookingType::Scheduled,
            consultation_start_time: 0,
            consultation_duration_in_second: 1800,
            consultation_channel: ConsultationChannel::Video,
            booked_at: 0,
            symptoms: "test".to_string(),
            consultation_fee: 100.0,
        });

        let result = ConsultationStateMachine::validate_transition(&state.current_state, &event);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), ConsultationState::ConsultationBooked);
    }

    #[test]
    fn test_invalid_reverse_transition() {
        let mut state = make_test_state();
        state.current_state = ConsultationState::PatientJoined;

        let event = ConsultationEvent::SessionCreated(SessionCreatedEvent {
            booking_id: "test-booking".to_string(),
            patient_identity: state.patient_identity.clone(),
            doctor_id: 123,
            session_provider: "test".to_string(),
            consultation_start_time: 0,
            consultation_duration_in_second: 1800,
            created_at: 0,
        });

        let result = ConsultationStateMachine::validate_transition(&state.current_state, &event);
        assert!(result.is_err());
    }

    #[test]
    fn test_idempotent_duplicate_event() {
        // Test that duplicate events are allowed (no-op transition)
        let booked_event = ConsultationEvent::ConsultationBooked(ConsultationBookedEvent {
            booking_id: "test-booking".to_string(),
            patient_identity: PatientIdentity {
                account_id: 456,
                user_profile_id: 789,
                tenant_id: 1,
                oidc_user_id: None,
            },
            doctor_id: 123,
            biz_unit_id: 1,
            payment_module_id: 1,
            booking_type: BookingType::Scheduled,
            consultation_start_time: 0,
            consultation_duration_in_second: 1800,
            consultation_channel: ConsultationChannel::Video,
            booked_at: 0,
            symptoms: "test".to_string(),
            consultation_fee: 100.0,
        });

        // First transition: None -> ConsultationBooked (should succeed)
        let result =
            ConsultationStateMachine::validate_transition(&ConsultationState::None, &booked_event);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), ConsultationState::ConsultationBooked);

        // Duplicate event: ConsultationBooked -> ConsultationBooked (should succeed as no-op)
        let result = ConsultationStateMachine::validate_transition(
            &ConsultationState::ConsultationBooked,
            &booked_event,
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), ConsultationState::ConsultationBooked);
    }
}
