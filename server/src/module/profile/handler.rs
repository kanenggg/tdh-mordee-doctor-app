use crate::core::auth::DoctorIdentity;
use crate::core::error::AppResult;
use crate::core::logging::RequestId;
use crate::model::onboarding::{Address, Documents, Education, SelectedWorkPlace};
use crate::model::ref_data::{AcademicPosition, Profession};
use crate::module::profile::repo::ProfileRepoTrait;
use axum::extract::State;
use axum::routing::get;
use axum::Json;
use axum::Router;
use serde::Serialize;
use std::sync::Arc;
use tdh_protocol::common::Localized;
use tracing::error;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DoctorProfile {
    pub profession: Profession,
    pub academic_position: AcademicPosition,
    pub citizen_id: String,
    pub first_name: Localized,
    pub last_name: Localized,
    pub department: i32,
    pub address: Address,
    pub work_place: SelectedWorkPlace,
    pub education: Education,
    pub documents: Documents,
}

#[derive(Serialize, ToSchema)]
#[serde(tag = "__type")]
pub enum DoctorProfileResponse {
    UserProfile(Box<DoctorProfile>),
    UserProfileNotFound,
}

#[derive(Serialize, ToSchema)]
#[serde(tag = "__type")]
pub enum DoctorIdResponse {
    DoctorId {
        #[serde(rename = "doctorId")]
        doctor_id: String,
    },
    DoctorIdNotFound,
}

#[utoipa::path(
    get,
    path = "/profile/v1",
    tag = "profile",
    responses(
        (status = 200, description = "Doctor profile", body = DoctorProfileResponse),
        (status = 401, description = "Unauthorized"),
    ),
    security(("TDH-SEC-IAM-USER-IDENTITY" = []))
)]
pub async fn get_profile(
    State(repo): State<Arc<dyn ProfileRepoTrait>>,
    request_id: RequestId,
    identity: DoctorIdentity,
) -> AppResult<Json<DoctorProfileResponse>> {
    let profile = repo
        .get_doctor_profile(identity.doctor_account_id)
        .await
        .inspect_err(|e| {
            error!(
                doctor_account_id = identity.doctor_account_id,
                request_id = request_id.0,
                service = "ProfileHandler",
                error = ?e,
                "get_doctor_profile failed"
            );
        })?;
    Ok(Json(match profile {
        Some(info) => DoctorProfileResponse::UserProfile(Box::new(info)),
        None => DoctorProfileResponse::UserProfileNotFound,
    }))
}

#[utoipa::path(
    get,
    path = "/profile/v1/doctor-id",
    tag = "profile",
    responses(
        (status = 200, description = "Doctor UUID or DoctorIdNotFound", body = DoctorIdResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("TDH-SEC-IAM-USER-IDENTITY" = []))
)]
pub async fn get_doctor_id(
    State(repo): State<Arc<dyn ProfileRepoTrait>>,
    request_id: RequestId,
    identity: DoctorIdentity,
) -> AppResult<Json<DoctorIdResponse>> {
    let uuid = repo
        .get_doctor_uuid(identity.doctor_account_id)
        .await
        .inspect_err(|e| {
            error!(
                doctor_account_id = identity.doctor_account_id,
                request_id = request_id.0,
                service = "ProfileHandler",
                error = ?e,
                "get_doctor_uuid failed"
            );
        })?;
    Ok(Json(match uuid {
        Some(doctor_id) => DoctorIdResponse::DoctorId { doctor_id },
        None => DoctorIdResponse::DoctorIdNotFound,
    }))
}

pub fn routes(repo: Arc<dyn ProfileRepoTrait>) -> Router {
    Router::new()
        .route("/v1", get(get_profile))
        .route("/v1/doctor-id", get(get_doctor_id))
        .with_state(repo)
}
