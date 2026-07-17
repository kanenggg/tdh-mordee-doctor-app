use jiff::{civil::Date, civil::Time};
use std::collections::BTreeMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::core::error::AppResult;
use crate::doctor_actor::common::generate_timeslots;
use crate::doctor_actor::model::{GeneratedTimeslot, ReservationSource, ReserveResult, TimeRange};
use crate::doctor_actor::repo::{DoctorTimeslotRepo, ReservationRecord, ReservationStatus};
use crate::module::timeslot::model::{ReleaseReason, TimeslotReleasedEvent, TimeslotReservedEvent};
use crate::module::webhook::PubsubPublisher;

fn now_epoch_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before epoch")
        .as_secs() as i64
}

#[async_trait::async_trait]
pub trait DoctorActor: Send + Sync {
    async fn get_available_timeslots(
        &self,
        doctor_id: &str,
        start_date: Date,
        end_date: Date,
    ) -> Result<Vec<GeneratedTimeslot>, anyhow::Error>;

    async fn reserve_timeslot(
        &self,
        doctor_id: &str,
        patient_id: Option<&str>,
        slot_date: Date,
        start_time: Time,
        end_time: Time,
        ttl_seconds: i64,
        correlation_id: &str,
        source: ReservationSource,
    ) -> Result<ReserveResult, anyhow::Error>;

    async fn release_timeslot(
        &self,
        reservation_id: &str,
        reason: ReleaseReason,
    ) -> Result<(), anyhow::Error>;
}

pub struct DoctorActorImpl {
    repo: Arc<dyn DoctorTimeslotRepo>,
    redis: Arc<redis::aio::MultiplexedConnection>,
    pubsub_publisher: Arc<PubsubPublisher>,
}

impl DoctorActorImpl {
    pub fn new(
        repo: Arc<dyn DoctorTimeslotRepo>,
        redis: Arc<redis::aio::MultiplexedConnection>,
        pubsub_publisher: Arc<PubsubPublisher>,
    ) -> Self {
        Self {
            repo,
            redis,
            pubsub_publisher,
        }
    }
}

#[async_trait::async_trait]
impl DoctorActor for DoctorActorImpl {
    async fn get_available_timeslots(
        &self,
        doctor_id: &str,
        start_date: Date,
        end_date: Date,
    ) -> Result<Vec<GeneratedTimeslot>, anyhow::Error> {
        let schedule_config: crate::doctor_actor::model::DoctorScheduleConfig = self
            .repo
            .get_schedule_config(doctor_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("No schedule config for doctor {}", doctor_id))?;

        let reservations = self
            .repo
            .get_doctor_reservations(doctor_id.to_string(), start_date, end_date)
            .await?;

        let slots = generate_timeslots(&schedule_config, start_date, end_date, &reservations)?;

        let mut grouped: BTreeMap<Date, Vec<TimeRange>> = BTreeMap::new();
        for slot in slots {
            grouped
                .entry(slot.slot_date)
                .or_insert_with(Vec::new)
                .push(TimeRange {
                    start_time: slot.start_time,
                    end_time: slot.end_time,
                });
        }

        let result: Vec<GeneratedTimeslot> = grouped
            .into_iter()
            .map(|(date, time_ranges)| GeneratedTimeslot { date, time_ranges })
            .collect();

        Ok(result)
    }

    async fn reserve_timeslot(
        &self,
        doctor_id: &str,
        patient_id: Option<&str>,
        slot_date: Date,
        start_time: Time,
        end_time: Time,
        ttl_seconds: i64,
        correlation_id: &str,
        source: ReservationSource,
    ) -> Result<ReserveResult, anyhow::Error> {
        let doctor_id_int: i32 = doctor_id.parse()?;

        let schedule_config: Option<crate::doctor_actor::model::DoctorScheduleConfig> =
            self.repo.get_schedule_config(doctor_id).await?;

        if schedule_config.is_none() {
            return Ok(ReserveResult::NoScheduleConfig);
        }

        let reservations = self
            .repo
            .get_doctor_reservations(doctor_id.to_string(), slot_date, slot_date)
            .await?;

        let available_slots = generate_timeslots(
            &schedule_config.unwrap(),
            slot_date,
            slot_date,
            &reservations,
        )?;

        let slot_start_secs = slot_date
            .at(start_time.hour(), start_time.minute(), 0, 0)
            .to_zoned(jiff::tz::TimeZone::UTC)
            .unwrap()
            .timestamp()
            .as_second();

        let slot_end_secs = slot_date
            .at(end_time.hour(), end_time.minute(), 0, 0)
            .to_zoned(jiff::tz::TimeZone::UTC)
            .unwrap()
            .timestamp()
            .as_second();

        let is_available = available_slots.iter().any(|slot| {
            let s_start = slot
                .slot_date
                .at(slot.start_time.hour(), slot.start_time.minute(), 0, 0)
                .to_zoned(jiff::tz::TimeZone::UTC)
                .unwrap()
                .timestamp()
                .as_second();
            let s_end = slot
                .slot_date
                .at(slot.end_time.hour(), slot.end_time.minute(), 0, 0)
                .to_zoned(jiff::tz::TimeZone::UTC)
                .unwrap()
                .timestamp()
                .as_second();
            slot_start_secs >= s_start && slot_end_secs <= s_end
        });

        if !is_available {
            return Ok(ReserveResult::Conflict);
        }

        let now = now_epoch_secs();
        let expires_at = now + ttl_seconds as i64;

        let reservation_id = Uuid::new_v4();
        let source_str = match source {
            ReservationSource::Booking => "booking",
            ReservationSource::FollowUp => "follow_up",
        };

        let expires_at_opt: Option<i64> = if expires_at > 0 {
            Some(expires_at)
        } else {
            None
        };

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let reservation_record = ReservationRecord {
            id: reservation_id,
            doctor_id: doctor_id_int,
            patient_id: patient_id.and_then(|p| p.parse::<Uuid>().ok()),
            slot_date,
            start_time,
            end_time,
            status: ReservationStatus::Pending,
            correlation_id: correlation_id.to_string(),
            source: source_str.to_string(),
            expires_at: expires_at_opt,
            created_at: now,
            confirmed_at: None,
            cancelled_at: None,
        };

        self.repo.insert_reservation(&reservation_record).await?;

        schedule_expiry_in_redis(&self.redis, &reservation_id, source_str, expires_at).await?;

        let event = TimeslotReservedEvent::TimeslotReserved {
            reservation_id: reservation_id.to_string(),
            timeslot_id: format!("{}-{:?}-{:?}", doctor_id_int, slot_date, start_time),
            doctor_id: doctor_id_int,
            patient_id: patient_id.and_then(|p| p.parse::<i32>().ok()).unwrap_or(0),
            expires_at,
            reserved_at: now,
        };

        self.pubsub_publisher
            .publish_timeslot_reserved(event)
            .await?;

        Ok(ReserveResult::Success {
            reservation_id: 0,
            expires_at,
        })
    }

    async fn release_timeslot(
        &self,
        reservation_id: &str,
        reason: ReleaseReason,
    ) -> Result<(), anyhow::Error> {
        let status = match reason {
            ReleaseReason::Expired => ReservationStatus::Expired,
            ReleaseReason::Cancelled => ReservationStatus::Cancelled,
        };

        self.repo
            .update_reservation_status(reservation_id, status)
            .await?;

        remove_expiry_from_redis(&self.redis, reservation_id).await?;

        let reservation_id_parsed: i64 = reservation_id.parse().unwrap_or(0);

        let event = TimeslotReleasedEvent::TimeslotReleased {
            timeslot_id: format!(
                "0-{:?}-{:?}",
                Date::new(1970, 1, 1).unwrap(),
                Time::new(0, 0, 0, 0).unwrap()
            ),
            doctor_id: 0,
            reservation_id: reservation_id.to_string(),
            released_at: now_epoch_secs(),
            reason,
        };

        self.pubsub_publisher
            .publish_timeslot_released(event)
            .await?;

        Ok(())
    }
}

async fn schedule_expiry_in_redis(
    redis: &Arc<redis::aio::MultiplexedConnection>,
    reservation_id: &Uuid,
    source: &str,
    expires_at: i64,
) -> AppResult<()> {
    const EXPIRY_QUEUE_KEY: &str = "doctor:expiry_queue";

    let payload = serde_json::json!({
        "reservation_id": reservation_id,
        "source": source,
    });

    let mut conn = redis.as_ref().clone();
    redis::cmd("ZADD")
        .arg(EXPIRY_QUEUE_KEY)
        .arg(expires_at)
        .arg(payload.to_string())
        .query_async::<()>(&mut conn)
        .await
        .map_err(|e| {
            crate::core::error::AppError::InternalError(format!("Redis ZADD failed: {}", e))
        })?;

    Ok(())
}

async fn remove_expiry_from_redis(
    redis: &Arc<redis::aio::MultiplexedConnection>,
    reservation_id: &str,
) -> AppResult<()> {
    const EXPIRY_QUEUE_KEY: &str = "doctor:expiry_queue";

    let _pattern = format!("%\"reservation_id\":\"{}%\"", reservation_id);

    let mut conn = redis.as_ref().clone();
    redis::cmd("ZREMRANGEBYSCORE")
        .arg(EXPIRY_QUEUE_KEY)
        .arg("-inf")
        .arg("+inf")
        .query_async::<()>(&mut conn)
        .await
        .map_err(|e| {
            crate::core::error::AppError::InternalError(format!(
                "Redis ZREMRANGEBYSCORE failed: {}",
                e
            ))
        })?;

    Ok(())
}
