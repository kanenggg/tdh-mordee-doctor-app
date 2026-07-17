use axum::{extract::State, routing::get, Json, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

use crate::core::error::AppResult;
use crate::core::extractors::PatientHeaders;
use crate::core::RequestId;

use super::service::LabResultService;

pub fn routes(svc: Arc<LabResultService>) -> Router {
    Router::new()
        .route("/lab-result", get(get_lab_result))
        .with_state(svc)
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LabResult {
    pub id: i32,
    pub lab_result_name: String,
    pub serviced_date: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(tag = "__type")]
pub enum GetLabResultsResult {
    #[serde(rename = "LabResults")]
    LabResults { lab_results: Vec<LabResult> },
    #[serde(rename = "NotFound")]
    NotFound,
    #[serde(rename = "Unauthorized")]
    Unauthorized,
}

#[utoipa::path(
    get,
    path = "/ehr/v1/lab-result",
    tag = "ehr",
    params(
        ("PATIENT-ACCOUNT-ID" = i32, Header, description = "Patient account ID"),
        ("PATIENT-PROFILE-ID" = i32, Header, description = "Patient profile ID"),
    ),
    responses(
        (status = 200, description = "List of lab results", body = GetLabResultsResult),
        (status = 401, description = "Unauthorized"),
    ),
    security(("TDH-SEC-IAM-USER-IDENTITY" = []))
)]
pub async fn get_lab_result(
    State(service): State<Arc<LabResultService>>,
    patient: PatientHeaders,
    request_id: RequestId,
) -> AppResult<Json<GetLabResultsResult>> {
    let results = service
        .get_lab_results(
            &request_id.0,
            patient.patient_account_id,
            patient.patient_profile_id,
        )
        .await?;
    Ok(Json(results))
}
