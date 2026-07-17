pub mod apm;
pub mod jade;

use tracing::warn;

use crate::core::error::AppResult;

pub use apm::{
    ApmAppointmentTime, ApmConsultationChannel, ApmDoctorRef, ApmDrugAllergy, ApmDurationUnit,
    ApmFollowUp, ApmFollowUpAppointment, ApmIcd10, ApmPastVisitDetail, ApmPastVisitSummaryNote,
    ApmVisitType,
};
pub use jade::{JadePolicy, JadePrescriptionItem};

use apm::{ApmClient, ApmPastVisitDetailResponse};
use jade::{JadeClient, JadePrescriptionResult};

#[derive(Debug, Clone)]
pub enum PastVisitDetailFromGateway {
    Success(PastVisitDetailBundle),
    NotFound,
    NotFulfilled,
}

#[derive(Debug, Clone)]
pub struct PastVisitDetailBundle {
    pub detail: ApmPastVisitDetail,
    pub prescription: Vec<JadePrescriptionItem>,
}

#[derive(Clone)]
pub struct PastVisitGateway {
    apm: ApmClient,
    jade: JadeClient,
}

impl PastVisitGateway {
    pub fn new(apm_base_uri: String, jade_base_uri: String, jade_policy: JadePolicy) -> Self {
        Self {
            apm: ApmClient::new(apm_base_uri),
            jade: JadeClient::new(jade_base_uri, jade_policy),
        }
    }

    pub async fn get_past_visit_detail(
        &self,
        request_id: &str,
        booking_id: &str,
    ) -> AppResult<PastVisitDetailFromGateway> {
        match self
            .apm
            .get_past_visit_detail(request_id, booking_id)
            .await?
        {
            ApmPastVisitDetailResponse::Success(detail) => {
                let prescription = self
                    .fetch_prescription(request_id, booking_id, &detail)
                    .await?;
                Ok(PastVisitDetailFromGateway::Success(PastVisitDetailBundle {
                    detail,
                    prescription,
                }))
            }
            ApmPastVisitDetailResponse::NotFound => Ok(PastVisitDetailFromGateway::NotFound),
            ApmPastVisitDetailResponse::NotFulfilled => {
                Ok(PastVisitDetailFromGateway::NotFulfilled)
            }
        }
    }

    async fn fetch_prescription(
        &self,
        request_id: &str,
        booking_id: &str,
        detail: &ApmPastVisitDetail,
    ) -> AppResult<Vec<JadePrescriptionItem>> {
        let Some(prescription_id) = detail.summary_note.prescription_id else {
            return Ok(Vec::new());
        };
        match self
            .jade
            .get_prescription_by_booking_id(request_id, booking_id)
            .await?
        {
            JadePrescriptionResult::Found(items) => Ok(items),
            JadePrescriptionResult::NotFound => {
                warn!(
                    booking_id = %booking_id,
                    prescription_id = prescription_id,
                    "APM referenced prescription_id but Jade returned NotFound; returning empty prescription list",
                );
                Ok(Vec::new())
            }
        }
    }
}
