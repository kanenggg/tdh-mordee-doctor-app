use reqwest::Client;
use std::time::Duration;

use crate::core::error::{AppError, AppResult};

use super::handler::{GetLabResultsResult, LabResult};

#[derive(Debug, Clone)]
pub struct EhrClient {
    client: Client,
    base_uri: String,
}

impl EhrClient {
    pub fn new(base_uri: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("failed to build HTTP client for EHR"),
            base_uri,
        }
    }

    pub async fn get_lab_results(
        &self,
        request_id: &str,
        patient_account_id: i32,
        patient_profile_id: i32,
    ) -> AppResult<GetLabResultsResult> {
        let url = format!("{}/ehr/lab-result", self.base_uri);
        let resp = self
            .client
            .get(&url)
            .header("X-Request-Id", request_id)
            .header("PATIENT-ACCOUNT-ID", patient_account_id.to_string())
            .header("PATIENT-PROFILE-ID", patient_profile_id.to_string())
            .send()
            .await?;

        match resp.status() {
            s if s.is_success() => {
                let results = resp.json::<Vec<LabResult>>().await?;
                Ok(GetLabResultsResult::LabResults {
                    lab_results: results,
                })
            }
            reqwest::StatusCode::UNAUTHORIZED => Ok(GetLabResultsResult::Unauthorized),
            reqwest::StatusCode::NOT_FOUND => Ok(GetLabResultsResult::NotFound),
            s => Err(AppError::InternalError(format!(
                "EHR service returned unexpected status: {}",
                s
            ))),
        }
    }
}
