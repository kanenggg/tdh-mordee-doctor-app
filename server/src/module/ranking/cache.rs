use async_trait::async_trait;
use deadpool_redis::Pool as RedisPool;
use redis::AsyncCommands;
use tracing::warn;
use uuid::Uuid;

use super::models::{AssociatePrivilege, DoctorProfile, RankedDoctorInfo};

const RANKING_INSTANT_KEY: &str = "ranking:instant";
const RANKING_SCHEDULED_KEY: &str = "ranking:scheduled";
const PROFILE_PREFIX: &str = "doctor:profile:";
const PRIVILEGE_PREFIX: &str = "privilege:specialty:";
const PROFILE_TTL_SECS: u64 = 300; // 5 minutes
const PRIVILEGE_TTL_SECS: u64 = 300; // 5 minutes

#[async_trait]
pub trait RankingCacheTrait: Send + Sync {
    /// Populate Redis sorted sets from database on startup.
    async fn warm_up(&self, doctors: &[RankedDoctorInfo]);

    /// Get cached doctor profile JSON for a specific language.
    async fn get_profile(&self, doctor_id: Uuid, lang_key: &str) -> Option<DoctorProfile>;

    /// Get cached doctor profile JSON for multiple doctors in a specific language.
    async fn get_profiles(&self, doctor_ids: &[Uuid], lang_key: &str)
        -> Vec<Option<DoctorProfile>>;

    /// Cache doctor profile with TTL for a specific language.
    async fn set_profile(&self, doctor_id: Uuid, lang_key: &str, profile: &DoctorProfile);

    /// Cache multiple doctor profiles with TTL for a specific language.
    async fn set_profiles(&self, profiles: &[(Uuid, DoctorProfile)], lang_key: &str);

    /// Invalidate cached profile.
    async fn invalidate_profile(&self, doctor_id: Uuid);

    /// Remove doctor from instant ranking set.
    async fn remove_from_instant(&self, doctor_id: Uuid);

    /// Add doctor to instant ranking set with score.
    async fn add_to_instant(&self, doctor_id: Uuid, score: f64);

    /// Remove doctor from scheduled ranking set.
    async fn remove_from_scheduled(&self, doctor_id: Uuid);

    /// Add doctor to scheduled ranking set with score.
    async fn add_to_scheduled(&self, doctor_id: Uuid, score: f64);

    /// Get count of doctors in instant set.
    async fn instant_count(&self) -> i64;

    /// Get count of doctors in scheduled set.
    async fn scheduled_count(&self) -> i64;

    /// Get cached privilege benefits for a specialty.
    async fn get_privileges(&self, specialty_id: i32) -> Option<Vec<AssociatePrivilege>>;

    /// Cache privilege benefits with TTL.
    async fn set_privileges(&self, specialty_id: i32, benefits: &[AssociatePrivilege]);

    /// Get doctor's current rank in scheduled set (1-based, from ZREVRANK).
    async fn get_scheduled_rank(&self, doctor_id: Uuid) -> i32;

    /// Get doctors' current ranks in scheduled set (1-based, from ZREVRANK).
    async fn get_scheduled_ranks(&self, doctor_ids: &[Uuid]) -> Vec<i32>;

    /// Get doctor's current rank in instant set (1-based, from ZREVRANK).
    async fn get_instant_rank(&self, doctor_id: Uuid) -> i32;

    /// Get doctors' current ranks in instant set (1-based, from ZREVRANK).
    async fn get_instant_ranks(&self, doctor_ids: &[Uuid]) -> Vec<i32>;
}

#[derive(Clone)]
pub struct RankingCache {
    pool: RedisPool,
}

impl RankingCache {
    pub fn new(pool: RedisPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl RankingCacheTrait for RankingCache {
    async fn warm_up(&self, doctors: &[RankedDoctorInfo]) {
        let mut conn = match self.pool.get().await {
            Ok(conn) => conn,
            Err(e) => {
                warn!(error = %e, "Failed to get connection from pool for warm-up");
                return;
            }
        };

        let temp_suffix = Uuid::new_v4();
        let instant_temp_key = format!("{RANKING_INSTANT_KEY}:warmup:{temp_suffix}");
        let scheduled_temp_key = format!("{RANKING_SCHEDULED_KEY}:warmup:{temp_suffix}");

        let cleanup: Result<(), redis::RedisError> = redis::cmd("DEL")
            .arg(&instant_temp_key)
            .arg(&scheduled_temp_key)
            .query_async(&mut conn)
            .await;
        if let Err(e) = cleanup {
            warn!(error = %e, "Failed to DEL temporary ranking warm-up sets");
            return;
        }

        let mut instant_count = 0usize;
        let mut scheduled_count = 0usize;
        let mut populate = redis::pipe();
        for doc in doctors {
            let member = doc.doctor_id.to_string();
            let score = doc.score as f64;
            if doc.instant_mode_enabled {
                populate
                    .cmd("ZADD")
                    .arg(&instant_temp_key)
                    .arg(score)
                    .arg(&member)
                    .ignore();
                instant_count += 1;
            }
            if doc.schedule_mode_enabled {
                populate
                    .cmd("ZADD")
                    .arg(&scheduled_temp_key)
                    .arg(score)
                    .arg(&member)
                    .ignore();
                scheduled_count += 1;
            }
        }

        let populated: Result<(), redis::RedisError> = populate.query_async(&mut conn).await;
        if let Err(e) = populated {
            warn!(error = %e, "Failed to populate temporary ranking warm-up sets");
            return;
        }

        let mut swap = redis::pipe();
        swap.atomic();
        if instant_count == 0 {
            swap.cmd("DEL").arg(RANKING_INSTANT_KEY).ignore();
        } else {
            swap.cmd("RENAME")
                .arg(&instant_temp_key)
                .arg(RANKING_INSTANT_KEY)
                .ignore();
        }
        if scheduled_count == 0 {
            swap.cmd("DEL").arg(RANKING_SCHEDULED_KEY).ignore();
        } else {
            swap.cmd("RENAME")
                .arg(&scheduled_temp_key)
                .arg(RANKING_SCHEDULED_KEY)
                .ignore();
        }

        let swapped: Result<(), redis::RedisError> = swap.query_async(&mut conn).await;
        if let Err(e) = swapped {
            warn!(error = %e, "Failed to swap warmed ranking sets into Redis");
            return;
        }

        tracing::info!(count = doctors.len(), "Redis warm-up complete");
    }

    async fn get_profile(&self, doctor_id: Uuid, lang_key: &str) -> Option<DoctorProfile> {
        let mut conn = self.pool.get().await.ok()?;
        let key = format!("{}{}:{}", PROFILE_PREFIX, doctor_id, lang_key);
        let result: Result<Option<String>, _> = conn.get(&key).await;
        match result {
            Ok(Some(json)) => serde_json::from_str(&json).ok(),
            _ => None,
        }
    }

    async fn get_profiles(
        &self,
        doctor_ids: &[Uuid],
        lang_key: &str,
    ) -> Vec<Option<DoctorProfile>> {
        let mut conn = match self.pool.get().await {
            Ok(conn) => conn,
            Err(e) => {
                warn!(error = %e, "Failed to get Redis connection for batched profile get");
                return vec![None; doctor_ids.len()];
            }
        };

        let mut pipe = redis::pipe();
        for doctor_id in doctor_ids {
            let key = format!("{}{}:{}", PROFILE_PREFIX, doctor_id, lang_key);
            pipe.cmd("GET").arg(key);
        }

        let result: Result<Vec<Option<String>>, redis::RedisError> =
            pipe.query_async(&mut conn).await;
        match result {
            Ok(values) => values
                .into_iter()
                .map(|value| value.and_then(|json| serde_json::from_str(&json).ok()))
                .collect(),
            Err(e) => {
                warn!(error = %e, "Failed to MGET doctor profiles from cache");
                vec![None; doctor_ids.len()]
            }
        }
    }

    async fn set_profile(&self, doctor_id: Uuid, lang_key: &str, profile: &DoctorProfile) {
        if let Ok(mut conn) = self.pool.get().await {
            let key = format!("{}{}:{}", PROFILE_PREFIX, doctor_id, lang_key);
            if let Ok(json) = serde_json::to_string(profile) {
                let result: Result<(), redis::RedisError> =
                    conn.set_ex(&key, &json, PROFILE_TTL_SECS).await;
                if let Err(e) = result {
                    warn!(error = %e, %doctor_id, "Failed to SET EX doctor profile in cache");
                }
            }
        }
    }

    async fn set_profiles(&self, profiles: &[(Uuid, DoctorProfile)], lang_key: &str) {
        if profiles.is_empty() {
            return;
        }

        let mut conn = match self.pool.get().await {
            Ok(conn) => conn,
            Err(e) => {
                warn!(error = %e, "Failed to get Redis connection for batched profile set");
                return;
            }
        };

        let mut pipe = redis::pipe();
        let mut queued = 0usize;
        for (doctor_id, profile) in profiles {
            let Ok(json) = serde_json::to_string(profile) else {
                warn!(%doctor_id, "Failed to serialize doctor profile for cache");
                continue;
            };
            let key = format!("{}{}:{}", PROFILE_PREFIX, doctor_id, lang_key);
            pipe.cmd("SETEX")
                .arg(key)
                .arg(PROFILE_TTL_SECS)
                .arg(json)
                .ignore();
            queued += 1;
        }

        if queued == 0 {
            return;
        }

        let result: Result<(), redis::RedisError> = pipe.query_async(&mut conn).await;
        if let Err(e) = result {
            warn!(error = %e, "Failed to SETEX doctor profiles in cache");
        }
    }

    async fn invalidate_profile(&self, doctor_id: Uuid) {
        if let Ok(mut conn) = self.pool.get().await {
            // Invalidate both language variants
            let key_th = format!("{}{}:th", PROFILE_PREFIX, doctor_id);
            let key_en = format!("{}{}:en", PROFILE_PREFIX, doctor_id);
            let result: Result<(), redis::RedisError> = conn.del(&[&key_th, &key_en]).await;
            if let Err(e) = result {
                warn!(error = %e, %doctor_id, "Failed to DEL doctor profile from cache");
            }
        }
    }

    async fn remove_from_instant(&self, doctor_id: Uuid) {
        if let Ok(mut conn) = self.pool.get().await {
            let result: Result<(), redis::RedisError> =
                conn.zrem(RANKING_INSTANT_KEY, doctor_id.to_string()).await;
            if let Err(e) = result {
                warn!(error = %e, %doctor_id, "Failed to ZREM from instant ranking");
            }
        }
    }

    async fn add_to_instant(&self, doctor_id: Uuid, score: f64) {
        if let Ok(mut conn) = self.pool.get().await {
            let result: Result<(), redis::RedisError> = conn
                .zadd(RANKING_INSTANT_KEY, doctor_id.to_string(), score)
                .await;
            if let Err(e) = result {
                warn!(error = %e, %doctor_id, "Failed to ZADD to instant ranking");
            }
        }
    }

    async fn remove_from_scheduled(&self, doctor_id: Uuid) {
        if let Ok(mut conn) = self.pool.get().await {
            let result: Result<(), redis::RedisError> = conn
                .zrem(RANKING_SCHEDULED_KEY, doctor_id.to_string())
                .await;
            if let Err(e) = result {
                warn!(error = %e, %doctor_id, "Failed to ZREM from scheduled ranking");
            }
        }
    }

    async fn add_to_scheduled(&self, doctor_id: Uuid, score: f64) {
        if let Ok(mut conn) = self.pool.get().await {
            let result: Result<(), redis::RedisError> = conn
                .zadd(RANKING_SCHEDULED_KEY, doctor_id.to_string(), score)
                .await;
            if let Err(e) = result {
                warn!(error = %e, %doctor_id, "Failed to ZADD to scheduled ranking");
            }
        }
    }

    async fn instant_count(&self) -> i64 {
        if let Ok(mut conn) = self.pool.get().await {
            conn.zcard(RANKING_INSTANT_KEY).await.unwrap_or(0)
        } else {
            0
        }
    }

    async fn scheduled_count(&self) -> i64 {
        if let Ok(mut conn) = self.pool.get().await {
            conn.zcard(RANKING_SCHEDULED_KEY).await.unwrap_or(0)
        } else {
            0
        }
    }

    async fn get_privileges(&self, specialty_id: i32) -> Option<Vec<AssociatePrivilege>> {
        let mut conn = self.pool.get().await.ok()?;
        let key = format!("{}{}", PRIVILEGE_PREFIX, specialty_id);
        let result: Result<Option<String>, _> = conn.get(&key).await;
        match result {
            Ok(Some(json)) => serde_json::from_str(&json).ok(),
            _ => None,
        }
    }

    async fn set_privileges(&self, specialty_id: i32, benefits: &[AssociatePrivilege]) {
        if let Ok(mut conn) = self.pool.get().await {
            let key = format!("{}{}", PRIVILEGE_PREFIX, specialty_id);
            if let Ok(json) = serde_json::to_string(benefits) {
                let result: Result<(), redis::RedisError> =
                    conn.set_ex(&key, &json, PRIVILEGE_TTL_SECS).await;
                if let Err(e) = result {
                    warn!(error = %e, specialty_id, "Failed to SET EX privilege benefits in cache");
                }
            }
        }
    }

    async fn get_scheduled_rank(&self, doctor_id: Uuid) -> i32 {
        let mut conn = match self.pool.get().await {
            Ok(conn) => conn,
            Err(e) => {
                warn!(error = %e, %doctor_id, "Failed to get Redis connection for scheduled rank lookup");
                return 0;
            }
        };
        let result: Result<Option<i64>, _> = conn
            .zrevrank(RANKING_SCHEDULED_KEY, doctor_id.to_string())
            .await;
        match result {
            Ok(Some(rank)) => (rank + 1) as i32, // 0-based → 1-based
            _ => return 0,
        }
    }

    async fn get_scheduled_ranks(&self, doctor_ids: &[Uuid]) -> Vec<i32> {
        get_ranks(&self.pool, RANKING_SCHEDULED_KEY, doctor_ids).await
    }

    async fn get_instant_rank(&self, doctor_id: Uuid) -> i32 {
        let mut conn = match self.pool.get().await {
            Ok(conn) => conn,
            Err(e) => {
                warn!(error = %e, %doctor_id, "Failed to get Redis connection for instant rank lookup");
                return 0;
            }
        };
        let result: Result<Option<i64>, _> = conn
            .zrevrank(RANKING_INSTANT_KEY, doctor_id.to_string())
            .await;
        match result {
            Ok(Some(rank)) => (rank + 1) as i32,
            _ => return 0,
        }
    }

    async fn get_instant_ranks(&self, doctor_ids: &[Uuid]) -> Vec<i32> {
        get_ranks(&self.pool, RANKING_INSTANT_KEY, doctor_ids).await
    }
}

async fn get_ranks(pool: &RedisPool, key: &str, doctor_ids: &[Uuid]) -> Vec<i32> {
    if doctor_ids.is_empty() {
        return vec![];
    }

    let mut conn = match pool.get().await {
        Ok(conn) => conn,
        Err(e) => {
            warn!(error = %e, ranking_key = key, "Failed to get Redis connection for batched rank lookup");
            return vec![0; doctor_ids.len()];
        }
    };

    let mut pipe = redis::pipe();
    for doctor_id in doctor_ids {
        pipe.cmd("ZREVRANK").arg(key).arg(doctor_id.to_string());
    }

    let result: Result<Vec<Option<i64>>, redis::RedisError> = pipe.query_async(&mut conn).await;
    match result {
        Ok(ranks) => ranks
            .into_iter()
            .map(|rank| rank.map(|rank| (rank + 1) as i32).unwrap_or(0))
            .collect(),
        Err(e) => {
            warn!(error = %e, ranking_key = key, "Failed to ZREVRANK ranking set");
            vec![0; doctor_ids.len()]
        }
    }
}

/// No-op cache implementation for when Redis is unavailable (degraded mode).
pub struct NoOpCache;

#[async_trait]
impl RankingCacheTrait for NoOpCache {
    async fn warm_up(&self, _doctors: &[RankedDoctorInfo]) {
        warn!("Redis unavailable — warm-up skipped");
    }
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
