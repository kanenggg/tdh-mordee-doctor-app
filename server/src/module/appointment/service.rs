use std::sync::Arc;

use async_trait::async_trait;
use tracing::{info, warn};

use crate::core::error::AppResult;

use serde_json::Value as JsonValue;

use super::external::{
    ConsultationClientTrait, ConsultationDetail, ConsultationLookup, IamClientTrait, IamLookup,
    MorDeeUserProfile, PaymentClientTrait, PaymentDetail, PaymentLookup,
};

#[derive(Debug, Default)]
pub struct PatientEhrInfo {
    pub heigth: Option<f32>,
    pub weight: Option<f32>,
    pub bmi: Option<f32>,
}

#[derive(Debug)]
pub struct AppointmentDetail {
    pub consultation: ConsultationDetail,
    pub profile: MorDeeUserProfile,
    pub payment: Option<PaymentDetail>,
    pub ehr: PatientEhrInfo,
}

#[derive(Debug)]
pub enum AppointmentDetailResult {
    Found(AppointmentDetail),
    AppointmentNotFound,
    PatientProfileNotFound,
}

#[derive(Debug)]
pub enum PastVisitsResult {
    Found(JsonValue),
    AppointmentNotFound,
}

#[async_trait]
pub trait AppointmentServiceTrait: Send + Sync {
    async fn get_appointment_detail(&self, booking_id: &str) -> AppResult<AppointmentDetailResult>;
}

#[derive(Clone)]
pub struct AppointmentService {
    consultation: Arc<dyn ConsultationClientTrait>,
    iam: Arc<dyn IamClientTrait>,
    payment: Arc<dyn PaymentClientTrait>,
}

impl AppointmentService {
    pub fn new(
        consultation: Arc<dyn ConsultationClientTrait>,
        iam: Arc<dyn IamClientTrait>,
        payment: Arc<dyn PaymentClientTrait>,
    ) -> Self {
        Self {
            consultation,
            iam,
            payment,
        }
    }
}

#[async_trait]
impl AppointmentServiceTrait for AppointmentService {
    async fn get_appointment_detail(&self, booking_id: &str) -> AppResult<AppointmentDetailResult> {
        let consultation = match self.consultation.get_appointment(booking_id).await? {
            ConsultationLookup::Found(d) => d,
            ConsultationLookup::NotFound => {
                info!("appointment not found in consultation upstream");
                return Ok(AppointmentDetailResult::AppointmentNotFound);
            }
        };

        let patient_account_id = consultation.patient.account_id;
        let payment_tx_id = consultation.payment_tx_id;

        let iam_future = self.iam.get_profile_by_account(patient_account_id);
        let payment_future = async {
            if payment_tx_id > 0 {
                self.payment.get_payment(payment_tx_id).await.map(Some)
            } else {
                Ok(None)
            }
        };

        let (iam_result, payment_result) = tokio::try_join!(iam_future, payment_future)?;

        let profile = match iam_result {
            IamLookup::Found(p) => p,
            IamLookup::NotFound => {
                warn!(%patient_account_id, "patient profile not found in IAM");
                return Ok(AppointmentDetailResult::PatientProfileNotFound);
            }
        };

        let payment = match payment_result {
            Some(PaymentLookup::Found(d)) => Some(d),
            Some(PaymentLookup::NotFound) | None => None,
        };

        Ok(AppointmentDetailResult::Found(AppointmentDetail {
            consultation,
            profile,
            payment,
            ehr: PatientEhrInfo::default(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::appointment::external::consultation_client::{
        ConsultationAppointmentTime, ConsultationDetail, ConsultationIdentity,
        ConsultationPrescreen,
    };
    use crate::module::appointment::external::iam_client::MorDeeUserProfile;
    use serde_json::json;

    struct FixedConsultation {
        payment_tx_id: i64,
    }

    #[async_trait]
    impl ConsultationClientTrait for FixedConsultation {
        async fn get_appointment(&self, booking_id: &str) -> AppResult<ConsultationLookup> {
            Ok(ConsultationLookup::Found(ConsultationDetail {
                booking_id: booking_id.to_string(),
                appointment_time: ConsultationAppointmentTime {
                    start_time: 1645940400,
                    end_time: 1645941300,
                },
                status: "Booked".to_string(),
                booking_type: "Schedule".to_string(),
                consultation_channel: "Video".to_string(),
                patient: ConsultationIdentity {
                    account_id: 124236,
                    profile_id: 200,
                },
                doctor: ConsultationIdentity {
                    account_id: 300,
                    profile_id: 400,
                },
                prescreen: ConsultationPrescreen {
                    symptom: "rash".to_string(),
                    duration: 7,
                    duration_unit: "day".to_string(),
                    attachments: vec![],
                    allergies: vec![],
                },
                payment_tx_id: self.payment_tx_id,
                payment_tx_ref_id: "".to_string(),
            }))
        }
    }

    struct NotFoundConsultation;

    #[async_trait]
    impl ConsultationClientTrait for NotFoundConsultation {
        async fn get_appointment(&self, _booking_id: &str) -> AppResult<ConsultationLookup> {
            Ok(ConsultationLookup::NotFound)
        }
    }

    struct FixedIam;

    #[async_trait]
    impl IamClientTrait for FixedIam {
        async fn get_profile_by_account(&self, _account_id: i32) -> AppResult<IamLookup> {
            Ok(IamLookup::Found(MorDeeUserProfile {
                first_name: Some("Bunyang".to_string()),
                last_name: Some("Lopez".to_string()),
                gender: Some("Female".to_string()),
                date_of_birth: Some("1957-03-22".to_string()),
            }))
        }
    }

    struct PanicIam;

    #[async_trait]
    impl IamClientTrait for PanicIam {
        async fn get_profile_by_account(&self, account_id: i32) -> AppResult<IamLookup> {
            panic!("IAM lookup should be skipped for account_id={}", account_id);
        }
    }

    struct PanicPayment;

    #[async_trait]
    impl PaymentClientTrait for PanicPayment {
        async fn get_payment(&self, payment_tx_id: i64) -> AppResult<PaymentLookup> {
            panic!(
                "payment lookup should be skipped for sentinel payment_tx_id={}",
                payment_tx_id
            );
        }
    }
}
