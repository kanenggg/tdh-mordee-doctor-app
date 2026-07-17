//! Client for IAM gatekeeper
//! `GET /iam/v1/internal/profile/by-account/{accountId}`.
//!
//! Upstream returns a discriminated union on `__type` with a `RawJson`
//! profile. We deserialize the profile as `MorDeeUserProfileV1` (the
//! patient profile shape), since the doctor app's appointment-detail
//! screen is patient-facing. See spec:
//! docs/superpowers/specs/2026-04-09-appointment-detail-bff-design.md.

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;
use tracing::{debug, warn};

use crate::core::error::{AppError, AppResult};

/// Outcome of an IAM profile lookup.
#[derive(Debug)]
pub enum IamLookup {
    Found(MorDeeUserProfile),
    NotFound, // covers AccountNotFound and ProfileNotFound
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MorDeeUserProfile {
    #[serde(default)]
    pub first_name: Option<String>,
    #[serde(default)]
    pub last_name: Option<String>,
    #[serde(default)]
    pub gender: Option<String>,
    #[serde(default)]
    pub date_of_birth: Option<String>,
    // Other IAM profile fields (email, phoneNumber, imageUrl) are
    // intentionally not modeled — we don't need them and don't want
    // to log them.
}

/// Raw wire envelope. The `__type` discriminator carries dot-namespaced
/// names per the upstream Scala definition.
#[derive(Debug, Deserialize)]
#[serde(tag = "__type")]
enum WireEnvelope {
    #[serde(rename = "InternalGetProfileByAccountId.Result.Success")]
    Success { profile: MorDeeUserProfile },
    #[serde(rename = "InternalGetProfileByAccountId.Result.AccountNotFound")]
    AccountNotFound {
        #[allow(dead_code)]
        msg: String,
    },
    #[serde(rename = "InternalGetProfileByAccountId.Result.ProfileNotFound")]
    ProfileNotFound {
        #[allow(dead_code)]
        msg: String,
    },
    #[serde(rename = "InternalGetProfileByAccountId.Result.Error")]
    Error { msg: String },
}

#[async_trait]
pub trait IamClientTrait: Send + Sync {
    async fn get_profile_by_account(&self, account_id: i32) -> AppResult<IamLookup>;
}

#[derive(Clone)]
pub struct IamClient {
    client: Client,
    base_uri: String,
}

impl IamClient {
    pub fn new(base_uri: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .expect("failed to build IAM HTTP client"),
            base_uri,
        }
    }
}

#[async_trait]
impl IamClientTrait for IamClient {
    #[tracing::instrument(name = "iam.get_profile_by_account", skip(self), fields(account_id = account_id))]
    async fn get_profile_by_account(&self, account_id: i32) -> AppResult<IamLookup> {
        let url = format!(
            "{}/v1/internal/profile/by-account/{}",
            self.base_uri, account_id
        );
        debug!(%url, "calling IAM upstream");

        let resp = match send_with_retry(&self.client, &url).await {
            Ok(r) => r,
            Err(e) => {
                warn!(error = %e, "iam upstream failed after retry");
                return Err(AppError::UpstreamError("iam".to_string()));
            }
        };

        let envelope: WireEnvelope = resp.json().await.map_err(|e| {
            warn!(error = %e, "iam upstream returned an unexpected response body");
            AppError::UpstreamError("iam".to_string())
        })?;

        Ok(match envelope {
            WireEnvelope::Success { profile } => IamLookup::Found(profile),
            WireEnvelope::AccountNotFound { .. } | WireEnvelope::ProfileNotFound { .. } => {
                IamLookup::NotFound
            }
            WireEnvelope::Error { msg } => {
                warn!(
                    msg_len = msg.len(),
                    "iam upstream Error variant; msg body redacted to avoid PII"
                );
                return Err(AppError::UpstreamError("iam".to_string()));
            }
        })
    }
}

/// Send a GET request once, retry once on transient transport failure
/// (network error or HTTP 5xx). No retry on 4xx or successful response.
async fn send_with_retry(client: &Client, url: &str) -> Result<reqwest::Response, reqwest::Error> {
    match client.get(url).send().await {
        Ok(r) if r.status().is_server_error() => {
            warn!(status = %r.status(), %url, attempt = 1, "transport 5xx, retrying");
            client.get(url).send().await?.error_for_status()
        }
        Ok(r) if r.status().is_client_error() => {
            warn!(status = %r.status(), %url, "upstream returned client error, no retry");
            r.error_for_status()
        }
        Ok(r) => r.error_for_status(),
        Err(e) if e.is_connect() || e.is_timeout() || e.is_request() => {
            warn!(error = %e, %url, attempt = 1, "transport error, retrying");
            client.get(url).send().await?.error_for_status()
        }
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn get_profile_by_account_accepts_success_variant() {
        let upstream = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/iam/v1/internal/profile/by-account/124236"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "__type": "InternalGetProfileByAccountId.Result.Success",
                "profile": {
                    "firstName": "Bunyang",
                    "lastName": "Lopez",
                    "gender": "Female",
                    "dateOfBirth": "1957-03-22"
                }
            })))
            .mount(&upstream)
            .await;

        let client = IamClient::new(format!("{}/iam", upstream.uri()));
        let result = client.get_profile_by_account(124236).await.unwrap();

        match result {
            IamLookup::Found(profile) => {
                assert_eq!(profile.first_name.as_deref(), Some("Bunyang"));
                assert_eq!(profile.last_name.as_deref(), Some("Lopez"));
                assert_eq!(profile.gender.as_deref(), Some("Female"));
                assert_eq!(profile.date_of_birth.as_deref(), Some("1957-03-22"));
            }
            IamLookup::NotFound => panic!("expected IAM profile"),
        }
    }

    #[tokio::test]
    async fn get_profile_by_account_maps_profile_not_found_to_not_found() {
        let upstream = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/iam/v1/internal/profile/by-account/124236"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "__type": "InternalGetProfileByAccountId.Result.ProfileNotFound",
                "msg": "profile missing"
            })))
            .mount(&upstream)
            .await;

        let client = IamClient::new(format!("{}/iam", upstream.uri()));
        let result = client.get_profile_by_account(124236).await.unwrap();

        match result {
            IamLookup::NotFound => {}
            IamLookup::Found(_) => panic!("expected NotFound"),
        }
    }
}
