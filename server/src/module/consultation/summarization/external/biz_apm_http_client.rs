use async_trait::async_trait;
use reqwest::Client;
use std::time::Duration;
use tdh_protocol::biz_apm::summary_note::{
    DrugAllergy as ProtoDrugAllergy, DurationUnit, Icd10 as ProtoIcd10, SummarizationRequest,
};
use tdh_protocol::biz_apm::SummarizationResult;
use tracing::{debug, info, warn};

use super::super::handler::SubmitSummaryNote;
use super::super::models::{DrugAllergyInfo, FollowUpInfo};
use super::external_http_client::{ConsultationSummarizationServiceTrait, SaveSummaryNoteResult};
use super::ToBizApmFollowUp;
use crate::core::error::{AppError, AppResult};

pub struct BizApmHttpClient {
    client: Client,
    base_uri: String,
}

impl BizApmHttpClient {
    pub fn new(base_uri: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("failed to build HTTP client for BizApm service"),
            base_uri,
        }
    }
}

#[async_trait]
impl ConsultationSummarizationServiceTrait for BizApmHttpClient {
    async fn save_summary_note(
        &self,
        request_id: &str,
        appointment_id: &str,
        summary_note: &SubmitSummaryNote,
        prescription_id: Option<i64>,
        follow_up_info: &FollowUpInfo,
        drug_allergy_info: Option<&DrugAllergyInfo>,
    ) -> AppResult<SaveSummaryNoteResult> {
        let url = format!("{}/v2/internal/submit-summary-note", self.base_uri);

        let icd10: Vec<ProtoIcd10> = summary_note
            .icd10
            .iter()
            .map(|i| ProtoIcd10 {
                code: i.code.clone(),
                description: i.description.clone(),
            })
            .collect();

        let illness_duration = DurationUnit {
            unit: summary_note.illness_duration.unit.clone(),
            value: summary_note.illness_duration.value,
        };

        let follow_up = Some(follow_up_info.clone()).to_biz_apm(appointment_id.to_string());

        let drug_allergies = match drug_allergy_info {
            Some(DrugAllergyInfo::HasDrugAllergies { drug_allergies }) => Some(
                drug_allergies
                    .iter()
                    .map(|a| ProtoDrugAllergy {
                        id: a.id,
                        display_name: a.display_text.clone(),
                    })
                    .collect(),
            ),
            Some(DrugAllergyInfo::NoDrugAllergies) | None => None,
        };

        let body = SummarizationRequest {
            booking_id: appointment_id.to_string(),
            chief_complaint: summary_note.chief_complaint.clone(),
            present_illness: summary_note.present_illness.clone(),
            diagnosis: summary_note.diagnosis.clone(),
            recommendations: summary_note.recommendations.clone(),
            icd10,
            illness_duration,
            prescription_id,
            note_to_staff: summary_note.note_to_staff.clone().unwrap_or_default(),
            follow_up,
            drug_allergies,
        };

        let has_drug_allergies = match drug_allergy_info {
            Some(DrugAllergyInfo::HasDrugAllergies { .. }) => true,
            Some(DrugAllergyInfo::NoDrugAllergies) | None => false,
        };

        info!(
            request_id = %request_id,
            appointment_id = %appointment_id,
            url = %url,
            "Calling BizApm service to save summary note"
        );
        debug!(
            request_id = %request_id,
            icd10_count = summary_note.icd10.len(),
            has_prescription = prescription_id.is_some(),
            has_drug_allergies,
            "Prepared BizApm summary-note request"
        );

        let resp = self
            .client
            .post(&url)
            .header("X-Request-Id", request_id)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                warn!(
                    request_id = %request_id,
                    error = %e,
                    "Failed to call BizApm summary-note service"
                );
                AppError::InternalError(format!("BizApm service request failed: {}", e))
            })?;

        let status = resp.status();
        let response_text = resp
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());

        debug!(
            request_id = %request_id,
            status = %status,
            response_bytes = response_text.len(),
            "BizApm response received"
        );

        if !status.is_success() {
            warn!(
                request_id = %request_id,
                status = %status,
                response_bytes = response_text.len(),
                "BizApm summary-note service returned error"
            );
            return Err(AppError::InternalError(format!(
                "BizApm service returned status {}",
                status
            )));
        }

        let response: SummarizationResult = serde_json::from_str(&response_text).map_err(|e| {
            warn!(
                request_id = %request_id,
                error = %e,
                response_bytes = response_text.len(),
                "Failed to deserialize BizApm response"
            );
            AppError::InternalError(format!("Failed to parse BizApm response: {}", e))
        })?;

        match response {
            SummarizationResult::Success {
                summary_note_id,
                patient_identity: _,
                biz_unit_id: _,
                biz_center_id: _,
            } => {
                info!(
                    request_id = %request_id,
                    appointment_id = %appointment_id,
                    summary_note_id,
                    "Summary note saved successfully"
                );
                Ok(SaveSummaryNoteResult::Success)
            }
            SummarizationResult::AlreadySubmitted {
                summary_note_id,
                patient_identity: _,
                biz_unit_id: _,
                biz_center_id: _,
            } => {
                info!(
                    request_id = %request_id,
                    appointment_id = %appointment_id,
                    summary_note_id,
                    "Summary note already exists"
                );
                Ok(SaveSummaryNoteResult::AlreadySubmitted)
            }
        }
    }
}
