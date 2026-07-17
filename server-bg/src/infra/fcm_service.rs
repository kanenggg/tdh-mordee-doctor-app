//! FCM Service for sending push notifications via Firebase Cloud Messaging API v1.
//!
//! Uses Application Default Credentials (ADC) via [`GcpTokenProvider`] for authentication,
//! which works transparently with GKE Workload Identity, Compute Engine metadata server,
//! and `GOOGLE_APPLICATION_CREDENTIALS` for local development.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration as StdDuration;
use tokio::time::Duration;
use tracing::{debug, error, info, instrument, warn};

use crate::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig};
use crate::config::{FcmConfig, RetryConfig};
use common::core::error::{AppError, AppResult};
use common::core::GcpTokenProvider;

#[derive(Debug, Clone, Serialize)]
struct FcmRequest {
    pub message: FcmMessage,
}

#[derive(Debug, Clone, Serialize)]
struct FcmMessage {
    pub token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notification: Option<FcmNotification>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize)]
struct FcmNotification {
    pub title: String,
    pub body: String,
}

#[derive(Debug, Deserialize)]
struct FcmErrorResponse {
    #[serde(default)]
    pub error: FcmErrorDetail,
}

#[derive(Debug, Deserialize, Default)]
struct FcmErrorDetail {
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum FcmError {
    #[error("Invalid FCM token: {0}")]
    InvalidToken(String),

    #[error("FCM authentication failed: {0}")]
    Unauthorized(String),

    #[error("FCM server error: {0}")]
    ServerError(String),

    #[error("FCM rate limited: {0}")]
    RateLimited(String),

    #[error("FCM quota exceeded: {0}")]
    QuotaExceeded(String),

    #[error("FCM message too large: {0}")]
    MessageTooLarge(String),

    #[error("FCM internal error: {0}")]
    Internal(String),

    #[error("FCM error: {0}")]
    Other(String),
}

impl FcmError {
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            FcmError::ServerError(_)
                | FcmError::RateLimited(_)
                | FcmError::QuotaExceeded(_)
                | FcmError::Internal(_)
        )
    }
}

impl From<FcmError> for AppError {
    fn from(err: FcmError) -> Self {
        match err {
            FcmError::InvalidToken(msg) => AppError::InvalidToken(msg),
            FcmError::Unauthorized(_msg) => AppError::Unauthorized,
            FcmError::ServerError(msg) => AppError::FcmError(msg),
            FcmError::RateLimited(msg) => AppError::FcmError(msg),
            FcmError::QuotaExceeded(msg) => AppError::FcmError(msg),
            FcmError::MessageTooLarge(msg) => AppError::FcmError(msg),
            FcmError::Internal(msg) => AppError::FcmError(msg),
            FcmError::Other(msg) => AppError::FcmError(msg),
        }
    }
}

#[derive(Clone)]
pub struct FcmService {
    http_client: reqwest::Client,
    config: Arc<FcmConfig>,
    token_provider: Arc<GcpTokenProvider>,
    retry_config: Arc<RetryConfig>,
    circuit_breaker: Option<CircuitBreaker>,
}

impl FcmService {
    pub fn new(
        config: Arc<FcmConfig>,
        token_provider: Arc<GcpTokenProvider>,
        retry_config: Arc<RetryConfig>,
    ) -> Self {
        // Configure HTTP client with connection pooling and timeouts optimized for high TPS
        let http_client = reqwest::Client::builder()
            .pool_max_idle_per_host(100) // Max idle connections per host
            .pool_idle_timeout(Duration::from_secs(90))
            .connect_timeout(StdDuration::from_secs(10))
            .timeout(StdDuration::from_secs(30))
            .tcp_keepalive(StdDuration::from_secs(60))
            .build()
            .expect("Failed to build HTTP client");

        let circuit_breaker = if config.circuit_breaker.enabled {
            warn!(
                failure_threshold = config.circuit_breaker.failure_threshold,
                success_threshold = config.circuit_breaker.success_threshold,
                open_timeout_secs = config.circuit_breaker.open_timeout_secs,
                "FCM circuit breaker is ENABLED — ensure this has been load-tested"
            );
            let cb_config = CircuitBreakerConfig {
                failure_threshold: config.circuit_breaker.failure_threshold,
                success_threshold: config.circuit_breaker.success_threshold,
                open_timeout: StdDuration::from_secs(config.circuit_breaker.open_timeout_secs),
            };
            Some(CircuitBreaker::new(cb_config))
        } else {
            info!("FCM circuit breaker is disabled");
            None
        };

        Self {
            http_client,
            config,
            token_provider,
            retry_config,
            circuit_breaker,
        }
    }

    fn mask_token(token: &str) -> String {
        let len = token.len();
        if len <= 8 {
            "***".to_string()
        } else {
            format!("{}...{}", &token[..4], &token[len - 4..])
        }
    }

    #[instrument(
        skip(self, token, data),
        fields(
            token_hint = %Self::mask_token(token),
            title,
            data_keys = data.keys().count(),
        )
    )]
    pub async fn send_notification(
        &self,
        token: &str,
        title: &str,
        body: &str,
        data: HashMap<String, String>,
    ) -> Result<(), FcmError> {
        let start = std::time::Instant::now();

        info!(
            "Sending FCM notification to token {}: title='{}'",
            Self::mask_token(token),
            title,
        );

        let access_token = self
            .get_access_token()
            .await
            .map_err(|e| FcmError::Other(format!("Failed to get access token: {}", e)))?;

        let fcm_request = FcmRequest {
            message: FcmMessage {
                token: token.to_string(),
                notification: Some(FcmNotification {
                    title: title.to_string(),
                    body: body.to_string(),
                }),
                data: if data.is_empty() { None } else { Some(data) },
            },
        };

        let url = format!(
            "{}/{}/messages:send",
            self.config.api_v1, self.config.project_id
        );

        debug!("Sending FCM request to: {}", url);

        let response = self
            .http_client
            .post(&url)
            .bearer_auth(&access_token)
            .json(&fcm_request)
            .send()
            .await
            .map_err(|e| FcmError::Other(format!("HTTP request failed: {}", e)))?;

        let status = response.status();

        if status.is_success() {
            let latency = start.elapsed();
            debug!(
                latency_ms = latency.as_millis(),
                "FCM notification sent successfully"
            );
            tracing::event!(
                tracing::Level::INFO,
                latency_ms = latency.as_millis(),
                status_code = status.as_u16(),
                "fcm.send_success"
            );
            return Ok(());
        }

        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Failed to read error response".to_string());

        let sanitized_error = if error_text.len() > 500 {
            format!("{}...(truncated)", &error_text[..500])
        } else {
            error_text.clone()
        };

        let latency = start.elapsed();
        error!(
            latency_ms = latency.as_millis(),
            status_code = status.as_u16(),
            "FCM request failed with status {}: {}",
            status,
            sanitized_error
        );
        tracing::event!(
            tracing::Level::ERROR,
            latency_ms = latency.as_millis(),
            status_code = status.as_u16(),
            "fcm.send_error"
        );

        if let Ok(error_response) = serde_json::from_str::<FcmErrorResponse>(&error_text) {
            return self.parse_fcm_error(status.as_u16(), error_response);
        }

        self.classify_error_by_status(status.as_u16(), &error_text)
    }

    #[instrument(
        skip(self, token, data),
        fields(
            token_hint = %Self::mask_token(token),
            title,
            max_retries,
        )
    )]
    pub async fn send_notification_with_retry(
        &self,
        token: &str,
        title: &str,
        body: &str,
        data: HashMap<String, String>,
        max_retries: u32,
    ) -> Result<(), FcmError> {
        // Check circuit breaker
        if let Some(ref cb) = self.circuit_breaker {
            if !cb.allow_request() {
                error!("Circuit breaker is OPEN, rejecting FCM send request");
                tracing::event!(
                    tracing::Level::WARN,
                    state = "open",
                    "fcm.circuit_breaker_reject"
                );
                return Err(FcmError::Internal("Circuit breaker is open".to_string()));
            }
        }

        tracing::event!(tracing::Level::INFO, "fcm.send_attempt");

        let mut last_error: Option<FcmError> = None;
        let start_time = std::time::Instant::now();
        let max_elapsed_time = StdDuration::from_millis(self.retry_config.max_elapsed_time_ms);

        for attempt in 0..=max_retries {
            // Check elapsed time deadline
            if start_time.elapsed() > max_elapsed_time {
                error!(
                    "FCM retry exceeded max elapsed time {:?}, returning last error",
                    max_elapsed_time
                );
                if let Some(ref cb) = self.circuit_breaker {
                    cb.record_failure();
                }
                return Err(last_error.unwrap_or_else(|| {
                    FcmError::Other(format!(
                        "Retry exceeded max elapsed time {:?}",
                        max_elapsed_time
                    ))
                }));
            }

            match self
                .send_notification(token, title, body, data.clone())
                .await
            {
                Ok(()) => {
                    let total_latency = start_time.elapsed();
                    debug!(
                        "FCM send with retry succeeded after {} attempts in {:?}",
                        attempt + 1,
                        total_latency
                    );
                    if let Some(ref cb) = self.circuit_breaker {
                        cb.record_success();
                    }
                    tracing::event!(
                        tracing::Level::INFO,
                        attempt = attempt + 1,
                        total_latency_ms = total_latency.as_millis(),
                        "fcm.send_retry_success"
                    );
                    return Ok(());
                }
                Err(e) => {
                    if !e.is_retryable() || attempt == max_retries {
                        if let Some(ref cb) = self.circuit_breaker {
                            cb.record_failure();
                        }
                        tracing::event!(
                            tracing::Level::ERROR,
                            attempt = attempt + 1,
                            error = %e,
                            error_type = %std::any::type_name_of_val(&e),
                            "fcm.send_final_failure"
                        );
                        return Err(e);
                    }

                    let base_delay_ms = self.retry_config.base_delay_ms * 2u64.pow(attempt);
                    let capped_delay_ms = base_delay_ms.min(self.retry_config.max_delay_ms);
                    let jitter_ms = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .subsec_nanos() as u64
                        % self.retry_config.jitter_ms;
                    let delay_ms = capped_delay_ms + jitter_ms;
                    let delay = Duration::from_millis(delay_ms);

                    let remaining = max_elapsed_time.saturating_sub(start_time.elapsed());
                    if delay > remaining {
                        warn!(
                            "FCM retry delay {:?} would exceed remaining time {:?}, returning last error",
                            delay, remaining
                        );
                        if let Some(ref cb) = self.circuit_breaker {
                            cb.record_failure();
                        }
                        return Err(e);
                    }

                    warn!(
                        "FCM send attempt {}/{} failed (retryable): {}. Retrying in {:?}",
                        attempt + 1,
                        max_retries + 1,
                        e,
                        delay
                    );
                    tracing::event!(
                        tracing::Level::WARN,
                        attempt = attempt + 1,
                        delay_ms,
                        error = %e,
                        "fcm.send_retry"
                    );

                    last_error = Some(e);
                    tokio::time::sleep(delay).await;
                }
            }
        }

        // Record failure if we exhausted retries
        if let Some(ref cb) = self.circuit_breaker {
            cb.record_failure();
        }

        Err(last_error.unwrap_or_else(|| FcmError::Other("Retry exhausted".to_string())))
    }

    const MAX_BULK_BATCH_SIZE: usize = 500;
    const BULK_TIMEOUT_SECS: u64 = 120;

    /// Send multiple FCM notifications concurrently with controlled concurrency.
    ///
    /// Enforces safety limits:
    /// - Maximum batch size of 500 requests (excess are rejected)
    /// - Overall 120-second timeout for the entire bulk operation
    /// - Bounded concurrency (default 100) to avoid overwhelming FCM API
    pub async fn send_bulk_notifications(
        &self,
        requests: Vec<(String, String, String, HashMap<String, String>)>,
        max_concurrent: Option<usize>,
    ) -> Vec<Result<(), FcmError>> {
        use futures::stream::{FuturesUnordered, StreamExt};

        if requests.is_empty() {
            return vec![];
        }

        let total = requests.len();
        if total > Self::MAX_BULK_BATCH_SIZE {
            warn!(
                requested = total,
                max = Self::MAX_BULK_BATCH_SIZE,
                "Bulk notification request exceeds maximum batch size, truncating"
            );
        }

        let requests: Vec<_> = requests
            .into_iter()
            .take(Self::MAX_BULK_BATCH_SIZE)
            .collect();
        let capped_total = requests.len();
        let max_concurrent = max_concurrent.unwrap_or(100).min(capped_total);
        let mut indexed_results: Vec<(usize, Result<(), FcmError>)> =
            Vec::with_capacity(capped_total);
        let mut request_iter = requests.into_iter().enumerate();
        let mut futures = FuturesUnordered::new();

        let deadline =
            tokio::time::Instant::now() + tokio::time::Duration::from_secs(Self::BULK_TIMEOUT_SECS);

        for _ in 0..max_concurrent {
            if let Some((idx, (token, title, body, data))) = request_iter.next() {
                let service = self.clone();
                futures.push(tokio::spawn(async move {
                    let result = service
                        .send_notification_with_retry(&token, &title, &body, data, 3)
                        .await;
                    (idx, result)
                }));
            }
        }

        loop {
            tokio::select! {
                result = futures.next() => {
                    match result {
                        Some(join_result) => {
                            let (idx, send_result) = join_result.unwrap_or_else(|e| {
                                error!("FCM send task failed: {}", e);
                                (0, Err(FcmError::Other(format!("Task join error: {}", e))))
                            });

                            if let Some((next_idx, (token, title, body, data))) = request_iter.next() {
                                let service = self.clone();
                                futures.push(tokio::spawn(async move {
                                    let result = service
                                        .send_notification_with_retry(&token, &title, &body, data, 3)
                                        .await;
                                    (next_idx, result)
                                }));
                            }

                            indexed_results.push((idx, send_result));
                        }
                        None => break,
                    }
                }
                _ = tokio::time::sleep_until(deadline) => {
                    error!(
                        completed = indexed_results.len(),
                        total = capped_total,
                        "Bulk notification operation timed out"
                    );
                    for (remaining_idx, _) in request_iter {
                        indexed_results.push((remaining_idx, Err(FcmError::Other(
                            "Bulk operation timed out".to_string(),
                        ))));
                    }
                    break;
                }
            }
        }

        indexed_results.sort_by_key(|(idx, _)| *idx);
        indexed_results
            .into_iter()
            .map(|(_, result)| result)
            .collect()
    }

    async fn get_access_token(&self) -> AppResult<String> {
        if let Ok(token) = std::env::var("FCM_ACCESS_TOKEN") {
            if !token.is_empty() {
                debug!("Using FCM access token from FCM_ACCESS_TOKEN env var");
                return Ok(token);
            }
        }
        self.token_provider.token().await
    }

    fn parse_fcm_error(
        &self,
        _status: u16,
        error_response: FcmErrorResponse,
    ) -> Result<(), FcmError> {
        let status_str = error_response
            .error
            .status
            .unwrap_or_else(|| "UNKNOWN".to_string());
        let message = error_response.error.message;

        match status_str.as_str() {
            "UNREGISTERED" | "NOT_FOUND" => {
                warn!("FCM token is unregistered: {}", message);
                Err(FcmError::InvalidToken(message))
            }
            "UNAUTHENTICATED" | "PERMISSION_DENIED" => {
                error!("FCM authentication failed: {}", message);
                Err(FcmError::Unauthorized(message))
            }
            "RESOURCE_EXHAUSTED" | "QUOTA_EXCEEDED" => {
                warn!("FCM quota exceeded: {}", message);
                Err(FcmError::QuotaExceeded(message))
            }
            "MESSAGE_TOO_BIG" => {
                error!("FCM message too large: {}", message);
                Err(FcmError::MessageTooLarge(message))
            }
            "INTERNAL" | "UNAVAILABLE" => {
                error!("FCM internal error: {}", message);
                Err(FcmError::Internal(message))
            }
            _ => {
                error!("FCM error ({}): {}", status_str, message);
                Err(FcmError::Other(format!("{}: {}", status_str, message)))
            }
        }
    }

    fn classify_error_by_status(&self, status: u16, error_text: &str) -> Result<(), FcmError> {
        match status {
            401 | 403 => {
                error!("FCM authentication failed: {}", error_text);
                Err(FcmError::Unauthorized(error_text.to_string()))
            }
            404 => {
                if error_text.contains("UNREGISTERED")
                    || error_text
                        .contains("The registration token is not a valid FCM registration token")
                {
                    warn!("FCM token is unregistered: {}", error_text);
                    Err(FcmError::InvalidToken(error_text.to_string()))
                } else {
                    error!("FCM not found: {}", error_text);
                    Err(FcmError::Other(error_text.to_string()))
                }
            }
            429 => {
                warn!("FCM rate limited: {}", error_text);
                Err(FcmError::RateLimited(error_text.to_string()))
            }
            500..=599 => {
                error!("FCM server error: {}", error_text);
                Err(FcmError::ServerError(error_text.to_string()))
            }
            _ => {
                error!("FCM request failed with status {}: {}", status, error_text);
                Err(FcmError::Other(format!("HTTP {}: {}", status, error_text)))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_fcm_request_serialization() {
        let mut data = HashMap::new();
        data.insert("key1".to_string(), "value1".to_string());
        data.insert("key2".to_string(), "value2".to_string());

        let notification = FcmNotification {
            title: "Test Title".to_string(),
            body: "Test Body".to_string(),
        };

        let message = FcmMessage {
            token: "test_token".to_string(),
            notification: Some(notification),
            data: Some(data.clone()),
        };

        let request = FcmRequest { message };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("test_token"));
        assert!(json.contains("Test Title"));
        assert!(json.contains("Test Body"));
        assert!(json.contains("key1"));
        assert!(json.contains("value1"));
    }

    #[test]
    fn test_fcm_message_without_data() {
        let notification = FcmNotification {
            title: "Test Title".to_string(),
            body: "Test Body".to_string(),
        };

        let message = FcmMessage {
            token: "test_token".to_string(),
            notification: Some(notification),
            data: None,
        };

        let request = FcmRequest { message };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("test_token"));
        assert!(json.contains("Test Title"));
        assert!(json.contains("Test Body"));
        assert!(!json.contains("\"data\""));
    }

    #[test]
    fn test_fcm_error_response_deserialization() {
        let json = r#"{
            "error": {
                "message": "The registration token is not a valid FCM registration token",
                "status": "UNREGISTERED"
            }
        }"#;

        let response: FcmErrorResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.error.status, Some("UNREGISTERED".to_string()));
    }
}
