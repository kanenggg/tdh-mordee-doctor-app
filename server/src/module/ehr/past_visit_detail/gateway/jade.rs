use rand::Rng;
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use std::time::Duration;
use tokio::time::sleep;

use crate::core::error::{AppError, AppResult};

#[derive(Debug, Clone)]
pub struct JadePrescriptionItem {
    pub med_id: i32,
    pub name: String,
    pub quantity: i32,
    pub unit: String,
    pub dosage_instructions: String,
}

#[derive(Debug, Clone)]
pub enum JadePrescriptionResult {
    Found(Vec<JadePrescriptionItem>),
    NotFound,
}

#[derive(Debug, Clone, Copy)]
pub struct JadePolicy {
    pub max_attempts: u32,
    pub base_backoff_ms: u64,
    pub max_backoff_ms: u64,
    pub per_attempt_timeout_secs: u64,
}

impl Default for JadePolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_backoff_ms: 100,
            max_backoff_ms: 500,
            per_attempt_timeout_secs: 3,
        }
    }
}

#[derive(Debug, Clone)]
pub struct JadeClient {
    client: Client,
    base_uri: String,
    policy: JadePolicy,
}

impl JadeClient {
    pub fn new(base_uri: String, policy: JadePolicy) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(policy.per_attempt_timeout_secs))
            .build()
            .expect("failed to build HTTP client for Jade");
        Self {
            client,
            base_uri,
            policy,
        }
    }

    pub async fn get_prescription_by_booking_id(
        &self,
        request_id: &str,
        booking_id: &str,
    ) -> AppResult<JadePrescriptionResult> {
        let url = format!("{}/prescription/{}", self.base_uri, booking_id);
        let mut last_err: Option<CallError> = None;
        for attempt in 1..=self.policy.max_attempts {
            match self.call_once(&url, request_id).await {
                Ok(result) => return Ok(result),
                Err(CallError::NonRetryable(e)) => return Err(e),
                Err(retryable) => {
                    if attempt == self.policy.max_attempts {
                        return Err(retryable.into_app_error());
                    }
                    last_err = Some(retryable);
                    sleep(Duration::from_millis(self.backoff_ms(attempt))).await;
                }
            }
        }
        Err(last_err
            .map(CallError::into_app_error)
            .unwrap_or_else(|| AppError::UpstreamError("jade: exhausted retries".into())))
    }

    async fn call_once(
        &self,
        url: &str,
        request_id: &str,
    ) -> Result<JadePrescriptionResult, CallError> {
        let resp = self
            .client
            .get(url)
            .header("X-Request-Id", request_id)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() || e.is_connect() {
                    CallError::RetryableTransport(e)
                } else {
                    CallError::NonRetryable(AppError::ReqwestError(e))
                }
            })?;

        let status = resp.status();
        if status.is_server_error() {
            let body = resp.text().await.unwrap_or_default();
            return Err(CallError::RetryableServer { status, body });
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(CallError::NonRetryable(AppError::UpstreamError(format!(
                "Jade prescription returned {}: {}",
                status, body
            ))));
        }

        let body = resp
            .text()
            .await
            .map_err(|e| CallError::NonRetryable(AppError::ReqwestError(e)))?;
        let wire: WirePrescriptionResponse = serde_json::from_str(&body).map_err(|e| {
            CallError::NonRetryable(AppError::InternalError(format!(
                "Failed to parse Jade prescription response: {}, body: {}",
                e, body
            )))
        })?;
        Ok(match wire {
            WirePrescriptionResponse::Prescription { items } => {
                JadePrescriptionResult::Found(items.into_iter().map(map_item).collect())
            }
            WirePrescriptionResponse::NotFound => JadePrescriptionResult::NotFound,
        })
    }

    fn backoff_ms(&self, attempt: u32) -> u64 {
        let shift = (attempt - 1).min(20);
        let exp = self.policy.base_backoff_ms.saturating_mul(1u64 << shift);
        let cap = exp.min(self.policy.max_backoff_ms);
        if cap == 0 {
            0
        } else {
            rand::rng().random_range(0..=cap)
        }
    }
}

enum CallError {
    RetryableTransport(reqwest::Error),
    RetryableServer { status: StatusCode, body: String },
    NonRetryable(AppError),
}

impl CallError {
    fn into_app_error(self) -> AppError {
        match self {
            Self::RetryableTransport(e) => AppError::ReqwestError(e),
            Self::RetryableServer { status, body } => {
                AppError::UpstreamError(format!("Jade prescription returned {}: {}", status, body))
            }
            Self::NonRetryable(e) => e,
        }
    }
}

fn map_item(w: WirePrescriptionItem) -> JadePrescriptionItem {
    JadePrescriptionItem {
        med_id: w.medicine_id,
        name: w.medicine_name,
        quantity: w.dosage,
        unit: w.container.map(|r| r.description).unwrap_or_default(),
        dosage_instructions: w
            .meal_instruction
            .map(|r| r.description)
            .unwrap_or_default(),
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "__type")]
enum WirePrescriptionResponse {
    Prescription {
        #[serde(default)]
        items: Vec<WirePrescriptionItem>,
    },
    NotFound,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WirePrescriptionItem {
    medicine_id: i32,
    medicine_name: String,
    dosage: i32,
    #[serde(default)]
    container: Option<WireRefData>,
    #[serde(default)]
    meal_instruction: Option<WireRefData>,
}

#[derive(Debug, Deserialize)]
struct WireRefData {
    #[allow(dead_code)]
    id: i32,
    description: String,
}
