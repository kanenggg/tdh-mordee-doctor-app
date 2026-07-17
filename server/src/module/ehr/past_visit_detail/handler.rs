use std::sync::Arc;

use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::core::error::AppResult;
use crate::core::RequestId;

use super::service::PastVisitDetailService;

pub fn routes(svc: Arc<PastVisitDetailService>) -> Router {
    Router::new()
        .route("/past-visit/{bookingId}", get(get_past_visit_detail))
        .with_state(svc)
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PastVisitDetailResponse {
    pub appointment_id: String,
    pub appointment_date: String,
    pub appointment_time: TimeRange,
    pub consultation_channel: String,
    pub doctor: DoctorInfo,
    pub summary_note: SummaryNote,
    pub prescription_items: Vec<PrescriptionItem>,
    pub follow_up: FollowUp,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimeRange {
    pub start_time: String,
    pub end_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DoctorInfo {
    pub id: String,
    pub name: String,
    pub specialties: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SummaryNote {
    pub present_illness: String,
    pub chief_complaint: String,
    pub diagnosis: String,
    pub recommendations: String,
    pub icd10: Vec<Icd10>,
    pub drug_allergy_info: DrugAllergyInfo,
    pub illness_duration: IllnessDuration,
    pub note_to_staff: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Icd10 {
    pub code: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "__type", rename_all_fields = "camelCase")]
pub enum DrugAllergyInfo {
    #[serde(rename_all = "camelCase")]
    NoDrugAllergies,
    #[serde(rename_all = "camelCase")]
    HasDrugAllergies { drug_allergies: Vec<DrugAllergy> },
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DrugAllergy {
    pub id: i32,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct IllnessDuration {
    pub value: i32,
    pub unit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PrescriptionItem {
    pub med_id: i32,
    pub name: String,
    pub quantity: i32,
    pub unit: String,
    pub dosage_instructions: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "__type", rename_all_fields = "camelCase")]
pub enum FollowUp {
    #[serde(rename_all = "camelCase")]
    AsNeeded { note_to_staff: String },
    #[serde(rename_all = "camelCase")]
    ScheduleAppointment {
        follow_up_date: String,
        follow_up_time: TimeRange,
        visit_type: String,
        note_to_patient: String,
        note_to_staff: String,
    },
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(tag = "__type")]
pub enum GetPastVisitDetailResult {
    PastVisitDetail(PastVisitDetailResponse),
    NotFound,
    NotFulfilled,
}

#[utoipa::path(
    get,
    path = "/ehr/v1/past-visit/{bookingId}",
    tag = "ehr",
    params(
        ("bookingId" = String, Path, description = "Booking ID"),
    ),
    responses(
        (status = 200, description = "Past visit detail (Success / NotFound / NotFulfilled)", body = GetPastVisitDetailResult),
    ),
    security(("TDH-SEC-IAM-USER-IDENTITY" = []))
)]
pub async fn get_past_visit_detail(
    State(service): State<Arc<PastVisitDetailService>>,
    Path(booking_id): Path<String>,
    request_id: RequestId,
) -> AppResult<Json<GetPastVisitDetailResult>> {
    let result = service
        .get_past_visit_detail(&request_id.0, &booking_id)
        .await?;
    Ok(Json(result))
}
