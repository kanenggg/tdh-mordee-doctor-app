use std::sync::Arc;

use async_trait::async_trait;
use tdh_protocol::biz_apm::consultation_event::{
    ConsultationEvent, ConsultationSummarizedEvent, PrescriptionInfo,
};
use tracing::{info, warn};

use crate::core::error::AppResult;
use crate::core::UserIdentity;
use crate::module::webhook::PubsubPublisher;

use super::cypto::SummarizationEncryptor;
use super::external::{
    ConsultationSummarizationServiceTrait, JadeServiceTrait, SaveSummaryNoteResult,
};
use super::handler::{
    GetDraftResponse, SaveDraftRequest, SaveDraftResult, SubmitRequest, SubmitResponse,
    SubmitSummaryNote,
};
use super::models::{
    EncryptedPayload, PrescriptionOption, SummarizationRecord, SummarizationStatus,
};
use super::repo::{FollowUpReservationRepo, SummarizationRepo};

#[async_trait]
pub trait SummarizationPublisher: Send + Sync {
    async fn publish_event(&self, topic: &str, event: &ConsultationEvent) -> AppResult<()>;
}

#[async_trait]
impl SummarizationPublisher for PubsubPublisher {
    async fn publish_event(&self, topic: &str, event: &ConsultationEvent) -> AppResult<()> {
        PubsubPublisher::publish(self, topic, event).await?;
        Ok(())
    }
}

pub struct SummarizationService {
    repo: Arc<dyn SummarizationRepo>,
    encryptor: Arc<SummarizationEncryptor>,
    jade_service: Arc<dyn JadeServiceTrait>,
    consultation_service: Arc<dyn ConsultationSummarizationServiceTrait>,
    publisher: Arc<dyn SummarizationPublisher>,
    consultations_topic: String,
    follow_up_repo: Arc<dyn FollowUpReservationRepo>,
}

impl SummarizationService {
    pub fn new(
        repo: Arc<dyn SummarizationRepo>,
        encryptor: Arc<SummarizationEncryptor>,
        jade_service: Arc<dyn JadeServiceTrait>,
        consultation_service: Arc<dyn ConsultationSummarizationServiceTrait>,
        publisher: Arc<dyn SummarizationPublisher>,
        consultations_topic: String,
        follow_up_repo: Arc<dyn FollowUpReservationRepo>,
    ) -> Self {
        Self {
            repo,
            encryptor,
            jade_service,
            consultation_service,
            publisher,
            consultations_topic,
            follow_up_repo,
        }
    }

    pub async fn get_draft(
        &self,
        user_id: &UserIdentity,
        appointment_id: &str,
    ) -> AppResult<GetDraftResponse> {
        let maybe_record = self.repo.get_summary(user_id, appointment_id).await?;

        match maybe_record {
            None => {
                info!(
                    appointment_id,
                    "No summarization record found, returning PendingRecord"
                );
                Ok(GetDraftResponse::PendingRecord)
            }
            Some(raw) => {
                if raw.doctor_account_id != user_id.account_id {
                    warn!(
                        appointment_id,
                        requested_by = user_id.account_id,
                        owner = raw.doctor_account_id,
                        "Unauthorized access attempt to summarization record"
                    );
                    return Ok(GetDraftResponse::PendingRecord);
                }

                let (summary_note, prescription, follow_up_info) =
                    if let Some(encrypted) = &raw.summary_note_encrypted {
                        let payload: EncryptedPayload = self.encryptor.decrypt(encrypted)?;
                        (
                            payload.summary_note,
                            payload.prescription,
                            payload.follow_up_info,
                        )
                    } else {
                        (None, None, None)
                    };

                Ok(GetDraftResponse::SummarizationRecord(SummarizationRecord {
                    appointment_id: raw.appointment_id,
                    status: raw.status,
                    doctor_account_id: raw.doctor_account_id,
                    summary_note,
                    prescription,
                    follow_up_info,
                }))
            }
        }
    }

    pub async fn save_draft(
        &self,
        user_id: &UserIdentity,
        req: SaveDraftRequest,
    ) -> AppResult<SaveDraftResult> {
        let appointment_id = &req.appointment_id;

        if let Some(existing) = self.repo.get_summary(user_id, appointment_id).await? {
            if existing.doctor_account_id != user_id.account_id {
                warn!(
                    appointment_id,
                    requested_by = user_id.account_id,
                    owner = existing.doctor_account_id,
                    "Unauthorized save_draft attempt"
                );
                return Ok(SaveDraftResult::Unauthorized);
            }

            if existing.status == SummarizationStatus::Submitted {
                info!(
                    appointment_id,
                    "Attempted to save draft on already submitted record"
                );
                return Ok(SaveDraftResult::AlreadySubmitted);
            }
        }

        let payload = EncryptedPayload {
            summary_note: req.summary_note,
            prescription: req.prescription,
            follow_up_info: req.follow_up_info,
        };
        let encrypted = self.encryptor.encrypt(&payload)?;

        self.repo
            .save_draft(
                appointment_id,
                user_id.account_id,
                user_id.user_profile_id,
                &encrypted,
            )
            .await?;

        info!(
            appointment_id,
            user_id.account_id, "Draft saved successfully"
        );
        Ok(SaveDraftResult::Success)
    }

    pub async fn save_and_submit(
        &self,
        request_id: &str,
        user_id: &UserIdentity,
        req: SubmitRequest,
    ) -> AppResult<SubmitResponse> {
        let appointment_id = &req.appointment_id;

        if let Some(existing) = self.repo.get_summary(user_id, appointment_id).await? {
            if existing.doctor_account_id != user_id.account_id {
                warn!(
                    appointment_id,
                    request_id,
                    requested_by = user_id.account_id,
                    owner = existing.doctor_account_id,
                    "Unauthorized submit attempt"
                );
                return Ok(SubmitResponse::Unauthorized);
            }

            if existing.status == SummarizationStatus::Submitted {
                info!(appointment_id, request_id, "Record already submitted");
                return Ok(SubmitResponse::AlreadySubmitted);
            }
        }

        let mut prescription_ref: Option<String> = None;
        let mut prescription_id: Option<i64> = None;

        let prescription_items = match &req.prescription.prescription_items {
            PrescriptionOption::Prescription(items) => items.as_slice(),
            PrescriptionOption::NoPrescription => &[],
        };

        if !prescription_items.is_empty() {
            match self
                .jade_service
                .create_prescription(
                    request_id,
                    appointment_id,
                    prescription_items,
                    0,
                    0,
                    "",
                    0,
                    req.prescription_expired_at,
                    req.prescription.drug_allergy_info.as_ref(),
                    // req.biz_unit_id,
                    // req.biz_center_id,
                    // req.doctor_id,
                    // req.patient_id,
                )
                .await
            {
                Ok(created) => {
                    prescription_id = Some(created.id);
                    prescription_ref = Some(created.code);
                }
                Err(e) => {
                    warn!(
                        appointment_id,
                        request_id,
                        error = %e,
                        "Jade prescription service call failed"
                    );
                    return Ok(SubmitResponse::PrescriptionServiceError {
                        message: e.to_string(),
                    });
                }
            }
        }

        let result = self
            .consultation_service
            .save_summary_note(
                request_id,
                appointment_id,
                &req.summary_note,
                prescription_id,
                &req.follow_up_info,
                req.prescription.drug_allergy_info.as_ref(),
            )
            .await;

        match result {
            Ok(SaveSummaryNoteResult::AlreadySubmitted) => {
                info!(
                    appointment_id,
                    request_id, "Summary note already submitted to biz-apm"
                );
                return Ok(SubmitResponse::AlreadySubmitted);
            }
            Ok(SaveSummaryNoteResult::Success) => {
                info!(
                    appointment_id,
                    request_id, "Summary note saved successfully to biz-apm"
                );
            }
            Err(e) => {
                warn!(
                    appointment_id,
                    request_id,
                    error = %e,
                    "Consultation service save_summary_note failed"
                );
                return Ok(SubmitResponse::ConsultationServiceError {
                    message: e.to_string(),
                });
            }
        }

        let domain_summary_note = req.summary_note.to_domain();

        let payload = EncryptedPayload {
            summary_note: Some(domain_summary_note),
            prescription: Some(req.prescription.clone()),
            follow_up_info: Some(req.follow_up_info.clone()),
        };
        let encrypted = self.encryptor.encrypt(&payload)?;

        self.repo
            .save_and_submit(
                appointment_id,
                user_id.account_id,
                user_id.user_profile_id,
                &encrypted,
            )
            .await?;

        if let Err(e) = self
            .publish_consultation_summarized_event(
                appointment_id,
                user_id.account_id,
                &req.summary_note,
                prescription_ref.as_deref(),
            )
            .await
        {
            warn!(
                appointment_id,
                request_id,
                error = %e,
                "Failed to publish ConsultationSummarized event"
            );
        }

        info!(
            appointment_id,
            request_id, user_id.account_id, "Summarization submitted successfully"
        );
        Ok(SubmitResponse::Success)
    }

    async fn publish_consultation_summarized_event(
        &self,
        booking_id: &str,
        doctor_id: i32,
        summary_note: &SubmitSummaryNote,
        prescription_ref: Option<&str>,
    ) -> AppResult<()> {
        let doctor_note = format_doctor_note(summary_note);

        let prescription_info = build_prescription_info(prescription_ref);

        let event = ConsultationEvent::ConsultationSummarized(ConsultationSummarizedEvent {
            booking_id: booking_id.to_string(),
            patient_identity: tdh_protocol::biz_apm::PatientIdentity {
                account_id: 0,
                user_profile_id: 0,
                tenant_id: 1,
                oidc_user_id: None,
            },
            doctor_id,
            doctor_note,
            prescription_info,
            summarized_at: jiff::Timestamp::now().as_second(),
        });

        self.publisher
            .publish_event(&self.consultations_topic, &event)
            .await?;
        Ok(())
    }
}

fn format_doctor_note(note: &SubmitSummaryNote) -> String {
    [
        &note.chief_complaint,
        &note.present_illness,
        &note.diagnosis,
        &note.recommendations,
    ]
    .iter()
    .filter(|s| !s.is_empty())
    .cloned()
    .cloned()
    .collect::<Vec<_>>()
    .join("\n\n")
}

fn build_prescription_info(prescription_ref: Option<&str>) -> PrescriptionInfo {
    match prescription_ref {
        Some(ref_code) => PrescriptionInfo {
            prescription_refcode: ref_code.to_string(),
            medicine_items: vec![],
            expire_at: jiff::Timestamp::now().as_second() + 86400 * 7,
        },
        None => PrescriptionInfo {
            prescription_refcode: String::new(),
            medicine_items: vec![],
            expire_at: 0,
        },
    }
}
