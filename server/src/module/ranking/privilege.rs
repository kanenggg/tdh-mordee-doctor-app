use async_trait::async_trait;
use reqwest::Client;
use std::time::Duration;
use tracing::warn;

use super::models::{PrivilegeBenefit, PrivilegeBenefitApiResponse};

#[async_trait]
pub trait PrivilegeServiceTrait: Send + Sync {
    async fn get_benefits(&self, specialty_id: i32) -> Vec<PrivilegeBenefit>;
}

#[derive(Clone)]
pub struct PrivilegeService {
    client: Client,
    base_uri: String,
}

impl PrivilegeService {
    pub fn new(base_uri: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .expect("failed to build privilege HTTP client"),
            base_uri,
        }
    }
}

#[async_trait]
impl PrivilegeServiceTrait for PrivilegeService {
    async fn get_benefits(&self, specialty_id: i32) -> Vec<PrivilegeBenefit> {
        let url = format!(
            "{}/internal/v1/benefit/list?specialtyId={}",
            self.base_uri, specialty_id
        );

        match self.client.get(&url).send().await {
            Ok(resp) => {
                let status = resp.status();
                if !status.is_success() {
                    warn!(specialty_id, %status, "Privilege service returned non-success status");
                    return vec![];
                }

                match resp.json::<PrivilegeBenefitApiResponse>().await {
                    Ok(data) => data.privilege_benefits,
                    Err(e) => {
                        warn!(specialty_id, error = %e, "Failed to parse privilege response");
                        vec![]
                    }
                }
            }
            Err(e) => {
                warn!(specialty_id, error = %e, "Failed to fetch privileges");
                vec![]
            }
        }
    }
}
