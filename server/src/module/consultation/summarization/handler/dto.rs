//! Data Transfer Objects for summarization API endpoints.
//!
//! Contains request and response types for the HTTP layer.
//! Separated from domain models (models.rs) to maintain clean architecture.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::model::ref_data::Icd10;

use super::super::models::{FollowUpInfo, Prescription, SummarizationRecord, SummaryNote};

// ─── Request types ────────────────────────────────────────────────────────────

/// Request body for `POST /consultation/v1/summarization/draft`.
///
/// All fields are optional to support partial saves (drafts).
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SaveDraftRequest {
    pub appointment_id: String,
    pub summary_note: Option<SummaryNote>,
    pub prescription: Option<Prescription>,
    pub follow_up_info: Option<FollowUpInfo>,
}

/// Summary note fields required at submit time — no optional strings.
#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SubmitSummaryNote {
    pub present_illness: String,
    pub chief_complaint: String,
    pub diagnosis: String,
    pub recommendations: String,
    pub icd10: Vec<Icd10>,
    pub illness_duration: SubmitIllnessDuration,
    pub note_to_staff: Option<String>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SubmitIllnessDuration {
    pub value: i32,
    pub unit: String,
}

/// Request body for `POST /consultation/v1/summarization/submit`.
///
/// All fields are required for submission (validated by serde deserialization).
#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SubmitRequest {
    pub appointment_id: String,
    pub prescription_expired_at: Option<i64>,
    pub summary_note: SubmitSummaryNote,
    pub prescription: Prescription,
    pub follow_up_info: FollowUpInfo,
}

impl SubmitSummaryNote {
    /// Convert to the domain `SummaryNote` for encryption/storage.
    pub fn to_domain(&self) -> SummaryNote {
        SummaryNote {
            present_illness: Some(self.present_illness.clone()),
            chief_complaint: Some(self.chief_complaint.clone()),
            diagnosis: Some(self.diagnosis.clone()),
            recommendations: Some(self.recommendations.clone()),
            icd10: self.icd10.clone(),
            illness_duration: Some(super::super::models::IllnessDuration {
                value: Some(self.illness_duration.value),
                unit: Some(self.illness_duration.unit.clone()),
            }),
            note_to_staff: self.note_to_staff.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn submit_request_deserializes_prescription_expired_at() {
        let req: SubmitRequest = serde_json::from_value(serde_json::json!({
            "appointmentId": "APT-001",
            "prescriptionExpiredAt": 1_718_035_200,
            "summaryNote": {
                "presentIllness": "Headache for 3 days",
                "chiefComplaint": "Headache",
                "diagnosis": "Tension headache",
                "recommendations": "Rest",
                "icd10": [{ "code": "G44.2", "description": "Tension-type headache" }],
                "illnessDuration": { "value": 3, "unit": "days" },
                "noteToStaff": null
            },
            "prescription": {
                "drugAllergyInfo": { "__type": "NoDrugAllergies" },
                "prescriptionItems": { "__type": "NoPrescription" }
            },
            "followUpInfo": {
                "__type": "NoFollowUp",
                "noteToStaff": null
            }
        }))
        .unwrap();

        assert_eq!(req.prescription_expired_at, Some(1_718_035_200));
    }
}

// ─── Response types ───────────────────────────────────────────────────────────

/// Response for `GET /consultation/v1/summarization/:appointment_id`
#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "__type")]
pub enum GetSummarizationResponse {
    /// No draft exists yet - appointment is in pending record state
    #[serde(rename = "PendingRecord")]
    PendingRecord,
    /// Existing summarization record (Draft or Submitted status)
    #[serde(rename = "SummarizationRecord")]
    SummarizationRecord(SummarizationRecord),
}

/// Alias for service layer compatibility
pub type GetDraftResponse = GetSummarizationResponse;

/// Response for `POST /consultation/v1/summarization/draft`.
#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "__type")]
pub enum SaveDraftResult {
    #[serde(rename = "SaveDraftResult.Success")]
    Success,
    #[serde(rename = "SaveDraftResult.AlreadySubmitted")]
    AlreadySubmitted,
    #[serde(rename = "SaveDraftResult.Unauthorized")]
    Unauthorized,
}

/// Response for `POST /consultation/v1/summarization/submit`.
#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "__type")]
pub enum SubmitResponse {
    #[serde(rename = "SubmitResponse.Success")]
    Success,
    #[serde(rename = "SubmitResponse.TimeslotIsNotAavailable")]
    TimeslotIsNotAavailable,
    #[serde(rename = "SubmitResponse.TimeslotConflict")]
    TimeslotConflict,
    /// Summary was already submitted — informational, not an error.
    #[serde(rename = "SubmitResponse.AlreadySubmitted")]
    AlreadySubmitted,
    #[serde(rename = "SubmitResponse.Unauthorized")]
    Unauthorized,
    /// Jade (prescription) service call failed — status stays Draft, doctor can retry.
    #[serde(rename = "SubmitResponse.PrescriptionServiceError")]
    #[serde(rename_all = "camelCase")]
    PrescriptionServiceError { message: String },
    /// Consultation service call failed — status stays Draft, doctor can retry.
    #[serde(rename = "SubmitResponse.ConsultationServiceError")]
    #[serde(rename_all = "camelCase")]
    ConsultationServiceError { message: String },
}
