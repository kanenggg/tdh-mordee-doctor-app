# Consultation Settings Implementation Plan

**Goal:** Build doctor consultation settings APIs for schedule config plus schedule and instant availability toggles.

**Architecture:** Add a top-level `/profile` router backed by PostgreSQL tables/functions. This is settings-only: it does not update ranking `doctor_availability`, Redis ranking sets, or Pub/Sub availability events.

Rust module ownership mirrors the URL paths:

- `profile::consultation_setting` owns `/profile/v1/consultation-setting/*`.
- `profile::availability` owns `/profile/v1/availability/*`.
- `profile::common` contains shared profile DTOs/helpers such as `bizUnitId` query parsing and success responses.

**Tech Stack:** Axum, SQLx, PostgreSQL, Utoipa, `axum-test`, `testcontainers`.

---

## Public API

Mount these routes under `/profile`:

- `PUT /profile/v1/consultation-setting/schedule-config/1`
  - Body:
    ```json
    {
      "specificDate": [{ "date": "2026-05-20", "periods": [{ "startTime": 540, "endTime": 720 }] }],
      "dayOfWeek": { "1": [{ "startTime": 540, "endTime": 1020 }] }
    }
    ```
  - Response: `{ "__type": "Success" }`

- `GET /profile/v1/consultation-setting/schedule-config/1`
  - Response: the saved schedule config.
  - If missing, return `{ "specificDate": [], "dayOfWeek": {} }`.

- `POST /profile/v1/availability/schedule`
  - Body: `{ "available": true, "bizUnitId": 1 }`
  - Response: `{ "__type": "Success" }`

- `POST /profile/v1/availability/instant`
  - Body: `{ "available": true, "bizUnitId": 1 }`
  - Response: `{ "__type": "Success" }`

- `GET /profile/v1/availability?bizUnitId=1`
  - Response:
    ```json
    {
      "__type": "Success",
      "bizUnitId": 1,
      "scheduleAvailable": true,
      "instantAvailable": false
    }
    ```
  - If missing, return `scheduleAvailable: false` and `instantAvailable: false`.

## Implementation Changes

- Finish `server/src/module/profile/consultation_setting` with models, handlers, service, repository, and `router(pool)`.
- Use `DoctorIdentity` so only doctor account types can access these endpoints.
- Use doctor account id from the IAM identity header as `doctor_id` in consultation settings tables.
- Store schedule config as JSONB in `consultation_schedule`; store instant availability in `consultation_instant`.
- Validate `bizUnitId > 0`, day-of-week keys `1..=7`, specific dates in `yyyy-mm-dd` format, and periods where `0 <= startTime < endTime <= 1440`.
- Treat `specificDate.date` as a string in `yyyy-mm-dd` format; store and return it unchanged.
- Mount the router in `bootstrap.rs` with `.nest("/profile", routers.profile)`.
- Register the handlers and schemas in `openapi.rs`.

## Database

Migrations:

- `db/postgres/migrations/20260520000000_consultation_settings.sql`
- `db/postgres/migrations/20260520001000_consultation_settings_defaults.sql`

Tables:

- `consultation_schedule(doctor_id, biz_unit_id, is_available, schedule_config, created_at, updated_at)`
- `consultation_instant(doctor_id, biz_unit_id, is_available, created_at, updated_at)`

Functions:

- `save_consultation_schedule_config`
- `get_consultation_schedule_config`
- `set_consultation_schedule_availability`
- `set_consultation_instant_availability`
- `get_consultation_availability`

## Test Plan

- `server/tests/consultation_settings_test.rs`
- Covered scenarios:
  - Missing auth returns `401`.
  - Non-doctor auth returns `403`.
  - `GET /profile/v1/availability` defaults both booleans to `false`.
  - Missing schedule config and availability DB records return default values from both API and DB functions.
  - Schedule and instant toggles persist independently.
  - Settings are isolated by `bizUnitId`.
  - Schedule config round-trips exactly.
  - Invalid `bizUnitId`, day-of-week, date, or time period returns `400`.

Verification commands:

```bash
cargo test --test consultation_settings_test
cargo test
cargo fmt --all --check
cargo clippy --all-targets --all-features
```

## Assumptions

- This feature intentionally does not touch ranking availability, Redis ranking caches, or Pub/Sub events.
- `/profile` is the required namespace, matching the original sketch.
- `doctor_id` in the new tables means doctor account id from the IAM identity header, not the UUID from the ranking `doctor` table.
- No schedule-config-to-timeslot integration is included in this v1.
