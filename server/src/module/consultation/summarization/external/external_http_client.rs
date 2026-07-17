use async_trait::async_trait;

use crate::core::error::AppResult;

use super::super::handler::SubmitSummaryNote;
use super::super::models::{DrugAllergyInfo, FollowUpInfo, PrescriptionItem};

// ─── Save summary note result ──────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveSummaryNoteResult {
    Success,
    AlreadySubmitted,
}

// ─── Created prescription ──────────────────────────────────────────────────────

/// Result of creating a prescription in biz-jade.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatedPrescription {
    /// biz-jade prescription id, forwarded to biz-apm as `prescription_id`.
    pub id: i64,
    /// biz-jade prescription ref code, used in the ConsultationSummarized event.
    pub code: String,
}

// ─── Jade service trait ───────────────────────────────────────────────────────

#[async_trait]
pub trait JadeServiceTrait: Send + Sync {
    async fn create_prescription(
        &self,
        request_id: &str,
        appointment_id: &str,
        items: &[PrescriptionItem],
        biz_unit_id: i32,
        biz_center_id: i32,
        doctor_id: &str,
        patient_profile_id: i32,
        prescription_expired_at: Option<i64>,
        drug_allergy_info: Option<&DrugAllergyInfo>,
    ) -> AppResult<CreatedPrescription>;
}

// ─── Consultation summarization service trait ─────────────────────────────────

#[async_trait]
pub trait ConsultationSummarizationServiceTrait: Send + Sync {
    async fn save_summary_note(
        &self,
        request_id: &str,
        appointment_id: &str,
        summary_note: &SubmitSummaryNote,
        prescription_id: Option<i64>,
        follow_up_info: &FollowUpInfo,
        drug_allergy_info: Option<&DrugAllergyInfo>,
    ) -> AppResult<SaveSummaryNoteResult>;
}

// ─── Stub implementations ─────────────────────────────────────────────────────

pub struct JadeServiceStub;

#[async_trait]
impl JadeServiceTrait for JadeServiceStub {
    async fn create_prescription(
        &self,
        request_id: &str,
        appointment_id: &str,
        items: &[PrescriptionItem],
        _biz_unit_id: i32,
        _biz_center_id: i32,
        _doctor_id: &str,
        _patient_id: i32,
        _prescription_expired_at: Option<i64>,
        _drug_allergy_info: Option<&DrugAllergyInfo>,
    ) -> AppResult<CreatedPrescription> {
        tracing::debug!(
            request_id = %request_id,
            appointment_id = %appointment_id,
            item_count = items.len(),
            "JadeServiceStub::create_prescription called (no-op)"
        );
        Ok(CreatedPrescription {
            id: 0,
            code: format!("stub-prescription-{}", appointment_id),
        })
    }
}

pub struct ConsultationSummarizationServiceStub;

#[async_trait]
impl ConsultationSummarizationServiceTrait for ConsultationSummarizationServiceStub {
    async fn save_summary_note(
        &self,
        request_id: &str,
        appointment_id: &str,
        _summary_note: &SubmitSummaryNote,
        _prescription_id: Option<i64>,
        _follow_up_info: &FollowUpInfo,
        _drug_allergy_info: Option<&DrugAllergyInfo>,
    ) -> AppResult<SaveSummaryNoteResult> {
        tracing::debug!(
            request_id = %request_id,
            appointment_id = %appointment_id,
            "ConsultationSummarizationServiceStub::save_summary_note called (no-op)"
        );
        Ok(SaveSummaryNoteResult::Success)
    }
}
