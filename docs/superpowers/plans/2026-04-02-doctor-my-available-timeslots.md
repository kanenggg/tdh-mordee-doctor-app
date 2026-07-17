# Doctor My-Available-Timeslots Endpoint Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `GET /timeslot/v1/my-available` endpoint that lets a doctor fetch their own available timeslots for a specific date, generating full-day slots (00:00–24:00 Bangkok time) based on `slot_duration` from schedule config, minus reserved slots from `doctor_reservations`.

**Architecture:** New handler in the existing `timeslot` module. Uses `DoctorIdentity` extractor for auth. Reads `slot_duration` from `DoctorScheduleConfig` via `DoctorTimeslotRepo`. Queries `doctor_reservations` for the given date. Generates full-day slots and excludes reserved ones. No new tables or repos needed.

**Tech Stack:** Rust, Axum, jiff (dates/times, Bangkok timezone), sqlx (Postgres), deadpool-redis, utoipa (OpenAPI), tdh-protocol (`DoctorTimeslot`)

---

## File Map

| File | Action | Responsibility |
|------|--------|----------------|
| `server/src/module/timeslot/handlers.rs` | Modify | Add `MyAvailableQuery`, `MyAvailableResponse`, `get_my_available_timeslots` handler |
| `server/src/module/timeslot/mod.rs` | Modify | Add route `.route("/my-available", get(handlers::get_my_available_timeslots))`, pass `DoctorTimeslotRepo` into state |
| `server/src/module/timeslot/commons.rs` | Modify | Add `generate_full_day_timeslots()` function |
| `server/src/doctor_actor/repo.rs` | Modify | Add `find_reservations_by_date()` method to `DoctorTimeslotRepo` trait + impl |
| `server/src/bootstrap.rs` | Modify | Wire `DoctorTimeslotRepo` into timeslot module |
| `server/src/openapi.rs` | Modify | Register new handler path and schemas |
| `server/tests/my_available_timeslot_test.rs` | Create | Integration tests |

---

### Task 1: Add `find_reservations_by_date` to `DoctorTimeslotRepo`

**Files:**
- Modify: `server/src/doctor_actor/repo.rs`

We need a method that queries `doctor_reservations` by doctor_id + date and returns `(Time, Time)` pairs for reserved slots. The existing `get_doctor_reservations` uses a SQL function that returns epoch timestamps — we need date+time-based query since we work in Bangkok timezone.

- [ ] **Step 1: Add trait method**

In `server/src/doctor_actor/repo.rs`, add to the `DoctorTimeslotRepo` trait (after `get_reservation_by_correlation`):

```rust
async fn find_reservations_by_date(
    &self,
    doctor_id: &str,
    date: jiff::civil::Date,
) -> Result<Vec<(jiff::civil::Time, jiff::civil::Time)>, anyhow::Error>;
```

- [ ] **Step 2: Implement for `DoctorTimeslotRepoImpl`**

Add the implementation inside the `impl DoctorTimeslotRepo for DoctorTimeslotRepoImpl` block:

```rust
async fn find_reservations_by_date(
    &self,
    doctor_id: &str,
    date: jiff::civil::Date,
) -> Result<Vec<(jiff::civil::Time, jiff::civil::Time)>, anyhow::Error> {
    let doctor_uuid: Uuid = doctor_id.parse()?;
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
    .bind(doctor_uuid)
    .bind(&date_str)
    .fetch_all(&self.pool)
    .await?;

    let mut result = Vec::with_capacity(rows.len());
    for (start_str, end_str) in rows {
        let start = jiff::civil::Time::strptime(&start_str, "%H:%M:%S")?;
        let end = jiff::civil::Time::strptime(&end_str, "%H:%M:%S")?;
        result.push((start, end));
    }
    Ok(result)
}
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check 2>&1 | head -20`
Expected: No errors related to `find_reservations_by_date`

- [ ] **Step 4: Commit**

```bash
git add server/src/doctor_actor/repo.rs
git commit -m "feat: add find_reservations_by_date to DoctorTimeslotRepo"
```

---

### Task 2: Add `generate_full_day_timeslots` to commons

**Files:**
- Modify: `server/src/module/timeslot/commons.rs`

This function generates all slots for a full day (00:00–24:00) with a given duration, excluding slots that overlap with reserved time ranges.

- [ ] **Step 1: Write tests for `generate_full_day_timeslots`**

Add at the bottom of the `#[cfg(test)] mod tests` block in `server/src/module/timeslot/commons.rs`:

```rust
#[test]
fn test_full_day_no_reservations_30min() {
    let date = Date::new(2024, 6, 15).unwrap();
    let result = super::generate_full_day_timeslots(date, 30, &[]);
    // 24 hours * 2 slots/hour = 48 slots
    assert_eq!(result.len(), 48);
    assert_eq!(result[0].start_time, Time::new(0, 0, 0, 0).unwrap());
    assert_eq!(result[0].end_time, Time::new(0, 30, 0, 0).unwrap());
    assert_eq!(result[47].start_time, Time::new(23, 30, 0, 0).unwrap());
    assert_eq!(result[47].end_time, Time::new(0, 0, 0, 0).unwrap());
}

#[test]
fn test_full_day_with_reservation_excludes_overlapping() {
    let date = Date::new(2024, 6, 15).unwrap();
    let reserved = vec![
        (Time::new(9, 0, 0, 0).unwrap(), Time::new(9, 30, 0, 0).unwrap()),
        (Time::new(14, 0, 0, 0).unwrap(), Time::new(14, 30, 0, 0).unwrap()),
    ];
    let result = super::generate_full_day_timeslots(date, 30, &reserved);
    // 48 - 2 = 46
    assert_eq!(result.len(), 46);
    // Verify the reserved slots are not present
    assert!(!result.iter().any(|s| s.start_time == Time::new(9, 0, 0, 0).unwrap()));
    assert!(!result.iter().any(|s| s.start_time == Time::new(14, 0, 0, 0).unwrap()));
}

#[test]
fn test_full_day_mid_slot_reservation_removes_both() {
    let date = Date::new(2024, 6, 15).unwrap();
    // Reservation from 09:15-09:45 overlaps both 09:00-09:30 and 09:30-10:00
    let reserved = vec![
        (Time::new(9, 15, 0, 0).unwrap(), Time::new(9, 45, 0, 0).unwrap()),
    ];
    let result = super::generate_full_day_timeslots(date, 30, &reserved);
    assert_eq!(result.len(), 46); // 48 - 2
    assert!(!result.iter().any(|s| s.start_time == Time::new(9, 0, 0, 0).unwrap()));
    assert!(!result.iter().any(|s| s.start_time == Time::new(9, 30, 0, 0).unwrap()));
}

#[test]
fn test_full_day_60min_slots() {
    let date = Date::new(2024, 6, 15).unwrap();
    let result = super::generate_full_day_timeslots(date, 60, &[]);
    assert_eq!(result.len(), 24);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test generate_full_day 2>&1 | tail -10`
Expected: Compilation error — `generate_full_day_timeslots` not found

- [ ] **Step 3: Implement `generate_full_day_timeslots`**

Add this function in `server/src/module/timeslot/commons.rs` above the `#[cfg(test)]` block:

```rust
/// Generate timeslots for a full day (00:00–24:00) with the given slot duration in minutes,
/// excluding slots that overlap with any reserved `(start_time, end_time)` pair.
///
/// All times are treated in Bangkok timezone (UTC+7). The `reserved` pairs are
/// `(start_time, end_time)` as `jiff::civil::Time` values from the `doctor_reservations` table.
pub fn generate_full_day_timeslots(
    date: Date,
    slot_duration_minutes: i32,
    reserved: &[(jiff::civil::Time, jiff::civil::Time)],
) -> Vec<DoctorTimeslot> {
    let total_minutes = 24 * 60;
    let slot_count = total_minutes / slot_duration_minutes;

    // Pre-convert reserved times to minute offsets for fast overlap checks
    let res_mins: Vec<(i32, i32)> = reserved
        .iter()
        .map(|(s, e)| {
            let s_min = s.hour() as i32 * 60 + s.minute() as i32;
            let e_min = e.hour() as i32 * 60 + e.minute() as i32;
            // Handle midnight crossing: if end <= start, treat end as next day
            let e_min = if e_min <= s_min { e_min + 24 * 60 } else { e_min };
            (s_min, e_min)
        })
        .collect();

    let mut timeslots = Vec::with_capacity(slot_count as usize);
    let mut slot_id = 1i64;

    for i in 0..slot_count {
        let s = i * slot_duration_minutes;
        let e = s + slot_duration_minutes;

        let overlaps = res_mins
            .iter()
            .any(|&(rs, re)| s < re && e > rs);

        if !overlaps {
            let start_time = jiff::civil::Time::new(
                (s / 60) as i8,
                (s % 60) as i8,
                0,
                0,
            ).unwrap();
            let end_time = jiff::civil::Time::new(
                ((e / 60) % 24) as i8,
                (e % 60) as i8,
                0,
                0,
            ).unwrap();

            timeslots.push(DoctorTimeslot {
                slot_id,
                slot_date: date,
                start_time,
                end_time,
            });
            slot_id += 1;
        }
    }

    timeslots
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test generate_full_day 2>&1 | tail -15`
Expected: All 4 tests pass

- [ ] **Step 5: Commit**

```bash
git add server/src/module/timeslot/commons.rs
git commit -m "feat: add generate_full_day_timeslots for whole-day slot generation"
```

---

### Task 3: Add the handler and wire the route

**Files:**
- Modify: `server/src/module/timeslot/handlers.rs`
- Modify: `server/src/module/timeslot/mod.rs`
- Modify: `server/src/bootstrap.rs`

- [ ] **Step 1: Add `DoctorTimeslotRepo` to `TimeslotState`**

In `server/src/module/timeslot/handlers.rs`, add the import:

```rust
use crate::doctor_actor::repo::DoctorTimeslotRepo;
```

Then add the field to `TimeslotState`:

```rust
#[derive(Clone)]
pub struct TimeslotState {
    pub service: Arc<TimeslotService>,
    pub idempotency_cache: Arc<Mutex<IdempotencyCache>>,
    pub redis: redis::aio::ConnectionManager,
    pub config: TimeslotConfig,
    pub doctor_timeslot_repo: Arc<dyn DoctorTimeslotRepo>,
}
```

- [ ] **Step 2: Add query, response types and handler**

In `server/src/module/timeslot/handlers.rs`, add these imports at the top (merge with existing):

```rust
use jiff::civil::Date;
use tdh_protocol::timeslot::timeslot::DoctorTimeslot;
use crate::core::auth::DoctorIdentity;
use crate::module::timeslot::commons::generate_full_day_timeslots;
```

Then add the query struct, response enum, and handler function after the existing `cancel_booking` handler:

```rust
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MyAvailableQuery {
    /// Date in YYYY-MM-DD format (Bangkok timezone)
    pub date: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "__type", rename_all = "PascalCase")]
pub enum MyAvailableResponse {
    Success {
        timeslots: Vec<DoctorTimeslot>,
    },
    NoScheduleConfig,
}

#[utoipa::path(
    get,
    path = "/my-available",
    tag = "timeslot",
    params(MyAvailableQuery),
    responses(
        (status = 200, description = "Doctor's available timeslots for the date", body = MyAvailableResponse),
        (status = 400, description = "Invalid date format"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden - not a doctor account"),
    )
)]
pub async fn get_my_available_timeslots(
    State(state): State<TimeslotState>,
    identity: DoctorIdentity,
    Query(query): Query<MyAvailableQuery>,
) -> AppResult<Json<MyAvailableResponse>> {
    let date: Date = query
        .date
        .parse()
        .map_err(|_| AppError::BadRequest("Invalid date format, expected YYYY-MM-DD".to_string()))?;

    let doctor_id = identity.doctor_account_id.to_string();

    // Get slot_duration from schedule config
    let schedule_config = state
        .doctor_timeslot_repo
        .get_schedule_config(&doctor_id)
        .await
        .map_err(|e| AppError::InternalError(format!("Failed to get schedule config: {}", e)))?;

    let slot_duration = match schedule_config {
        Some(config) => config.slot_duration,
        None => return Ok(Json(MyAvailableResponse::NoScheduleConfig)),
    };

    // Get reserved slots for this date
    let reserved = state
        .doctor_timeslot_repo
        .find_reservations_by_date(&doctor_id, date)
        .await
        .map_err(|e| AppError::InternalError(format!("Failed to get reservations: {}", e)))?;

    let timeslots = generate_full_day_timeslots(date, slot_duration, &reserved);

    Ok(Json(MyAvailableResponse::Success { timeslots }))
}
```

- [ ] **Step 3: Update `router()` in `mod.rs` to accept `DoctorTimeslotRepo` and add the route**

In `server/src/module/timeslot/mod.rs`, update the `router` function signature to accept the repo:

```rust
pub async fn router(
    pg_pool: sqlx::PgPool,
    cfg: &AppConfig,
    pubsub_publisher: Arc<PubsubPublisher>,
    cancel_token: CancellationToken,
    doctor_timeslot_repo: Arc<dyn crate::doctor_actor::repo::DoctorTimeslotRepo>,
) -> AppResult<(Router, JoinHandle<()>)> {
```

Add the new field when constructing `TimeslotState`:

```rust
let state = TimeslotState {
    service: service.clone(),
    idempotency_cache: Arc::new(Mutex::new(idempotency_cache)),
    redis: redis_manager.clone(),
    config: cfg.timeslot.clone(),
    doctor_timeslot_repo,
};
```

Add the route to the router:

```rust
let router = Router::new()
    .route("/available", get(handlers::get_available_timeslot))
    .route("/my-available", get(handlers::get_my_available_timeslots))
    .route("/reserve", post(handlers::reserve_timeslot))
    .route("/confirm", post(handlers::confirm_booking))
    .route("/cancel", post(handlers::cancel_booking))
    .with_state(state);
```

- [ ] **Step 4: Wire `DoctorTimeslotRepo` in bootstrap.rs**

In `server/src/bootstrap.rs`, inside `init_routers()`, create the `DoctorTimeslotRepoImpl` and pass it to the timeslot router. Add the import:

```rust
use crate::doctor_actor::repo::DoctorTimeslotRepoImpl;
```

Then before the `let (timeslot_router, timeslot_worker_handle)` line, create the repo:

```rust
let doctor_timeslot_repo: Arc<dyn crate::doctor_actor::repo::DoctorTimeslotRepo> =
    Arc::new(DoctorTimeslotRepoImpl::new(deps.pg_pool.clone(), deps.redis_pool.clone()));
```

Update the timeslot router call to pass the repo:

```rust
let (timeslot_router, timeslot_worker_handle) = module::timeslot::router(
    deps.pg_pool.clone(),
    cfg,
    deps.pubsub_publisher.clone(),
    cancel_token.clone(),
    doctor_timeslot_repo,
).await?;
```

- [ ] **Step 5: Verify it compiles**

Run: `cargo check 2>&1 | head -30`
Expected: No errors (warnings are OK)

- [ ] **Step 6: Commit**

```bash
git add server/src/module/timeslot/handlers.rs server/src/module/timeslot/mod.rs server/src/bootstrap.rs
git commit -m "feat: add GET /timeslot/v1/my-available endpoint for doctor's own available slots"
```

---

### Task 4: Register in OpenAPI

**Files:**
- Modify: `server/src/openapi.rs`

- [ ] **Step 1: Read `openapi.rs` to find the timeslot section**

Read `server/src/openapi.rs` and locate where timeslot paths and schemas are registered.

- [ ] **Step 2: Add the new handler path and schemas**

Add `crate::module::timeslot::handlers::get_my_available_timeslots` to the `paths(...)` list in the timeslot section.

Add `crate::module::timeslot::handlers::MyAvailableQuery` and `crate::module::timeslot::handlers::MyAvailableResponse` to the `schemas(...)` list.

- [ ] **Step 3: Verify it compiles**

Run: `cargo check 2>&1 | head -20`
Expected: No errors

- [ ] **Step 4: Commit**

```bash
git add server/src/openapi.rs
git commit -m "feat: register my-available endpoint in OpenAPI spec"
```

---

### Task 5: Integration tests

**Files:**
- Create: `server/tests/my_available_timeslot_test.rs`

- [ ] **Step 1: Examine existing test patterns**

Read an existing integration test file (e.g., `server/tests/` directory) to understand how `TestServer` is set up with mock repos. The tests use manual mock structs implementing repo traits.

- [ ] **Step 2: Write integration tests**

Create `server/tests/my_available_timeslot_test.rs` with tests that:

1. Test happy path: doctor with schedule config and no reservations → returns full day of slots
2. Test with reservations: some slots excluded
3. Test no schedule config → returns `NoScheduleConfig` response
4. Test invalid date format → returns 400
5. Test non-doctor identity → returns 403

The mock `DoctorTimeslotRepo` must implement the full trait (all 6 methods). Only `get_schedule_config` and `find_reservations_by_date` need real logic; the rest can return `unimplemented!()` or defaults.

- [ ] **Step 3: Run tests**

Run: `cargo test my_available 2>&1 | tail -20`
Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add server/tests/my_available_timeslot_test.rs
git commit -m "test: add integration tests for GET /timeslot/v1/my-available"
```

---

### Task 6: Final verification

- [ ] **Step 1: Run full build**

Run: `cargo build 2>&1 | tail -10`
Expected: Build succeeds

- [ ] **Step 2: Run all tests**

Run: `cargo test 2>&1 | tail -20`
Expected: All tests pass

- [ ] **Step 3: Run clippy**

Run: `cargo clippy 2>&1 | tail -20`
Expected: No new warnings from our changes
