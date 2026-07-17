use std::sync::Arc;

use crate::core::error::AppResult;

use super::gateway::EhrClient;
use super::handler::GetLabResultsResult;

#[derive(Clone)]
pub struct LabResultService {
    gateway: Arc<EhrClient>,
}

impl LabResultService {
    pub fn new(gateway: Arc<EhrClient>) -> Self {
        Self { gateway }
    }

    pub async fn get_lab_results(
        &self,
        request_id: &str,
        patient_account_id: i32,
        patient_profile_id: i32,
    ) -> AppResult<GetLabResultsResult> {
        self.gateway
            .get_lab_results(request_id, patient_account_id, patient_profile_id)
            .await
    }
}
