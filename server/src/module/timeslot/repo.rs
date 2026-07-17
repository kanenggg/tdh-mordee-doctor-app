use deadpool_redis::{redis::AsyncCommands, Pool as RedisPool};
use jiff::civil::Date;
use jiff::{civil::Time, Timestamp};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::doctor_actor::model::{
    AdHocSchedule, DoctorReservation, DoctorScheduleConfig, RoutineSchedule, TimeRange,
};
use crate::module::timeslot::model::{Reservation, Timeslot};

pub use crate::doctor_actor::model::ReservationSource;

#[async_trait::async_trait]
pub trait TimeslotRepo: Send + Sync {
    async fn get_doctor_reservations(
        &self,
        doctor_id: String,
        from_date: Date,
        to_date: Date,
    ) -> Result<Vec<DoctorReservation>, anyhow::Error>;

    async fn get_schedule_config(
        &self,
        doctor_id: &str,
    ) -> Result<Option<DoctorScheduleConfig>, anyhow::Error>;

    async fn find_available_timeslots(
        &self,
        doctor_id: i32,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<Timeslot>, anyhow::Error>;

    async fn get_timeslot(&self, timeslot_id: &str) -> Result<Option<Timeslot>, anyhow::Error>;

    async fn reserve_timeslot(
        &self,
        timeslot: &Timeslot,
        reservation: &Reservation,
    ) -> Result<(), anyhow::Error>;

    async fn confirm_reservation(
        &self,
        reservation_id: &str,
        booking_id: &str,
        payment_reference: &str,
        confirmed_at: i64,
    ) -> Result<Option<String>, anyhow::Error>;

    async fn get_reservation(
        &self,
        reservation_id: &str,
    ) -> Result<Option<Reservation>, anyhow::Error>;

    async fn cancel_reservation(
        &self,
        reservation_id: Option<&str>,
        booking_id: Option<&str>,
        cancelled_at: i64,
    ) -> Result<bool, anyhow::Error>;

    async fn find_reservation_by_booking_id(
        &self,
        booking_id: &str,
    ) -> Result<Option<Reservation>, anyhow::Error>;

    async fn find_expired_reservations(&self, now: i64) -> Result<Vec<Reservation>, anyhow::Error>;

    async fn update_timeslot_status(
        &self,
        timeslot_id: &str,
        status: crate::module::timeslot::model::TimeslotStatus,
    ) -> Result<(), anyhow::Error>;

    async fn find_overlapping_timeslots(
        &self,
        doctor_id: i32,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<Timeslot>, anyhow::Error>;
}

pub struct TimeslotRepoImpl {
    pool: PgPool,
    redis_pool: RedisPool,
}

impl TimeslotRepoImpl {
    pub fn new(pool: PgPool, redis_pool: RedisPool) -> Self {
        Self { pool, redis_pool }
    }
}

#[async_trait::async_trait]
impl TimeslotRepo for TimeslotRepoImpl {
    async fn get_doctor_reservations(
        &self,
        doctor_id: String,
        from_date: Date,
        to_date: Date,
    ) -> Result<Vec<DoctorReservation>, anyhow::Error> {
        let doctor_id_int: i32 = doctor_id.parse()?;

        // Convert jiff Date to i64 (seconds since epoch) for sqlx
        let from_timestamp = from_date
            .at(0, 0, 0, 0)
            .to_zoned(jiff::tz::TimeZone::UTC)?
            .timestamp()
            .as_second();
        let to_timestamp = to_date
            .at(23, 59, 59, 0)
            .to_zoned(jiff::tz::TimeZone::UTC)?
            .timestamp()
            .as_second();

        let query =
            sqlx::query_as::<_, (i64, i64, i64)>(
                r#"
            SELECT reservation_id::bigint, EXTRACT(EPOCH FROM reserved_from)::bigint, EXTRACT(EPOCH FROM reserved_until)::bigint
            FROM get_doctor_reservations($1::int, to_timestamp($2::bigint), to_timestamp($3::bigint))
            "#,
            )
            .bind(doctor_id_int)
            .bind(from_timestamp)
            .bind(to_timestamp);

        let rows = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            query.fetch_all(&self.pool),
        )
        .await
        .map_err(|_| anyhow::anyhow!("Query timed out after 10 seconds"))?
        .map_err(|e| anyhow::anyhow!("Failed to get doctor reservations: {}", e))?;

        let mut result: Vec<DoctorReservation> = Vec::new();
        for row in rows {
            // Convert unix timestamps (seconds) to jiff::Timestamp
            let reserved_from = Timestamp::from_second(row.1)?;
            let reserved_until = Timestamp::from_second(row.2)?;

            result.push(DoctorReservation {
                reservation_id: row.0,
                reserved_from,
                reserved_until,
            });
        }

        Ok(result)
    }

    async fn get_schedule_config(
        &self,
        doctor_id: &str,
    ) -> Result<Option<DoctorScheduleConfig>, anyhow::Error> {
        // Try Redis first
        let mut conn = self.redis_pool.get().await?;
        let key = format!("doctor:{}:schedule_config", doctor_id);

        let schedule_json: Option<String> = conn.get(&key).await?;

        match schedule_json {
            Some(json) => {
                let config: DoctorScheduleConfig = serde_json::from_str(&json)?;
                Ok(Some(config))
            }
            None => {
                tracing::info!("No schedule config found for doctor {}", doctor_id);
                Ok(None)
            }
        }
    }

    async fn find_available_timeslots(
        &self,
        _doctor_id: i32,
        _start_time: i64,
        _end_time: i64,
    ) -> Result<Vec<Timeslot>, anyhow::Error> {
        Ok(Vec::new())
    }

    async fn get_timeslot(&self, _timeslot_id: &str) -> Result<Option<Timeslot>, anyhow::Error> {
        Ok(None)
    }

    async fn reserve_timeslot(
        &self,
        _timeslot: &Timeslot,
        _reservation: &Reservation,
    ) -> Result<(), anyhow::Error> {
        Ok(())
    }

    async fn confirm_reservation(
        &self,
        _reservation_id: &str,
        _booking_id: &str,
        _payment_reference: &str,
        _confirmed_at: i64,
    ) -> Result<Option<String>, anyhow::Error> {
        Ok(None)
    }

    async fn get_reservation(
        &self,
        _reservation_id: &str,
    ) -> Result<Option<Reservation>, anyhow::Error> {
        Ok(None)
    }

    async fn cancel_reservation(
        &self,
        _reservation_id: Option<&str>,
        _booking_id: Option<&str>,
        _cancelled_at: i64,
    ) -> Result<bool, anyhow::Error> {
        Ok(false)
    }

    async fn find_reservation_by_booking_id(
        &self,
        _booking_id: &str,
    ) -> Result<Option<Reservation>, anyhow::Error> {
        Ok(None)
    }

    async fn find_expired_reservations(
        &self,
        _now: i64,
    ) -> Result<Vec<Reservation>, anyhow::Error> {
        Ok(Vec::new())
    }

    async fn update_timeslot_status(
        &self,
        _timeslot_id: &str,
        _status: crate::module::timeslot::model::TimeslotStatus,
    ) -> Result<(), anyhow::Error> {
        Ok(())
    }

    async fn find_overlapping_timeslots(
        &self,
        _doctor_id: i32,
        _start_time: i64,
        _end_time: i64,
    ) -> Result<Vec<Timeslot>, anyhow::Error> {
        Ok(Vec::new())
    }
}
