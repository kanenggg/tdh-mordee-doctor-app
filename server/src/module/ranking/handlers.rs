use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Json,
};
use futures::future::join_all;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use uuid::Uuid;

use crate::core::error::{AppError, AppResult};

use super::cache::RankingCacheTrait;
use super::language::{Language, LanguagePreference};
use super::models::{
    AssociatePrivilege, DoctorListData, DoctorListResponse, DoctorProfile, DoctorProfileResponse,
    PageToken, PagingMetaData, RankingQuery,
};
use super::privilege::PrivilegeServiceTrait;
use super::repo::{ListMode, RankingRepoTrait};

#[derive(Clone)]
pub struct RankingState {
    pub repo: Arc<dyn RankingRepoTrait>,
    pub cache: Arc<dyn RankingCacheTrait>,
    pub privilege_svc: Arc<dyn PrivilegeServiceTrait>,
}

/// GET /ranking/v1/doctors/instant
#[utoipa::path(
    get,
    path = "/ranking/v1/doctors/instant",
    params(RankingQuery),
    responses(
        (status = 200, description = "List of instant-available doctors", body = DoctorListResponse),
        (status = 400, description = "Invalid page token"),
    )
)]
pub async fn list_instant_doctors(
    LanguagePreference(lang): LanguagePreference,
    Query(params): Query<RankingQuery>,
    State(state): State<RankingState>,
) -> AppResult<impl IntoResponse> {
    list_doctors(ListMode::Instant, params, state, &lang).await
}

/// GET /ranking/v1/doctors/scheduled
#[utoipa::path(
    get,
    path = "/ranking/v1/doctors/scheduled",
    params(RankingQuery),
    responses(
        (status = 200, description = "List of scheduled-available doctors", body = DoctorListResponse),
        (status = 400, description = "Invalid page token"),
    )
)]
pub async fn list_scheduled_doctors(
    LanguagePreference(lang): LanguagePreference,
    Query(params): Query<RankingQuery>,
    State(state): State<RankingState>,
) -> AppResult<impl IntoResponse> {
    list_doctors(ListMode::Scheduled, params, state, &lang).await
}

/// GET /ranking/v1/doctor/{doctor_uuid}
#[utoipa::path(
    get,
    path = "/ranking/v1/doctor/{doctor_uuid}",
    params(("doctor_uuid" = String, Path, description = "Doctor UUID")),
    responses(
        (status = 200, description = "Doctor profile", body = DoctorProfileResponse),
    )
)]
pub async fn get_doctor_profile(
    LanguagePreference(lang): LanguagePreference,
    Path(doctor_uuid): Path<String>,
    State(state): State<RankingState>,
) -> AppResult<impl IntoResponse> {
    let doctor_uuid = Uuid::parse_str(&doctor_uuid)
        .map_err(|_| AppError::BadRequest("Invalid doctor UUID".to_string()))?;

    // Try cache first — always fetch fresh ranks from Redis
    if let Some(cached) = state.cache.get_profile(doctor_uuid, lang.json_key()).await {
        let (ranked, i_ranked) = fetch_ranks(doctor_uuid, &state).await;
        let profile = with_ranks(cached, ranked, i_ranked);
        return Ok(Json(DoctorProfileResponse {
            message: "DOCTOR_PROFILE_SUCCEDDED".to_string(),
            return_type: "object".to_string(),
            data: profile,
        }));
    }

    // Fallback to database
    let profile = state.repo.get_doctor(doctor_uuid, &lang).await?;

    match profile {
        Some(profile) => {
            // Enrich immutably with privileges and Redis ranks
            let privileges = if let Some(department_id) = profile.department_id {
                fetch_privileges(department_id, &state).await
            } else {
                vec![]
            };
            let (ranked, i_ranked) = fetch_ranks(doctor_uuid, &state).await;

            let profile = with_privileges(profile, privileges);
            let profile = with_ranks(profile, ranked, i_ranked);

            // Cache profile WITHOUT rank data (ranks are always fetched fresh from Redis)
            let cacheable = DoctorProfile {
                ranked: 0,
                i_ranked: 0,
                ..profile.clone()
            };
            state
                .cache
                .set_profile(doctor_uuid, lang.json_key(), &cacheable)
                .await;

            Ok(Json(DoctorProfileResponse {
                message: "DOCTOR_PROFILE_SUCCEDDED".to_string(),
                return_type: "object".to_string(),
                data: profile,
            }))
        }
        None => {
            // Return empty profile matching legacy behavior
            Ok(Json(DoctorProfileResponse {
                message: "DOCTOR_PROFILE_SUCCEDDED".to_string(),
                return_type: "object".to_string(),
                data: empty_doctor_profile(),
            }))
        }
    }
}

async fn list_doctors(
    mode: ListMode,
    params: RankingQuery,
    state: RankingState,
    lang: &Language,
) -> AppResult<impl IntoResponse> {
    let size = params.size.min(100);

    let page_token = match &params.page_token {
        Some(token_str) => {
            let token =
                PageToken::decode(token_str).map_err(|e| AppError::BadRequest(e.to_string()))?;
            Some(token)
        }
        None => None,
    };

    let raw_doctors = state
        .repo
        .list_doctors(mode, page_token.as_ref(), size, lang)
        .await?;

    let doctor_ids: Vec<Uuid> = raw_doctors
        .iter()
        .filter_map(|doctor| Uuid::parse_str(&doctor.usid).ok())
        .collect();
    let page_contains_all_doctors = page_token.is_none() && (raw_doctors.len() as u32) < size;
    let total_from_page = raw_doctors.len() as i64;
    let (privileges_by_department, scheduled_ranks, instant_ranks, total) =
        if page_contains_all_doctors {
            let (privileges_by_department, scheduled_ranks, instant_ranks) = tokio::join!(
                fetch_privileges_by_department(&raw_doctors, &state),
                state.cache.get_scheduled_ranks(&doctor_ids),
                state.cache.get_instant_ranks(&doctor_ids),
            );
            (
                privileges_by_department,
                scheduled_ranks,
                instant_ranks,
                total_from_page,
            )
        } else {
            let (privileges_by_department, scheduled_ranks, instant_ranks, total) = tokio::join!(
                fetch_privileges_by_department(&raw_doctors, &state),
                state.cache.get_scheduled_ranks(&doctor_ids),
                state.cache.get_instant_ranks(&doctor_ids),
                state.repo.count_doctors(mode),
            );
            (
                privileges_by_department,
                scheduled_ranks,
                instant_ranks,
                total?,
            )
        };
    let ranks_by_doctor: HashMap<Uuid, (i32, i32)> = doctor_ids
        .into_iter()
        .zip(scheduled_ranks.into_iter().zip(instant_ranks.into_iter()))
        .collect();

    // Enrich immutably with privileges and Redis ranks.
    let doctors: Vec<DoctorProfile> = {
        let mut enriched = Vec::with_capacity(raw_doctors.len());
        let mut cacheable_profiles = Vec::with_capacity(raw_doctors.len());
        for doctor in raw_doctors {
            let privileges = doctor
                .department_id
                .and_then(|department_id| privileges_by_department.get(&department_id))
                .cloned()
                .unwrap_or_default();
            let uuid = Uuid::parse_str(&doctor.usid).ok();

            let cacheable = with_privileges(doctor, privileges);
            if let Some(uuid) = uuid {
                cacheable_profiles.push((uuid, cacheable.clone()));
            }

            let (ranked, i_ranked) = uuid
                .and_then(|uuid| ranks_by_doctor.get(&uuid).copied())
                .unwrap_or((0, 0));
            let doctor = with_ranks(cacheable, ranked, i_ranked);
            enriched.push(doctor);
        }
        state
            .cache
            .set_profiles(&cacheable_profiles, lang.json_key())
            .await;
        enriched
    };

    // Generate next page token
    let next_page_token = if (doctors.len() as u32) < size {
        None
    } else {
        doctors.last().map(|last| {
            let uuid = Uuid::parse_str(&last.usid).unwrap_or_default();
            PageToken::new(last.score, uuid).encode()
        })
    };

    Ok(Json(DoctorListResponse {
        message: "DOCTOR_LISTING_SUCCEDDED".to_string(),
        return_type: "object".to_string(),
        data: DoctorListData {
            doctors,
            paging_meta_data: PagingMetaData {
                size,
                total,
                next_page_token,
            },
        },
    }))
}

fn with_privileges(profile: DoctorProfile, privileges: Vec<AssociatePrivilege>) -> DoctorProfile {
    DoctorProfile {
        associate_privileges: privileges,
        ..profile
    }
}

fn with_ranks(profile: DoctorProfile, ranked: i32, i_ranked: i32) -> DoctorProfile {
    DoctorProfile {
        ranked,
        i_ranked,
        ..profile
    }
}

async fn fetch_ranks(doctor_uuid: Uuid, state: &RankingState) -> (i32, i32) {
    let ranked = state.cache.get_scheduled_rank(doctor_uuid).await;
    let i_ranked = state.cache.get_instant_rank(doctor_uuid).await;
    (ranked, i_ranked)
}

async fn fetch_privileges(department_id: i32, state: &RankingState) -> Vec<AssociatePrivilege> {
    // Check cache first (cached as AssociatePrivilege)
    if let Some(cached) = state.cache.get_privileges(department_id).await {
        return cached;
    }

    // Fetch from service, convert to legacy shape, and cache
    let benefits: Vec<AssociatePrivilege> = state
        .privilege_svc
        .get_benefits(department_id)
        .await
        .into_iter()
        .map(Into::into)
        .collect();
    state.cache.set_privileges(department_id, &benefits).await;
    benefits
}

async fn fetch_privileges_by_department(
    doctors: &[DoctorProfile],
    state: &RankingState,
) -> HashMap<i32, Vec<AssociatePrivilege>> {
    let department_ids: HashSet<i32> = doctors
        .iter()
        .filter_map(|doctor| doctor.department_id)
        .collect();

    let mut privileges_by_department = HashMap::with_capacity(department_ids.len());
    let department_privileges =
        join_all(department_ids.into_iter().map(|department_id| async move {
            let privileges = fetch_privileges(department_id, state).await;
            (department_id, privileges)
        }))
        .await;
    for (department_id, privileges) in department_privileges {
        privileges_by_department.insert(department_id, privileges);
    }

    privileges_by_department
}

fn empty_doctor_profile() -> DoctorProfile {
    DoctorProfile {
        usid: String::new(),
        name: vec![],
        profile_image: String::new(),
        specialties: vec![],
        channels: vec![],
        work_place: vec![],
        counseling_areas: vec![],
        work_experience: vec![],
        specialty_desc: vec![],
        consultation_case: 0,
        rating: 0.0,
        available_language: vec![],
        consultation_fee: 0.0,
        consultation_duration: 0,
        department_id: None,
        associate_privileges: vec![],
        score: 0,
        ranked: 0,
        i_ranked: 0,
    }
}
