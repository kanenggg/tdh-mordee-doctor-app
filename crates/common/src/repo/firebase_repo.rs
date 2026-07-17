use crate::config::FirebaseConfig;
use crate::core::error::{AppError, AppResult};
use crate::core::GcpTokenProvider;
use backoff::future::retry_notify;
use backoff::{Error as BackoffError, ExponentialBackoff};
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub struct FirebaseRepo {
    client: reqwest::Client,
    database_url: String,
    token_provider: Arc<GcpTokenProvider>,
    retry_config: ExponentialBackoff,
}

impl FirebaseRepo {
    pub async fn new(
        config: &FirebaseConfig,
        token_provider: Arc<GcpTokenProvider>,
        retry_config: ExponentialBackoff,
    ) -> AppResult<Self> {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| AppError::InternalError(format!("Failed to build HTTP client: {e}")))?;
        let database_url = config.database_url.trim_end_matches('/').to_string();
        Ok(Self {
            client,
            database_url,
            token_provider,
            retry_config,
        })
    }

    fn is_retryable_http_error(msg: &str) -> bool {
        let msg_lower = msg.to_lowercase();
        msg_lower.contains("timed out")
            || msg_lower.contains("timeout")
            || msg_lower.contains("connection")
            || msg.contains("503")
            || msg.contains("502")
            || msg.contains("504")
            || msg.contains("429")
            || msg.contains("500")
    }

    fn is_rtdb_retryable_error(err: &AppError) -> bool {
        match err {
            AppError::InternalError(msg) => Self::is_retryable_http_error(msg),
            AppError::ReqwestError(e) => {
                let msg = e.to_string().to_lowercase();
                msg.contains("timed out")
                    || msg.contains("timeout")
                    || msg.contains("connection")
                    || msg.contains("dns")
                    || msg.contains("connect")
                    || Self::is_retryable_http_error(&msg)
            }
            _ => false,
        }
    }

    async fn rtdb_retry<F, T, Fut>(&self, operation: F, context: &str) -> AppResult<T>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = AppResult<T>>,
    {
        let mut op = operation;
        retry_notify(
            self.retry_config.clone(),
            || {
                let result_fut = op();
                async move {
                    result_fut.await.map_err(|e| {
                        if Self::is_rtdb_retryable_error(&e) {
                            BackoffError::transient(e)
                        } else {
                            BackoffError::permanent(e)
                        }
                    })
                }
            },
            |err, next_wait: Duration| {
                tracing::warn!(
                    context,
                    error = %err,
                    wait_ms = next_wait.as_millis(),
                    "RTDB operation failed, retrying"
                );
            },
        )
        .await
    }

    pub async fn set(&self, path: &str, value: &serde_json::Value) -> AppResult<()> {
        let client = self.client.clone();
        let token_provider = self.token_provider.clone();
        let path = path.to_string();
        let database_url = self.database_url.clone();
        let value = value.clone();
        let context = format!("set(path={})", path);

        self.rtdb_retry(
            move || {
                let client = client.clone();
                let token_provider = token_provider.clone();
                let path = path.clone();
                let database_url = database_url.clone();
                let value = value.clone();
                async move {
                    let token = token_provider.token().await?;
                    let url_path = path.trim_start_matches('/');
                    let url = if url_path.is_empty() {
                        format!("{}.json?access_token={}", database_url, token)
                    } else {
                        format!("{}/{}.json?access_token={}", database_url, url_path, token)
                    };
                    let res = client
                        .put(&url)
                        .json(&value)
                        .send()
                        .await
                        .map_err(|e| AppError::InternalError(e.to_string()))?;
                    if !res.status().is_success() {
                        let status = res.status();
                        let body = res.text().await.unwrap_or_default();
                        return Err(AppError::InternalError(format!(
                            "Firebase PUT failed {}: {}",
                            status, body
                        )));
                    }
                    Ok(())
                }
            },
            &context,
        )
        .await
    }

    pub async fn get(&self, path: &str) -> AppResult<Option<serde_json::Value>> {
        let client = self.client.clone();
        let token_provider = self.token_provider.clone();
        let path = path.to_string();
        let database_url = self.database_url.clone();
        let context = format!("get(path={})", path);

        self.rtdb_retry(
            move || {
                let client = client.clone();
                let token_provider = token_provider.clone();
                let path = path.clone();
                let database_url = database_url.clone();
                async move {
                    let token = token_provider.token().await?;
                    let url_path = path.trim_start_matches('/');
                    let url = if url_path.is_empty() {
                        format!("{}.json?access_token={}", database_url, token)
                    } else {
                        format!("{}/{}.json?access_token={}", database_url, url_path, token)
                    };
                    let res = client
                        .get(&url)
                        .send()
                        .await
                        .map_err(|e| AppError::InternalError(e.to_string()))?;
                    if !res.status().is_success() {
                        let status = res.status();
                        let body = res.text().await.unwrap_or_default();
                        return Err(AppError::InternalError(format!(
                            "Firebase GET failed {}: {}",
                            status, body
                        )));
                    }
                    let value: serde_json::Value = res
                        .json()
                        .await
                        .map_err(|e| AppError::InternalError(e.to_string()))?;
                    if value.is_null() {
                        Ok(None)
                    } else {
                        Ok(Some(value))
                    }
                }
            },
            &context,
        )
        .await
    }

    pub async fn update(&self, path: &str, value: &serde_json::Value) -> AppResult<()> {
        let client = self.client.clone();
        let token_provider = self.token_provider.clone();
        let path = path.to_string();
        let database_url = self.database_url.clone();
        let value = value.clone();
        let context = format!("update(path={})", path);

        self.rtdb_retry(
            move || {
                let client = client.clone();
                let token_provider = token_provider.clone();
                let path = path.clone();
                let database_url = database_url.clone();
                let value = value.clone();
                async move {
                    let token = token_provider.token().await?;
                    let url_path = path.trim_start_matches('/');
                    let url = if url_path.is_empty() {
                        format!("{}.json?access_token={}", database_url, token)
                    } else {
                        format!("{}/{}.json?access_token={}", database_url, url_path, token)
                    };
                    let res = client
                        .patch(&url)
                        .json(&value)
                        .send()
                        .await
                        .map_err(|e| AppError::InternalError(e.to_string()))?;
                    if !res.status().is_success() {
                        let status = res.status();
                        let body = res.text().await.unwrap_or_default();
                        return Err(AppError::InternalError(format!(
                            "Firebase PATCH failed {}: {}",
                            status, body
                        )));
                    }
                    Ok(())
                }
            },
            &context,
        )
        .await
    }

    pub async fn exists(&self, path: &str) -> AppResult<bool> {
        let value = self.get(path).await?;
        Ok(value.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_retryable_http_error() {
        assert!(FirebaseRepo::is_retryable_http_error("request timed out"));
        assert!(FirebaseRepo::is_retryable_http_error("connection timeout"));
        assert!(FirebaseRepo::is_retryable_http_error("HTTP 503"));
        assert!(FirebaseRepo::is_retryable_http_error("HTTP 429"));
        assert!(FirebaseRepo::is_retryable_http_error("HTTP 502"));
        assert!(FirebaseRepo::is_retryable_http_error("HTTP 504"));
        assert!(FirebaseRepo::is_retryable_http_error("HTTP 500"));
        assert!(!FirebaseRepo::is_retryable_http_error("HTTP 404"));
        assert!(!FirebaseRepo::is_retryable_http_error("HTTP 400"));
    }

    #[test]
    fn test_is_rtdb_retryable_error() {
        // InternalError with retryable messages
        assert!(FirebaseRepo::is_rtdb_retryable_error(
            &AppError::InternalError("timed out".to_string())
        ));
        assert!(FirebaseRepo::is_rtdb_retryable_error(
            &AppError::InternalError("HTTP 503".to_string())
        ));

        // Non-retryable errors
        assert!(!FirebaseRepo::is_rtdb_retryable_error(
            &AppError::Unauthorized
        ));
        assert!(!FirebaseRepo::is_rtdb_retryable_error(
            &AppError::InternalError("HTTP 404".to_string())
        ));
    }
}
