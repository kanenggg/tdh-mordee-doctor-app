//! Client for consultation-rs `GET /internal/v1/appointment/reserved-timeslots`.
//! See spec: docs/superpowers/specs/2026-06-18-doctor-available-timeslots-design.md

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;
use tracing::{debug, warn};

use crate::core::error::{AppError, AppResult};

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReservedTimeSlot {
    pub booking_id: String,
    pub start_time: i64,
    pub end_time: i64,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReservedTimeslotsResponse {
    pub reserved_timeslots: Vec<ReservedTimeSlot>,
}

#[async_trait]
pub trait ReservedTimeslotsClientTrait: Send + Sync {
    async fn get_reserved_timeslots(
        &self,
        doctor_profile_id: i32,
        from_datetime: &str,
        to_datetime: &str,
    ) -> AppResult<Vec<ReservedTimeSlot>>;
}

#[derive(Clone)]
pub struct ReservedTimeslotsClient {
    client: Client,
    base_uri: String,
}

impl ReservedTimeslotsClient {
    pub fn new(base_uri: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .expect("failed to build reserved-timeslots HTTP client"),
            base_uri,
        }
    }
}

#[async_trait]
impl ReservedTimeslotsClientTrait for ReservedTimeslotsClient {
    #[tracing::instrument(name = "consultation.get_reserved_timeslots", skip(self), fields(doctor_profile_id, from_datetime = %from_datetime, to_datetime = %to_datetime))]
    async fn get_reserved_timeslots(
        &self,
        doctor_profile_id: i32,
        from_datetime: &str,
        to_datetime: &str,
    ) -> AppResult<Vec<ReservedTimeSlot>> {
        let url = format!(
            "{}/internal/v1/appointment/reserved-timeslots?doctorProfileId={}&from_datetime={}&to_datetime={}",
            self.base_uri, doctor_profile_id, from_datetime, to_datetime
        );
        debug!(%url, "calling reserved-timeslots upstream");

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .map_err(|e| {
                warn!(error = %e, "reserved-timeslots upstream failed");
                AppError::UpstreamError("consultation".to_string())
            })?;

        let body: ReservedTimeslotsResponse = resp.json().await.map_err(|e| {
            warn!(error = %e, "reserved-timeslots upstream returned unexpected body");
            AppError::UpstreamError("consultation".to_string())
        })?;

        Ok(body.reserved_timeslots)
    }
}
