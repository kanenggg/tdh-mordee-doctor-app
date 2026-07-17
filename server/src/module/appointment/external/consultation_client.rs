//! Client for consultation-rs `GET /internal/v1/appointment/{bookingId}`.
//!
//! See spec:
//! docs/superpowers/specs/2026-04-09-appointment-detail-bff-design.md.

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Deserializer};
use std::time::Duration;
use tracing::{debug, warn};

use crate::core::error::{AppError, AppResult};

/// Outcome of a consultation lookup. Wraps the upstream discriminated
/// union (`success` | `appointmentNotFound`).
#[derive(Debug)]
pub enum ConsultationLookup {
    Found(ConsultationDetail),
    NotFound,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsultationDetail {
    pub booking_id: String,
    pub appointment_time: ConsultationAppointmentTime,
    pub status: String,
    pub booking_type: String,
    pub consultation_channel: String,
    pub patient: ConsultationIdentity,
    #[allow(dead_code)]
    pub doctor: ConsultationIdentity,
    pub prescreen: ConsultationPrescreen,
    pub payment_tx_id: i64,
    #[serde(default, deserialize_with = "empty_string_for_null")]
    pub payment_tx_ref_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsultationAppointmentTime {
    pub start_time: i64,
    pub end_time: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsultationIdentity {
    pub account_id: i32,
    pub profile_id: i32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsultationPrescreen {
    pub symptom: String,
    pub duration: i32,
    #[serde(alias = "duration_unit")]
    pub duration_unit: String,
    #[serde(default)]
    pub attachments: Vec<String>,
    #[serde(default)]
    pub allergies: Vec<String>,
}

/// Raw upstream wire envelope on `__type`. Internal — never leaks out.
#[derive(Debug, Deserialize)]
#[serde(tag = "__type")]
enum WireEnvelope {
    #[serde(rename = "Success", alias = "success")]
    Success(ConsultationDetail),
    #[serde(rename = "AppointmentNotFound", alias = "appointmentNotFound")]
    AppointmentNotFound,
}

#[async_trait]
pub trait ConsultationClientTrait: Send + Sync {
    async fn get_appointment(&self, booking_id: &str) -> AppResult<ConsultationLookup>;
}

#[derive(Clone)]
pub struct ConsultationClient {
    client: Client,
    base_uri: String,
}

impl ConsultationClient {
    pub fn new(base_uri: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .expect("failed to build consultation HTTP client"),
            base_uri,
        }
    }
}

#[async_trait]
impl ConsultationClientTrait for ConsultationClient {
    /// Caller MUST pre-validate `booking_id` to be a safe URL path
    /// segment (e.g., `[A-Za-z0-9-]+`). The handler in `mod.rs`
    /// enforces this at the request boundary; this client does not
    /// re-encode the value before splicing it into the URL.
    #[tracing::instrument(name = "consultation.get_appointment_detail", skip(self), fields(booking_id = %booking_id))]
    async fn get_appointment(&self, booking_id: &str) -> AppResult<ConsultationLookup> {
        let url = format!("{}/internal/v1/appointment/{}", self.base_uri, booking_id);
        debug!(%url, "calling consultation upstream");

        // One transparent retry on transport failure.
        let resp = match send_with_retry(&self.client, &url).await {
            Ok(r) => r,
            Err(e) => {
                warn!(error = %e, "consultation upstream failed after retry");
                return Err(AppError::UpstreamError("consultation".to_string()));
            }
        };

        let status = resp.status();
        let content_type = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_string();
        let body = resp.text().await.map_err(|e| {
            warn!(
                error = %e,
                %status,
                content_type = %content_type,
                "consultation upstream response body could not be read"
            );
            AppError::UpstreamError("consultation".to_string())
        })?;

        let envelope: WireEnvelope = serde_json::from_str(&body).map_err(|e| {
            let body_shape = serde_json::from_str::<serde_json::Value>(&body)
                .map(|value| json_shape_summary(&value))
                .unwrap_or_else(|_| "non-json".to_string());
            warn!(
                error = %e,
                %status,
                content_type = %content_type,
                body_len = body.len(),
                body_shape = %body_shape,
                "consultation upstream returned an unexpected response body"
            );
            AppError::UpstreamError("consultation".to_string())
        })?;

        Ok(match envelope {
            WireEnvelope::Success(d) => ConsultationLookup::Found(d),
            WireEnvelope::AppointmentNotFound => ConsultationLookup::NotFound,
        })
    }
}

fn empty_string_for_null<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Option::<String>::deserialize(deserializer)?.unwrap_or_default())
}

fn json_shape_summary(value: &serde_json::Value) -> String {
    fn summarize(value: &serde_json::Value, depth: usize) -> String {
        match value {
            serde_json::Value::Null => "null".to_string(),
            serde_json::Value::Bool(_) => "bool".to_string(),
            serde_json::Value::Number(_) => "number".to_string(),
            serde_json::Value::String(_) => "string".to_string(),
            serde_json::Value::Array(_) => "array".to_string(),
            serde_json::Value::Object(map) => {
                if depth >= 2 {
                    return "object".to_string();
                }

                let mut keys = map.keys().collect::<Vec<_>>();
                keys.sort();
                let fields = keys
                    .into_iter()
                    .map(|key| format!("{}:{}", key, summarize(&map[key], depth + 1)))
                    .collect::<Vec<_>>()
                    .join(",");
                format!("object{{{}}}", fields)
            }
        }
    }

    summarize(value, 0)
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

    fn success_body() -> serde_json::Value {
        json!({
            "__type": "Success",
            "bookingId": "BK20260227810949",
            "appointmentTime": {
                "startTime": 1645940400,
                "endTime": 1645941300
            },
            "status": "Booked",
            "bookingType": "Schedule",
            "consultationChannel": "Video",
            "patient": {
                "accountId": 124236,
                "profileId": 200
            },
            "doctor": {
                "accountId": 300,
                "profileId": 400
            },
            "prescreen": {
                "symptom": "rash",
                "duration": 7,
                "durationUnit": "day",
                "attachments": ["att-ref-001"],
                "allergies": ["Amoxicillin"]
            },
            "paymentTxId": 1042,
            "paymentTxRefId": "tx-ref-2026-02-27-001"
        })
    }

    #[test]
    fn deserializes_pascal_case_success_from_consultation_rs() {
        let envelope: WireEnvelope = serde_json::from_value(success_body()).unwrap();

        match envelope {
            WireEnvelope::Success(detail) => {
                assert_eq!(detail.booking_id, "BK20260227810949");
                assert_eq!(detail.payment_tx_id, 1042);
                assert_eq!(detail.payment_tx_ref_id, "tx-ref-2026-02-27-001");
                assert_eq!(detail.patient.account_id, 124236);
                assert_eq!(detail.prescreen.attachments, vec!["att-ref-001"]);
            }
            WireEnvelope::AppointmentNotFound => panic!("expected Success"),
        }
    }

    #[test]
    fn deserializes_pending_success_without_payment_ref() {
        let mut body = success_body();
        body["status"] = json!("Pending");
        body["paymentTxId"] = json!(0);
        body["paymentTxRefId"] = serde_json::Value::Null;

        let envelope: WireEnvelope = serde_json::from_value(body).unwrap();

        match envelope {
            WireEnvelope::Success(detail) => {
                assert_eq!(detail.status, "Pending");
                assert_eq!(detail.payment_tx_id, 0);
                assert_eq!(detail.payment_tx_ref_id, "");
            }
            WireEnvelope::AppointmentNotFound => panic!("expected Success"),
        }
    }

    #[test]
    fn deserializes_prescreen_legacy_snake_case_duration_unit() {
        let mut body = success_body();
        let prescreen = body["prescreen"].as_object_mut().unwrap();
        prescreen.remove("durationUnit");
        prescreen.insert("duration_unit".to_string(), json!("week"));

        let envelope: WireEnvelope = serde_json::from_value(body).unwrap();

        match envelope {
            WireEnvelope::Success(detail) => {
                assert_eq!(detail.prescreen.duration_unit, "week");
            }
            WireEnvelope::AppointmentNotFound => panic!("expected Success"),
        }
    }

    #[test]
    fn json_shape_summary_redacts_values_but_reports_field_types() {
        let shape = json_shape_summary(&json!({
            "__type": "Success",
            "bookingId": "BK20260227810949",
            "paymentTxRefId": null,
            "prescreen": {
                "symptom": "sensitive text",
                "duration_unit": "day",
                "attachments": ["att-ref-001"]
            }
        }));

        assert!(shape.contains("__type:string"));
        assert!(shape.contains("paymentTxRefId:null"));
        assert!(shape.contains("prescreen:object"));
        assert!(shape.contains("duration_unit:string"));
        assert!(!shape.contains("BK20260227810949"));
        assert!(!shape.contains("sensitive text"));
        assert!(!shape.contains("att-ref-001"));
    }

    #[test]
    fn deserializes_pascal_case_appointment_not_found_from_consultation_rs() {
        let envelope: WireEnvelope =
            serde_json::from_value(json!({"__type": "AppointmentNotFound"})).unwrap();

        match envelope {
            WireEnvelope::AppointmentNotFound => {}
            WireEnvelope::Success(_) => panic!("expected AppointmentNotFound"),
        }
    }

    #[tokio::test]
    async fn get_appointment_calls_preferred_internal_v1_route() {
        let apm = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/internal/v1/appointment/BK20260227810949"))
            .respond_with(ResponseTemplate::new(200).set_body_json(success_body()))
            .mount(&apm)
            .await;

        let client = ConsultationClient::new(apm.uri());
        let lookup = client
            .get_appointment("BK20260227810949")
            .await
            .expect("preferred internal v1 route should succeed");

        match lookup {
            ConsultationLookup::Found(detail) => {
                assert_eq!(detail.booking_id, "BK20260227810949");
            }
            ConsultationLookup::NotFound => panic!("expected found appointment"),
        }
    }
}
