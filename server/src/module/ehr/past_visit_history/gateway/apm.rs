//! biz-apm consultation client for patient past visits.
//!
//! `GET /internal/v1/appointments` returns a patient's completed appointments
//! in biz-apm's native appointment shape (nested `appointmentTime`, embedded
//! `doctor` with identity ids plus a `firstName`/`lastName` snapshot), so no
//! separate doctor lookup is needed. The service layer converts this origin
//! model into the API response.

use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;

use crate::core::error::{AppError, AppResult};

/// A single appointment in biz-apm's native (origin) shape.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApmAppointment {
    pub booking_id: String,
    pub appointment_time: ApmAppointmentTime,
    pub doctor: ApmDoctor,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApmAppointmentTime {
    pub start_time: i64,
    pub end_time: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApmDoctor {
    pub account_id: i32,
    pub profile_id: i32,
    pub first_name: String,
    pub last_name: String,
}

/// Outcome of fetching a patient's past visits from biz-apm.
#[derive(Debug, Clone)]
pub enum ApmPastVisitsResponse {
    Found(Vec<ApmAppointment>),
    NotFound,
    Unauthorized,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppointmentsBody {
    appointments: Vec<ApmAppointment>,
}

#[derive(Debug, Clone)]
pub struct ApmClient {
    client: Client,
    base_uri: String,
}

impl ApmClient {
    pub fn new(base_uri: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("failed to build HTTP client for biz-apm"),
            base_uri,
        }
    }

    /// `GET {base_uri}/internal/v1/appointments?patientAccountId=&patientProfileId=`
    pub async fn get_past_visits(
        &self,
        request_id: &str,
        patient_account_id: i32,
        patient_profile_id: i32,
    ) -> AppResult<ApmPastVisitsResponse> {
        let url = format!("{}/internal/v1/appointments", self.base_uri);
        let resp = self
            .client
            .get(&url)
            .header("X-Request-Id", request_id)
            .header("Accept-Language", "en-US")
            .query(&[
                ("patientAccountId", patient_account_id.to_string()),
                ("patientProfileId", patient_profile_id.to_string()),
            ])
            .send()
            .await?;

        match resp.status() {
            s if s.is_success() => {
                let body = resp.text().await?;
                let parsed: AppointmentsBody = serde_json::from_str(&body).map_err(|e| {
                    AppError::InternalError(format!(
                        "Failed to parse biz-apm appointments response: {}, body: {}",
                        e, body
                    ))
                })?;
                Ok(ApmPastVisitsResponse::Found(parsed.appointments))
            }
            reqwest::StatusCode::UNAUTHORIZED => Ok(ApmPastVisitsResponse::Unauthorized),
            reqwest::StatusCode::NOT_FOUND => Ok(ApmPastVisitsResponse::NotFound),
            s => Err(AppError::UpstreamError(format!(
                "biz-apm appointments returned unexpected status: {}",
                s
            ))),
        }
    }
}
