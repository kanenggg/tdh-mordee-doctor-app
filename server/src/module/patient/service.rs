use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;
use tracing::{debug, warn};

#[derive(Debug, Deserialize)]
struct PatientProfileResponse {
    #[serde(rename = "__type")]
    #[allow(dead_code)]
    response_type: String,
    profile: PatientProfile,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct PatientProfile {
    #[serde(default)]
    first_name: Option<String>,
    #[serde(default)]
    last_name: Option<String>,
    #[serde(default)]
    gender: Option<String>,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    phone_number: Option<String>,
    #[serde(default)]
    image_url: Option<String>,
    #[serde(default)]
    date_of_birth: Option<String>,
}

#[derive(Clone)]
pub struct PatientService {
    client: Client,
    iam_gatekeeper_base_uri: String,
}

impl PatientService {
    pub fn new(iam_gatekeeper_base_uri: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .expect("failed to build HTTP client"),
            iam_gatekeeper_base_uri,
        }
    }

    /// Fetch patient display name from IAM gatekeeper.
    /// Falls back to "Patient #<account_id>" on any error.
    pub async fn get_patient_name(&self, patient_account_id: i32) -> String {
        match self.fetch_patient_name(patient_account_id).await {
            Ok(name) if !name.trim().is_empty() => name,
            Ok(_) => {
                debug!(patient_account_id, "Patient name was empty, using fallback");
                fallback_name(patient_account_id)
            }
            Err(e) => {
                warn!(
                    patient_account_id,
                    error = %e,
                    "Failed to fetch patient name, using fallback"
                );
                fallback_name(patient_account_id)
            }
        }
    }

    async fn fetch_patient_name(&self, patient_account_id: i32) -> Result<String, reqwest::Error> {
        let url = format!(
            "{}/v1/internal/profile/by-account/{}",
            self.iam_gatekeeper_base_uri, patient_account_id
        );

        debug!(url = %url, "Fetching patient name");

        let resp = self.client.get(&url).send().await?.error_for_status()?;
        let response: PatientProfileResponse = resp.json().await?;
        let profile = response.profile;

        let full_name = match (profile.first_name, profile.last_name) {
            (Some(first), Some(last)) => format!("{} {}", first.trim(), last.trim()),
            (Some(first), None) => first.trim().to_string(),
            (None, Some(last)) => last.trim().to_string(),
            (None, None) => String::new(),
        };

        Ok(full_name)
    }
}

fn fallback_name(patient_account_id: i32) -> String {
    format!("Patient #{}", patient_account_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn test_fallback_name() {
        assert_eq!(fallback_name(123), "Patient #123");
    }

    #[tokio::test]
    async fn get_patient_name_fetches_profile_from_iam_gatekeeper() {
        let upstream = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/internal/profile/by-account/123"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "__type": "InternalGetProfileByAccountId.Result.Success",
                "profile": {
                    "firstName": "Jane",
                    "lastName": "Patient"
                }
            })))
            .expect(1)
            .mount(&upstream)
            .await;

        let svc = PatientService::new(upstream.uri());

        assert_eq!(svc.get_patient_name(123).await, "Jane Patient");
    }
}
