pub mod apm;

use crate::core::error::AppResult;

pub use apm::ApmAppointment;

use apm::{ApmClient, ApmPastVisitsResponse};

#[derive(Debug, Clone)]
pub enum PastVisitsFromGateway {
    Found(Vec<ApmAppointment>),
    NotFound,
    Unauthorized,
}

#[derive(Clone)]
pub struct PastVisitGateway {
    apm: ApmClient,
}

impl PastVisitGateway {
    pub fn new(apm_base_uri: String) -> Self {
        Self {
            apm: ApmClient::new(apm_base_uri),
        }
    }

    pub async fn get_past_visits(
        &self,
        request_id: &str,
        patient_account_id: i32,
        patient_profile_id: i32,
    ) -> AppResult<PastVisitsFromGateway> {
        Ok(
            match self
                .apm
                .get_past_visits(request_id, patient_account_id, patient_profile_id)
                .await?
            {
                ApmPastVisitsResponse::Found(appointments) => {
                    PastVisitsFromGateway::Found(appointments)
                }
                ApmPastVisitsResponse::NotFound => PastVisitsFromGateway::NotFound,
                ApmPastVisitsResponse::Unauthorized => PastVisitsFromGateway::Unauthorized,
            },
        )
    }
}
