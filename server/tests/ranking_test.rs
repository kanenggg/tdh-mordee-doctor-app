//! Integration tests for doctor ranking API endpoints.
//!
//! Uses mock implementations of RankingRepoTrait, RankingCacheTrait, and
//! PrivilegeServiceTrait to test handlers in isolation.
//!
//! Test coverage:
//! - GET /ranking/v1/doctors/instant - first page, pagination, empty results
//! - GET /ranking/v1/doctors/scheduled - basic listing
//! - GET /ranking/v1/doctor/{uuid} - found, not found
//! - Page token: invalid token → 400

use async_trait::async_trait;
use axum::Router;
use axum_test::TestServer;
use deadpool_redis::{Config as RedisConfig, Runtime};
use futures::FutureExt;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use testcontainers::{runners::AsyncRunner, ContainerAsync};
use testcontainers_modules::redis::{Redis, REDIS_PORT};
use uuid::Uuid;

mod common;

use server::module::ranking::cache::{RankingCache, RankingCacheTrait};
use server::module::ranking::language::Language;
use server::module::ranking::models::{
    AssociatePrivilege, ChannelInfo, DoctorProfile, LocalizedName, PageToken, PrivilegeBenefit,
    RankedDoctorInfo, SpecialtyInfo, WorkplaceInfo,
};
use server::module::ranking::privilege::PrivilegeServiceTrait;
use server::module::ranking::repo::{ListMode, RankingRepo, RankingRepoTrait};

// ============================================================================
// Mock Implementations
// ============================================================================

fn make_doctor(i: usize, score: i32) -> DoctorProfile {
    DoctorProfile {
        usid: test_uuid(i).to_string(),
        name: vec![LocalizedName {
            lang_code: "en-US".to_string(),
            name: format!("Doctor {}, M.D.", i),
        }],
        profile_image: format!("https://example.com/doctor-{}.png", i),
        specialties: vec![SpecialtyInfo {
            id: 10,
            name: "Mental Health".to_string(),
            lang_code: "en-US".to_string(),
        }],
        channels: vec![ChannelInfo {
            channel_type: "Chat".to_string(),
            duration: 15,
            price: 300.0,
            currency: "THB".to_string(),
        }],
        work_place: vec![WorkplaceInfo {
            id: 1,
            name: "Test Hospital".to_string(),
            lang_code: "en-US".to_string(),
        }],
        counseling_areas: vec![LocalizedName {
            lang_code: "en-US".to_string(),
            name: "Stress and anxiety\nSleep support".to_string(),
        }],
        work_experience: vec![LocalizedName {
            lang_code: "en-US".to_string(),
            name: "Test Hospital".to_string(),
        }],
        specialty_desc: vec![LocalizedName {
            lang_code: "en-US".to_string(),
            name: "Telehealth consultation".to_string(),
        }],
        consultation_case: 100 + i as i32,
        rating: 4.5,
        available_language: vec!["th-TH".to_string(), "en-US".to_string()],
        consultation_fee: 300.0,
        consultation_duration: 15,
        department_id: Some(10),
        associate_privileges: vec![],
        score,
        ranked: i as i32,
        i_ranked: i as i32,
    }
}

fn test_uuid(i: usize) -> Uuid {
    Uuid::parse_str(&format!(
        "a0000{:03}-{:04}-4000-8000-00000000{:04}",
        i, i, i
    ))
    .unwrap()
}

async fn seed_ranked_doctor(
    pool: &PgPool,
    doctor_id: Uuid,
    account_id: i32,
    is_active: bool,
    score: i32,
    instant: bool,
    scheduled: bool,
    with_config: bool,
) {
    sqlx::query(
        r#"
        INSERT INTO department (department_id, name, counseling_areas)
        VALUES (10, $1::jsonb, $2::jsonb)
        ON CONFLICT (department_id) DO NOTHING
        "#,
    )
    .bind(json!({"th": "สุขภาพจิต", "en": "Mental Health"}))
    .bind(json!({
        "th": "ความเครียดและความวิตกกังวล\nการนอนหลับ",
        "en": "Stress and anxiety\nSleep support"
    }))
    .execute(pool)
    .await
    .expect("failed to seed department counseling areas");

    sqlx::query(
        r#"
        INSERT INTO doctor_profile (
            doctor_id, doctor_account_id, doctor_profile_id, citizen_id,
            first_name, last_name, department_id, license_number,
            specialty, special_interest, address_detail, sub_district,
            district, province, postal_code, work_place, additional_workplace,
            profile_image_url, id_card_image_url, book_bank_image_url,
            medical_license_image_url, is_active
        ) VALUES (
            $1, $2, $3, $4,
            $5::jsonb, $6::jsonb, 10, $7,
            $8::jsonb, ARRAY['Telehealth consultation'], 'addr', '{}'::jsonb,
            '{}'::jsonb, '{}'::jsonb, 10000, $9::jsonb, $10::jsonb,
            $11, 'id.png', 'bank.png', 'license.png', $12
        )
        "#,
    )
    .bind(doctor_id)
    .bind(account_id)
    .bind(account_id + 1000)
    .bind(format!("{account_id:013}"))
    .bind(json!({"th": format!("ชื่อ{account_id}"), "en": format!("Doctor{account_id}")}))
    .bind(json!({"th": "ทดสอบ", "en": "Test"}))
    .bind(format!("LIC-{account_id}"))
    .bind(json!({"id": 208, "name": {"th": "สุขภาพจิต", "en": "Mental Health"}}))
    .bind(json!([
        {"id": 1, "name": {"th": "โรงพยาบาลทดสอบ", "en": "Test Hospital"}}
    ]))
    .bind(json!([
        {"id": 2, "name": {"th": "คลินิกทดสอบ", "en": "Test Clinic"}}
    ]))
    .bind(format!("https://example.com/{account_id}.png"))
    .bind(is_active)
    .execute(pool)
    .await
    .expect("failed to seed doctor_profile");

    sqlx::query("INSERT INTO doctor_score (doctor_id, score) VALUES ($1, $2)")
        .bind(doctor_id)
        .bind(score)
        .execute(pool)
        .await
        .expect("failed to seed doctor_score");

    sqlx::query(
        r#"
        INSERT INTO consultation_instant (
            doctor_id, biz_unit_id, is_available
        ) VALUES ($1, 1, $2)
        "#,
    )
    .bind(account_id)
    .bind(instant)
    .execute(pool)
    .await
    .expect("failed to seed consultation_instant");

    sqlx::query(
        r#"
        INSERT INTO consultation_schedule (
            doctor_id, biz_unit_id, is_available
        ) VALUES ($1, 1, $2)
        "#,
    )
    .bind(account_id)
    .bind(scheduled)
    .execute(pool)
    .await
    .expect("failed to seed consultation_schedule");

    sqlx::query("INSERT INTO doctor_rating (doctor_id, rating) VALUES ($1, 4.7)")
        .bind(doctor_id)
        .execute(pool)
        .await
        .expect("failed to seed doctor_rating");

    sqlx::query("INSERT INTO doctor_case (doctor_id, case_amount) VALUES ($1, 42)")
        .bind(doctor_id)
        .execute(pool)
        .await
        .expect("failed to seed doctor_case");

    if with_config {
        sqlx::query(
            r#"
            INSERT INTO doctor_consultation_config (
                doctor_id, supported_languages, channel_types,
                duration_minutes, doctor_fee_amount
            ) VALUES (
                $1, '{en}'::language_code_enum[],
                '{chat,video}'::channel_type_enum[], 25, 500.00
            )
            "#,
        )
        .bind(doctor_id)
        .execute(pool)
        .await
        .expect("failed to seed doctor_consultation_config");
    }
}

struct MockRankingRepo {
    doctors: Vec<DoctorProfile>,
}

impl MockRankingRepo {
    fn new(count: usize) -> Self {
        let doctors: Vec<DoctorProfile> = (1..=count)
            .map(|i| make_doctor(i, 100 - i as i32))
            .collect();
        Self { doctors }
    }
}

#[async_trait]
impl RankingRepoTrait for MockRankingRepo {
    async fn list_doctors(
        &self,
        _mode: ListMode,
        page_token: Option<&PageToken>,
        size: u32,
        _lang: &Language,
    ) -> server::core::error::AppResult<Vec<DoctorProfile>> {
        let filtered: Vec<DoctorProfile> = match page_token {
            Some(token) => self
                .doctors
                .iter()
                .filter(|d| {
                    let uuid = Uuid::parse_str(&d.usid).unwrap_or_default();
                    (d.score, uuid) < (token.s, token.u)
                })
                .take(size as usize)
                .cloned()
                .collect(),
            None => self.doctors.iter().take(size as usize).cloned().collect(),
        };
        Ok(filtered)
    }

    async fn get_doctor(
        &self,
        doctor_id: Uuid,
        _lang: &Language,
    ) -> server::core::error::AppResult<Option<DoctorProfile>> {
        Ok(self
            .doctors
            .iter()
            .find(|d| d.usid == doctor_id.to_string())
            .cloned())
    }

    async fn get_all_ranked_doctors(
        &self,
    ) -> server::core::error::AppResult<Vec<RankedDoctorInfo>> {
        Ok(vec![])
    }

    async fn count_doctors(&self, _mode: ListMode) -> server::core::error::AppResult<i64> {
        Ok(self.doctors.len() as i64)
    }

    async fn get_doctor_score(
        &self,
        doctor_id: Uuid,
    ) -> server::core::error::AppResult<Option<i32>> {
        Ok(self
            .doctors
            .iter()
            .find(|d| d.usid == doctor_id.to_string())
            .map(|d| d.score))
    }
}

struct MockCache;

#[async_trait]
impl RankingCacheTrait for MockCache {
    async fn warm_up(&self, _doctors: &[RankedDoctorInfo]) {}
    async fn get_profile(&self, _doctor_id: Uuid, _lang_key: &str) -> Option<DoctorProfile> {
        None
    }
    async fn get_profiles(
        &self,
        doctor_ids: &[Uuid],
        _lang_key: &str,
    ) -> Vec<Option<DoctorProfile>> {
        vec![None; doctor_ids.len()]
    }
    async fn set_profile(&self, _doctor_id: Uuid, _lang_key: &str, _profile: &DoctorProfile) {}
    async fn set_profiles(&self, _profiles: &[(Uuid, DoctorProfile)], _lang_key: &str) {}
    async fn invalidate_profile(&self, _doctor_id: Uuid) {}
    async fn remove_from_instant(&self, _doctor_id: Uuid) {}
    async fn add_to_instant(&self, _doctor_id: Uuid, _score: f64) {}
    async fn remove_from_scheduled(&self, _doctor_id: Uuid) {}
    async fn add_to_scheduled(&self, _doctor_id: Uuid, _score: f64) {}
    async fn instant_count(&self) -> i64 {
        0
    }
    async fn scheduled_count(&self) -> i64 {
        0
    }
    async fn get_privileges(&self, _specialty_id: i32) -> Option<Vec<AssociatePrivilege>> {
        None
    }
    async fn set_privileges(&self, _specialty_id: i32, _benefits: &[AssociatePrivilege]) {}
    async fn get_scheduled_rank(&self, _doctor_id: Uuid) -> i32 {
        0
    }
    async fn get_scheduled_ranks(&self, doctor_ids: &[Uuid]) -> Vec<i32> {
        vec![0; doctor_ids.len()]
    }
    async fn get_instant_rank(&self, _doctor_id: Uuid) -> i32 {
        0
    }
    async fn get_instant_ranks(&self, doctor_ids: &[Uuid]) -> Vec<i32> {
        vec![0; doctor_ids.len()]
    }
}

struct MockCacheWithCount {
    count: i64,
}

#[async_trait]
impl RankingCacheTrait for MockCacheWithCount {
    async fn warm_up(&self, _doctors: &[RankedDoctorInfo]) {}
    async fn get_profile(&self, _doctor_id: Uuid, _lang_key: &str) -> Option<DoctorProfile> {
        None
    }
    async fn get_profiles(
        &self,
        doctor_ids: &[Uuid],
        _lang_key: &str,
    ) -> Vec<Option<DoctorProfile>> {
        vec![None; doctor_ids.len()]
    }
    async fn set_profile(&self, _doctor_id: Uuid, _lang_key: &str, _profile: &DoctorProfile) {}
    async fn set_profiles(&self, _profiles: &[(Uuid, DoctorProfile)], _lang_key: &str) {}
    async fn invalidate_profile(&self, _doctor_id: Uuid) {}
    async fn remove_from_instant(&self, _doctor_id: Uuid) {}
    async fn add_to_instant(&self, _doctor_id: Uuid, _score: f64) {}
    async fn remove_from_scheduled(&self, _doctor_id: Uuid) {}
    async fn add_to_scheduled(&self, _doctor_id: Uuid, _score: f64) {}
    async fn instant_count(&self) -> i64 {
        self.count
    }
    async fn scheduled_count(&self) -> i64 {
        self.count
    }
    async fn get_privileges(&self, _specialty_id: i32) -> Option<Vec<AssociatePrivilege>> {
        None
    }
    async fn set_privileges(&self, _specialty_id: i32, _benefits: &[AssociatePrivilege]) {}
    async fn get_scheduled_rank(&self, _doctor_id: Uuid) -> i32 {
        0
    }
    async fn get_scheduled_ranks(&self, doctor_ids: &[Uuid]) -> Vec<i32> {
        vec![0; doctor_ids.len()]
    }
    async fn get_instant_rank(&self, _doctor_id: Uuid) -> i32 {
        0
    }
    async fn get_instant_ranks(&self, doctor_ids: &[Uuid]) -> Vec<i32> {
        vec![0; doctor_ids.len()]
    }
}

struct MockPrivilegeService;

#[async_trait]
impl PrivilegeServiceTrait for MockPrivilegeService {
    async fn get_benefits(&self, department_id: i32) -> Vec<PrivilegeBenefit> {
        if department_id != 10 {
            return vec![];
        }

        vec![PrivilegeBenefit {
            privilege_id: 2011,
            privilege_display_name: "Test Insurance".to_string(),
            provider_id: 100,
            provider_name: "Test Provider".to_string(),
            provider_abbreviation: "TP".to_string(),
            package_type_id: 1,
            package_type_name: Some("Gold".to_string()),
            benefits: vec!["Claimable and deduct from OPD".to_string()],
            instruction_html: "<p>Test instructions</p>".to_string(),
            company_logo_url: Some("https://example.com/logo.png".to_string()),
        }]
    }
}

struct CountingPrivilegeService {
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl PrivilegeServiceTrait for CountingPrivilegeService {
    async fn get_benefits(&self, department_id: i32) -> Vec<PrivilegeBenefit> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        MockPrivilegeService.get_benefits(department_id).await
    }
}

struct SlowCountRepo {
    inner: MockRankingRepo,
    delay: Duration,
}

#[async_trait]
impl RankingRepoTrait for SlowCountRepo {
    async fn list_doctors(
        &self,
        mode: ListMode,
        page_token: Option<&PageToken>,
        size: u32,
        lang: &Language,
    ) -> server::core::error::AppResult<Vec<DoctorProfile>> {
        self.inner.list_doctors(mode, page_token, size, lang).await
    }

    async fn get_doctor(
        &self,
        doctor_id: Uuid,
        lang: &Language,
    ) -> server::core::error::AppResult<Option<DoctorProfile>> {
        self.inner.get_doctor(doctor_id, lang).await
    }

    async fn get_all_ranked_doctors(
        &self,
    ) -> server::core::error::AppResult<Vec<RankedDoctorInfo>> {
        self.inner.get_all_ranked_doctors().await
    }

    async fn count_doctors(&self, mode: ListMode) -> server::core::error::AppResult<i64> {
        tokio::time::sleep(self.delay).await;
        self.inner.count_doctors(mode).await
    }

    async fn get_doctor_score(
        &self,
        doctor_id: Uuid,
    ) -> server::core::error::AppResult<Option<i32>> {
        self.inner.get_doctor_score(doctor_id).await
    }
}

struct CountCallsRepo {
    inner: MockRankingRepo,
    count_calls: Arc<AtomicUsize>,
}

#[async_trait]
impl RankingRepoTrait for CountCallsRepo {
    async fn list_doctors(
        &self,
        mode: ListMode,
        page_token: Option<&PageToken>,
        size: u32,
        lang: &Language,
    ) -> server::core::error::AppResult<Vec<DoctorProfile>> {
        self.inner.list_doctors(mode, page_token, size, lang).await
    }

    async fn get_doctor(
        &self,
        doctor_id: Uuid,
        lang: &Language,
    ) -> server::core::error::AppResult<Option<DoctorProfile>> {
        self.inner.get_doctor(doctor_id, lang).await
    }

    async fn get_all_ranked_doctors(
        &self,
    ) -> server::core::error::AppResult<Vec<RankedDoctorInfo>> {
        self.inner.get_all_ranked_doctors().await
    }

    async fn count_doctors(&self, mode: ListMode) -> server::core::error::AppResult<i64> {
        self.count_calls.fetch_add(1, Ordering::SeqCst);
        self.inner.count_doctors(mode).await
    }

    async fn get_doctor_score(
        &self,
        doctor_id: Uuid,
    ) -> server::core::error::AppResult<Option<i32>> {
        self.inner.get_doctor_score(doctor_id).await
    }
}

struct SlowRankCache {
    delay: Duration,
}

#[async_trait]
impl RankingCacheTrait for SlowRankCache {
    async fn warm_up(&self, _doctors: &[RankedDoctorInfo]) {}
    async fn get_profile(&self, _doctor_id: Uuid, _lang_key: &str) -> Option<DoctorProfile> {
        None
    }
    async fn get_profiles(
        &self,
        doctor_ids: &[Uuid],
        _lang_key: &str,
    ) -> Vec<Option<DoctorProfile>> {
        vec![None; doctor_ids.len()]
    }
    async fn set_profile(&self, _doctor_id: Uuid, _lang_key: &str, _profile: &DoctorProfile) {}
    async fn set_profiles(&self, _profiles: &[(Uuid, DoctorProfile)], _lang_key: &str) {}
    async fn invalidate_profile(&self, _doctor_id: Uuid) {}
    async fn remove_from_instant(&self, _doctor_id: Uuid) {}
    async fn add_to_instant(&self, _doctor_id: Uuid, _score: f64) {}
    async fn remove_from_scheduled(&self, _doctor_id: Uuid) {}
    async fn add_to_scheduled(&self, _doctor_id: Uuid, _score: f64) {}
    async fn instant_count(&self) -> i64 {
        0
    }
    async fn scheduled_count(&self) -> i64 {
        0
    }
    async fn get_privileges(&self, _specialty_id: i32) -> Option<Vec<AssociatePrivilege>> {
        None
    }
    async fn set_privileges(&self, _specialty_id: i32, _benefits: &[AssociatePrivilege]) {}
    async fn get_scheduled_rank(&self, _doctor_id: Uuid) -> i32 {
        0
    }
    async fn get_scheduled_ranks(&self, doctor_ids: &[Uuid]) -> Vec<i32> {
        tokio::time::sleep(self.delay).await;
        vec![0; doctor_ids.len()]
    }
    async fn get_instant_rank(&self, _doctor_id: Uuid) -> i32 {
        0
    }
    async fn get_instant_ranks(&self, doctor_ids: &[Uuid]) -> Vec<i32> {
        tokio::time::sleep(self.delay).await;
        vec![0; doctor_ids.len()]
    }
}

struct SlowPrivilegeService {
    delay: Duration,
}

#[async_trait]
impl PrivilegeServiceTrait for SlowPrivilegeService {
    async fn get_benefits(&self, department_id: i32) -> Vec<PrivilegeBenefit> {
        tokio::time::sleep(self.delay).await;
        MockPrivilegeService.get_benefits(department_id).await
    }
}

// ============================================================================
// Test Setup
// ============================================================================

fn build_test_app(doctor_count: usize) -> TestServer {
    let repo: Arc<dyn RankingRepoTrait> = Arc::new(MockRankingRepo::new(doctor_count));
    let cache: Arc<dyn RankingCacheTrait> = Arc::new(MockCache);
    let privilege: Arc<dyn PrivilegeServiceTrait> = Arc::new(MockPrivilegeService);

    let router = server::module::ranking::router(repo, cache, privilege);
    let app = Router::new().nest("/ranking/v1", router);

    TestServer::new(app).unwrap()
}

// ============================================================================
// Tests: PostgreSQL Repository Source Mapping
// ============================================================================

#[tokio::test]
async fn repo_lists_doctors_from_doctor_profile_and_consultation_config() {
    let (_container, pool) = common::setup_postgres().await;
    let repo = RankingRepo::new(pool.clone(), 1.2);

    let top = Uuid::parse_str("018f0000-0000-7000-8000-000000000001").unwrap();
    let lower = Uuid::parse_str("018f0000-0000-7000-8000-000000000002").unwrap();
    let inactive = Uuid::parse_str("018f0000-0000-7000-8000-000000000003").unwrap();

    seed_ranked_doctor(&pool, lower, 7002, true, 80, true, true, true).await;
    seed_ranked_doctor(&pool, top, 7001, true, 95, true, true, true).await;
    seed_ranked_doctor(&pool, inactive, 7003, false, 100, true, true, true).await;

    let doctors = repo
        .list_doctors(ListMode::Instant, None, 10, &Language::English)
        .await
        .unwrap();

    assert_eq!(doctors.len(), 2);
    assert_eq!(doctors[0].usid, top.to_string());
    assert_eq!(doctors[0].name[0].name, "Doctor7001 Test");
    assert_eq!(doctors[0].profile_image, "https://example.com/7001.png");
    assert_eq!(doctors[0].specialties[0].id, 10);
    assert_eq!(doctors[0].specialties[0].name, "Mental Health");
    assert_eq!(doctors[0].department_id, Some(10));
    assert_eq!(doctors[0].counseling_areas.len(), 1);
    assert_eq!(
        doctors[0].counseling_areas[0].name,
        "Stress and anxiety\nSleep support"
    );
    assert_eq!(doctors[0].counseling_areas[0].lang_code, "en-US");
    assert_eq!(doctors[0].specialty_desc.len(), 1);
    assert_eq!(doctors[0].specialty_desc[0].lang_code, "en-US");
    assert_eq!(doctors[0].specialty_desc[0].name, "Mental Health");
    assert_eq!(doctors[0].work_place[0].name, "Test Hospital");
    assert_eq!(doctors[0].work_experience.len(), 2);
    assert_eq!(doctors[0].work_experience[0].lang_code, "en-US");
    assert_eq!(doctors[0].work_experience[0].name, "Test Hospital");
    assert_eq!(doctors[0].work_experience[1].name, "Test Clinic");
    assert_eq!(doctors[0].available_language, vec!["en-US"]);
    assert_eq!(doctors[0].consultation_duration, 25);
    assert_eq!(doctors[0].consultation_fee, 600.0);
    assert_eq!(doctors[0].channels.len(), 2);
    assert_eq!(doctors[0].channels[0].channel_type, "Chat");
    assert_eq!(doctors[0].channels[0].duration, 25);
    assert_eq!(doctors[0].channels[0].price, 600.0);
    assert_eq!(doctors[0].consultation_case, 42);
    assert_eq!(doctors[0].rating, 4.7);

    let count = repo.count_doctors(ListMode::Instant).await.unwrap();
    assert_eq!(count, 2);
}

#[tokio::test]
async fn repo_paginates_equal_scores_by_doctor_id_desc() {
    let (_container, pool) = common::setup_postgres().await;
    let repo = RankingRepo::new(pool.clone(), 1.0);

    let lowest = Uuid::parse_str("018f0000-0000-7000-8000-000000000051").unwrap();
    let middle = Uuid::parse_str("018f0000-0000-7000-8000-000000000052").unwrap();
    let highest = Uuid::parse_str("018f0000-0000-7000-8000-000000000053").unwrap();

    seed_ranked_doctor(&pool, lowest, 7051, true, 50, true, false, false).await;
    seed_ranked_doctor(&pool, highest, 7053, true, 50, true, false, false).await;
    seed_ranked_doctor(&pool, middle, 7052, true, 50, true, false, false).await;

    let first_page = repo
        .list_doctors(ListMode::Instant, None, 2, &Language::English)
        .await
        .unwrap();

    assert_eq!(first_page.len(), 2);
    assert_eq!(first_page[0].usid, highest.to_string());
    assert_eq!(first_page[1].usid, middle.to_string());

    let token = PageToken::new(first_page[1].score, middle);
    let second_page = repo
        .list_doctors(ListMode::Instant, Some(&token), 2, &Language::English)
        .await
        .unwrap();

    assert_eq!(second_page.len(), 1);
    assert_eq!(second_page[0].usid, lowest.to_string());
}

#[tokio::test]
async fn repo_lists_available_doctors_without_score_as_zero() {
    let (_container, pool) = common::setup_postgres().await;
    let repo = RankingRepo::new(pool.clone(), 1.2);
    let doctor_id = Uuid::parse_str("018f0000-0000-7000-8000-000000000041").unwrap();

    seed_ranked_doctor(&pool, doctor_id, 7041, true, 50, true, true, false).await;
    sqlx::query("DELETE FROM doctor_score WHERE doctor_id = $1")
        .bind(doctor_id)
        .execute(&pool)
        .await
        .expect("failed to remove doctor_score row");

    let doctors = repo
        .list_doctors(ListMode::Instant, None, 10, &Language::English)
        .await
        .unwrap();

    assert_eq!(doctors.len(), 1);
    assert_eq!(doctors[0].usid, doctor_id.to_string());
    assert_eq!(doctors[0].score, 0);

    let ranked = repo.get_all_ranked_doctors().await.unwrap();
    assert_eq!(ranked.len(), 1);
    assert_eq!(ranked[0].doctor_id, doctor_id);
    assert_eq!(ranked[0].score, 0);
    assert!(ranked[0].instant_mode_enabled);
    assert!(ranked[0].schedule_mode_enabled);
}

#[tokio::test]
async fn repo_uses_department_as_specialty_when_profile_specialty_is_empty() {
    let (_container, pool) = common::setup_postgres().await;
    let repo = RankingRepo::new(pool.clone(), 1.2);
    let doctor_id = Uuid::parse_str("018f0000-0000-7000-8000-000000000042").unwrap();

    sqlx::query(
        r#"
        INSERT INTO department (department_id, name)
        VALUES (10, $1::jsonb)
        "#,
    )
    .bind(json!({"th": "สุขภาพใจ", "en": "Mental Wellness"}))
    .execute(&pool)
    .await
    .expect("failed to seed department");

    seed_ranked_doctor(&pool, doctor_id, 7042, true, 50, true, true, false).await;
    sqlx::query("UPDATE doctor_profile SET specialty = '[]'::jsonb WHERE doctor_id = $1")
        .bind(doctor_id)
        .execute(&pool)
        .await
        .expect("failed to clear profile specialty");

    let doctors = repo
        .list_doctors(ListMode::Instant, None, 10, &Language::English)
        .await
        .unwrap();

    assert_eq!(doctors.len(), 1);
    assert_eq!(doctors[0].specialties.len(), 1);
    assert_eq!(doctors[0].specialties[0].id, 10);
    assert_eq!(doctors[0].specialties[0].name, "Mental Wellness");
    assert_eq!(doctors[0].specialties[0].lang_code, "en-US");
}

#[tokio::test]
async fn repo_uses_defaults_when_consultation_config_is_missing() {
    let (_container, pool) = common::setup_postgres().await;
    let repo = RankingRepo::new(pool.clone(), 1.2);
    let doctor_id = Uuid::parse_str("018f0000-0000-7000-8000-000000000011").unwrap();

    seed_ranked_doctor(&pool, doctor_id, 7011, true, 50, true, false, false).await;

    let doctor = repo
        .get_doctor(doctor_id, &Language::English)
        .await
        .unwrap()
        .expect("doctor should be found");

    assert_eq!(doctor.available_language, vec!["th-TH", "en-US"]);
    assert_eq!(doctor.consultation_duration, 15);
    assert_eq!(doctor.consultation_fee, 0.0);
    assert_eq!(doctor.channels.len(), 3);
    assert_eq!(doctor.channels[0].channel_type, "Voice");
    assert_eq!(doctor.channels[1].channel_type, "Chat");
    assert_eq!(doctor.channels[2].channel_type, "Video");
    assert_eq!(doctor.specialties[0].id, 10);
    assert_eq!(doctor.specialties[0].name, "Mental Health");
}

fn build_test_app_with_deps(
    repo: Arc<dyn RankingRepoTrait>,
    cache: Arc<dyn RankingCacheTrait>,
    privilege: Arc<dyn PrivilegeServiceTrait>,
) -> TestServer {
    let router = server::module::ranking::router(repo, cache, privilege);
    let app = Router::new().nest("/ranking/v1", router);

    TestServer::new(app).unwrap()
}

#[tokio::test]
async fn repo_warmup_reads_ranked_active_profiles_only() {
    let (_container, pool) = common::setup_postgres().await;
    let repo = RankingRepo::new(pool.clone(), 1.0);

    let active = Uuid::parse_str("018f0000-0000-7000-8000-000000000021").unwrap();
    let inactive = Uuid::parse_str("018f0000-0000-7000-8000-000000000022").unwrap();

    seed_ranked_doctor(&pool, active, 7021, true, 70, true, false, false).await;
    seed_ranked_doctor(&pool, inactive, 7022, false, 90, true, true, false).await;

    let ranked = repo.get_all_ranked_doctors().await.unwrap();

    assert_eq!(ranked.len(), 1);
    assert_eq!(ranked[0].doctor_id, active);
    assert_eq!(ranked[0].score, 70);
    assert!(ranked[0].instant_mode_enabled);
    assert!(!ranked[0].schedule_mode_enabled);
}

#[tokio::test]
async fn repo_get_doctor_score_reads_through_doctor_profile() {
    let (_container, pool) = common::setup_postgres().await;
    let repo = RankingRepo::new(pool.clone(), 1.0);

    let with_score = Uuid::parse_str("018f0000-0000-7000-8000-000000000031").unwrap();
    let without_score = Uuid::parse_str("018f0000-0000-7000-8000-000000000032").unwrap();
    let missing_profile = Uuid::parse_str("018f0000-0000-7000-8000-000000000033").unwrap();

    seed_ranked_doctor(&pool, with_score, 7031, true, 88, true, true, false).await;
    seed_ranked_doctor(&pool, without_score, 7032, true, 40, true, true, false).await;
    sqlx::query("DELETE FROM doctor_score WHERE doctor_id = $1")
        .bind(without_score)
        .execute(&pool)
        .await
        .expect("failed to remove doctor_score row");

    assert_eq!(repo.get_doctor_score(with_score).await.unwrap(), Some(88));
    assert_eq!(repo.get_doctor_score(without_score).await.unwrap(), Some(0));
    assert_eq!(repo.get_doctor_score(missing_profile).await.unwrap(), None);
}

// ============================================================================
// Tests: List Instant Doctors
// ============================================================================

#[tokio::test]
async fn list_instant_runs_independent_enrichment_work_concurrently() {
    let delay = Duration::from_millis(200);
    let server = build_test_app_with_deps(
        Arc::new(SlowCountRepo {
            inner: MockRankingRepo::new(1),
            delay,
        }),
        Arc::new(SlowRankCache { delay }),
        Arc::new(SlowPrivilegeService { delay }),
    );

    let started = Instant::now();
    let resp = server
        .get("/ranking/v1/doctors/instant")
        .add_query_param("size", "1")
        .await;
    let elapsed = started.elapsed();

    resp.assert_status_ok();
    assert!(
        elapsed < Duration::from_millis(450),
        "independent count, privilege, and rank work should overlap; elapsed={elapsed:?}"
    );
}

#[tokio::test]
async fn test_list_instant_first_page() {
    let server = build_test_app(25);

    let resp = server
        .get("/ranking/v1/doctors/instant")
        .add_query_param("size", "10")
        .await;

    resp.assert_status_ok();
    let body: Value = resp.json();

    assert_eq!(body["message"], "DOCTOR_LISTING_SUCCEDDED");
    assert_eq!(body["returnType"], "object");

    let doctors = body["data"]["doctors"].as_array().unwrap();
    assert_eq!(doctors.len(), 10);

    // Verify sorted by score descending
    let first_score = doctors[0]["score"].as_f64().unwrap();
    let last_score = doctors[9]["score"].as_f64().unwrap();
    assert!(first_score > last_score);

    // Verify nextPageToken is present
    let next_token = body["data"]["pagingMetaData"]["nextPageToken"]
        .as_str()
        .unwrap();
    assert!(!next_token.is_empty());

    assert_eq!(body["data"]["pagingMetaData"]["size"], 10);

    // Verify doctor fields
    let first_doc = &doctors[0];
    assert!(first_doc["usid"].as_str().is_some());
    assert!(first_doc["name"].as_array().is_some());
    assert!(first_doc["profileImage"].as_str().is_some());
    assert!(first_doc["specialties"].as_array().is_some());
    assert!(first_doc["channels"].as_array().is_some());
    assert!(first_doc["workPlace"].as_array().is_some());
    assert!(first_doc["consultationCase"].as_i64().is_some());
    assert!(first_doc["rating"].as_f64().is_some());
    assert!(first_doc["consultationFee"].as_f64().is_some());

    // Verify privilege enrichment (legacy AssociatePrivilege shape)
    let privileges = first_doc["associatePrivileges"].as_array().unwrap();
    assert_eq!(privileges.len(), 1);
    assert_eq!(privileges[0]["id"], 2011);
    assert_eq!(privileges[0]["name"], "Test Insurance");
    assert_eq!(privileges[0]["logoUrl"], "https://example.com/logo.png");
    assert_eq!(privileges[0]["isDefault"], false);
    assert_eq!(privileges[0]["isConnect"], false);
    assert_eq!(privileges[0]["discountPercent"], 0);
    assert_eq!(
        privileges[0]["benefitDescription"],
        "Claimable and deduct from OPD"
    );
    assert_eq!(
        privileges[0]["privilegeDescription"],
        "<p>Test instructions</p>"
    );
    let policy_types = privileges[0]["policyTypes"].as_array().unwrap();
    assert_eq!(policy_types.len(), 1);
    assert_eq!(policy_types[0]["policyTypeId"], 1);
    assert_eq!(policy_types[0]["insuranceType"], "Gold");
    assert_eq!(policy_types[0]["displayName"], "Test Insurance");
}

#[tokio::test]
async fn list_instant_fetches_privileges_once_per_department() {
    let calls = Arc::new(AtomicUsize::new(0));
    let server = build_test_app_with_deps(
        Arc::new(MockRankingRepo::new(10)),
        Arc::new(MockCache),
        Arc::new(CountingPrivilegeService {
            calls: calls.clone(),
        }),
    );

    let resp = server
        .get("/ranking/v1/doctors/instant")
        .add_query_param("size", "10")
        .await;

    resp.assert_status_ok();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn list_instant_total_uses_repo_count_when_cache_count_is_stale() {
    let server = build_test_app_with_deps(
        Arc::new(MockRankingRepo::new(3)),
        Arc::new(MockCacheWithCount { count: 999 }),
        Arc::new(MockPrivilegeService),
    );

    let resp = server
        .get("/ranking/v1/doctors/instant")
        .add_query_param("size", "2")
        .await;

    resp.assert_status_ok();
    let body: Value = resp.json();
    assert_eq!(body["data"]["pagingMetaData"]["total"], 3);
}

#[tokio::test]
async fn list_instant_skips_count_when_first_page_contains_all_doctors() {
    let count_calls = Arc::new(AtomicUsize::new(0));
    let server = build_test_app_with_deps(
        Arc::new(CountCallsRepo {
            inner: MockRankingRepo::new(3),
            count_calls: count_calls.clone(),
        }),
        Arc::new(MockCacheWithCount { count: 999 }),
        Arc::new(MockPrivilegeService),
    );

    let resp = server
        .get("/ranking/v1/doctors/instant")
        .add_query_param("size", "10")
        .await;

    resp.assert_status_ok();
    let body: Value = resp.json();
    assert_eq!(body["data"]["pagingMetaData"]["total"], 3);
    assert_eq!(count_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn test_list_instant_pagination() {
    let server = build_test_app(25);

    // Get first page
    let resp1 = server
        .get("/ranking/v1/doctors/instant")
        .add_query_param("size", "10")
        .await;
    resp1.assert_status_ok();
    let body1: Value = resp1.json();
    let page1_doctors = body1["data"]["doctors"].as_array().unwrap();
    let next_token = body1["data"]["pagingMetaData"]["nextPageToken"]
        .as_str()
        .unwrap();

    // Get second page
    let resp2 = server
        .get("/ranking/v1/doctors/instant")
        .add_query_param("size", "10")
        .add_query_param("pageToken", next_token)
        .await;
    resp2.assert_status_ok();
    let body2: Value = resp2.json();
    let page2_doctors = body2["data"]["doctors"].as_array().unwrap();

    // No overlap between pages
    let page1_uuids: Vec<&str> = page1_doctors
        .iter()
        .map(|d| d["usid"].as_str().unwrap())
        .collect();
    let page2_uuids: Vec<&str> = page2_doctors
        .iter()
        .map(|d| d["usid"].as_str().unwrap())
        .collect();

    for uuid in &page2_uuids {
        assert!(
            !page1_uuids.contains(uuid),
            "Duplicate doctor across pages: {}",
            uuid
        );
    }

    // Page 2 scores should be lower than page 1's last score
    let page1_last_score = page1_doctors.last().unwrap()["score"].as_f64().unwrap();
    let page2_first_score = page2_doctors[0]["score"].as_f64().unwrap();
    assert!(page2_first_score < page1_last_score);
}

#[tokio::test]
async fn test_list_instant_last_page_no_token() {
    let server = build_test_app(5);

    let resp = server
        .get("/ranking/v1/doctors/instant")
        .add_query_param("size", "10")
        .await;
    resp.assert_status_ok();
    let body: Value = resp.json();

    let doctors = body["data"]["doctors"].as_array().unwrap();
    assert_eq!(doctors.len(), 5);

    // No nextPageToken on last page
    assert!(body["data"]["pagingMetaData"]["nextPageToken"].is_null());
}

#[tokio::test]
async fn test_list_instant_empty() {
    let server = build_test_app(0);

    let resp = server
        .get("/ranking/v1/doctors/instant")
        .add_query_param("size", "10")
        .await;
    resp.assert_status_ok();
    let body: Value = resp.json();

    let doctors = body["data"]["doctors"].as_array().unwrap();
    assert!(doctors.is_empty());
    assert!(body["data"]["pagingMetaData"]["nextPageToken"].is_null());
}

// ============================================================================
// Tests: List Scheduled Doctors
// ============================================================================

#[tokio::test]
async fn test_list_scheduled_doctors() {
    let server = build_test_app(15);

    let resp = server
        .get("/ranking/v1/doctors/scheduled")
        .add_query_param("size", "10")
        .await;
    resp.assert_status_ok();
    let body: Value = resp.json();

    assert_eq!(body["message"], "DOCTOR_LISTING_SUCCEDDED");
    let doctors = body["data"]["doctors"].as_array().unwrap();
    assert_eq!(doctors.len(), 10);
}

// ============================================================================
// Tests: Get Doctor Profile
// ============================================================================

#[tokio::test]
async fn test_get_doctor_found() {
    let server = build_test_app(5);
    let uuid = test_uuid(1);

    let resp = server.get(&format!("/ranking/v1/doctor/{}", uuid)).await;
    resp.assert_status_ok();
    let body: Value = resp.json();

    assert_eq!(body["message"], "DOCTOR_PROFILE_SUCCEDDED");
    assert_eq!(body["returnType"], "object");
    assert_eq!(body["data"]["usid"], uuid.to_string());
    assert!(body["data"]["score"].as_f64().is_some());
    assert_eq!(body["data"]["specialties"][0]["id"], 10);
    assert_eq!(
        body["data"]["counselingAreas"][0]["name"],
        "Stress and anxiety\nSleep support"
    );
    assert!(
        body["data"]["workExperience"].as_array().is_some(),
        "doctor detail response must include legacy workExperience field"
    );
    assert_eq!(body["data"]["workExperience"][0]["langCode"], "en-US");
    assert_eq!(body["data"]["workExperience"][0]["name"], "Test Hospital");
}

#[tokio::test]
async fn test_get_doctor_not_found() {
    let server = build_test_app(5);
    let uuid = Uuid::new_v4();

    let resp = server.get(&format!("/ranking/v1/doctor/{}", uuid)).await;
    resp.assert_status_ok();
    let body: Value = resp.json();

    // Returns empty profile (legacy compat)
    assert_eq!(body["message"], "DOCTOR_PROFILE_SUCCEDDED");
    assert_eq!(body["data"]["usid"], "");
    assert_eq!(body["data"]["score"], 0.0);
}

// ============================================================================
// Tests: Invalid Page Token
// ============================================================================

#[tokio::test]
async fn test_invalid_page_token() {
    let server = build_test_app(10);

    let resp = server
        .get("/ranking/v1/doctors/instant")
        .add_query_param("size", "10")
        .add_query_param("pageToken", "invalid-token")
        .await;

    resp.assert_status(axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_invalid_doctor_uuid() {
    let server = build_test_app(5);

    let resp = server.get("/ranking/v1/doctor/not-a-uuid").await;

    resp.assert_status(axum::http::StatusCode::BAD_REQUEST);
}

// ============================================================================
// Tests: Default Size
// ============================================================================

#[tokio::test]
async fn test_default_size() {
    let server = build_test_app(35);

    let resp = server.get("/ranking/v1/doctors/instant").await;
    resp.assert_status_ok();
    let body: Value = resp.json();

    // Default size is 30
    let doctors = body["data"]["doctors"].as_array().unwrap();
    assert_eq!(doctors.len(), 30);
    assert_eq!(body["data"]["pagingMetaData"]["size"], 30);
}

#[tokio::test]
async fn test_size_capped_at_100() {
    let server = build_test_app(35);

    let resp = server
        .get("/ranking/v1/doctors/instant")
        .add_query_param("size", "200")
        .await;
    resp.assert_status_ok();
    let body: Value = resp.json();

    // Size capped at 100, but we only have 35 doctors
    assert_eq!(body["data"]["pagingMetaData"]["size"], 100);
}

async fn setup_ranking_redis() -> (ContainerAsync<Redis>, RankingCache) {
    let container = Redis::default().start().await.unwrap();
    let host_port = container.get_host_port_ipv4(REDIS_PORT).await.unwrap();
    let redis_url = format!("redis://127.0.0.1:{host_port}/");
    let pool = RedisConfig::from_url(redis_url)
        .create_pool(Some(Runtime::Tokio1))
        .expect("test Redis pool should be created");

    (container, RankingCache::new(pool))
}

#[tokio::test]
async fn redis_batch_profile_and_rank_lookups_preserve_input_order() {
    let (_container, cache) = setup_ranking_redis().await;
    let first = test_uuid(101);
    let second = test_uuid(102);
    let missing = test_uuid(103);
    let first_profile = make_doctor(101, 95);
    let second_profile = make_doctor(102, 80);

    cache
        .set_profiles(
            &[
                (first, first_profile.clone()),
                (second, second_profile.clone()),
            ],
            "en",
        )
        .await;
    cache.add_to_scheduled(first, 95.0).await;
    cache.add_to_scheduled(second, 80.0).await;
    cache.add_to_instant(second, 80.0).await;
    cache.add_to_instant(first, 95.0).await;

    let profiles = cache.get_profiles(&[second, missing, first], "en").await;
    assert_eq!(
        profiles[0].as_ref().map(|p| &p.usid),
        Some(&second_profile.usid)
    );
    assert!(profiles[1].is_none());
    assert_eq!(
        profiles[2].as_ref().map(|p| &p.usid),
        Some(&first_profile.usid)
    );

    assert_eq!(
        cache.get_scheduled_ranks(&[second, missing, first]).await,
        vec![2, 0, 1]
    );
    assert_eq!(
        cache.get_instant_ranks(&[second, missing, first]).await,
        vec![2, 0, 1]
    );
}

#[tokio::test]
async fn redis_warm_up_replaces_live_ranking_sets_without_stale_members() {
    let (_container, cache) = setup_ranking_redis().await;
    let stale = test_uuid(111);
    let instant = test_uuid(112);
    let scheduled = test_uuid(113);

    cache.add_to_instant(stale, 500.0).await;
    cache.add_to_scheduled(stale, 500.0).await;

    cache
        .warm_up(&[
            RankedDoctorInfo {
                doctor_id: instant,
                score: 90,
                instant_mode_enabled: true,
                schedule_mode_enabled: false,
            },
            RankedDoctorInfo {
                doctor_id: scheduled,
                score: 80,
                instant_mode_enabled: false,
                schedule_mode_enabled: true,
            },
        ])
        .await;

    assert_eq!(cache.get_instant_rank(stale).await, 0);
    assert_eq!(cache.get_scheduled_rank(stale).await, 0);
    assert_eq!(cache.get_instant_rank(instant).await, 1);
    assert_eq!(cache.get_scheduled_rank(scheduled).await, 1);
    assert_eq!(cache.instant_count().await, 1);
    assert_eq!(cache.scheduled_count().await, 1);
}

#[tokio::test]
async fn redis_rank_lookup_returns_zero_when_pool_connection_fails() {
    let pool = RedisConfig::from_url("redis://127.0.0.1:1/")
        .create_pool(Some(Runtime::Tokio1))
        .expect("test Redis pool should be created");
    let cache = RankingCache::new(pool);
    let doctor_id = Uuid::new_v4();

    let scheduled = AssertUnwindSafe(cache.get_scheduled_rank(doctor_id))
        .catch_unwind()
        .await;
    let instant = AssertUnwindSafe(cache.get_instant_rank(doctor_id))
        .catch_unwind()
        .await;

    assert_eq!(scheduled.expect("scheduled rank lookup must not panic"), 0);
    assert_eq!(instant.expect("instant rank lookup must not panic"), 0);
}
