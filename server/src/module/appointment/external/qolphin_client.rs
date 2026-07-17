//! Client for qolphin past-visit endpoints.

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use std::time::Duration;
use tracing::{debug, warn};

use crate::core::error::{AppError, AppResult};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireListResponse {
    #[serde(alias = "pastVisits")]
    past_visits: Option<Vec<JsonValue>>,
}

#[async_trait]
pub trait QolphinClientTrait: Send + Sync {
    async fn get_past_visits(
        &self,
        user_account_id: i32,
        language: &str,
    ) -> AppResult<Vec<JsonValue>>;
    async fn get_past_visit_detail(
        &self,
        booking_id: &str,
        language: &str,
    ) -> AppResult<Option<JsonValue>>;
}

#[derive(Clone)]
pub struct QolphinClient {
    client: Client,
    base_uri: String,
}

impl QolphinClient {
    pub fn new(base_uri: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .expect("failed to build qolphin HTTP client"),
            base_uri,
        }
    }
}

#[async_trait]
impl QolphinClientTrait for QolphinClient {
    #[tracing::instrument(
        name = "qolphin.get_past_visits",
        skip(self, language),
        fields(user_account_id = user_account_id, language = language)
    )]
    async fn get_past_visits(
        &self,
        user_account_id: i32,
        language: &str,
    ) -> AppResult<Vec<JsonValue>> {
        let url = format!("{}/v1/internal/past-visit", self.base_uri);
        debug!(%url, user_account_id, %language, "calling qolphin upstream");

        let resp = match send_with_query(
            &self.client,
            &url,
            &[("userAccountId", user_account_id.to_string())],
            language,
        )
        .await
        {
            Ok(r) => r,
            Err(e) => {
                warn!(error = %e, "qolphin upstream failed after retry");
                return Err(AppError::UpstreamError("qolphin".to_string()));
            }
        };

        let body = resp.text().await.map_err(|e| {
            warn!(error = %e, "qolphin upstream failed to read response body");
            AppError::UpstreamError("qolphin".to_string())
        })?;

        debug!(
            response_bytes = body.len(),
            "qolphin upstream response received"
        );

        // qolphin may return wrapped { past_visits: [...] } or raw array
        // Try wrapped format first
        if let Ok(wire) = serde_json::from_str::<WireListResponse>(&body) {
            if let Some(visits) = wire.past_visits {
                return Ok(visits);
            }
        }

        // Fallback: try parsing as raw array
        let visits: Vec<JsonValue> = serde_json::from_str(&body).map_err(|e| {
            warn!(
                error = %e,
                response_bytes = body.len(),
                "qolphin upstream returned unexpected response body"
            );
            AppError::UpstreamError("qolphin".to_string())
        })?;

        Ok(visits)
    }

    #[tracing::instrument(
        name = "qolphin.get_past_visit_detail",
        skip(self, language),
        fields(booking_id = booking_id, language = language)
    )]
    async fn get_past_visit_detail(
        &self,
        booking_id: &str,
        language: &str,
    ) -> AppResult<Option<JsonValue>> {
        let url = format!("{}/v1/internal/past-visit-detail", self.base_uri);
        debug!(%url, booking_id, %language, "calling qolphin upstream for past-visit-detail");

        let resp = match send_with_query(
            &self.client,
            &url,
            &[("bookingId", booking_id.to_string())],
            language,
        )
        .await
        {
            Ok(r) => r,
            Err(e) if e.status().map(|s| s.as_u16()) == Some(404) => {
                return Ok(None);
            }
            Err(e) => {
                warn!(error = %e, "qolphin upstream failed");
                return Err(AppError::UpstreamError("qolphin".to_string()));
            }
        };

        let body = resp.text().await.map_err(|e| {
            warn!(error = %e, "qolphin upstream failed to read response body");
            AppError::UpstreamError("qolphin".to_string())
        })?;

        debug!(
            response_bytes = body.len(),
            "qolphin upstream response received for past-visit-detail"
        );

        // qolphin returns raw consultation object, not wrapped in { past_visit: ... }
        let visit: JsonValue = serde_json::from_str(&body).map_err(|e| {
            warn!(
                error = %e,
                response_bytes = body.len(),
                "qolphin upstream returned unexpected response body for past-visit-detail"
            );
            AppError::UpstreamError("qolphin".to_string())
        })?;

        Ok(Some(visit))
    }
}

async fn send_with_query(
    client: &Client,
    url: &str,
    params: &[(&str, String)],
    accept_language: &str,
) -> Result<reqwest::Response, reqwest::Error> {
    let build = || {
        client
            .get(url)
            .query(params)
            .header("Accept-Language", accept_language)
    };

    match build().send().await {
        Ok(r) if r.status().is_server_error() => {
            warn!(status = %r.status(), %url, attempt = 1, "transport 5xx, retrying");
            build().send().await?.error_for_status()
        }
        Ok(r) if r.status().is_client_error() => {
            let err = r.error_for_status_ref().unwrap_err();
            let response_body = r.text().await.unwrap_or_default();
            warn!(
                status = %err.status().unwrap_or_default(),
                %url,
                response_bytes = response_body.len(),
                "upstream returned client error, no retry"
            );
            Err(err)
        }
        Ok(r) => r.error_for_status(),
        Err(e) if e.is_connect() || e.is_timeout() || e.is_request() => {
            warn!(error = %e, %url, attempt = 1, "transport error, retrying");
            build().send().await?.error_for_status()
        }
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn get_past_visits_passes_user_account_id_as_query_param() {
        let upstream = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/v1/internal/past-visit"))
            .and(query_param("userAccountId", "124236"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "pastVisits": [
                    {
                        "bookingId": "BK20220101000001",
                        "status": "Completed"
                    }
                ]
            })))
            .mount(&upstream)
            .await;

        let client = QolphinClient::new(upstream.uri());
        let result = client.get_past_visits(124236, "th-TH").await.unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["bookingId"], "BK20220101000001");
    }

    #[tokio::test]
    async fn get_past_visits_returns_empty_list_when_no_visits() {
        let upstream = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/v1/internal/past-visit"))
            .and(query_param("userAccountId", "999"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "pastVisits": [] })))
            .mount(&upstream)
            .await;

        let client = QolphinClient::new(upstream.uri());
        let result = client.get_past_visits(999, "th-TH").await.unwrap();

        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn get_past_visit_detail_passes_booking_id_as_query_param() {
        let upstream = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/v1/internal/past-visit-detail"))
            .and(query_param("bookingId", "BK20220101000001"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "bookingId": "BK20220101000001",
                "status": "Completed"
            })))
            .mount(&upstream)
            .await;

        let client = QolphinClient::new(upstream.uri());
        let result = client
            .get_past_visit_detail("BK20220101000001", "th-TH")
            .await
            .unwrap();

        assert!(result.is_some());
        let visit = result.unwrap();
        assert_eq!(visit["bookingId"], "BK20220101000001");
        assert_eq!(visit["status"], "Completed");
    }

    #[tokio::test]
    async fn get_past_visit_detail_returns_none_when_not_found() {
        let upstream = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/v1/internal/past-visit-detail"))
            .and(query_param("bookingId", "BK99999999"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&upstream)
            .await;

        let client = QolphinClient::new(upstream.uri());
        let result = client
            .get_past_visit_detail("BK99999999", "th-TH")
            .await
            .unwrap();

        assert!(result.is_none());
    }
}
