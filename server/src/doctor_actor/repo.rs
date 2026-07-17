use deadpool_redis::{redis::AsyncCommands, Pool as RedisPool};
use jiff::civil::Date;
use jiff::{civil::Time, Timestamp};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::doctor_actor::model::{DoctorReservation, DoctorScheduleConfig};

#[async_trait::async_trait]
pub trait DoctorTimeslotRepo: Send + Sync {
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

    async fn insert_reservation(
        &self,
        reservation: &ReservationRecord,
    ) -> Result<(), anyhow::Error>;

    async fn update_reservation_status(
        &self,
        reservation_id: &str,
        status: ReservationStatus,
    ) -> Result<(), anyhow::Error>;

    async fn get_reservation_by_correlation(
        &self,
        correlation_id: &str,
    ) -> Result<Option<ReservationRecord>, anyhow::Error>;

    async fn find_reservations_by_date(
        &self,
        doctor_id: &str,
        date: Date,
    ) -> Result<Vec<(Time, Time)>, anyhow::Error>;
}

#[derive(Debug, Clone)]
pub struct DoctorTimeslotRepoImpl {
    pool: PgPool,
    redis_pool: RedisPool,
}

impl DoctorTimeslotRepoImpl {
    pub fn new(pool: PgPool, redis_pool: RedisPool) -> Self {
        Self { pool, redis_pool }
    }
}

#[async_trait::async_trait]
impl DoctorTimeslotRepo for DoctorTimeslotRepoImpl {
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

    async fn insert_reservation(
        &self,
        reservation: &ReservationRecord,
    ) -> Result<(), anyhow::Error> {
        // Convert jiff Date to string for sqlx (YYYY-MM-DD format)
        let slot_date_str = reservation.slot_date.to_string();
        let start_time_str = reservation.start_time.to_string();
        let end_time_str = reservation.end_time.to_string();

        sqlx::query(
            r#"
            INSERT INTO doctor_reservations
            (id, doctor_id, patient_id, slot_date, start_time, end_time,
             status, correlation_id, source, expires_at, created_at)
            VALUES ($1, $2, $3, $4::date, $5::time, $6::time, $7, $8, $9, to_timestamp($10::bigint), to_timestamp($11::bigint))
            "#,
        )
        .bind(reservation.id)
        .bind(reservation.doctor_id)
        .bind(reservation.patient_id)
        .bind(slot_date_str)
        .bind(start_time_str)
        .bind(end_time_str)
        .bind(reservation.status)
        .bind(&reservation.correlation_id)
        .bind(reservation.source.as_str())
        .bind(reservation.expires_at)
        .bind(reservation.created_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn update_reservation_status(
        &self,
        reservation_id: &str,
        status: ReservationStatus,
    ) -> Result<(), anyhow::Error> {
        let reservation_uuid: Uuid = reservation_id.parse()?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let cancelled_at = if matches!(
            status,
            ReservationStatus::Cancelled | ReservationStatus::Expired
        ) {
            Some(now)
        } else {
            None
        };

        sqlx::query(
            r#"
            UPDATE doctor_reservations
            SET status = $1, cancelled_at = CASE WHEN $2 IN ('Cancelled', 'Expired') THEN to_timestamp($3::bigint) ELSE NULL END
            WHERE id = $4 AND status = 'Pending'
            "#,
        )
        .bind(status)
        .bind(matches!(status, ReservationStatus::Cancelled | ReservationStatus::Expired))
        .bind(cancelled_at)
        .bind(reservation_uuid)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_reservation_by_correlation(
        &self,
        correlation_id: &str,
    ) -> Result<Option<ReservationRecord>, anyhow::Error> {
        let row = sqlx::query_as::<_, (
            Uuid, i32, Option<Uuid>, String, String, String,
            ReservationStatus, String, String, Option<i64>, i64,
            Option<i64>, Option<i64>
        )>(
            r#"
            SELECT id, doctor_id, patient_id, slot_date::text, start_time::text, end_time::text,
                   status, correlation_id, source, EXTRACT(EPOCH FROM expires_at)::bigint, EXTRACT(EPOCH FROM created_at)::bigint,
                   EXTRACT(EPOCH FROM confirmed_at)::bigint, EXTRACT(EPOCH FROM cancelled_at)::bigint
            FROM doctor_reservations
            WHERE correlation_id = $1
            LIMIT 1
            "#,
        )
        .bind(correlation_id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            // Parse date and time strings back to jiff types
            let slot_date = Date::strptime(&row.3, "%Y-%m-%d")?;
            let start_time = Time::strptime(&row.4, "%H:%M:%S")?;
            let end_time = Time::strptime(&row.5, "%H:%M:%S")?;

            Ok(Some(ReservationRecord {
                id: row.0,
                doctor_id: row.1,
                patient_id: row.2,
                slot_date,
                start_time,
                end_time,
                status: row.6,
                correlation_id: row.7,
                source: row.8,
                expires_at: row.9,
                created_at: row.10,
                confirmed_at: row.11,
                cancelled_at: row.12,
            }))
        } else {
            Ok(None)
        }
    }

    async fn find_reservations_by_date(
        &self,
        doctor_id: &str,
        date: Date,
    ) -> Result<Vec<(Time, Time)>, anyhow::Error> {
        let doctor_id_int: i32 = doctor_id.parse()?;
        let date_str = date.to_string();

        let rows = sqlx::query_as::<_, (String, String)>(
            r#"
            SELECT start_time::text, end_time::text
            FROM doctor_reservations
            WHERE doctor_id = $1
              AND slot_date = $2::date
              AND status IN ('Pending', 'Confirmed')
            "#,
        )
        .bind(doctor_id_int)
        .bind(&date_str)
        .fetch_all(&self.pool)
        .await?;

        let mut result = Vec::with_capacity(rows.len());
        for (start_str, end_str) in rows {
            let start = Time::strptime(&start_str, "%H:%M:%S")?;
            let end = Time::strptime(&end_str, "%H:%M:%S")?;
            result.push((start, end));
        }
        Ok(result)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type)]
#[sqlx(
    type_name = "reservation_status_enum",
    rename_all = "SCREAMING_SNAKE_CASE"
)]
pub enum ReservationStatus {
    Pending,
    Confirmed,
    Cancelled,
    Expired,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ReservationRecord {
    pub id: Uuid,
    pub doctor_id: i32,
    pub patient_id: Option<Uuid>,
    pub slot_date: Date,
    pub start_time: Time,
    pub end_time: Time,
    pub status: ReservationStatus,
    pub correlation_id: String,
    pub source: String,
    pub expires_at: Option<i64>,
    pub created_at: i64,
    pub confirmed_at: Option<i64>,
    pub cancelled_at: Option<i64>,
}
