//! Follow-up transformation logic
//!
//! Transforms client DTO FollowUpInfo to biz-apm's FollowUp enum

use tdh_protocol::biz_apm::follow_up::{
    FollowUp, FollowUpAppointment, VisitType as ProtoVisitType,
};
use tdh_protocol::biz_apm::ConsultationChannel;

use super::super::models::{FollowUpInfo, VisitType};

fn to_proto_visit_type(vt: &VisitType) -> ProtoVisitType {
    match vt {
        VisitType::FollowUp => ProtoVisitType::FollowUp,
        VisitType::LabResult => ProtoVisitType::LabResult,
        VisitType::PrescriptionRefill => ProtoVisitType::PrescriptionRefill,
    }
}

pub trait ToBizApmFollowUp {
    fn to_biz_apm(self, parent_booking_id: String) -> FollowUp;
}

impl ToBizApmFollowUp for Option<FollowUpInfo> {
    fn to_biz_apm(self, parent_booking_id: String) -> FollowUp {
        match self {
            Some(FollowUpInfo::ScheduleAppointment {
                appointment_start_datetime: appointment_start,
                appointment_end_datetime: appointment_end,
                visit_types,
                note_to_patient,
                note_to_staff,
            }) => {
                let mapped: Vec<ProtoVisitType> =
                    visit_types.iter().map(to_proto_visit_type).collect();

                FollowUp::Appointment(FollowUpAppointment {
                    parent_booking_id,
                    appointment_start,
                    appointment_end,
                    visit_types: mapped,
                    additional_note_to_patient: note_to_patient.unwrap_or_default(),
                    note_to_staff: note_to_staff.unwrap_or_default(),
                    consultation_channel: ConsultationChannel::Video,
                    consultation_fee: 0.0,
                })
            }
            Some(FollowUpInfo::NoFollowUp { .. }) | None => FollowUp::AsNeeded,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transform_schedule_appointment() {
        let follow_up_info = Some(FollowUpInfo::ScheduleAppointment {
            appointment_start_datetime: 1705276800,
            appointment_end_datetime: 1705278600,
            visit_types: vec![VisitType::FollowUp],
            note_to_patient: Some("Check symptoms".to_string()),
            note_to_staff: Some("Monitor recovery".to_string()),
        });

        let result = follow_up_info.to_biz_apm("booking-123".to_string());

        match result {
            FollowUp::Appointment(appointment) => {
                assert_eq!(appointment.parent_booking_id, "booking-123");
                assert_eq!(appointment.appointment_start, 1705276800);
                assert_eq!(appointment.appointment_end, 1705278600);
                assert_eq!(appointment.visit_types.len(), 1);
                assert_eq!(appointment.additional_note_to_patient, "Check symptoms");
                assert_eq!(appointment.note_to_staff, "Monitor recovery");
            }
            _ => panic!("Expected FollowUp::Appointment"),
        }
    }

    #[test]
    fn test_transform_multiple_visit_types() {
        let follow_up_info = Some(FollowUpInfo::ScheduleAppointment {
            appointment_start_datetime: 1705276800,
            appointment_end_datetime: 1705278600,
            visit_types: vec![VisitType::FollowUp, VisitType::LabResult],
            note_to_patient: None,
            note_to_staff: None,
        });

        let result = follow_up_info.to_biz_apm("booking-456".to_string());

        match result {
            FollowUp::Appointment(appointment) => {
                assert_eq!(appointment.visit_types.len(), 2);
            }
            _ => panic!("Expected FollowUp::Appointment"),
        }
    }

    #[test]
    fn test_transform_no_follow_up() {
        let follow_up_info = Some(FollowUpInfo::NoFollowUp {
            note_to_staff: Some("No follow-up needed".to_string()),
        });

        let result = follow_up_info.to_biz_apm("booking-123".to_string());

        assert!(matches!(result, FollowUp::AsNeeded));
    }

    #[test]
    fn test_transform_none() {
        let result = None.to_biz_apm("booking-123".to_string());

        assert!(matches!(result, FollowUp::AsNeeded));
    }

    #[test]
    fn test_transform_schedule_with_empty_visit_types() {
        let follow_up_info = Some(FollowUpInfo::ScheduleAppointment {
            appointment_start_datetime: 1705276800,
            appointment_end_datetime: 1705278600,
            visit_types: vec![],
            note_to_patient: None,
            note_to_staff: None,
        });

        let result = follow_up_info.to_biz_apm("booking-123".to_string());

        match result {
            FollowUp::Appointment(appointment) => {
                assert_eq!(appointment.visit_types.len(), 0);
            }
            _ => panic!("Expected FollowUp::Appointment"),
        }
    }
}
