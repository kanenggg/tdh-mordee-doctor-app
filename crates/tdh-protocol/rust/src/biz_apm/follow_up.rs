use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::biz_apm::ConsultationChannel;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub enum VisitType {
    #[serde(rename = "FollowUp")]
    FollowUp,
    #[serde(rename = "LabResult")]
    LabResult,
    #[serde(rename = "PrecriptionRefill")]
    PrescriptionRefill,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FollowUpAppointment {
    pub parent_booking_id: String,
    pub appointment_start: i64,
    pub appointment_end: i64,
    pub visit_types: Vec<VisitType>,
    pub additional_note_to_patient: String,
    pub note_to_staff: String,
    pub consultation_channel: ConsultationChannel,
    pub consultation_fee: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "__type")]
pub enum FollowUp {
    AsNeeded,
    Appointment(FollowUpAppointment),
}
