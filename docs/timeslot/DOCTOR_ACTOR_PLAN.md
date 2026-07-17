# Doctor Actor Refactoring - Implementation Plan

## Overview

Introduce a **shared `DoctorActor` trait** in standalone `server/src/doctor_actor/` module. Both **timeslot HTTP handlers** and **consultation summarization** will depend on this actor.

The actor will start with timeslot behaviors, but **can extend with other doctor behaviors** later:
- Timeslot: `get_available_timeslots` (on-the-fly generation only), `reserve_timeslot`, `release_timeslot`
- Future: `enable_instant`, `disable_instant`, `toggle_availability`, etc.

## On-the-Fly Timeslot Generation (Only Mode)

**Important:** The doctor actor supports **on-the-fly timeslot generation ONLY** (no pre-created timeslots).

All timeslots are generated dynamically based on:
- Routine schedules (day-of-week patterns)
- Ad-hoc schedules (specific date overrides)
- Existing reservations (to avoid conflicts)

**Return Type Simplification:** On-the-fly generation returns **simple time ranges** (start/end only):
- Returns `Vec<TimeRange>` (start_time, end_time) instead of full timeslot objects
- No `timeslot_id` or `slot_id` because timeslots aren't stored in database
- Lighter payload, simpler API
- Reservations are created directly from time ranges (not linked to pre-created timeslots)

**Note:** No legacy pre-created timeslots support. All doctors must have schedule configs to generate timeslots on-the-fly.

## Legacy System Compatibility

**Important:** The legacy system uses **pre-created timeslots with `i64` IDs**.

For backward compatibility:
- `timeslot_id`: `i64` (not `String`)
- `reservation_id`: `i64` (not `String`)
- Timeslots are pre-generated in the legacy system
- Database columns should use `BIGINT` for these IDs

**Note:** The current `Timeslot` model in `module/timeslot/models.rs` uses `String` for `timeslot_id`. The new `doctor_actor` module will use `i64` for all ID types to match the legacy system. Consider migrating the existing models when implementing this refactoring.

## Key Design Principle: Behavior Abstraction for Testability

The `DoctorActor` uses **behavior injection** - accepting trait objects for side effects like:
- **Rate limiting** - `RateLimiterBehavior` trait
- **Event publishing** - `EventPublisherBehavior` trait  
- **Idempotency caching** - `IdempotencyCacheBehavior` trait
- **Time source** - `TimeSource` trait

This allows easy unit testing by injecting mock implementations without depending on external services (Redis, Pub/Sub, etc.).

---

## Architecture

```
┌──────────────────────────────────────────────┐
│ doctor_actor/ (STANDALONE MODULE)      │
│                                         │
│  DoctorActorImpl                         │
│  - repo: Arc<dyn DoctorTimeslotRepo>     │
│  - rate_limiter: Arc<dyn RateLimiterBehavior>  ← injectable
│  - event_publisher: Arc<dyn EventPublisherBehavior> ← injectable
│  - idempotency: Arc<dyn IdempotencyCacheBehavior> ← injectable
│  - time_source: Arc<dyn TimeSource>      ← injectable
│                                         │
│  DoctorActor trait                       │
│  - get_available_timeslots()             │ ← on-the-fly generation only
│  - reserve_timeslot()                    │ ← reserve by time range
│  - release_timeslot()                   │
│  - [future behaviors...]                 │
└────────────┬─────────────────────────────┘
              │
              │ uses
              v
┌──────────────────────────────────────────────┐
│ doctor_actor/repo.rs                    │
│ - DoctorTimeslotRepo trait                │
│ - get_doctor_reservations(date_range)   │ ← supports period
│ - get_schedule_config()                  │ ← for on-the-fly generation
│ - create_reservation()                   │ ← create from time range
│ - update_reservation(reservation_id)       │ ← update by reservation
└──────────────────────────────────────────────┘

┌──────────────────────────────────────────────┐
│ doctor_actor/commons.rs                   │
│ - generate_timeslots()                    │ ← on-the-fly generation logic
│   (moved from get_available_timeslot.rs)   │
└──────────────────────────────────────────────┘

┌──────────────────────────────────────────────┐
│ doctor_actor/behaviors.rs                │
│ - RateLimiterBehavior trait               │
│ - EventPublisherBehavior trait           │
│ - IdempotencyCacheBehavior trait         │
│ - TimeSource trait                       │
│ - Production implementations              │
└──────────────────────────────────────────────┘
```

---

## Module Structure

```
server/src/doctor_actor/
├── mod.rs                    # Module exports
├── repo.rs                    # DoctorTimeslotRepo trait + impl
├── actor.rs                   # DoctorActor trait + impl
├── behaviors.rs               # Behavior traits + production impls
├── models.rs                  # Data models
└── commons.rs                 # Reuse timeslot/commons.rs
```

---

## Repository Trait (DoctorTimeslotRepo)

**Note:** No pre-created timeslots. All timeslots generated on-the-fly.

```rust
#[async_trait]
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

    async fn create_reservation(
        &self,
        reservation: &TimeslotReservation,
    ) -> Result<i64, anyhow::Error>;  // Returns reservation_id

    async fn find_reservation(
        &self,
        reservation_id: i64,
    ) -> Result<Option<TimeslotReservation>, anyhow::Error>;

    async fn confirm_reservation(
        &self,
        reservation_id: i64,
        booking_id: &str,
        payment_reference: &str,
        confirmed_at: i64,
    ) -> Result<(), anyhow::Error>;

    async fn cancel_reservation(
        &self,
        reservation_id: i64,
        cancelled_at: i64,
    ) -> Result<(), anyhow::Error>;

    async fn find_reservations_by_status(
        &self,
        doctor_id: &str,
        start_date: Date,
        end_date: Date,
        status: Option<ReservationStatus>,
    ) -> Result<Vec<TimeslotReservation>, anyhow::Error>;
}
```

---

## Behavior Traits (for Testability)

### Event Types (No timeslot_id)

**Note:** Events no longer include `timeslot_id` since timeslots aren't pre-created.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "__type", rename_all = "PascalCase")]
pub enum TimeslotReservedEvent {
    TimeslotReserved {
        reservation_id: i64,
        doctor_id: i32,
        patient_id: i32,
        slot_date: Date,
        start_time: Time,
        end_time: Time,
        expires_at: i64,
        reserved_at: i64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "__type", rename_all = "PascalCase")]
pub enum TimeslotConfirmedEvent {
    TimeslotConfirmed {
        reservation_id: i64,
        booking_id: String,
        doctor_id: i32,
        patient_id: i32,
        confirmed_at: i64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReleaseReason {
    Expired,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "__type", rename_all = "PascalCase")]
pub enum TimeslotReleasedEvent {
    TimeslotReleased {
        doctor_id: i32,
        patient_id: i32,
        reservation_id: i64,
        slot_date: Date,
        start_time: Time,
        end_time: Time,
        released_at: i64,
        reason: ReleaseReason,
    },
}
```

### RateLimiterBehavior
```rust
#[async_trait]
pub trait RateLimiterBehavior: Send + Sync {
    async fn check_and_increment(
        &self,
        patient_id: i32,
    ) -> Result<Option<crate::module::timeslot::models::RateLimitType>, anyhow::Error>;

    fn daily_limit(&self) -> i32;
    fn weekly_limit(&self) -> i32;
    fn get_seconds_until_window_reset(&self, limit_type: RateLimitType) -> i32;
}
```

### EventPublisherBehavior
```rust
#[async_trait]
pub trait EventPublisherBehavior: Send + Sync {
    async fn publish_timeslot_reserved(
        &self,
        event: crate::module::timeslot::models::TimeslotReservedEvent,
    ) -> Result<(), anyhow::Error>;

    async fn publish_timeslot_confirmed(
        &self,
        event: crate::module::timeslot::models::TimeslotConfirmedEvent,
    ) -> Result<(), anyhow::Error>;

    async fn publish_timeslot_released(
        &self,
        event: crate::module::timeslot::models::TimeslotReleasedEvent,
    ) -> Result<(), anyhow::Error>;
}
```

### IdempotencyCacheBehavior
```rust
#[async_trait]
pub trait IdempotencyCacheBehavior: Send + Sync {
    async fn get_cached_response(
        &self,
        correlation_id: &str,
    ) -> Result<Option<crate::module::timeslot::idempotency::CachedReserveResponse>, anyhow::Error>;

    async fn cache_response(
        &self,
        correlation_id: &str,
        response: &crate::module::timeslot::idempotency::CachedReserveResponse,
        ttl_seconds: i32,
    ) -> Result<(), anyhow::Error>;
}
```

### TimeSource

```rust
pub trait TimeSource: Send + Sync {
    fn now_epoch_secs(&self) -> i64;
}

struct SystemTimeSource;

impl TimeSource for SystemTimeSource {
    fn now_epoch_secs(&self) -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before epoch")
            .as_secs() as i64
    }
}
```

---

## Data Models for Period Queries

### On-the-Fly Timeslot Generation (Only Mode)

All timeslots are generated dynamically based on doctor's schedule configuration:

- Timeslots are not stored in database
- Generated on-demand from schedule config
- Returns simple time ranges (start/end only)
- No `timeslot_id` or `slot_id` needed
- Reservations created directly from time ranges

### Data Models (for doctor_actor module)

```rust
// Simple time range for on-the-fly generation
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimeRange {
    pub start_time: Time,
    pub end_time: Time,
}

// On-the-fly generation result (grouped by date)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedTimeslot {
    pub date: Date,
    pub time_ranges: Vec<TimeRange>,
}

// Reservation created from time range (no timeslot_id)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimeslotReservation {
    pub id: i64,
    pub doctor_id: i32,
    pub patient_id: i32,
    pub slot_date: Date,           // Date of the time slot
    pub start_time: Time,          // Start time of the slot
    pub end_time: Time,            // End time of the slot
    pub status: ReservationStatus,
    pub correlation_id: String,
    pub booking_id: Option<String>,
    pub payment_reference: Option<String>,
    pub expires_at: i64,
    pub created_at: i64,
    pub confirmed_at: Option<i64>,
    pub cancelled_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReserveResult {
    pub __type: String,
    pub reservation_id: Option<i64>,
    pub expires_at: Option<i64>,
    pub current_count: Option<i32>,
    pub max_allowed: Option<i32>,
    pub retry_after_seconds: Option<i32>,
}
```

### pageToken-Based Pagination

```rust
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DoctorTimeslot {
    pub slot_id: i64,  // Timeslot ID is i64 (legacy pre-created timeslots)
    pub slot_date: Date,
    pub start_time: jiff::civil::Time,
    pub end_time: jiff::civil::Time,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListTimeslotsResponse {
    pub timeslots: Vec<AvailableTimeslot>,
    pub next_page_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListReservationsParams {
    pub start_date: Date,
    pub end_date: Date,
    pub status: Option<ReservationStatus>,
    pub page_token: Option<String>,
    pub page_size: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListReservationsResponse {
    pub reservations: Vec<Reservation>,
    pub next_page_token: Option<String>,
}

// Helper: encode/decode pageToken (base64 cursor)
impl ListTimeslotsResponse {
    fn encode_page_token(cursor: &(Date, Time)) -> String {
        use base64::{engine::general_purpose::STANDARD, Engine};
        let cursor_str = format!("{}|{}", cursor.0, cursor.1);
        STANDARD.encode(cursor_str)
    }

    fn decode_page_token(token: &str) -> Result<(Date, Time), anyhow::Error> {
        use base64::{engine::general_purpose::STANDARD, Engine};
        let decoded = STANDARD.decode(token)?;
        let cursor_str = String::from_utf8(decoded)?;
        let parts: Vec<&str> = cursor_str.split('|').collect();
        if parts.len() != 2 {
            return Err(anyhow::anyhow!("Invalid pageToken format"));
        }
        let date = parts[0].parse::<Date>()?;
        let time = parts[1].parse::<Time>()?;
        Ok((date, time))
    }
}
```
```rust
pub trait TimeSource: Send + Sync {
    fn now_epoch_secs(&self) -> i64;
}

struct SystemTimeSource;

impl TimeSource for SystemTimeSource {
    fn now_epoch_secs(&self) -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before epoch")
            .as_secs() as i64
    }
}
```

---

## DoctorActor Trait

```rust
#[async_trait]
pub trait DoctorActor: Send + Sync {
    // Timeslot behaviors (on-the-fly generation only)
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
        date: Date,
        start_time: Time,
        end_time: Time,
        ttl_seconds: i64,
        correlation_id: &str,
        source: ReservationSource,
    ) -> Result<ReserveResult, anyhow::Error>;

    async fn release_timeslot(
        &self,
        reservation_id: i64,
        reason: ReleaseReason,
    ) -> Result<(), anyhow::Error>;

    // Future behaviors will be added here
}
```

### Query Period Support

All time-based queries support **date/time range filtering** to avoid large data transfers:

- **`get_available_timeslots`**: Returns timeslots within `[start_date, end_date]`
  - Optional time filters: `start_time` and `end_time` to narrow results further
  - Database query uses indexed columns for efficient filtering

- **`get_reservations`**: Returns reservations within `[start_date, end_date]`
  - Optional status filter: `Pending`, `Confirmed`, `Cancelled`, `Expired`
  - Supports periodic data fetching (e.g., weekly, monthly views)

### Pagination Support (pageToken-based)

For large date ranges, add cursor-based pagination using pageToken:

```rust
pub struct ListTimeslotsParams {
    pub start_date: Date,
    pub end_date: Date,
    pub page_token: Option<String>,
    pub page_size: Option<usize>,
}

pub struct ListTimeslotsResponse {
    pub timeslots: Vec<AvailableTimeslot>,
    pub next_page_token: Option<String>,
}

async fn get_available_timeslots(
    &self,
    doctor_id: &str,
    params: ListTimeslotsParams,
) -> Result<ListTimeslotsResponse, anyhow::Error>;
```

---

## DoctorActorImpl (Production Implementation)

```rust
pub struct DoctorActorImpl {
    repo: Arc<dyn DoctorTimeslotRepo>,
    rate_limiter: Arc<dyn RateLimiterBehavior>,
    event_publisher: Arc<dyn EventPublisherBehavior>,
    idempotency: Arc<dyn IdempotencyCacheBehavior>,
    time_source: Arc<dyn TimeSource>,
}

impl DoctorActorImpl {
    pub fn new(
        repo: Arc<dyn DoctorTimeslotRepo>,
        rate_limiter: Arc<dyn RateLimiterBehavior>,
        event_publisher: Arc<dyn EventPublisherBehavior>,
        idempotency: Arc<dyn IdempotencyCacheBehavior>,
        time_source: Arc<dyn TimeSource>,
    ) -> Self {
        Self {
            repo,
            rate_limiter,
            event_publisher,
            idempotency,
            time_source,
        }
    }

    // Helper method for production use (creates concrete impls)
    pub fn new_production(
        repo: Arc<dyn DoctorTimeslotRepo>,
        pg_pool: sqlx::PgPool,
        pubsub_publisher: Arc<crate::module::webhook::PubsubPublisher>,
        redis_url: &str,
    ) -> Result<Self, anyhow::Error> {
        use crate::module::timeslot::rate_limiter::RateLimiter;
        use crate::module::timeslot::idempotency::IdempotencyCache;
        use crate::module::doctor_actor::behaviors::{
            RateLimiterBehaviorImpl, EventPublisherBehaviorImpl,
            IdempotencyCacheBehaviorImpl, SystemTimeSource,
        };

        let rate_limiter = Arc::new(RateLimiterBehaviorImpl::new(
            pg_pool.clone(),
            10, // daily_limit
            50, // weekly_limit
        ));

        let event_publisher = Arc::new(EventPublisherBehaviorImpl::new(pubsub_publisher));

        let idempotency = Arc::new(IdempotencyCacheBehaviorImpl::new(redis_url).await?);

        let time_source = Arc::new(SystemTimeSource);

        Ok(Self::new(
            repo,
            rate_limiter,
            event_publisher,
            idempotency,
            time_source,
        ))
    }
}

#[async_trait]
impl DoctorActor for DoctorActorImpl {
    async fn get_available_timeslots(
        &self,
        doctor_id: &str,
        start_date: Date,
        end_date: Date,
    ) -> Result<Vec<GeneratedTimeslot>, anyhow::Error> {
        // Get schedule configuration (required for on-the-fly generation)
        let schedule_config = self.repo.get_schedule_config(doctor_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("No schedule config found for doctor {}", doctor_id))?;

        // Get existing reservations for the date range
        let reservations = self.repo.get_doctor_reservations(
            doctor_id.to_string(),
            start_date,
            end_date,
        ).await?;

        // Generate timeslots using commons::generate_timeslots logic
        let generated_timeslots = generate_timeslots(
            &schedule_config,
            start_date,
            end_date,
            &reservations,
        )?;

        // Group by date and convert to simple time ranges
        let mut grouped: std::collections::BTreeMap<Date, Vec<TimeRange>> = std::collections::BTreeMap::new();

        for slot in generated_timeslots {
            grouped.entry(slot.slot_date)
                .or_insert_with(Vec::new)
                .push(TimeRange {
                    start_time: slot.start_time,
                    end_time: slot.end_time,
                });
        }

        let result = grouped.into_iter()
            .map(|(date, time_ranges)| GeneratedTimeslot { date, time_ranges })
            .collect();

        Ok(result)
    }

    async fn reserve_timeslot(
        &self,
        doctor_id: &str,
        patient_id: Option<&str>,
        date: Date,
        start_time: Time,
        end_time: Time,
        ttl_seconds: i64,
        correlation_id: &str,
        source: ReservationSource,
    ) -> Result<ReserveResult, anyhow::Error> {
        // Check rate limit
        if let Some(patient_id) = patient_id {
            if let Some(limit_type) = self.rate_limiter.check_and_increment(patient_id).await? {
                let current_count = match limit_type {
                    RateLimitType::Daily => self.rate_limiter.daily_limit() + 1,
                    RateLimitType::Weekly => self.rate_limiter.weekly_limit() + 1,
                };

                return Ok(ReserveResult {
                    __type: "RateLimitExceeded".to_string(),
                    reservation_id: None,
                    expires_at: None,
                    current_count: Some(current_count),
                    max_allowed: Some(match limit_type {
                        RateLimitType::Daily => self.rate_limiter.daily_limit(),
                        RateLimitType::Weekly => self.rate_limiter.weekly_limit(),
                    }),
                    retry_after_seconds: Some(
                        self.rate_limiter.get_seconds_until_window_reset(limit_type),
                    ),
                });
            }
        }

        // Check idempotency
        if let Some(cached) = self.idempotency.get_cached_response(correlation_id).await? {
            return Ok(cached);
        }

        // Validate time range doesn't overlap with existing reservations
        let doctor_id_parsed = doctor_id.parse::<i32>()?;
        let patient_id_parsed = patient_id.unwrap_or(0);

        let reservations = self.repo.get_doctor_reservations(
            doctor_id.to_string(),
            date,
            date,
        ).await?;

        let slot_start = time_to_minutes(start_time);
        let slot_end = time_to_minutes(end_time);

        let conflicts = reservations.iter().any(|r| {
            // Check if reservation overlaps with requested time range
            (r.slot_date == date) && (r.status == ReservationStatus::Pending)
                && (time_to_minutes(r.end_time) > slot_start)
                && (time_to_minutes(r.start_time) < slot_end)
        });

        if conflicts {
            return Ok(ReserveResult {
                __type: "AlreadyReserved".to_string(),
                reservation_id: None,
                expires_at: None,
                current_count: None,
                max_allowed: None,
                retry_after_seconds: None,
            });
        }

        // Create reservation from time range (no timeslot_id)
        let now = self.time_source.now_epoch_secs();
        let expires_at = now + ttl_seconds;

        let reservation = TimeslotReservation {
            id: 0, // Will be set by DB
            doctor_id: doctor_id_parsed,
            patient_id: patient_id_parsed,
            slot_date: date,
            start_time,
            end_time,
            status: ReservationStatus::Pending,
            correlation_id: correlation_id.to_string(),
            booking_id: None,
            payment_reference: None,
            expires_at,
            created_at: now,
            confirmed_at: None,
            cancelled_at: None,
        };

        let reservation_id = self.repo.create_reservation(&reservation).await?;

        // Publish event (no timeslot_id)
        let event = TimeslotReservedEvent::TimeslotReserved {
            reservation_id,
            doctor_id: doctor_id_parsed,
            patient_id: patient_id_parsed,
            slot_date: date,
            start_time,
            end_time,
            expires_at,
            reserved_at: now,
        };

        self.event_publisher.publish_timeslot_reserved(event).await?;

        // Cache result
        let result = ReserveResult {
            __type: "Success".to_string(),
            reservation_id: Some(reservation_id),
            expires_at: Some(expires_at),
            current_count: None,
            max_allowed: None,
            retry_after_seconds: None,
        };

        self.idempotency.cache_response(correlation_id, &result, ttl_seconds as i32).await?;

        Ok(result)
    }

    async fn release_timeslot(
        &self,
        reservation_id: i64,
        reason: ReleaseReason,
    ) -> Result<(), anyhow::Error> {
        let now = self.time_source.now_epoch_secs();

        // Cancel reservation
        self.repo.cancel_reservation(reservation_id, now).await?;

        // Get reservation for event publishing
        if let Some(reservation) = self.repo.find_reservation(reservation_id).await? {
            // Publish release event (no timeslot_id)
            let event = TimeslotReleasedEvent::TimeslotReleased {
                doctor_id: reservation.doctor_id,
                patient_id: reservation.patient_id,
                reservation_id,
                slot_date: reservation.slot_date,
                start_time: reservation.start_time,
                end_time: reservation.end_time,
                released_at: now,
                reason,
            };

            self.event_publisher.publish_timeslot_released(event).await?;
        }

        Ok(())
    }
}

// Helper function to convert Time to minutes since midnight
fn time_to_minutes(time: Time) -> i32 {
    time.hour() as i32 * 60 + time.minute() as i32
}

// Re-use commons::generate_timeslots function
fn generate_timeslots(
    schedule_conf: &DoctorScheduleConfig,
    from_date: Date,
    to_date: Date,
    reservations: &[DoctorReservation],
) -> Result<Vec<DoctorTimeslot>, anyhow::Error> {
    // Implementation moved from commons.rs
    // (same logic as current get_available_timeslot.rs)
    crate::module::doctor_actor::commons::generate_timeslots(schedule_conf, from_date, to_date, reservations)
}
```

---

## Usage Examples

### Client: Get Available Timeslots

```rust
let start_date = Date::from_ymd(2024, 1, 15).unwrap();
let end_date = Date::from_ymd(2024, 1, 21).unwrap();

let timeslots = actor.get_available_timeslots(
    "doctor123",
    start_date,
    end_date,
).await?;

// timeslots contains GeneratedTimeslot grouped by date
// Each GeneratedTimeslot has:
//   - date: Date
//   - time_ranges: Vec<TimeRange> (start_time, end_time)
```

### Client: Reserve a Timeslot

```rust
let date = Date::from_ymd(2024, 1, 15).unwrap();
let start_time = Time::from_hms(9, 0, 0).unwrap();
let end_time = Time::from_hms(9, 30, 0).unwrap();

let result = actor.reserve_timeslot(
    "doctor123",
    Some("patient123"),
    date,
    start_time,
    end_time,
    900,  // TTL: 15 minutes
    "correlation-abc123",
    ReservationSource::Patient,
).await?;

match result.__type.as_str() {
    "Success" => {
        println!("Reserved! Reservation ID: {:?}", result.reservation_id);
    }
    "AlreadyReserved" => {
        println!("Timeslot already reserved");
    }
    "RateLimitExceeded" => {
        println!("Rate limited. Retry after: {:?}", result.retry_after_seconds);
    }
    _ => {}
}
```

---

## Unit Testing Example

### Test On-the-Fly Timeslot Generation

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct MockRateLimiter;
    struct MockEventPublisher;
    struct MockIdempotencyCache;
    struct MockTimeSource;

    struct MockRepo {
        configs: HashMap<String, DoctorScheduleConfig>,
        reservations: Vec<DoctorReservation>,
    }

    impl MockRepo {
        fn new() -> Self {
            Self {
                configs: HashMap::new(),
                reservations: Vec::new(),
            }
        }

        fn with_config(mut self, doctor_id: &str, config: DoctorScheduleConfig) -> Self {
            self.configs.insert(doctor_id.to_string(), config);
            self
        }

        fn with_reservations(mut self, reservations: Vec<DoctorReservation>) -> Self {
            self.reservations = reservations;
            self
        }
    }

    #[async_trait]
    impl RateLimiterBehavior for MockRateLimiter {
        async fn check_and_increment(&self, _patient_id: i32) -> Result<Option<RateLimitType>, anyhow::Error> {
            Ok(None) // No rate limit
        }
        fn daily_limit(&self) -> i32 { 10 }
        fn weekly_limit(&self) -> i32 { 50 }
        fn get_seconds_until_window_reset(&self, _limit_type: RateLimitType) -> i32 { 60 }
    }

    #[async_trait]
    impl EventPublisherBehavior for MockEventPublisher {
        async fn publish_timeslot_reserved(&self, _event: TimeslotReservedEvent) -> Result<(), anyhow::Error> {
            Ok(())
        }
        async fn publish_timeslot_confirmed(&self, _event: TimeslotConfirmedEvent) -> Result<(), anyhow::Error> {
            Ok(())
        }
        async fn publish_timeslot_released(&self, _event: TimeslotReleasedEvent) -> Result<(), anyhow::Error> {
            Ok(())
        }
    }

    #[async_trait]
    impl IdempotencyCacheBehavior for MockIdempotencyCache {
        async fn get_cached_response(&self, _correlation_id: &str) -> Result<Option<CachedReserveResponse>, anyhow::Error> {
            Ok(None) // No cached response
        }
        async fn cache_response(&self, _correlation_id: &str, _response: &CachedReserveResponse, _ttl_seconds: i32) -> Result<(), anyhow::Error> {
            Ok(())
        }
    }

    impl TimeSource for MockTimeSource {
        fn now_epoch_secs(&self) -> i64 {
            1234567890 // Fixed time for deterministic tests
        }
    }

    #[tokio::test]
    async fn test_reserve_timeslot_success() {
        let mock_repo = Arc::new(MockRepo::new());
        let mock_rate_limiter = Arc::new(MockRateLimiter);
        let mock_event_publisher = Arc::new(MockEventPublisher);
        let mock_idempotency = Arc::new(MockIdempotencyCache);
        let mock_time_source = Arc::new(MockTimeSource);

        let actor = DoctorActorImpl::new(
            mock_repo,
            mock_rate_limiter,
            mock_event_publisher,
            mock_idempotency,
            mock_time_source,
        );

        let date = Date::from_ymd(2024, 1, 15).unwrap();
        let start_time = Time::from_hms(9, 0, 0).unwrap();
        let end_time = Time::from_hms(9, 30, 0).unwrap();

        let result = actor.reserve_timeslot(
            "doctor123",
            Some("patient123"),
            date,
            start_time,
            end_time,
            900,  // TTL: 15 minutes
            "correlation-123",
            ReservationSource::Patient,
        ).await;

        assert!(result.is_ok());
        let reserve_result = result.unwrap();
        assert_eq!(reserve_result.__type, "Success");
        assert!(reserve_result.reservation_id.is_some());
    }

    #[tokio::test]
    async fn test_get_available_timeslots_on_the_fly() {
        use crate::module::timeslot::repo::{RoutineSchedule, TimeRange};

        let config = DoctorScheduleConfig {
            routine: vec![RoutineSchedule {
                day_of_week: 1, // Monday
                times: vec![TimeRange {
                    start_time: Time::from_hms(9, 0, 0).unwrap(),
                    end_time: Time::from_hms(10, 0, 0).unwrap(),
                }],
            }],
            ad_hoc: Vec::new(),
            slot_duration: 30,
        };

        let mock_repo = Arc::new(
            MockRepo::new()
                .with_config("doctor123", config)
        );

        let mock_rate_limiter = Arc::new(MockRateLimiter);
        let mock_event_publisher = Arc::new(MockEventPublisher);
        let mock_idempotency = Arc::new(MockIdempotencyCache);
        let mock_time_source = Arc::new(MockTimeSource);

        let actor = DoctorActorImpl::new(
            mock_repo,
            mock_rate_limiter,
            mock_event_publisher,
            mock_idempotency,
            mock_time_source,
        );

        let start_date = Date::from_ymd(2024, 1, 1).unwrap(); // Monday
        let end_date = Date::from_ymd(2024, 1, 1).unwrap();

        let result = actor.get_available_timeslots(
            "doctor123",
            start_date,
            end_date,
        ).await;

        assert!(result.is_ok());
        let generated = result.unwrap();
        assert_eq!(generated.len(), 1); // One day
        assert_eq!(generated[0].date, start_date);
        assert_eq!(generated[0].time_ranges.len(), 2); // Two 30-min slots
        assert_eq!(generated[0].time_ranges[0].start_time, Time::from_hms(9, 0, 0).unwrap());
        assert_eq!(generated[0].time_ranges[0].end_time, Time::from_hms(9, 30, 0).unwrap());
    }
```

### Test On-the-Fly Generation

```rust
    #[tokio::test]
    async fn test_generate_available_timeslots_on_the_fly() {
        use crate::module::timeslot::repo::{RoutineSchedule, TimeRange};

        let config = DoctorScheduleConfig {
            routine: vec![RoutineSchedule {
                day_of_week: 1, // Monday
                times: vec![TimeRange {
                    start_time: Time::from_hms(9, 0, 0).unwrap(),
                    end_time: Time::from_hms(10, 0).unwrap(),
                }],
            }],
            ad_hoc: Vec::new(),
            slot_duration: 30,
        };

        let mut mock_repo = MockRepo::new().with_config("doctor123", config);

        mock_repo.expect_get_doctor_reservations()
            .returning(|_doctor_id, _from_date, _to_date| {
                Ok(vec![])
            });

        mock_repo.expect_get_schedule_config()
            .returning(|doctor_id| {
                Ok(Some(mock_repo.configs.get(doctor_id).cloned().unwrap()))
            });

        let mock_rate_limiter = Arc::new(MockRateLimiter);
        let mock_event_publisher = Arc::new(MockEventPublisher);
        let mock_idempotency = Arc::new(MockIdempotencyCache);
        let mock_time_source = Arc::new(MockTimeSource);

        let actor = DoctorActorImpl::new(
            Arc::new(mock_repo),
            mock_rate_limiter,
            mock_event_publisher,
            mock_idempotency,
            mock_time_source,
        );

        let start_date = Date::from_ymd(2024, 1, 1).unwrap(); // Monday
        let end_date = Date::from_ymd(2024, 1, 1).unwrap();

        let result = actor.generate_available_timeslots(
            "doctor123",
            start_date,
            end_date,
        ).await;

        assert!(result.is_ok());
        let generated = result.unwrap();
        assert_eq!(generated.len(), 1); // One day
        assert_eq!(generated[0].date, start_date);
        assert_eq!(generated[0].time_ranges.len(), 2); // Two 30-min slots
        assert_eq!(generated[0].time_ranges[0].start_time, Time::from_hms(9, 0, 0).unwrap());
        assert_eq!(generated[0].time_ranges[0].end_time, Time::from_hms(9, 30, 0).unwrap());
    }

    #[tokio::test]
    async fn test_get_available_timeslots_returns_on_the_fly_when_schedule_config_exists() {
        let config = DoctorScheduleConfig {
            routine: vec![],
            ad_hoc: Vec::new(),
            slot_duration: 30,
        };

        let mut mock_repo = MockRepo::new().with_config("doctor123", config);

        mock_repo.expect_get_schedule_config()
            .returning(|doctor_id| {
                Ok(Some(mock_repo.configs.get(doctor_id).cloned().unwrap()))
            });

        let actor = DoctorActorImpl::new(
            Arc::new(mock_repo),
            Arc::new(MockRateLimiter),
            Arc::new(MockEventPublisher),
            Arc::new(MockIdempotencyCache),
            Arc::new(MockTimeSource),
        );

        let start_date = Date::from_ymd(2024, 1, 1).unwrap();
        let end_date = Date::from_ymd(2024, 1, 7).unwrap();

        let params = ListTimeslotsParams {
            start_date,
            end_date,
            start_time: None,
            end_time: None,
            page_token: None,
            page_size: None,
        };

        let result = actor.get_available_timeslots("doctor123", params).await;

        assert!(result.is_ok());
        match result.unwrap() {
            GetAvailableTimeslotsResult::OnTheFly { generated } => {
                // Should return on-the-fly generated timeslots
                assert!(generated.is_empty()); // Empty schedule = empty result
            }
            GetAvailableTimeslotsResult::PreCreated { .. } => {
                panic!("Expected OnTheFly result when schedule config exists");
            }
        }
    }
```

### pageToken-Based Pagination Examples

**Example 1: Paginated query for large date ranges**
```rust
// Client-side pagination loop
async fn fetch_all_timeslots(
    actor: &Arc<dyn DoctorActor>,
    doctor_id: &str,
    start_date: Date,
    end_date: Date,
) -> Result<Vec<AvailableTimeslot>, anyhow::Error> {
    let mut all_timeslots = Vec::new();
    let mut page_token: Option<String> = None;
    let page_size = 50;

    loop {
        let params = ListTimeslotsParams {
            start_date,
            end_date,
            start_time: None,
            end_time: None,
            page_token,
            page_size: Some(page_size),
        };

        let result = actor.get_available_timeslots(doctor_id, params).await?;
        all_timeslots.extend(result.timeslots);

        match result.next_page_token {
            Some(token) => page_token = Some(token),
            None => break,
        }
    }

    Ok(all_timeslots)
}
```

**Example 2: Server-side implementation with cursor**
```rust
// In DoctorActorImpl
async fn get_available_timeslots(
    &self,
    doctor_id: &str,
    params: ListTimeslotsParams,
) -> Result<ListTimeslotsResponse, anyhow::Error> {
    let page_size = params.page_size.unwrap_or(100).min(500); // max 500 per page

    // Decode pageToken to get cursor position
    let cursor = match &params.page_token {
        Some(token) => Some(ListTimeslotsResponse::decode_page_token(token)?),
        None => None,
    };

    // Build query with cursor filter
    let timeslots = self.repo.find_available_timeslots_paginated(
        doctor_id,
        params.start_date,
        params.end_date,
        params.start_time,
        params.end_time,
        cursor.as_ref(),
        page_size,
    ).await?;

    // Encode next page token if more results available
    let next_page_token = if timeslots.len() == page_size {
        if let Some(last_slot) = timeslots.last() {
            Some(ListTimeslotsResponse::encode_page_token(&(
                last_slot.start_date,
                last_slot.start_time,
            )))
        } else {
            None
        }
    } else {
        None
    };

    Ok(ListTimeslotsResponse {
        timeslots,
        next_page_token,
    })
}
```

---

### Performance Monitoring

Track period query performance with tracing:

```rust
// In repo implementation
async fn find_available_timeslots_paginated(
    &self,
    doctor_id: &str,
    start_date: Date,
    end_date: Date,
    start_time: Option<Time>,
    end_time: Option<Time>,
    cursor: Option<&(Date, Time)>,
    page_size: usize,
) -> Result<Vec<AvailableTimeslot>, anyhow::Error> {
    let span = tracing::info_span!(
        "find_available_timeslots_paginated",
        doctor_id = %doctor_id,
        date_range_days = (end_date - start_date).total_days(),
        page_size,
        has_cursor = cursor.is_some()
    );

    let _enter = span.enter();

    let query = sqlx::query_as::<_, AvailableTimeslot>(/* ... */)
        .bind(doctor_id)
        .bind(start_date)
        .bind(end_date)
        .bind(start_time)
        .bind(end_time)
        .bind(cursor.map(|c| c.0))  // cursor date
        .bind(cursor.map(|c| c.1))  // cursor time
        .bind(page_size as i64 + 1);  // fetch one extra to check if more pages

    let result = query.fetch_all(&self.pool).await?;

    tracing::info!(
        "Retrieved {} timeslots (requested: {})",
        result.len(),
        page_size
    );

    Ok(result)
}
```

---

## Database Optimization

### Indexes for Efficient Queries

To support efficient period queries and cursor-based pagination, create these indexes:

```sql
-- Primary index for timeslot queries with date range + cursor
CREATE INDEX idx_timeslots_doctor_date_time_status
ON timeslots(doctor_id, start_date, start_time, status)
WHERE status = 'FREE';

-- Index for reservation queries
CREATE INDEX idx_reservations_doctor_created_status
ON reservations(doctor_id, created_at, status);

-- Composite index for pagination (date + time)
CREATE INDEX idx_timeslots_doctor_datetime_cursor
ON timeslots(doctor_id, start_date, start_time, end_time, status);
```

### Query Patterns

**1. Find available timeslots with cursor pagination**
```sql
-- First page (no cursor)
SELECT * FROM timeslots
WHERE doctor_id = $1
  AND start_date >= $2  -- from_date
  AND start_date <= $3  -- to_date
  AND status = 'FREE'
  AND ($4::time IS NULL OR start_time >= $4)
  AND ($5::time IS NULL OR end_time <= $5)
ORDER BY start_date, start_time
LIMIT $6 + 1;  -- fetch one extra to determine if more pages

-- Subsequent pages (with cursor)
SELECT * FROM timeslots
WHERE doctor_id = $1
  AND start_date >= $2
  AND start_date <= $3
  AND status = 'FREE'
  AND (start_date > $7::date OR (start_date = $7::date AND start_time > $8::time))  -- cursor position
  AND ($4::time IS NULL OR start_time >= $4)
  AND ($5::time IS NULL OR end_time <= $5)
ORDER BY start_date, start_time
LIMIT $6 + 1;
```

**2. Find reservations with status filter**
```sql
SELECT * FROM reservations
WHERE doctor_id = $1
  AND created_at >= $2
  AND created_at <= $3
  AND ($4::reservation_status_enum IS NULL OR status = $4)
ORDER BY created_at DESC
LIMIT $5 + 1;
```

```rust
// In repo implementation
async fn find_available_timeslots(
    &self,
    doctor_id: i32,
    start_time: i64,
    end_time: i64,
) -> Result<Vec<Timeslot>, anyhow::Error> {
    let span = tracing::info_span!(
        "find_available_timeslots",
        doctor_id = %doctor_id,
        date_range_days = (end_time - start_time) / 86400
    );

    let _enter = span.enter();

    let query = sqlx::query_as::<_, Timeslot>(/* ... */)
        .bind(doctor_id)
        .bind(start_time)
        .bind(end_time);

    let result = query.fetch_all(&self.pool).await?;
    tracing::info!(
        "Retrieved {} timeslots for date range",
        result.len()
    );

    Ok(result)
}
```

### Query Limits

Protect against excessive queries:

```rust
// In actor implementation
const MAX_QUERY_DAYS: i64 = 365; // 1 year max
const DEFAULT_PAGE_SIZE: usize = 100;
const MAX_PAGE_SIZE: usize = 500; // Prevent excessive memory usage

async fn get_available_timeslots(
    &self,
    doctor_id: &str,
    params: ListTimeslotsParams,
) -> Result<ListTimeslotsResponse, anyhow::Error> {
    // Validate date range
    let days = (params.end_date - params.start_date).total_days();
    if days > MAX_QUERY_DAYS {
        return Err(anyhow::anyhow!(
            "Date range too large: {} days (max: {})",
            days,
            MAX_QUERY_DAYS
        ));
    }

    // Validate and clamp page_size
    let page_size = params.page_size
        .unwrap_or(DEFAULT_PAGE_SIZE)
        .min(MAX_PAGE_SIZE);

    // ... rest of implementation
}
```

### Best Practices for pageToken Pagination

1. **Use Bounded Date Ranges**: Always provide `start_date` and `end_date`
   - Recommended: Query at most 7-30 days at a time
   - For longer ranges, use pagination with pageToken

2. **Add Time Filters When Possible**: Narrow results by `start_time`/`end_time`
   - Reduces data transfer for specific time windows (morning/afternoon)

3. **Filter by Status**: Use status filter in `get_reservations`
   - Only fetch needed statuses (e.g., `Pending` for expiry worker)

4. **Index Optimization**: Ensure composite indexes match query patterns
   - Include cursor columns in index for efficient pagination
   - Order matters: most selective columns first

5. **Client-Side Pagination**: Store and pass `next_page_token`
   - Never assume page numbers (use cursor instead)
   - Handle `next_page_token == None` as end of results

6. **pageToken Validation**: Decode and validate tokens on server
   - Reject malformed or expired tokens
   - Return clear error for invalid tokens

7. **Performance Monitoring**: Track pagination metrics
   - Pages fetched per request
   - Cursor positions and query performance
   - Time to decode/encode tokens

---

## Period/Range Query Strategy

### pageToken-Based Pagination

pageToken uses **cursor-based pagination** where the token encodes the last result's position:

**Benefits:**
- Stable results even when data changes during pagination
- No duplicate or missing records
- Efficient database queries using cursor-based WHERE clauses
- Simple API: client just stores and passes the token

**Token Format (base64-encoded cursor):**
```
base64("2024-01-15|09:00:00")  -> encoded token
```

**Implementation Pattern:**
1. Fetch `page_size + 1` results
2. If exactly `page_size + 1` results, return first `page_size` with `next_page_token`
3. If less than `page_size + 1` results, return all with `next_page_token = None`
4. Client passes `next_page_token` as `page_token` for next request
5. Server decodes token to get cursor, filters results > cursor

---

## Migration Path from TimeslotService

The existing `TimeslotService` can be gradually migrated:

```rust
// Phase 3: Temporary compatibility layer
impl TimeslotService {
    pub fn from_doctor_actor(actor: Arc<dyn DoctorActor>) -> Self {
        Self {
            // Delegate to actor internally
            actor,
        }
    }
}

// Eventually remove TimeslotService when all callers updated
```

---

## Comparison: Pre-Created vs On-the-Fly Generation

### Flow Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                 GET /timeslot/v1/available                │
│                   doctor_id: "doctor123"                  │
│                   start_date: "2024-01-01"               │
│                   end_date: "2024-01-07"                 │
└────────────────────────┬────────────────────────────────────┘
                     │
                     ▼
        ┌──────────────────────────────┐
        │  DoctorActor::             │
        │  get_available_timeslots()  │
        └────────────┬───────────────┘
                     │
        ┌────────────┴────────────┐
        │                         │
        ▼                         ▼
┌───────────────────┐    ┌──────────────────────────┐
│ Schedule Config   │    │  No Schedule Config    │
│ exists?          │    │  (legacy mode)        │
└────────┬──────────┘    └────────────┬───────────┘
         │                              │
         ▼                              ▼
┌───────────────────┐         ┌──────────────────────────┐
│ On-the-Fly Mode  │         │ Pre-Created Mode      │
│                  │         │                      │
│ 1. Get schedule  │         │ 1. Query database     │
│    config         │         │    for timeslots      │
│                  │         │                      │
│ 2. Get existing  │         │ 2. Apply pagination  │
│    reservations   │         │    (cursor-based)    │
│                  │         │                      │
│ 3. Generate     │         │ 3. Return with       │
│    time slots     │         │    next_page_token    │
│                  │         │                      │
│ 4. Filter        │         │                      │
│    reserved      │         │                      │
│    slots         │         │                      │
│                  │         │                      │
│ 5. Return       │         │                      │
│    TimeRange[]   │         │                      │
│    (no IDs)      │         │                      │
└───────────────────┘         └──────────────────────────┘
```

### Key Differences

| Aspect | Pre-Created | On-the-Fly |
|--------|-------------|-------------|
| **Source** | Database table | Schedule config + reservations |
| **IDs** | Has `timeslot_id: i64` | No IDs (just time ranges) |
| **Pagination** | Yes (pageToken) | No (all generated at once) |
| **Data volume** | Paginated (configurable) | Complete range (limited by date range) |
| **Database queries** | Query timeslots table | Query schedule + reservations |
| **Performance** | O(page_size) | O(date_range_days × slots_per_day) |
| **Use case** | Legacy system, large datasets | Doctors with schedule configs |

---

## On-the-Fly Timeslot Generation Strategy

### When to Use On-the-Fly Generation

**On-the-fly generation** is used when a doctor has a **schedule configuration**:

```rust
// DoctorActorImpl::get_available_timeslots()
async fn get_available_timeslots(
    &self,
    doctor_id: &str,
    params: ListTimeslotsParams,
) -> Result<GetAvailableTimeslotsResult, anyhow::Error> {
    // Check if doctor has schedule config
    if let Some(schedule_config) = self.repo.get_schedule_config(doctor_id).await? {
        // On-the-fly generation from schedule config
        self.generate_available_timeslots(doctor_id, params.start_date, params.end_date)
            .await
            .map(GetAvailableTimeslotsResult::OnTheFly)
    } else {
        // Pre-created timeslots with pagination
        // ... query database for pre-created timeslots
    }
}
```

### Generation Process

1. **Fetch Schedule Config:** Get `DoctorScheduleConfig` from Redis/DB
2. **Fetch Reservations:** Get existing reservations for the date range
3. **Generate Time Slots:** Apply schedule rules to generate available time ranges
4. **Filter Reserved Slots:** Remove time ranges that overlap with existing reservations
5. **Return Simple Ranges:** Return `Vec<TimeRange>` (start/end only)

### Logic Moved from `get_available_timeslot.rs`

The generation logic from `module/timeslot/get_available_timeslot.rs` will be moved:

**From:** `GetAvailableTimeslotService::get_available_timeslots_internal()`
**To:** `DoctorActorImpl::generate_available_timeslots()`

**Key Changes:**
- Return type simplified from `Vec<DoctorTimeslot>` to `Vec<GeneratedTimeslot>`
- Each `GeneratedTimeslot` groups time ranges by date
- No `slot_id` (timeslots aren't pre-created)
- Uses `generate_timeslots()` from `doctor_actor/commons.rs`

### Response Type Comparison

**Pre-Created Timeslots:**
```json
{
  "preCreated": {
    "timeslots": [
      {
        "timeslotId": 12345678,
        "doctorId": 123,
        "slotDate": "2024-01-01",
        "startTime": "09:00:00",
        "endTime": "09:30:00",
        "isInstant": false,
        "status": "Free"
      }
    ],
    "nextPageToken": "base64encodedcursor"
  }
}
```

**On-the-Fly Generated:**
```json
{
  "onTheFly": {
    "generated": [
      {
        "date": "2024-01-01",
        "timeRanges": [
          {
            "startTime": "09:00:00",
            "endTime": "09:30:00"
          },
          {
            "startTime": "09:30:00",
            "endTime": "10:00:00"
          }
        ]
      }
    ]
  }
}
```

---

## HTTP Handler Example (pageToken Pagination)

```rust
// In module/timeslot/handlers.rs
use axum::extract::{Query, State};
use serde::Deserialize;

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GetAvailableTimeslotsQuery {
    pub start_date: String,  // "2024-01-15"
    pub end_date: String,    // "2024-01-21"
    pub start_time: Option<String>,  // "09:00:00"
    pub end_time: Option<String>,    // "17:00:00"
    pub page_token: Option<String>,
    pub page_size: Option<usize>,
}

#[utoipa::path(
    get,
    path = "/timeslot/v1/available",
    params(
        ("start_date" = String, Query, description = "Start date (YYYY-MM-DD)"),
        ("end_date" = String, Query, description = "End date (YYYY-MM-DD)"),
        ("start_time" = Option<String>, Query, description = "Start time filter (HH:MM:SS)"),
        ("end_time" = Option<String>, Query, description = "End time filter (HH:MM:SS)"),
        ("page_token" = Option<String>, Query, description = "Pagination token from previous response"),
        ("page_size" = Option<usize>, Query, description = "Items per page (default: 100, max: 500)"),
    ),
    responses(
        (status = 200, description = "Success", body = ListTimeslotsResponse),
        (status = 400, description = "Invalid parameters")
    ),
    tag = "timeslot"
)]
pub async fn get_available_timeslot(
    State(state): State<TimeslotState>,
    Query(query): Query<GetAvailableTimeslotsQuery>,
    _identity: DoctorIdentity,
) -> AppResult<Json<ListTimeslotsResponse>> {
    let start_date = parse_date(&query.start_date)?;
    let end_date = parse_date(&query.end_date)?;

    let params = ListTimeslotsParams {
        start_date,
        end_date,
        start_time: query.start_time.as_ref().map(|t| parse_time(t)).transpose()?,
        end_time: query.end_time.as_ref().map(|t| parse_time(t)).transpose()?,
        page_token: query.page_token,
        page_size: query.page_size,
    };

    let response = state.service
        .get_available_timeslots(&_identity.doctor_account_id, params)
        .await?;

    Ok(Json(response))
}
```

---

## Implementation Phases

### Phase 1: Doctor Actor Module Structure
- [ ] Create `doctor_actor/mod.rs` with exports
- [ ] Create `doctor_actor/models.rs` with data models:
  - Define `TimeRange`, `GeneratedTimeslot`, `TimeslotReservation`, `ReserveResult`
  - Define event types (no `timeslot_id` since timeslots aren't pre-created)
- [ ] Create `doctor_actor/repo.rs` with `DoctorTimeslotRepo` trait + impl:
  - `get_schedule_config()` - for on-the-fly generation
  - `get_doctor_reservations()` - for conflict checking
  - `create_reservation()` - create from time range (no timeslot_id)
  - `find_reservation()`, `confirm_reservation()`, `cancel_reservation()`
- [ ] Create `doctor_actor/behaviors.rs` with behavior traits + production impls:
  - `RateLimiterBehavior` + `RateLimiterBehaviorImpl`
  - `EventPublisherBehavior` + `EventPublisherBehaviorImpl`
  - `IdempotencyCacheBehavior` + `IdempotencyCacheBehaviorImpl`
  - `TimeSource` + `SystemTimeSource`
- [ ] Create `doctor_actor/actor.rs` with `DoctorActor` trait + `DoctorActorImpl`:
  - `get_available_timeslots()` - on-the-fly generation only
  - `reserve_timeslot()` - reserve by date/time range
  - `release_timeslot()` - cancel reservation
- [ ] Create `doctor_actor/commons.rs`:
  - Copy `generate_timeslots()` function from `module/timeslot/commons.rs`
- [ ] Update `server/src/lib.rs` to include `pub mod doctor_actor;`

### Phase 2: Database Migration
- [ ] Create migration for `doctor_reservations` table (no pre-created timeslots)
- [ ] Add `slot_date`, `start_time`, `end_time` columns to reservations table
- [ ] Add index for period queries:
  - `idx_reservations_doctor_date` on (doctor_id, slot_date, status)
- [ ] Test migration locally

### Phase 3: Update Timeslot Module
- [ ] Update `module/timeslot/mod.rs` to use `DoctorActor` via `DoctorActorImpl::new_production()`
- [ ] Update `module/timeslot/handlers.rs` to call actor methods
- [ ] Update handlers to accept date/time range instead of timeslot_id
- [ ] Remove `TimeslotService` (replaced by DoctorActor)
- [ ] Remove or deprecate `get_available_timeslot.rs` (logic moved to actor)

### Phase 4: Update Consultation Summarization
- [ ] Rewrite `consultation/summarization/follow_up_repo.rs` to use `DoctorActor`
- [ ] Update `module/consultation/mod.rs` to accept `DoctorActor`
- [ ] Simplify `summarization/service.rs` follow-up flow

### Phase 5: Bootstrap Wiring
- [ ] Create shared `DoctorActor` instance in `bootstrap.rs` using `new_production()`
- [ ] Pass to both `timeslot` and `consultation` modules

### Phase 6: Testing
- [ ] Unit tests for `DoctorActor` with mock behaviors
- [ ] Unit tests for on-the-fly generation
- [ ] Unit tests for conflict checking during reservation
- [ ] Integration tests for timeslot generation flow
- [ ] Integration tests for reservation flow
- [ ] Integration tests for follow-up flow
- [ ] Manual testing

---

## Estimated Effort: 5-6 days (simplified with on-the-fly only)

- Phase 1: Doctor Actor Module Structure - 1.5 days
- Phase 2: Database Migration - 0.5 days (simpler, no pre-created timeslots)
- Phase 3: Update Timeslot Module - 1.5 days
- Phase 4: Update Consultation Summarization - 1 day
- Phase 5: Bootstrap Wiring - 0.5 days
- Phase 6: Testing - 1 day

**Additional considerations:**
- Query optimization and monitoring (indexes, tracing, limits)
- Period query validation (max range enforcement)
- No pagination needed (all timeslots generated at once)
- **Database schema:** Add `slot_date`, `start_time`, `end_time` to reservations
- **On-the-fly generation:** Move logic from `get_available_timeslot.rs` to `doctor_actor/actor.rs`
- **Conflict checking:** Validate time ranges don't overlap with existing reservations
- **Schedule config integration:** Support both routine (day-of-week) and ad-hoc schedules
- **Event publishing:** Update events to include time ranges instead of timeslot_id

---

## Behavior Implementations

### RateLimiterBehaviorImpl (wraps existing RateLimiter)

```rust
// doctor_actor/behaviors.rs
use crate::module::timeslot::rate_limiter::RateLimiter;

pub struct RateLimiterBehaviorImpl {
    inner: RateLimiter,
}

impl RateLimiterBehaviorImpl {
    pub fn new(pg_pool: sqlx::PgPool, daily_limit: i32, weekly_limit: i32) -> Self {
        Self {
            inner: RateLimiter::new(pg_pool, daily_limit, weekly_limit),
        }
    }
}

#[async_trait]
impl RateLimiterBehavior for RateLimiterBehaviorImpl {
    async fn check_and_increment(&self, patient_id: i32) -> Result<Option<RateLimitType>, anyhow::Error> {
        self.inner.check_and_increment(patient_id).await
    }

    fn daily_limit(&self) -> i32 {
        self.inner.daily_limit()
    }

    fn weekly_limit(&self) -> i32 {
        self.inner.weekly_limit()
    }

    fn get_seconds_until_window_reset(&self, limit_type: RateLimitType) -> i32 {
        self.inner.get_seconds_until_window_reset(limit_type)
    }
}
```

### EventPublisherBehaviorImpl (wraps PubsubPublisher)

```rust
use crate::module::webhook::PubsubPublisher;

pub struct EventPublisherBehaviorImpl {
    inner: Arc<PubsubPublisher>,
}

impl EventPublisherBehaviorImpl {
    pub fn new(publisher: Arc<PubsubPublisher>) -> Self {
        Self { inner: publisher }
    }
}

#[async_trait]
impl EventPublisherBehavior for EventPublisherBehaviorImpl {
    async fn publish_timeslot_reserved(&self, event: TimeslotReservedEvent) -> Result<(), anyhow::Error> {
        self.inner.publish_timeslot_reserved(event).await
    }

    async fn publish_timeslot_confirmed(&self, event: TimeslotConfirmedEvent) -> Result<(), anyhow::Error> {
        self.inner.publish_timeslot_confirmed(event).await
    }

    async fn publish_timeslot_released(&self, event: TimeslotReleasedEvent) -> Result<(), anyhow::Error> {
        self.inner.publish_timeslot_released(event).await
    }
}
```

### IdempotencyCacheBehaviorImpl (wraps IdempotencyCache)

```rust
use crate::module::timeslot::idempotency::IdempotencyCache;

pub struct IdempotencyCacheBehaviorImpl {
    inner: IdempotencyCache,
}

impl IdempotencyCacheBehaviorImpl {
    pub async fn new(redis_url: &str) -> Result<Self, anyhow::Error> {
        Ok(Self {
            inner: IdempotencyCache::new(redis_url).await?,
        })
    }
}

#[async_trait]
impl IdempotencyCacheBehavior for IdempotencyCacheBehaviorImpl {
    async fn get_cached_response(&self, correlation_id: &str) -> Result<Option<CachedReserveResponse>, anyhow::Error> {
        self.inner.get_cached_response(correlation_id).await
    }

    async fn cache_response(
        &self,
        correlation_id: &str,
        response: &CachedReserveResponse,
        ttl_seconds: i32,
    ) -> Result<(), anyhow::Error> {
        self.inner.cache_response(correlation_id, response, ttl_seconds).await
    }
}
```

---

## Example: Behavior Usage in Bootstrap

```rust
// bootstrap.rs
pub async fn init_routers(
    pg_pool: sqlx::PgPool,
    cfg: &AppConfig,
    // ... other dependencies
) -> Result<Router, anyhow::Error> {
    let firestore = Arc::new(FirestoreRepo::new(...));
    let pubsub_publisher = Arc::new(PubsubPublisher::new(...));

    // Create shared DoctorActor with production behaviors
    let doctor_repo = Arc::new(DoctorTimeslotRepoImpl::new(pg_pool.clone(), redis_pool));
    let doctor_actor = Arc::new(DoctorActorImpl::new_production(
        doctor_repo,
        pg_pool.clone(),
        pubsub_publisher.clone(),
        &cfg.redis.url,
    ).await?);

    // Timeslot router uses DoctorActor
    let (timeslot_router, timeslot_worker) = module::timeslot::router(
        pg_pool.clone(),
        cfg,
        pubsub_publisher.clone(),
        doctor_actor.clone(), // NEW: shared actor
        cancel_token,
    ).await?;

    // Consultation router also uses DoctorActor
    let consultation_router = module::consultation::router(
        firestore.clone(),
        cfg,
        doctor_actor.clone(), // NEW: shared actor
    ).await?;

    Ok(app_router
        .nest("/timeslot/v1", timeslot_router)
        .nest("/consultation/v1", consultation_router))
}
```
