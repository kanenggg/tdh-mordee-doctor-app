use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde_json;
use tdh_protocol::serde_compat::*;

fn make_patient_identity() -> PatientIdentity {
    PatientIdentity {
        account_id: 123,
        user_profile_id: 456,
        tenant_id: 789,
        oidc_user_id: Some("auth-0".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== ConsultationEvent Tests (JSON Serialization) =====

    #[test]
    fn test_consultation_event_timeslot_reserved_roundtrip() {
        let original = ConsultationEvent::TimeslotReserved(TimeslotReservedEvent {
            booking_id: "booking-123".to_string(),
            patient_identity: make_patient_identity(),
            doctor_id: 2443,
            biz_unit_id: 1,
            reserved_from: 1677648000,
            reservation_duration_sec: 1800,
            consultation_channel: ConsultationChannel::Video,
            reserved_at: 1677648000,
        });

        // Test JSON serialization roundtrip
        let json = serde_json::to_string(&original).unwrap();
        let decoded: ConsultationEvent = serde_json::from_str(&json).unwrap();

        match decoded {
            ConsultationEvent::TimeslotReserved(e) => {
                assert_eq!(e.booking_id, "booking-123");
                assert_eq!(e.doctor_id, 2443);
                assert_eq!(e.biz_unit_id, 1);
                assert_eq!(e.consultation_channel, ConsultationChannel::Video);
            }
            _ => panic!("Wrong variant!"),
        }

        // Test booking_id helper method
        assert_eq!(original.booking_id(), "booking-123");
        assert_eq!(original.doctor_id(), 2443);
    }

    #[test]
    fn test_consultation_event_booked() {
        let original = ConsultationEvent::ConsultationBooked(ConsultationBookedEvent {
            booking_id: "booking-456".to_string(),
            patient_identity: make_patient_identity(),
            doctor_id: 2443,
            biz_unit_id: 1,
            payment_module_id: 789,
            booking_type: BookingType::Scheduled,
            consultation_start_time: 1677648000,
            consultation_duration_in_second: 1800,
            consultation_channel: ConsultationChannel::Video,
            booked_at: 1677648000,
            symptoms: "Headache and fever".to_string(),
            consultation_fee: 500.0,
        });

        let json = serde_json::to_string(&original).unwrap();
        let decoded: ConsultationEvent = serde_json::from_str(&json).unwrap();

        match decoded {
            ConsultationEvent::ConsultationBooked(e) => {
                assert_eq!(e.booking_id, "booking-456");
                assert_eq!(e.consultation_fee, 500.0);
                assert_eq!(e.booking_type, BookingType::Scheduled);
                assert_eq!(e.symptoms, "Headache and fever");
            }
            _ => panic!("Wrong variant!"),
        }
    }

    #[test]
    fn test_consultation_event_session_created() {
        let original = ConsultationEvent::SessionCreated(SessionCreatedEvent {
            booking_id: "booking-789".to_string(),
            patient_identity: make_patient_identity(),
            doctor_id: 2443,
            session_provider: "agora".to_string(),
            consultation_start_time: 1677648000,
            consultation_duration_in_second: 1800,
            created_at: 1677648000,
        });

        let json = serde_json::to_string(&original).unwrap();
        let decoded: ConsultationEvent = serde_json::from_str(&json).unwrap();

        match decoded {
            ConsultationEvent::SessionCreated(e) => {
                assert_eq!(e.booking_id, "booking-789");
                assert_eq!(e.session_provider, "agora");
                assert_eq!(e.created_at, 1677648000);
            }
            _ => panic!("Wrong variant!"),
        }
    }

    #[test]
    fn test_consultation_event_participant_joined() {
        let patient_joined = ConsultationEvent::PatientJoined(PatientJoinedEvent {
            booking_id: "booking-123".to_string(),
            patient_identity: make_patient_identity(),
            doctor_id: 2443,
            joined_at: 1677648000,
        });

        let doctor_joined = ConsultationEvent::DoctorJoined(DoctorJoinedEvent {
            booking_id: "booking-123".to_string(),
            patient_identity: make_patient_identity(),
            doctor_id: 2443,
            joined_at: 1677648001,
        });

        // Test patient joined
        let json = serde_json::to_string(&patient_joined).unwrap();
        let decoded: ConsultationEvent = serde_json::from_str(&json).unwrap();
        match decoded {
            ConsultationEvent::PatientJoined(e) => {
                assert_eq!(e.booking_id, "booking-123");
                assert_eq!(e.joined_at, 1677648000);
            }
            _ => panic!("Wrong variant!"),
        }

        // Test doctor joined
        let json = serde_json::to_string(&doctor_joined).unwrap();
        let decoded: ConsultationEvent = serde_json::from_str(&json).unwrap();
        match decoded {
            ConsultationEvent::DoctorJoined(e) => {
                assert_eq!(e.booking_id, "booking-123");
                assert_eq!(e.joined_at, 1677648001);
            }
            _ => panic!("Wrong variant!"),
        }
    }

    #[test]
    fn test_consultation_event_session_terminated() {
        let original = ConsultationEvent::SessionTerminated(SessionTerminatedEvent {
            booking_id: "booking-terminated".to_string(),
            patient_identity: make_patient_identity(),
            doctor_id: 2443,
            termination_code: TerminationCode::SuccessfulSession {
                patient_joined_at: 1677648000,
                doctor_joined_at: 1677648001,
            },
            terminated_by: SessionParticipant::Doctor,
            terminated_at: 1677649800,
        });

        let json = serde_json::to_string(&original).unwrap();
        let decoded: ConsultationEvent = serde_json::from_str(&json).unwrap();

        match decoded {
            ConsultationEvent::SessionTerminated(e) => {
                assert_eq!(e.booking_id, "booking-terminated");
                assert_eq!(e.terminated_by, SessionParticipant::Doctor);
                match e.termination_code {
                    TerminationCode::SuccessfulSession { .. } => {}
                    _ => panic!("Wrong termination code!"),
                }
            }
            _ => panic!("Wrong variant!"),
        }
    }

    #[test]
    fn test_consultation_event_summarized_with_prescription() {
        let medicine = Medicine {
            medicine_id: 1001,
            price_plan_id: 2001,
            medicine_amount: 30,
            medicine_name_th: "พาราเซตามอล".to_string(),
            medicine_name_en: "Paracetamol".to_string(),
            medicine_instruction_en: "Take 1 tablet every 6 hours".to_string(),
            medicine_instruction_th: "รับประทานครั้งละ 1 เม็ด ทุก 6 ชั่วโมง".to_string(),
            medicine_image_url: "https://example.com/paracetamol.jpg".to_string(),
            medicine_unit_price: 5.50,
        };

        let prescription = PrescriptionInfo {
            prescription_refcode: "RX123456".to_string(),
            medicine_items: vec![medicine],
            expire_at: 1677734400, // 1 day later
        };

        let original = ConsultationEvent::ConsultationSummarized(ConsultationSummarizedEvent {
            booking_id: "booking-summarized".to_string(),
            patient_identity: make_patient_identity(),
            doctor_id: 2443,
            doctor_note: "Patient showing good recovery".to_string(),
            prescription_info: prescription,
            summarized_at: 1677649800,
        });

        let json = serde_json::to_string(&original).unwrap();
        let decoded: ConsultationEvent = serde_json::from_str(&json).unwrap();

        match decoded {
            ConsultationEvent::ConsultationSummarized(e) => {
                assert_eq!(e.doctor_note, "Patient showing good recovery");
                assert_eq!(e.prescription_info.prescription_refcode, "RX123456");
                assert_eq!(e.prescription_info.medicine_items.len(), 1);
                assert_eq!(
                    e.prescription_info.medicine_items[0].medicine_name_en,
                    "Paracetamol"
                );
                assert_eq!(e.prescription_info.medicine_items[0].medicine_amount, 30);
            }
            _ => panic!("Wrong variant!"),
        }
    }

    #[test]
    fn test_termination_codes() {
        let codes = vec![
            TerminationCode::SuccessfulSession {
                patient_joined_at: 1677648000,
                doctor_joined_at: 1677648001,
            },
            TerminationCode::PatientAbsent {
                doctor_joined_at: 1677648001,
            },
            TerminationCode::DoctorAbsent {
                patient_joined_at: 1677648000,
            },
            TerminationCode::BothPartiesAbsent,
            TerminationCode::TechnicalError {
                error_message: "Connection failed".to_string(),
            },
        ];

        for code in codes {
            let json = serde_json::to_string(&code).unwrap();
            let decoded: TerminationCode = serde_json::from_str(&json).unwrap();

            match (&code, &decoded) {
                (
                    TerminationCode::SuccessfulSession {
                        patient_joined_at: patient_time1,
                        doctor_joined_at: doctor_time1,
                    },
                    TerminationCode::SuccessfulSession {
                        patient_joined_at: patient_time2,
                        doctor_joined_at: doctor_time2,
                    },
                ) => {
                    assert_eq!(patient_time1, patient_time2);
                    assert_eq!(doctor_time1, doctor_time2);
                }
                (
                    TerminationCode::PatientAbsent {
                        doctor_joined_at: time1,
                    },
                    TerminationCode::PatientAbsent {
                        doctor_joined_at: time2,
                    },
                ) => {
                    assert_eq!(time1, time2);
                }
                (
                    TerminationCode::DoctorAbsent {
                        patient_joined_at: time1,
                    },
                    TerminationCode::DoctorAbsent {
                        patient_joined_at: time2,
                    },
                ) => {
                    assert_eq!(time1, time2);
                }
                (TerminationCode::BothPartiesAbsent, TerminationCode::BothPartiesAbsent) => {}
                (
                    TerminationCode::TechnicalError {
                        error_message: msg1,
                    },
                    TerminationCode::TechnicalError {
                        error_message: msg2,
                    },
                ) => {
                    assert_eq!(msg1, msg2);
                }
                _ => panic!("Termination code mismatch!"),
            }
        }
    }

    // ===== Basic Type Tests =====

    #[test]
    fn test_patient_identity_serialization() {
        let patient_with_oidc = PatientIdentity {
            account_id: 123,
            user_profile_id: 456,
            tenant_id: 789,
            oidc_user_id: Some("auth-0".to_string()),
        };

        let json = serde_json::to_string(&patient_with_oidc).unwrap();
        assert!(json.contains("\"accountId\":123"));
        assert!(json.contains("\"oidcUserId\":\"auth-0\""));

        let decoded: PatientIdentity = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.oidc_user_id.as_ref().unwrap(), "auth-0");

        // Test without OIDC user ID
        let patient_without_oidc = PatientIdentity {
            account_id: 123,
            user_profile_id: 456,
            tenant_id: 789,
            oidc_user_id: None,
        };

        let json = serde_json::to_string(&patient_without_oidc).unwrap();
        assert!(!json.contains("oidcUserId"));
    }

    #[test]
    fn test_consultation_channel_enum() {
        let channels = vec![
            ConsultationChannel::Video,
            ConsultationChannel::Chat,
            ConsultationChannel::Voice,
        ];

        for channel in channels {
            let json = serde_json::to_string(&channel).unwrap();
            let decoded: ConsultationChannel = serde_json::from_str(&json).unwrap();
            assert_eq!(decoded, channel);
        }
    }

    #[test]
    fn test_booking_type_enum() {
        let booking_scheduled = BookingType::Scheduled;
        let json = serde_json::to_string(&booking_scheduled).unwrap();
        assert!(json.contains("scheduled"));

        let booking_instant = BookingType::Instant;
        let json = serde_json::to_string(&booking_instant).unwrap();
        assert!(json.contains("instant"));
    }

    #[test]
    fn test_session_participant_enum() {
        let participants = vec![
            SessionParticipant::Patient,
            SessionParticipant::Doctor,
            SessionParticipant::System,
        ];

        for participant in participants {
            let json = serde_json::to_string(&participant).unwrap();
            let decoded: SessionParticipant = serde_json::from_str(&json).unwrap();
            assert_eq!(decoded, participant);
        }
    }

    // ===== Localization Tests =====

    #[test]
    fn test_localized_string() {
        let localized = LocalizedString {
            th: "สวัสดี".to_string(),
            en: "Hello".to_string(),
        };

        let json = serde_json::to_string(&localized).unwrap();
        let decoded: LocalizedString = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.th, "สวัสดี");
        assert_eq!(decoded.en, "Hello");
    }

    // ===== Consultation State Tests =====

    #[test]
    fn test_appointment_card_status() {
        let statuses = vec![
            AppointmentCardStatus::UpComing,
            AppointmentCardStatus::Missed,
            AppointmentCardStatus::Completed,
            AppointmentCardStatus::PendingRecord,
        ];

        for status in statuses {
            let json = serde_json::to_string(&status).unwrap();
            let decoded: AppointmentCardStatus = serde_json::from_str(&json).unwrap();

            match (&status, &decoded) {
                (AppointmentCardStatus::UpComing, AppointmentCardStatus::UpComing) => {}
                (AppointmentCardStatus::Missed, AppointmentCardStatus::Missed) => {}
                (AppointmentCardStatus::Completed, AppointmentCardStatus::Completed) => {}
                (AppointmentCardStatus::PendingRecord, AppointmentCardStatus::PendingRecord) => {}
                _ => panic!("Status mismatch!"),
            }
        }
    }

    #[test]
    fn test_consultation_phase() {
        let phases = vec![
            ConsultationPhase::PreSession,
            ConsultationPhase::Session,
            ConsultationPhase::PostSession,
            ConsultationPhase::Completed,
        ];

        for phase in phases {
            let json = serde_json::to_string(&phase).unwrap();
            let decoded: ConsultationPhase = serde_json::from_str(&json).unwrap();
            assert_eq!(decoded, phase);
        }
    }

    #[test]
    fn test_consultation_state_transitions() {
        let pre_session_states = vec![
            ConsultationState::None,
            ConsultationState::TimeslotReserved,
            ConsultationState::ConsultationBooked,
        ];

        let session_states = vec![
            ConsultationState::SessionCreated,
            ConsultationState::PatientJoined,
            ConsultationState::DoctorJoined,
            ConsultationState::AllParticipantJoined,
            ConsultationState::SessionTerminated,
        ];

        let terminal_states = vec![
            ConsultationState::ConsultationCancelled,
            ConsultationState::ReservationCancelled,
            ConsultationState::ReservationExpired,
            ConsultationState::FollowUpCancelled,
            ConsultationState::FollowUpRequestExpired,
        ];

        // Test that terminal states are properly identified
        for state in terminal_states {
            assert!(state.is_terminal(), "State {:?} should be terminal", state);
        }

        // Test that session states are properly identified
        for state in session_states {
            assert!(
                state.is_session(),
                "State {:?} should be session state",
                state
            );
        }

        // Test that pre-session states are not terminal or session
        for state in pre_session_states {
            assert!(
                !state.is_terminal(),
                "State {:?} should not be terminal",
                state
            );
            assert!(
                !state.is_session(),
                "State {:?} should not be session state",
                state
            );
        }
    }

    #[test]
    fn test_consultation_state_phase_mapping() {
        let state_phase_pairs = vec![
            (
                ConsultationState::TimeslotReserved,
                ConsultationPhase::PreSession,
            ),
            (
                ConsultationState::ConsultationBooked,
                ConsultationPhase::PreSession,
            ),
            (
                ConsultationState::SessionCreated,
                ConsultationPhase::Session,
            ),
            (
                ConsultationState::AllParticipantJoined,
                ConsultationPhase::Session,
            ),
            (
                ConsultationState::ConsultationSummarized,
                ConsultationPhase::PostSession,
            ),
            (
                ConsultationState::FollowUpRequired,
                ConsultationPhase::PostSession,
            ),
        ];

        for (state, expected_phase) in state_phase_pairs {
            let phase = state.phase();
            assert_eq!(
                phase, expected_phase,
                "State {:?} should map to phase {:?}",
                state, expected_phase
            );
        }
    }

    #[test]
    fn test_appointment_time() {
        let appointment_time = AppointmentTime {
            start_time: "09:00".to_string(),
            end_time: "09:30".to_string(),
        };

        let json = serde_json::to_string(&appointment_time).unwrap();
        let decoded: AppointmentTime = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.start_time, "09:00");
        assert_eq!(decoded.end_time, "09:30");
    }

    // ===== Notification Tests =====

    #[test]
    fn test_notification_type() {
        let types = vec![
            NotificationType::Appointment,
            NotificationType::Consultation,
            NotificationType::System,
            NotificationType::Broadcast,
        ];

        for notification_type in types {
            let json = serde_json::to_string(&notification_type).unwrap();
            let decoded: NotificationType = serde_json::from_str(&json).unwrap();
            assert_eq!(decoded, notification_type);
        }
    }

    #[test]
    fn test_notification_payload_serialization() {
        let mut data = std::collections::HashMap::new();
        data.insert("booking_id".to_string(), serde_json::json!("booking-123"));
        data.insert("doctor_id".to_string(), serde_json::json!(2443));

        let payload = NotificationPayload {
            notification_type: NotificationType::Consultation,
            doctor_account_ids: Some(vec![1, 2, 3]),
            title: "Consultation Update".to_string(),
            body: "Your consultation is starting soon".to_string(),
            data: Some(data),
            category: Some("consultation".to_string()),
            scheduled_at: None,
        };

        let json = serde_json::to_string(&payload).unwrap();
        let decoded: NotificationPayload = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.notification_type, NotificationType::Consultation);
        assert_eq!(decoded.title, "Consultation Update");
        assert_eq!(decoded.body, "Your consultation is starting soon");
        assert_eq!(decoded.doctor_account_ids, Some(vec![1, 2, 3]));
        assert_eq!(decoded.category, Some("consultation".to_string()));

        let data = decoded.data.unwrap();
        assert_eq!(
            data.get("booking_id").unwrap(),
            &serde_json::json!("booking-123")
        );
        assert_eq!(data.get("doctor_id").unwrap(), &serde_json::json!(2443));
    }

    #[test]
    fn test_pubsub_message() {
        let message = PubsubMessage {
            data: BASE64.encode("Hello, World!"),
            message_id: "message-123".to_string(),
            publish_time: "2023-03-01T10:00:00Z".to_string(),
            attributes: {
                let mut attrs = std::collections::HashMap::new();
                attrs.insert("source".to_string(), "test".to_string());
                attrs
            },
        };

        let json = serde_json::to_string(&message).unwrap();
        let decoded: PubsubMessage = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.message_id, "message-123");
        assert_eq!(decoded.publish_time, "2023-03-01T10:00:00Z");

        // Test data decoding
        let decoded_data = decoded.decode_data().unwrap();
        assert_eq!(String::from_utf8(decoded_data).unwrap(), "Hello, World!");

        // Test UTF-8 decoding convenience method
        let decoded_text = decoded.decode_data_utf8().unwrap();
        assert_eq!(decoded_text, "Hello, World!");
    }

    #[test]
    fn test_pubsub_push_message() {
        let push_message = PubsubPushMessage {
            message: PubsubMessage {
                data: BASE64.encode("test data"),
                message_id: "msg-456".to_string(),
                publish_time: "2023-03-01T10:30:00Z".to_string(),
                attributes: std::collections::HashMap::new(),
            },
            subscription: "projects/test/subscriptions/test-sub".to_string(),
        };

        let json = serde_json::to_string(&push_message).unwrap();
        let decoded: PubsubPushMessage = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.subscription, "projects/test/subscriptions/test-sub");
        assert_eq!(decoded.message.message_id, "msg-456");
    }

    // ===== Follow-up Event Tests =====

    #[test]
    fn test_follow_up_events() {
        let follow_up_required = ConsultationEvent::FollowUpRequired(FollowUpRequiredEvent {
            previous_booking_id: "booking-123".to_string(),
            follow_up_id: "followup-456".to_string(),
            patient_identity: make_patient_identity(),
            doctor_id: 2443,
            biz_unit_id: 1,
            consultation_start_time: 1677734400,
            consultation_duration_in_second: 1800,
            consultation_fee: 300.0,
            consultation_channel: ConsultationChannel::Video,
            additional_patient_note: Some("Need medication review".to_string()),
            internal_note: None,
            created_at: 1677648000,
        });

        let json = serde_json::to_string(&follow_up_required).unwrap();
        let decoded: ConsultationEvent = serde_json::from_str(&json).unwrap();

        match decoded {
            ConsultationEvent::FollowUpRequired(e) => {
                assert_eq!(e.previous_booking_id, "booking-123");
                assert_eq!(e.follow_up_id, "followup-456");
                assert_eq!(e.consultation_fee, 300.0);
                assert_eq!(
                    e.additional_patient_note.as_ref().unwrap(),
                    "Need medication review"
                );
            }
            _ => panic!("Wrong variant!"),
        }

        let follow_up_accepted =
            ConsultationEvent::PatientAcceptedFollowUp(PatientAcceptedFollowUpEvent {
                previous_booking_id: "booking-123".to_string(),
                follow_up_id: "followup-456".to_string(),
                patient_identity: make_patient_identity(),
                doctor_id: 2443,
                consultation_start_time: 1677734400,
                consultation_duration_in_second: 1800,
                consultation_fee: 300.0,
                symptoms: "Still having headaches".to_string(),
                consultation_channel: ConsultationChannel::Video,
                created_at: 1677648100,
            });

        let json = serde_json::to_string(&follow_up_accepted).unwrap();
        let decoded: ConsultationEvent = serde_json::from_str(&json).unwrap();

        match decoded {
            ConsultationEvent::PatientAcceptedFollowUp(e) => {
                assert_eq!(e.previous_booking_id, "booking-123");
                assert_eq!(e.symptoms, "Still having headaches");
            }
            _ => panic!("Wrong variant!"),
        }
    }
}
