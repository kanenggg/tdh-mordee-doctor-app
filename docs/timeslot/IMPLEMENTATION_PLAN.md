# Doctor Actor Phase 1 — Implementation Plan

## Overview

Implement **DoctorActor** as a shared domain component that encapsulates doctor timeslot logic. The `doctor_actor` module is a **pure domain library** with no HTTP routes. The `timeslot` module owns routes/handlers and delegates to DoctorActor via a thin `TimeslotService` adapter.

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│  server/src/doctor_actor/ (Pure Domain Library)  │
│                                                     │
│  pub trait DoctorActor {                              │
│    get_available_timeslots(...)                       │
│    reserve_timeslot(...)                              │
│    release_timeslot(...)                              │
│  }                                                   │
│                                                      │
│  struct DoctorActorImpl {                              │
│    repo: Arc<dyn DoctorTimeslotRepo>                  │
│    rate_limiter: Arc<dyn RateLimiterBehavior>          │
│    event_publisher: Arc<dyn EventPublisherBehavior>      │
│    idempotency: Arc<dyn IdempotencyCacheBehavior>    │
│    time_source: Arc<dyn TimeSource>                   │
│  }                                                    │
└────────────┬──────────────────────────────────────────────┘
             │ Arc<dyn DoctorActor>
    ┌────────┴────────┐
    ▼                 ▼
┌───────────┐    ┌───────────┐
│  module/  │    │ module/   │
│  timeslot  │    │consultation│
│  (HTTP)    │    │ (Future)  │
└───────────┘    └───────────┘
```

**Design Principles:**
- **DoctorActor** = Generic domain logic + performance-focused
- **TimeslotService** = Simple API adapter (HTTP ↔ Actor)
- **Behavior traits** = Injectable dependencies for testability
- **On-the-fly generation** = No pre-created timeslots, only schedule config + reservations

---

## API Surface — Phase 1

### GET /timeslot/v1/available/me

**Purpose:** Doctor retrieves their own available time slots

**Access Control:**
- Requires canonical `account_type == 2` (doctor)
- Returns `403 Forbidden` for non-doctors
- `doctor_id` from `DoctorIdentity` extractor (header)

**Request:**
```
GET /timeslot/v1/available/me?startDate=2026-04-15&endDate=2026-04-21
tdh-sec-iam-user-identity: { "accountId": 123, "accountType": 2, ... }
```

**Response 200 — Success:**
```json
{
  "__type": "AvailableTimeslots",
  "timeslots": [
    {
      "date": "2026-04-15",
      "timeRanges": [
        { "startTime": "09:00:00", "endTime": "09:30:00" },
        { "startTime": "09:30:00", "endTime": "10:00:00" }
      ]
    },
    {
      "date": "2026-04-16",
      "timeRanges": [
        { "startTime": "14:00:00", "endTime": "14:30:00" }
      ]
    }
  ]
}
```

**Response 200 — Error:**
```json
{ "__type": "NoScheduleConfig" }
```

**Response 400:**
```json
{ "error": "start_time must be in future" }
{ "error": "Date range must not exceed 30 days" }
```

**Response 403:**
```json
{ "error": "Forbidden" }
```

**Response 401:**
```json
{ "error": "Unauthorized" }
```

---

### POST /timeslot/v1/reserve

**Purpose:** Patient reserves a time slot

**Access Control:**
- Requires authenticated user
- `patient_id` from `UserIdentity.account_id` or `UserIdentity.user_profile_id`
- `doctor_id` from request body

**Request:**
```json
{
  "doctorId": 123,
  "slotDate": "2026-04-15",
  "startTime": "09:00:00",
  "endTime": "09:30:00",
  "reservationTtlSeconds": 300,
  "correlationId": "optional-uuid-for-idempotency"
}
```

**Response 200 — Success:**
```json
{
  "__type": "Success",
  "reservationId": "550e8400-e29b-41d4-a716-446655440000",
  "expiresAt": 1713172800
}
```

**Response 200 — Already Reserved:**
```json
{ "__type": "AlreadyReserved" }
```

**Response 200 — No Schedule Config:**
```json
{ "__type": "NoScheduleConfig" }
```

**Response 200 — Rate Limit Exceeded:**
```json
{
  "__type": "RateLimitExceeded",
  "limitType": "DAILY",
  "currentCount": 11,
  "maxAllowed": 10,
  "retryAfterSeconds": 3600
}
```

---

### POST /timeslot/v1/confirm

**Purpose:** Confirm reservation after payment

**Request:**
```json
{
  "reservationId": "550e8400-e29b-41d4-a716-446655440000",
  "bookingId": "BK-12345",
  "paymentReference": "PAY-67890"
}
```

**Response 200:**
```json
{ "__type": "Success" }
```
or
```json
{ "__type": "NotFound" }
```

---

### POST /timeslot/v1/cancel

**Purpose:** Cancel reservation

**Request (by reservation ID):**
```json
{
  "reservationId": "550e8400-e29b-41d4-a716-446655440000"
}
```

**Request (by booking ID):**
```json
{
  "bookingId": "BK-12345"
}
```

**Response 200:**
```json
{ "__type": "Success" }
```
or
```json
{ "__type": "NotFound" }
```

**Response 400:**
```json
{ "error": "Must provide either reservation_id or booking_id" }
```

---

## Phase 1 Task Breakdown

### Phase 1A: Core Error & Identity

**[T1A-1]** Add `Forbidden` to `AppError` enum
- File: `server/src/core/error.rs`
- Add variant: `Forbidden` with message string
- Map to `StatusCode::FORBIDDEN` in `IntoResponse` impl
- Estimated: 15 min

**[T1A-2]** Update `DoctorIdentity` extractor to validate account_type
- File: `server/src/core/auth.rs`
- In `DoctorIdentity::from_request_parts`, after deserializing `UserIdentity`:
  - Check `u.account_type == 2` (doctor)
  - If not, return `Err(AppError::Forbidden)`
- Estimated: 30 min

---

### Phase 1B: Doctor Actor Models

**[T1B-1]** Refactor `doctor_actor/models.rs` — align with DOCTOR_ACTOR_PLAN.md
- File: `server/src/doctor_actor/models.rs`
- **Add:**
  - `GeneratedTimeslot`: `{ date: Date, time_ranges: Vec<TimeRange> }`
  - `TimeslotReservation`: `{ id: i64, doctor_id: i32, patient_id: i32, slot_date: Date, start_time: Time, end_time: Time, status: ReservationStatus, correlation_id: String, booking_id: Option<String>, expires_at: i64, created_at: i64, confirmed_at: Option<i64>, cancelled_at: Option<i64> }`
  - `ReserveResult` (new enum): `Success { reservation_id: i64, expires_at: i64 } | Conflict | NoScheduleConfig | RateLimitExceeded { limit_type: RateLimitType, current_count: i32, max_allowed: i32, retry_after_seconds: i32 }`
  - Event types (NO `timeslot_id`):
    - `TimeslotReservedEvent`: `{ reservation_id: i64, doctor_id: i32, patient_id: i32, slot_date: Date, start_time: Time, end_time: Time, expires_at: i64, reserved_at: i64 }`
    - `TimeslotConfirmedEvent`: `{ reservation_id: i64, booking_id: String, doctor_id: i32, patient_id: i32, confirmed_at: i64 }`
    - `TimeslotReleasedEvent`: `{ reservation_id: i64, doctor_id: i32, patient_id: i32, slot_date: Date, start_time: Time, end_time: Time, released_at: i64, reason: ReleaseReason }`
- **Remove:**
  - `DoctorTimeslot` (replaced by `GeneratedTimeslot`)
  - Old `ReserveResult` (replaced by new enum)
- **Keep:**
  - `ReservationSource`: `Booking | FollowUp`
  - `TimeRange`, `RoutineSchedule`, `AdHocSchedule`, `DoctorScheduleConfig`
  - `DoctorReservation` (but check if this overlaps with `TimeslotReservation`)
- Estimated: 2 hours

**[T1B-2]** Resolve model duplication
- `DoctorScheduleConfig`, `RoutineSchedule`, `AdHocSchedule`, `TimeRange`, `DoctorReservation` are duplicated between:
  - `doctor_actor/models.rs`
  - `module/timeslot/repo.rs`
- **Decision:** Keep in `doctor_actor/models.rs`, have `module/timeslot/repo.rs` import from `crate::doctor_actor::models`
- Estimated: 30 min

---

### Phase 1C: Doctor Actor Behaviors

**[T1C-1]** Create `doctor_actor/behaviors.rs` — replace old `behaviour.rs`
- File: `server/src/doctor_actor/behaviors.rs` (delete `behaviour.rs`)
- **Define traits:**
  ```rust
  #[async_trait]
  pub trait RateLimiterBehavior: Send + Sync {
      async fn check_and_increment(&self, patient_id: i32) -> Result<Option<RateLimitType>, anyhow::Error>;
      fn daily_limit(&self) -> i32;
      fn weekly_limit(&self) -> i32;
      fn get_seconds_until_window_reset(&self, limit_type: RateLimitType) -> i32;
  }

  #[async_trait]
  pub trait EventPublisherBehavior: Send + Sync {
      async fn publish_timeslot_reserved(&self, event: TimeslotReservedEvent) -> Result<(), anyhow::Error>;
      async fn publish_timeslot_confirmed(&self, event: TimeslotConfirmedEvent) -> Result<(), anyhow::Error>;
      async fn publish_timeslot_released(&self, event: TimeslotReleasedEvent) -> Result<(), anyhow::Error>;
  }

  #[async_trait]
  pub trait IdempotencyCacheBehavior: Send + Sync {
      async fn get_cached_response(&self, correlation_id: &str) -> Result<Option<CachedReserveResponse>, anyhow::Error>;
      async fn cache_response(&self, correlation_id: &str, response: &CachedReserveResponse, ttl_seconds: i32) -> Result<(), anyhow::Error>;
  }

  pub trait TimeSource: Send + Sync {
      fn now_epoch_secs(&self) -> i64;
  }
  ```
- **Implement production wrappers:**
  - `RateLimiterBehaviorImpl`: wraps `crate::module::timeslot::rate_limiter::RateLimiter`
  - `EventPublisherBehaviorImpl`: wraps `crate::module::webhook::PubsubPublisher`
  - `IdempotencyCacheBehaviorImpl`: wraps `crate::module::timeslot::idempotency::IdempotencyCache`
  - `SystemTimeSource`: `impl TimeSource` using `std::time::SystemTime`
- Estimated: 3 hours

---

### Phase 1D: Doctor Actor Repository

**[T1D-1]** Refactor `doctor_actor/repo.rs` — add missing trait methods
- File: `server/src/doctor_actor/repo.rs`
- **Update `DoctorTimeslotRepo` trait:**
  ```rust
  async fn find_reservation(&self, reservation_id: i64) -> Result<Option<TimeslotReservation>, anyhow::Error>;
  async fn confirm_reservation(&self, reservation_id: i64, booking_id: &str, payment_reference: &str, confirmed_at: i64) -> Result<(), anyhow::Error>;
  async fn cancel_reservation(&self, reservation_id: i64, cancelled_at: i64) -> Result<(), anyhow::Error>;
  async fn find_reservations_by_status(&self, doctor_id: &str, start_date: Date, end_date: Date, status: Option<ReservationStatus>) -> Result<Vec<TimeslotReservation>, anyhow::Error>;
  ```
- **Implement in `DoctorTimeslotRepoImpl`:**
  - `find_reservation`: `SELECT * FROM doctor_reservations WHERE id = $1`
  - `confirm_reservation`: `UPDATE doctor_reservations SET status = 'Confirmed', booking_id = $2, payment_reference = $3, confirmed_at = to_timestamp($4) WHERE id = $1 AND status = 'Pending'`
  - `cancel_reservation`: `UPDATE doctor_reservations SET status = 'Cancelled', cancelled_at = to_timestamp($2) WHERE id = $1`
  - `find_reservations_by_status`: `SELECT * FROM doctor_reservations WHERE doctor_id = $1 AND slot_date >= $2 AND slot_date <= $3 AND ($4::reservation_status_enum IS NULL OR status = $4)`
- **Update existing methods to use new models:**
  - `get_doctor_reservations`: Keep returning `Vec<DoctorReservation>` for `commons::generate_timeslots` compatibility
  - `get_schedule_config`: Keep returning `Option<DoctorScheduleConfig>`
  - `insert_reservation`: Update to use `TimeslotReservation` model, return `i64` (reservation_id)
- Estimated: 3 hours

---

### Phase 1E: Doctor Actor Core

**[T1E-1]** Refactor `doctor_actor/actor.rs` — inject behavior traits
- File: `server/src/doctor_actor/actor.rs`
- **Update `DoctorActor` trait:**
  ```rust
  async fn get_available_timeslots(
      &self,
      doctor_id: &str,
      start_date: Date,
      end_date: Date,
  ) -> Result<Vec<GeneratedTimeslot>, anyhow::Error>;
  ```
- **Update `DoctorActorImpl` struct:**
  ```rust
  pub struct DoctorActorImpl {
      repo: Arc<dyn DoctorTimeslotRepo>,
      rate_limiter: Arc<dyn RateLimiterBehavior>,
      event_publisher: Arc<dyn EventPublisherBehavior>,
      idempotency: Arc<dyn IdempotencyCacheBehavior>,
      time_source: Arc<dyn TimeSource>,
  }
  ```
- **Update `DoctorActorImpl::new()`:** Accept 5 behavior trait Arcs
- **Add `DoctorActorImpl::new_production()`:**
  ```rust
  pub fn new_production(
      repo: Arc<dyn DoctorTimeslotRepo>,
      pg_pool: PgPool,
      pubsub_publisher: Arc<crate::module::webhook::PubsubPublisher>,
      redis_url: &str,
  ) -> Result<Self, anyhow::Error> {
      // Create concrete implementations
      let rate_limiter = Arc::new(RateLimiterBehaviorImpl::new(
          pg_pool.clone(),
          10, // daily_limit
          50, // weekly_limit
      ));
      let event_publisher = Arc::new(EventPublisherBehaviorImpl::new(pubsub_publisher));
      let idempotency = Arc::new(IdempotencyCacheBehaviorImpl::new(redis_url).await?);
      let time_source = Arc::new(SystemTimeSource);

      Ok(Self::new(repo, rate_limiter, event_publisher, idempotency, time_source))
  }
  ```
- **Update `get_available_timeslots()`:**
  - Return `Vec<GeneratedTimeslot>` (grouped by date)
  - Generate timeslots, then group by date into `GeneratedTimeslot`
- **Update `reserve_timeslot()`:**
  - Check rate limit via `self.rate_limiter.check_and_increment()`
  - Check idempotency via `self.idempotency.get_cached_response()`
  - Validate schedule config exists
  - Check for conflicts with existing reservations
  - Create reservation via `self.repo.insert_reservation()`
  - Publish event via `self.event_publisher.publish_timeslot_reserved()`
  - Cache result via `self.idempotency.cache_response()`
  - Return appropriate `ReserveResult` variant
  - **NO Redis expiry scheduling** — that's adapter responsibility
- **Update `release_timeslot()`:**
  - Fetch reservation via `self.repo.find_reservation()`
  - Cancel via `self.repo.cancel_reservation()`
  - Publish event via `self.event_publisher.publish_timeslot_released()`
- **Remove:** Direct Redis dependency, direct PubSub dependency
- Estimated: 4 hours

---

### Phase 1F: Module Cleanup

**[T1F-1]** Update `doctor_actor/mod.rs`
- File: `server/src/doctor_actor/mod.rs`
- Delete `behaviour.rs` (replaced by `behaviors.rs`)
- Add `behaviors` module
- Update exports:
  ```rust
  pub use models::{GeneratedTimeslot, TimeRange, TimeslotReservation, ...};
  pub use repo::DoctorTimeslotRepo;
  pub use actor::{DoctorActor, DoctorActorImpl};
  ```
- Estimated: 15 min

---

### Phase 1G: Database Migration

**[T1G-1]** Fix migration 20260401000000_add_doctor_reservations_table.sql
- File: `db/postgres/migrations/20260401000000_add_doctor_reservations_table.sql`
- **Bug:** Enum type created AFTER table uses it
- **Fix:** Move `CREATE TYPE reservation_status_enum` block BEFORE `CREATE TABLE doctor_reservations`
- **Add index:** `idx_doc_res_doctor_date_status` on `(doctor_id, slot_date, status)` for period queries
- **Verify:** `created_at` column exists as `TIMESTAMPTZ`
- Estimated: 30 min

---

### Phase 2: Timeslot Module Refactor

**[T2-1]** Update `timeslot/handlers.rs` — new API surface
- File: `server/src/module/timeslot/handlers.rs`
- **New handler:** `get_my_available_timeslots`
  - Extractor: `DoctorIdentity` (validates canonical account_type == 2)
  - Query params: `startDate`, `endDate` (YYYY-MM-DD)
  - Call `TimeslotService.get_my_available_timeslots(doctor_id, start_date, end_date)`
  - Response: `GetMyAvailableTimeslotsResponse { __type: "AvailableTimeslots", timeslots: Vec<GeneratedTimeslot> }`
- **Update handler:** `reserve_timeslot`
  - Request body: `{ doctorId, slotDate, startTime, endTime, reservationTtlSeconds, correlationId }`
  - Parse date/time strings to `jiff::civil::Date` and `jiff::civil::Time`
  - Call `TimeslotService.reserve_timeslot(doctor_id, patient_id, date, start_time, end_time, ttl, correlation_id)`
  - Response: `ReserveTimeslotResponse` (maps to `ReserveResult`)
- **Keep:** `confirm_booking`, `cancel_booking` (mostly unchanged)
- **Add validation:** TTL clamping via config
- **Schedule expiry:** For successful reserve, schedule in Redis expiry queue (adapter responsibility)
- Estimated: 3 hours

**[T2-2]** Update `timeslot/mod.rs` router
- File: `server/src/module/timeslot/mod.rs`
- Add route: `.route("/available/me", get(handlers::get_my_available_timeslots))`
- Update `TimeslotState` to include `Arc<dyn DoctorActor>` (for Phase 2B)
- Estimated: 30 min

**[T2-3]** Update `timeslot/service.rs` — adapter for DoctorActor
- File: `server/src/module/timeslot/service.rs`
- **Add method:** `get_my_available_timeslots(doctor_id, start_date, end_date)` → `Vec<GeneratedTimeslot>`
  - Delegate to `actor.get_available_timeslots()`
  - Map to response type
- **Update method:** `reserve_timeslot(doctor_id, patient_id, slot_date, start_time, end_time, ttl, correlation_id)`
  - Delegate to `actor.reserve_timeslot()`
  - Map `ReserveResult` to `CachedReserveResponse`
  - Keep idempotency cache handling (actor also checks, but adapter adds cache layer)
- **Add field:** `TimeslotService.actor: Arc<dyn DoctorActor>`
- **Update constructor:** Accept `Arc<dyn DoctorActor>`
- **Deprecate:** `get_available_timeslots()` with epoch timestamps (old API)
- Estimated: 2 hours

**[T2-4]** Delete `timeslot/get_available_timeslot.rs`
- File: `server/src/module/timeslot/get_available_timeslot.rs`
- Logic absorbed into `DoctorActorImpl::get_available_timeslots()`
- Tests moved to `doctor_actor/tests.rs` (new file)
- Estimated: 15 min

---

### Phase 3: Bootstrap Wiring

**[T3-1]** Wire DoctorActor in bootstrap.rs
- File: `server/src/bootstrap.rs`
- In `init_routers()`:
  ```rust
  // Create shared DoctorActor
  let doctor_actor_repo = Arc::new(
      doctor_actor::repo::DoctorTimeslotRepoImpl::new(
          deps.pg_pool.clone(),
          deps.redis_pool.clone(),
      )
  );
  let doctor_actor: Arc<dyn doctor_actor::DoctorActor> = Arc::new(
      doctor_actor::actor::DoctorActorImpl::new_production(
          doctor_actor_repo,
          deps.pg_pool.clone(),
          deps.pubsub_publisher.clone(),
          &cfg.redis.url,
      ).await?
  );

  // Pass to timeslot router (update signature)
  let (timeslot_router, timeslot_worker_handle) = module::timeslot::router(
      deps.pg_pool.clone(),
      cfg,
      deps.pubsub_publisher.clone(),
      doctor_actor.clone(),  // NEW
      cancel_token.clone(),
  ).await?;

  // Pass to consultation router (update signature)
  let consultation_router = module::consultation::router(
      cfg.service.biz_apm_base_uri.clone(),
      deps.pg_pool.clone(),
      &cfg.paseto.summarization_key,
      cfg.service.biz_jade_service_base_uri.clone(),
      cfg.service.biz_apm_base_uri.clone(),
      deps.pubsub_publisher.clone(),
      cfg.pubsub.topics.consultations.clone(),
      doctor_actor.clone(),  // NEW (instead of timeslot_repo)
  )?;
  ```
- Remove: `timeslot_repo` creation for consultation (replaced by `doctor_actor`)
- Estimated: 1 hour

---

### Phase 4: Testing

**[T4-1]** Unit tests for DoctorActor
- File: `server/src/doctor_actor/tests.rs` (new)
- Test `get_available_timeslots()`:
  - Mock repo with schedule config
  - Mock all behavior traits
  - Assert grouped by date, time ranges correct
- Test `reserve_timeslot()`:
  - Success: Creates reservation, publishes event, caches result
  - Conflict: Returns `Conflict` variant
  - Rate limit: Returns `RateLimitExceeded` variant
  - Idempotency: Returns cached response on retry
- Test `release_timeslot()`:
  - Finds reservation, cancels, publishes event
- Estimated: 4 hours

**[T4-2]** Integration tests for full flow
- File: `server/tests/timeslot_integration_test.rs` (new)
- Set up test server with `axum-test`
- Use real `DoctorActorImpl` with mock behaviors
- Test flow:
  1. `GET /available/me` → returns timeslots
  2. `POST /reserve` → success
  3. `POST /confirm` → success
  4. `POST /cancel` → success
- Test error paths:
  - `GET /available/me` with non-doctor → 403
  - `POST /reserve` with conflict → `AlreadyReserved`
- Estimated: 3 hours

---

### Phase 5: OpenAPI Registration

**[T5-1]** Update OpenAPI paths
- File: `server/src/openapi.rs`
- Add new paths under `paths(...)` mod:
  - `/timeslot/v1/available/me` — GET
  - `/timeslot/v1/reserve` — POST (updated body)
- Remove old `/timeslot/v1/available` path (replaced by `/available/me`)
- Ensure all new request/response types have `#[derive(ToSchema)]`
- Estimated: 1 hour

---

## Task Dependencies

```
[1A-1] ──┐
[1A-2] ──┤─► [1B-1] ──► [1B-2] ──┐
           │                    │                │
           └────────────────────┘                └──► [1C-1] ──┐
                                                           │
[1G-1] ─────────────────────────────────────────────────┤
                                                           │
                                                           └──► [1D-1] ──► [1E-1] ──► [1F-1]
                                                                                    │
[2-1] ─────────────────────────────────────────────────────────┘
[2-2] ◄── [2-1]
[2-3] ◄── [2-1]
[2-4] ◄── [1E-1] (logic moved)

[3-1] ◄── [2-2] (needs actor)
       ◄── [1E-1] (needs actor)

[4-1] ◄── [1E-1] (needs DoctorActor trait)
[4-2] ◄── [3-1] (needs wired routes)

[5-1] ◄── [2-1] (needs request/response types)
```

---

## Estimated Timeline

| Phase | Tasks | Effort | Parallelizable? |
|--------|--------|----------------|
| 1A (Core) | T1A-1, T1A-2 | 45 min | No (depends on error.rs) |
| 1B (Models) | T1B-1, T1B-2 | 2.5 hours | No (T1B-2 after T1B-1) |
| 1C (Behaviors) | T1C-1 | 3 hours | Yes (after T1B-1) |
| 1D (Repo) | T1D-1 | 3 hours | Yes (after T1B-1) |
| 1E (Actor) | T1E-1 | 4 hours | No (depends on T1C-1, T1D-1) |
| 1F (Cleanup) | T1F-1 | 15 min | No (after T1E-1) |
| 1G (DB) | T1G-1 | 30 min | Yes (independent) |
| 2 (Timeslot) | T2-1, T2-2, T2-3, T2-4 | 6 hours | No (T2-2 after T2-1, T2-3 after T1E-1) |
| 3 (Bootstrap) | T3-1 | 1 hour | No (after T2-2, T1E-1) |
| 4 (Testing) | T4-1, T4-2 | 7 hours | Yes (after T1E-1, T3-1) |
| 5 (OpenAPI) | T5-1 | 1 hour | No (after T2-1) |

**Total Effort:** ~30 hours (~4 days for 1 dev, ~2 days for 2 devs in parallel)

**Critical Path:** 1A → 1B → (1C || 1D) → 1E → 2 → 3 → 4 → 5

---

## Risk Mitigation

| Risk | Mitigation |
|-------|------------|
| Model duplication causes compilation errors | Keep models in `doctor_actor/models.rs` only, import from timeslot module |
| Doctor timeslot generation regression | Port existing `commons::generate_timeslots()` with existing unit tests |
| Rate limiter logic not covered | Add comprehensive unit tests with mock rate limiter |
| Redis expiry scheduling missed | Add integration test verifying expiry queue receives entries |
| Backward compatibility breaking | Old `/available` endpoint removed, `/available/me` replaces it — update client side-by-side |

---

## Success Criteria

- [ ] `GET /timeslot/v1/available/me` returns doctor's own timeslots, rejects non-doctors with 403
- [ ] `POST /timeslot/v1/reserve` accepts date/time range, enforces rate limit, creates reservation
- [ ] `POST /timeslot/v1/confirm` converts Pending → Confirmed
- [ ] `POST /timeslot/v1/cancel` cancels by reservation or booking ID
- [ ] All DoctorActor methods have unit tests with mocked behaviors
- [ ] Integration tests cover full happy path and error cases
- [ ] Bootstrap creates shared DoctorActor instance, passes to timeslot and consultation
- [ ] OpenAPI spec includes all new endpoints
- [ ] Database migration runs successfully on fresh DB
- [ ] No compilation errors, `cargo clippy` passes
