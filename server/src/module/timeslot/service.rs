use crate::core::error::{AppError, AppResult};
use crate::module::timeslot::model::{
    RateLimitType, ReleaseReason, Reservation, Timeslot, TimeslotConfirmedEvent,
    TimeslotReleasedEvent, TimeslotReservedEvent, TimeslotStatus,
};
use crate::module::timeslot::repo::TimeslotRepo;
use crate::module::webhook::PubsubPublisher;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedReserveResponse {
    pub __type: String,
    pub reservation_id: Option<String>,
    pub expires_at: Option<i64>,
    pub current_count: Option<i32>,
    pub max_allowed: Option<i32>,
    pub retry_after_seconds: Option<i64>,
}

#[derive(Clone)]
pub struct IdempotencyCache {
    client: redis::aio::ConnectionManager,
}

impl IdempotencyCache {
    pub async fn new(redis_url: &str) -> AppResult<Self> {
        let client = redis::Client::open(redis_url)
            .map_err(|e| AppError::InternalError(format!("Redis connection failed: {}", e)))?;
        let manager = redis::aio::ConnectionManager::new(client)
            .await
            .map_err(|e| AppError::InternalError(format!("Redis manager failed: {}", e)))?;
        Ok(Self { client: manager })
    }

    pub async fn get_cached_response(
        &mut self,
        correlation_id: &str,
    ) -> AppResult<Option<CachedReserveResponse>> {
        let key = format!("correlation:{}", correlation_id);
        let value: Option<String> = self
            .client
            .get(&key)
            .await
            .map_err(|e| AppError::InternalError(format!("Redis GET failed: {}", e)))?;

        match value {
            Some(v) => {
                let cached: CachedReserveResponse = serde_json::from_str(&v).map_err(|e| {
                    AppError::InternalError(format!("Failed to deserialize cache: {}", e))
                })?;
                Ok(Some(cached))
            }
            None => Ok(None),
        }
    }

    pub async fn cache_response(
        &mut self,
        correlation_id: &str,
        response: &CachedReserveResponse,
        ttl_seconds: i32,
    ) -> Result<(), AppError> {
        let key = format!("correlation:{}", correlation_id);
        let value = serde_json::to_string::<CachedReserveResponse>(response)
            .map_err::<AppError, _>(|e| {
                AppError::InternalError(format!("Failed to serialize cache: {}", e))
            })?;

        self.client
            .set_ex::<_, _, ()>(key, value, ttl_seconds as u64)
            .await
            .map_err::<AppError, _>(|e| {
                AppError::InternalError(format!("Redis SET failed: {}", e))
            })?;

        Ok(())
    }
}

#[derive(Clone)]
pub struct RateLimiter {
    pool: PgPool,
    daily_limit: i32,
    weekly_limit: i32,
}

impl RateLimiter {
    pub fn new(pool: PgPool, daily_limit: i32, weekly_limit: i32) -> Self {
        Self {
            pool,
            daily_limit,
            weekly_limit,
        }
    }

    pub fn daily_limit(&self) -> i32 {
        self.daily_limit
    }

    pub fn weekly_limit(&self) -> i32 {
        self.weekly_limit
    }

    pub async fn check_and_increment(&self, patient_id: i32) -> AppResult<Option<RateLimitType>> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let daily_window = now - (now % 86400);
        let weekly_window = now - ((now - 345600) % 604800);

        let daily_count = self
            .get_or_increment_count(patient_id, "daily", daily_window)
            .await?;

        if daily_count > self.daily_limit {
            return Ok(Some(RateLimitType::Daily));
        }

        let weekly_count = self
            .get_or_increment_count(patient_id, "weekly", weekly_window)
            .await?;

        if weekly_count > self.weekly_limit {
            return Ok(Some(RateLimitType::Weekly));
        }

        Ok(None)
    }

    async fn get_or_increment_count(
        &self,
        patient_id: i32,
        limit_type: &str,
        window_start: i64,
    ) -> AppResult<i32> {
        let result = sqlx::query(
            r#"
            UPDATE rate_limit_counts
            SET count = count + 1
            WHERE patient_id = $1 AND limit_type = $2 AND window_start = $3
            RETURNING count
            "#,
        )
        .bind(patient_id)
        .bind(limit_type)
        .bind(window_start)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = result {
            return Ok(row.get("count"));
        }

        sqlx::query(
            r#"
            INSERT INTO rate_limit_counts (patient_id, limit_type, window_start, count)
            VALUES ($1, $2, $3, 1)
            "#,
        )
        .bind(patient_id)
        .bind(limit_type)
        .bind(window_start)
        .execute(&self.pool)
        .await?;

        Ok(1)
    }

    pub fn get_seconds_until_window_reset(&self, limit_type: RateLimitType) -> i64 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        match limit_type {
            RateLimitType::Daily => {
                let next_day = ((now / 86400) + 1) * 86400;
                next_day - now
            }
            RateLimitType::Weekly => {
                let week_start = now - ((now - 345600) % 604800);
                let next_week = week_start + 604800;
                next_week - now
            }
        }
    }
}

fn now_epoch_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before epoch")
        .as_secs() as i64
}

#[derive(Clone)]
pub struct TimeslotService {
    repo: Arc<dyn TimeslotRepo>,
    rate_limiter: RateLimiter,
    pubsub_publisher: Arc<PubsubPublisher>,
}

impl TimeslotService {
    pub fn new(
        repo: Arc<dyn TimeslotRepo>,
        rate_limiter: RateLimiter,
        pubsub_publisher: Arc<PubsubPublisher>,
    ) -> Self {
        Self {
            repo,
            rate_limiter,
            pubsub_publisher,
        }
    }

    pub async fn get_available_timeslots(
        &self,
        doctor_id: i32,
        start_time: i64,
        end_time: i64,
    ) -> AppResult<Vec<Timeslot>> {
        let timeslots = self
            .repo
            .find_available_timeslots(doctor_id, start_time, end_time)
            .await?;

        Ok(timeslots)
    }

    pub async fn reserve_timeslot(
        &self,
        timeslot_id: &str,
        doctor_id: i32,
        patient_id: i32,
        reservation_ttl_seconds: i32,
        correlation_id: &str,
        idempotency_cache: &mut IdempotencyCache,
    ) -> AppResult<CachedReserveResponse> {
        if let Some(cached) = idempotency_cache
            .get_cached_response(correlation_id)
            .await?
        {
            return Ok(cached);
        }

        if let Some(limit_type) = self.rate_limiter.check_and_increment(patient_id).await? {
            let current_count = match limit_type {
                RateLimitType::Daily => self.rate_limiter.daily_limit() + 1,
                RateLimitType::Weekly => self.rate_limiter.weekly_limit() + 1,
            };

            let max_allowed = match limit_type {
                RateLimitType::Daily => self.rate_limiter.daily_limit(),
                RateLimitType::Weekly => self.rate_limiter.weekly_limit(),
            };

            let response = CachedReserveResponse {
                __type: "RateLimitExceeded".to_string(),
                reservation_id: None,
                expires_at: None,
                current_count: Some(current_count),
                max_allowed: Some(max_allowed),
                retry_after_seconds: Some(
                    self.rate_limiter.get_seconds_until_window_reset(limit_type),
                ),
            };

            idempotency_cache
                .cache_response(correlation_id, &response, reservation_ttl_seconds)
                .await?;

            return Ok(response);
        }

        let timeslot = self
            .repo
            .get_timeslot(timeslot_id)
            .await?
            .ok_or_else(|| AppError::BadRequest("Timeslot not found".to_string()))?;

        if timeslot.status != TimeslotStatus::Free {
            return Ok(CachedReserveResponse {
                __type: "AlreadyReserved".to_string(),
                reservation_id: None,
                expires_at: None,
                current_count: None,
                max_allowed: None,
                retry_after_seconds: None,
            });
        }

        let now = now_epoch_secs();
        let expires_at = now + reservation_ttl_seconds as i64;

        let reservation = Reservation {
            id: Uuid::new_v4().to_string(),
            timeslot_id: timeslot_id.to_string(),
            doctor_id,
            patient_id,
            status: crate::module::timeslot::model::ReservationStatus::Pending,
            correlation_id: correlation_id.to_string(),
            booking_id: None,
            payment_reference: None,
            expires_at,
            created_at: now,
            confirmed_at: None,
            cancelled_at: None,
        };

        self.repo.reserve_timeslot(&timeslot, &reservation).await?;

        let event = TimeslotReservedEvent::TimeslotReserved {
            reservation_id: reservation.id.clone(),
            timeslot_id: timeslot_id.to_string(),
            doctor_id,
            patient_id,
            expires_at,
            reserved_at: now,
        };

        self.pubsub_publisher
            .publish_timeslot_reserved(event)
            .await?;

        let response = CachedReserveResponse {
            __type: "Success".to_string(),
            reservation_id: Some(reservation.id),
            expires_at: Some(expires_at),
            current_count: None,
            max_allowed: None,
            retry_after_seconds: None,
        };

        idempotency_cache
            .cache_response(correlation_id, &response, reservation_ttl_seconds)
            .await?;

        Ok(response)
    }

    pub async fn confirm_booking(
        &self,
        reservation_id: &str,
        payment_reference: &str,
        booking_id: &str,
    ) -> AppResult<Option<String>> {
        let now = now_epoch_secs();

        match self
            .repo
            .confirm_reservation(reservation_id, booking_id, payment_reference, now)
            .await?
        {
            Some(timeslot_id) => {
                let reservation = self
                    .repo
                    .get_reservation(reservation_id)
                    .await?
                    .ok_or_else(|| AppError::BadRequest("Reservation not found".to_string()))?;

                let event = TimeslotConfirmedEvent::TimeslotConfirmed {
                    reservation_id: reservation_id.to_string(),
                    timeslot_id: timeslot_id.clone(),
                    booking_id: booking_id.to_string(),
                    doctor_id: reservation.doctor_id,
                    patient_id: reservation.patient_id,
                    confirmed_at: now,
                };

                self.pubsub_publisher
                    .publish_timeslot_confirmed(event)
                    .await?;

                Ok(Some(timeslot_id))
            }
            None => Ok(None),
        }
    }

    pub async fn cancel_booking(
        &self,
        reservation_id: Option<&str>,
        booking_id: Option<&str>,
        reason: ReleaseReason,
    ) -> AppResult<bool> {
        let now = now_epoch_secs();

        if self
            .repo
            .cancel_reservation(reservation_id, booking_id, now)
            .await?
        {
            let reservation = if let Some(rid) = reservation_id {
                self.repo.get_reservation(rid).await?
            } else if let Some(bid) = booking_id {
                self.repo.find_reservation_by_booking_id(bid).await?
            } else {
                return Err(AppError::BadRequest(
                    "Must provide either reservation_id or booking_id".to_string(),
                ));
            };

            let reservation = match reservation {
                Some(r) => r,
                None => {
                    tracing::warn!("Reservation not found after successful cancellation");
                    return Ok(true);
                }
            };

            let event = TimeslotReleasedEvent::TimeslotReleased {
                timeslot_id: reservation.timeslot_id,
                doctor_id: reservation.doctor_id,
                reservation_id: reservation.id,
                released_at: now,
                reason,
            };

            self.pubsub_publisher
                .publish_timeslot_released(event)
                .await?;

            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub async fn process_expired_reservations(&self) -> AppResult<Vec<String>> {
        let now = now_epoch_secs();

        let expired = self.repo.find_expired_reservations(now).await?;
        let mut released_timeslots = Vec::new();

        for reservation in expired {
            self.repo
                .update_timeslot_status(&reservation.timeslot_id, TimeslotStatus::Free)
                .await?;

            let event = TimeslotReleasedEvent::TimeslotReleased {
                timeslot_id: reservation.timeslot_id.clone(),
                doctor_id: reservation.doctor_id,
                reservation_id: reservation.id.clone(),
                released_at: now,
                reason: ReleaseReason::Expired,
            };

            self.pubsub_publisher
                .publish_timeslot_released(event)
                .await?;

            released_timeslots.push(reservation.timeslot_id);
        }

        Ok(released_timeslots)
    }
}
