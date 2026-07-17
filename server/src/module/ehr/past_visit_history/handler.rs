use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};

use crate::core::error::AppResult;
use crate::core::RequestId;

use super::service::PastVisitHistoryService;

#[derive(Debug, Clone, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct PastVisitQuery {
    pub patient_account_id: i32,
    pub patient_profile_id: i32,
}

pub fn routes(svc: Arc<PastVisitHistoryService>) -> Router {
    Router::new()
        .route("/past-visit", get(get_past_visits))
        .with_state(svc)
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PastVisitDoctorInfo {
    pub doctor_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PastVisit {
    pub booking_id: String,
    pub consultation_start_time: i64,
    pub consultation_end_time: i64,
    pub doctor_info: PastVisitDoctorInfo,
}

impl PastVisit {
    pub fn new(
        booking_id: String,
        consultation_start_time: i64,
        consultation_end_time: i64,
        doctor_name: String,
    ) -> Self {
        Self {
            booking_id,
            consultation_start_time,
            consultation_end_time,
            doctor_info: PastVisitDoctorInfo { doctor_name },
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(tag = "__type")]
pub enum GetPastVisitHistoryResult {
    #[serde(rename = "PastVisits")]
    PastVisits {
        #[serde(rename = "pastVisits")]
        past_visits: Vec<PastVisit>,
    },
    #[serde(rename = "NotFound")]
    NotFound,
    #[serde(rename = "Unauthorized")]
    Unauthorized,
}

#[utoipa::path(
    get,
    path = "/ehr/v1/past-visit",
    tag = "ehr",
    params(PastVisitQuery),
    responses(
        (status = 200, description = "List of past visits", body = GetPastVisitHistoryResult),
        (status = 401, description = "Unauthorized"),
    ),
    security(("TDH-SEC-IAM-USER-IDENTITY" = []))
)]
pub async fn get_past_visits(
    State(service): State<Arc<PastVisitHistoryService>>,
    Query(query): Query<PastVisitQuery>,
    request_id: RequestId,
) -> AppResult<Json<GetPastVisitHistoryResult>> {
    let visits = service
        .get_past_visits(
            &request_id.0,
            query.patient_account_id,
            query.patient_profile_id,
        )
        .await?;
    Ok(Json(visits))
}
