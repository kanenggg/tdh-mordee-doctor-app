use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, warn};

use crate::core::error::{AppError, AppResult};
use crate::core::gcs_signed_url::GcsSignedUrlGenerator;
use crate::module::appointment::external::{ConsultationClientTrait, ConsultationLookup};

/// Public result of an eKYC lookup against the eagle service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EkycInfo {
    Available {
        document_url: String,
        liveness_url: String,
        full_name: String,
        birth_date: String,
        gender: String,
    },
    NotAvailable,
}

/// Public detail returned when eKYC is found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EkycDetail {
    pub document_image_url: String,
    pub liveness_image_url: String,
    pub full_name: String,
    pub birth_date: String,
    pub gender: String,
}

/// Result of eKYC lookup by booking ID.
#[derive(Debug)]
pub enum EkycResult {
    Found(EkycDetail),
    AppointmentNotFound,
    EkycNotAvailable,
}

#[async_trait]
pub trait EkycServiceTrait: Send + Sync {
    async fn get_ekyc_by_booking_id(&self, booking_id: &str) -> AppResult<EkycResult>;
}

#[derive(Clone)]
pub struct EkycService {
    consultation: Arc<dyn ConsultationClientTrait>,
    ekyc: Arc<EkycClient>,
    gcp: Arc<dyn GcsSignedUrlGenerator>,
}

impl EkycService {
    pub fn new(
        consultation: Arc<dyn ConsultationClientTrait>,
        ekyc: Arc<EkycClient>,
        gcp: Arc<dyn GcsSignedUrlGenerator>,
    ) -> Self {
        Self {
            consultation,
            ekyc,
            gcp,
        }
    }
}

#[async_trait]
impl EkycServiceTrait for EkycService {
    async fn get_ekyc_by_booking_id(&self, booking_id: &str) -> AppResult<EkycResult> {
        let consultation = match self.consultation.get_appointment(booking_id).await? {
            ConsultationLookup::Found(detail) => detail,
            ConsultationLookup::NotFound => {
                return Ok(EkycResult::AppointmentNotFound);
            }
        };

        let patient_account_id = consultation.patient.account_id;
        let ekyc_info = self.ekyc.fetch_by_account_id(patient_account_id).await?;

        let result = match ekyc_info {
            EkycInfo::Available {
                document_url,
                liveness_url,
                full_name,
                birth_date,
                gender,
            } => {
                let document_image_url = self
                    .gcp
                    .generate_signed_url(&document_url)
                    .await
                    .map_err(|e| {
                        AppError::UpstreamError(format!("gcs signed url generation failed: {e}"))
                    })?;
                let liveness_image_url = self
                    .gcp
                    .generate_signed_url(&liveness_url)
                    .await
                    .map_err(|e| {
                        AppError::UpstreamError(format!("gcs signed url generation failed: {e}"))
                    })?;

                EkycResult::Found(EkycDetail {
                    document_image_url,
                    liveness_image_url,
                    full_name,
                    birth_date,
                    gender,
                })
            }
            EkycInfo::NotAvailable => EkycResult::EkycNotAvailable,
        };

        Ok(match result {
            EkycResult::Found(detail) => EkycResult::Found(detail),
            EkycResult::AppointmentNotFound => EkycResult::AppointmentNotFound,
            EkycResult::EkycNotAvailable => EkycResult::EkycNotAvailable,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "__type", rename_all = "camelCase")]
enum EagleKycResponse {
    #[serde(rename_all = "camelCase")]
    ValidKycUserInfo {
        ekyc_session_result: EagleSessionResult,
    },
    NoKycUserInfo,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EagleSessionResult {
    #[serde(default)]
    document_image_url: String,
    #[serde(default)]
    selfie_image_url: String,
    #[serde(default)]
    first_name: String,
    #[serde(default)]
    last_name: String,
    #[serde(default)]
    birth_date: String,
    #[serde(default)]
    gender: String,
}

#[derive(Clone)]
pub struct EkycClient {
    client: Client,
    eagle_base_uri: String,
}

impl EkycClient {
    pub fn new(eagle_base_uri: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .expect("failed to build HTTP client"),
            eagle_base_uri,
        }
    }

    /// Fetch eKYC info for a patient account from the eagle service.
    pub async fn fetch_by_account_id(&self, patient_account_id: i32) -> AppResult<EkycInfo> {
        let url = format!(
            "{}/internal/v1/user/{}",
            self.eagle_base_uri, patient_account_id
        );

        debug!(url = %url, patient_account_id, "Fetching eKYC info from eagle");

        let resp = self.client.get(&url).send().await.map_err(|e| {
            warn!(error = %e, "eagle transport error");
            AppError::UpstreamError(format!("eagle transport error: {}", e))
        })?;

        let status = resp.status();
        if !status.is_success() {
            warn!(%status, "eagle returned non-success status");
            return Err(AppError::UpstreamError(format!(
                "eagle returned status {}",
                status
            )));
        }

        let parsed: EagleKycResponse = resp.json().await.map_err(|e| {
            warn!(error = %e, "failed to decode eagle response");
            AppError::UpstreamError(format!("eagle decode error: {}", e))
        })?;

        Ok(match parsed {
            EagleKycResponse::ValidKycUserInfo {
                ekyc_session_result,
            } => {
                let full_name = format!(
                    "{} {}",
                    ekyc_session_result.first_name.trim(),
                    ekyc_session_result.last_name.trim()
                )
                .trim()
                .to_string();
                EkycInfo::Available {
                    document_url: ekyc_session_result.document_image_url,
                    liveness_url: ekyc_session_result.selfie_image_url,
                    full_name,
                    birth_date: ekyc_session_result.birth_date,
                    gender: ekyc_session_result.gender,
                }
            }
            EagleKycResponse::NoKycUserInfo => EkycInfo::NotAvailable,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_valid_kyc_user_info() {
        let raw = serde_json::json!({
            "__type": "validKycUserInfo",
            "ekycSessionResult": {
                "documentImageUrl": "https://example.com/doc.png",
                "selfieImageUrl": "https://example.com/selfie.png"
            }
        });
        let parsed: EagleKycResponse = serde_json::from_value(raw).unwrap();
        match parsed {
            EagleKycResponse::ValidKycUserInfo {
                ekyc_session_result,
            } => {
                assert_eq!(
                    ekyc_session_result.document_image_url,
                    "https://example.com/doc.png"
                );
                assert_eq!(
                    ekyc_session_result.selfie_image_url,
                    "https://example.com/selfie.png"
                );
            }
            _ => panic!("expected ValidKycUserInfo"),
        }
    }

    #[test]
    fn deserializes_no_kyc_user_info() {
        let raw = serde_json::json!({ "__type": "noKycUserInfo" });
        let parsed: EagleKycResponse = serde_json::from_value(raw).unwrap();
        assert!(matches!(parsed, EagleKycResponse::NoKycUserInfo));
    }
}
