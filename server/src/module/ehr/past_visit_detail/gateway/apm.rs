use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;

use crate::core::error::{AppError, AppResult};

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "__type")]
pub enum ApmPastVisitDetailResponse {
    Success(ApmPastVisitDetail),
    NotFound,
    NotFulfilled,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApmPastVisitDetail {
    pub booking_id: String,
    pub appointment_time: ApmAppointmentTime,
    pub consultation_channel: ApmConsultationChannel,
    pub doctor: ApmDoctorRef,
    pub summary_note: ApmPastVisitSummaryNote,
    pub follow_up: ApmFollowUp,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApmAppointmentTime {
    pub start_time: i64,
    pub end_time: i64,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ApmConsultationChannel {
    Video,
    Voice,
    Chat,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApmDoctorRef {
    pub doctor_id: i32,
    pub doctor_account_id: i32,
    pub doctor_profile_id: i32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApmPastVisitSummaryNote {
    pub present_illness: String,
    pub chief_complaint: String,
    pub diagnosis: String,
    pub recommendations: String,
    pub icd10: Vec<ApmIcd10>,
    #[serde(default)]
    pub drug_allergies: Option<Vec<ApmDrugAllergy>>,
    pub illness_duration: ApmDurationUnit,
    pub note_to_staff: String,
    #[serde(default)]
    pub prescription_id: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApmIcd10 {
    pub code: String,
    pub description: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApmDrugAllergy {
    pub id: i32,
    pub display_name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApmDurationUnit {
    pub value: i32,
    pub unit: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "__type")]
pub enum ApmFollowUp {
    AsNeeded,
    Appointment(ApmFollowUpAppointment),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApmFollowUpAppointment {
    pub parent_booking_id: String,
    pub appointment_start: i64,
    pub appointment_end: i64,
    pub visit_types: Vec<ApmVisitType>,
    pub additional_note_to_patient: String,
    pub note_to_staff: String,
    pub consultation_channel: ApmConsultationChannel,
    pub consultation_fee: f64,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub enum ApmVisitType {
    FollowUp,
    LabResult,
    PrecriptionRefill,
}

#[derive(Debug, Clone)]
pub struct ApmClient {
    client: Client,
    base_uri: String,
}

impl ApmClient {
    pub fn new(base_uri: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("failed to build HTTP client for APM"),
            base_uri,
        }
    }

    pub async fn get_past_visit_detail(
        &self,
        request_id: &str,
        booking_id: &str,
    ) -> AppResult<ApmPastVisitDetailResponse> {
        let url = format!(
            "{}/internal/v1/appointment/{}/consultation-summary",
            self.base_uri, booking_id
        );
        let resp = self
            .client
            .get(&url)
            .header("X-Request-Id", request_id)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::UpstreamError(format!(
                "APM past-visit returned {}: {}",
                status, body
            )));
        }

        let body = resp.text().await?;
        serde_json::from_str(&body).map_err(|e| {
            AppError::InternalError(format!(
                "Failed to parse APM past-visit response: {}, body: {}",
                e, body
            ))
        })
    }
}
