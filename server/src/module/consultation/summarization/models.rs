use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::model::ref_data::Icd10;

// ─── Visit type ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub enum VisitType {
    #[serde(rename = "FollowUp")]
    FollowUp,
    #[serde(rename = "LabResult")]
    LabResult,
    #[serde(rename = "PrecriptionRefill")]
    PrescriptionRefill,
}

// ─── Domain types ─────────────────────────────────────────────────────────────
// These are the consultation domain's canonical representation of summarization
// data. They use lenient Option<> fields to support partial saves (drafts).

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SummaryNote {
    pub present_illness: Option<String>,
    pub chief_complaint: Option<String>,
    pub diagnosis: Option<String>,
    pub recommendations: Option<String>,
    pub icd10: Vec<Icd10>,
    pub illness_duration: Option<IllnessDuration>,
    pub note_to_staff: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "__type")]
pub enum DrugAllergyInfo {
    #[serde(rename = "HasDrugAllergies")]
    #[serde(rename_all = "camelCase")]
    HasDrugAllergies { drug_allergies: Vec<DrugAllergy> },
    #[serde(rename = "NoDrugAllergies")]
    NoDrugAllergies,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DrugAllergy {
    pub id: i32,
    pub display_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct IllnessDuration {
    pub value: Option<i32>,
    pub unit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "__type", content = "items")]
pub enum PrescriptionOption {
    #[serde(rename = "NoPrescription")]
    NoPrescription,
    #[serde(rename = "Prescription")]
    Prescription(Vec<PrescriptionItem>),
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Prescription {
    pub drug_allergy_info: Option<DrugAllergyInfo>,
    pub prescription_items: PrescriptionOption,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PrescriptionItem {
    pub price_plan_id: i32,
    pub medicine_id: i32,
    pub medicine_name: String,
    pub dose: RefDataWithAmount<i32>,
    pub quantity: i32,
    pub route: RefData,
    pub frequency: RefData,
    pub indication: RefData,
    pub meal_instruction: RefData,
    pub duration: RefDataWithAmount<i32>,
    pub cautions: Option<String>,
    pub remark: Option<String>,
    pub note_to_patient: Option<String>,
    pub unit_price: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RefDataWithAmount<T: Serialize> {
    pub id: i32,
    pub value: T,
    pub unit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RefData {
    pub id: i32,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "__type")]
pub enum FollowUpInfo {
    #[serde(rename = "ScheduleAppointment")]
    #[serde(rename_all = "camelCase")]
    ScheduleAppointment {
        appointment_start_datetime: i64,
        appointment_end_datetime: i64,
        #[serde(default)]
        visit_types: Vec<VisitType>,
        note_to_patient: Option<String>,
        note_to_staff: Option<String>,
    },
    #[serde(rename = "NoFollowUp")]
    #[serde(rename_all = "camelCase")]
    NoFollowUp { note_to_staff: Option<String> },
}

impl FollowUpInfo {
    pub fn visit_types(&self) -> &[VisitType] {
        match self {
            FollowUpInfo::ScheduleAppointment { visit_types, .. } => visit_types,
            FollowUpInfo::NoFollowUp { .. } => &[],
        }
    }

    pub fn note_to_patient(&self) -> Option<&String> {
        match self {
            FollowUpInfo::ScheduleAppointment {
                note_to_patient, ..
            } => note_to_patient.as_ref(),
            FollowUpInfo::NoFollowUp { .. } => None,
        }
    }

    pub fn note_to_staff(&self) -> Option<&String> {
        match self {
            FollowUpInfo::ScheduleAppointment { note_to_staff, .. } => note_to_staff.as_ref(),
            FollowUpInfo::NoFollowUp { note_to_staff } => note_to_staff.as_ref(),
        }
    }

    pub fn start_datetime(&self) -> Option<i64> {
        match self {
            FollowUpInfo::ScheduleAppointment {
                appointment_start_datetime: follow_start_datetime,
                ..
            } => Some(*follow_start_datetime),
            FollowUpInfo::NoFollowUp { .. } => None,
        }
    }

    pub fn end_datetime(&self) -> Option<i64> {
        match self {
            FollowUpInfo::ScheduleAppointment {
                appointment_end_datetime: follow_end_datetime,
                ..
            } => Some(*follow_end_datetime),
            FollowUpInfo::NoFollowUp { .. } => None,
        }
    }
}

// ─── Status ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, sqlx::Type, ToSchema)]
#[sqlx(type_name = "summarization_status_enum", rename_all = "PascalCase")]
pub enum SummarizationStatus {
    Draft,
    Submitted,
}

// ─── Domain record (decrypted, what the service layer works with) ─────────────

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SummarizationRecord {
    pub appointment_id: String,
    pub status: SummarizationStatus,
    pub doctor_account_id: i32,
    // TODO:send to consultation
    pub summary_note: Option<SummaryNote>,
    // TODO:send to prescription(jade)
    pub prescription: Option<Prescription>,
    // TODO:reserve timeslot (not implement yet)
    pub follow_up_info: Option<FollowUpInfo>,
}

// ─── Encrypted Payload ────────────────────────────────────────────────────────

/// Internal struct for encrypting/decrypting the full payload.
/// This combines all sensitive data into a single encrypted blob stored in DB.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EncryptedPayload {
    pub summary_note: Option<SummaryNote>,
    pub prescription: Option<Prescription>,
    pub follow_up_info: Option<FollowUpInfo>,
}

// Note: Request/Response types (DTOs) have been moved to dto.rs
// This file contains only domain/business models.
