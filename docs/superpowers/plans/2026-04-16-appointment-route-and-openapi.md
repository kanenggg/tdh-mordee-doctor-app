# Appointment Detail Route + OpenAPI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire the existing appointment BFF clients, mapper, and models into a live HTTP route `GET /appointment/v1/{bookingId}` with OpenAPI spec support.

**Architecture:** Add a handler in `handlers.rs` using `DoctorIdentity` + `Path<String>` extractors, create an `AppointmentState` shared state, expose a `router(cfg)` function from `mod.rs`, wire it in `bootstrap.rs`, and register the handler + schemas in `openapi.rs`. The three upstream clients (consultation, IAM, payment) and mapper are already implemented and tested.

**Tech Stack:** Rust, Axum 0.8, utoipa 5 (OpenAPI), tokio 1 (`try_join!` for parallel upstream calls), tracing.

**Design spec:** `docs/superpowers/specs/2026-04-09-appointment-detail-bff-design.md`

---

## File Map

| File | Action | Responsibility |
|------|--------|----------------|
| `server/src/module/appointment/handlers.rs` | Replace | `AppointmentState` struct + `get_appointment_detail` handler with `#[utoipa::path]` |
| `server/src/module/appointment/mod.rs` | Modify | Add `router(cfg)` that constructs clients, validates URL templates, wires state |
| `server/src/bootstrap.rs` | Modify | Wire appointment router in `init_routers` + `build_app` |
| `server/src/openapi.rs` | Modify | Register handler path + response schemas + appointment tag |

---

### Task 1: Replace `handlers.rs` with state + handler

**Files:**
- Replace: `server/src/module/appointment/handlers.rs`

- [ ] **Step 1: Write `handlers.rs`**

The handler orchestrates: (1) call consultation upstream → short-circuit on not-found, (2) call IAM + payment in parallel via `tokio::try_join!`, (3) compose via `mapper::compose`.

```rust
//! `GET /appointment/v1/{bookingId}` handler.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use jiff::civil::date;
use tracing::{info, instrument, warn};

use crate::core::auth::DoctorIdentity;
use crate::core::error::AppResult;

use super::consultation_client::ConsultationClientTrait;
use super::iam_client::IamClientTrait;
use super::mapper;
use super::models::ApiResponse;
use super::payment_client::PaymentClientTrait;

/// Shared state injected via Axum `State`.
#[derive(Clone)]
pub struct AppointmentState {
    pub consultation: Arc<dyn ConsultationClientTrait>,
    pub iam: Arc<dyn IamClientTrait>,
    pub payment: Arc<dyn PaymentClientTrait>,
    pub insurance_template: String,
    pub coupon_template: String,
}

/// `GET /appointment/v1/{bookingId}`
#[utoipa::path(
    get,
    path = "/appointment/v1/{bookingId}",
    tag = "appointment",
    params(
        ("bookingId" = String, Path, description = "Booking ID (e.g. BK20220227810949)")
    ),
    responses(
        (status = 200, description = "Success | AppointmentNotFound | PatientProfileNotFound",
         body = ApiResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden (account_type != 2)"),
        (status = 502, description = "Upstream service unavailable"),
    ),
    security(("TDH-SEC-IAM-USER-IDENTITY" = []))
)]
#[instrument(
    name = "appointment_detail",
    skip(state),
    fields(booking_id = %booking_id, doctor_account_id = %identity.doctor_account_id)
)]
pub async fn get_appointment_detail(
    State(state): State<AppointmentState>,
    identity: DoctorIdentity,
    Path(booking_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    // 1. Consultation lookup — short-circuit if not found.
    let consultation = match state.consultation.get_appointment(&booking_id).await? {
        super::consultation_client::ConsultationLookup::Found(d) => d,
        super::consultation_client::ConsultationLookup::NotFound => {
            info!("appointment not found in consultation upstream");
            return Ok(Json(ApiResponse::AppointmentNotFound));
        }
    };

    let patient_account_id = consultation.patient.account_id;
    let payment_tx_id = consultation.payment_tx_id;

    // 2. Parallel IAM + payment lookups.
    let iam_future = state.iam.get_profile_by_account(patient_account_id);
    let payment_future = state.payment.get_payment(payment_tx_id);

    let (iam_result, payment_result) =
        tokio::try_join!(
            async { iam_future.await },
            async { payment_future.await },
        )?;

    // 3. IAM not-found → PatientProfileNotFound.
    let profile = match iam_result {
        super::iam_client::IamLookup::Found(p) => p,
        super::iam_client::IamLookup::NotFound => {
            warn!(%patient_account_id, "patient profile not found in IAM");
            return Ok(Json(ApiResponse::PatientProfileNotFound));
        }
    };

    // 4. Payment not-found is soft-missing (booking not yet paid).
    let payment = match payment_result {
        super::payment_client::PaymentLookup::Found(d) => Some(d),
        super::payment_client::PaymentLookup::NotFound => None,
    };

    // 5. Compose.
    let templates = mapper::Templates {
        insurance: &state.insurance_template,
        coupon: &state.coupon_template,
    };
    let today = date(
        jiff::civil::Date::today(jiff::tz::TimeZone::UTC).year(),
        jiff::civil::Date::today(jiff::tz::TimeZone::UTC).month(),
        jiff::civil::Date::today(jiff::tz::TimeZone::UTC).day(),
    );

    let body = mapper::compose(consultation, profile, payment, templates, today);

    info!(
        has_insurance = ?body.payment.as_ref().map(|p| p.has_insurance),
        "appointment detail composed successfully"
    );

    Ok(Json(ApiResponse::Success(Box::new(body))))
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check 2>&1 | head -40`
Expected: May fail because `mod.rs` doesn't declare `handlers` as a submodule yet — that's fine. We fix it in Task 2.

---

### Task 2: Wire `router()` in `mod.rs`

**Files:**
- Modify: `server/src/module/appointment/mod.rs`

- [ ] **Step 1: Update `mod.rs`**

Add `pub mod handlers;` and create the `router()` function that:
- Constructs the three upstream clients from config
- Validates the URL templates at startup
- Returns an Axum `Router` with the state

```rust
//! BFF aggregator for the doctor "appointment detail" screen.
//!
//! This module is being rewritten — see
//! docs/superpowers/specs/2026-04-09-appointment-detail-bff-design.md
//! and docs/superpowers/plans/2026-04-09-appointment-detail-bff-aggregator.md.

pub mod consultation_client;
pub mod handlers;
pub mod iam_client;
pub mod mapper;
pub mod models;
pub mod payment_client;

use std::sync::Arc;

use axum::{routing::get, Router};

use crate::config::AppConfig;
use crate::core::error::AppError;

use self::consultation_client::ConsultationClient;
use self::iam_client::IamClient;
use self::mapper::validate_url_template;
use self::payment_client::PaymentClient;

pub fn router(cfg: &AppConfig) -> Result<Router, AppError> {
    // Startup validation: URL templates must contain exactly one placeholder.
    let insurance_tpl = validate_url_template(
        &cfg.insurance.condition_url_template,
        "{insurerKey}",
    )
    .map_err(|msg| AppError::InternalError(format!("Invalid insurance config: {}", msg)))?
    .to_string();

    let coupon_tpl = validate_url_template(
        &cfg.coupon.condition_url_template,
        "{couponKey}",
    )
    .map_err(|msg| AppError::InternalError(format!("Invalid coupon config: {}", msg)))?
    .to_string();

    let state = handlers::AppointmentState {
        consultation: Arc::new(ConsultationClient::new(
            cfg.service.consultation_internal_base_uri.clone(),
        )),
        iam: Arc::new(IamClient::new(
            cfg.service.iam_gatekeeper_base_uri.clone(),
        )),
        payment: Arc::new(PaymentClient::new(
            cfg.service.payment_internal_base_uri.clone(),
        )),
        insurance_template: insurance_tpl,
        coupon_template: coupon_tpl,
    };

    Ok(Router::new()
        .route("/{bookingId}", get(handlers::get_appointment_detail))
        .with_state(state))
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check 2>&1 | head -60`
Expected: May still fail on unused import warnings or similar. Fix any issues. The `router` function signature returns `Result<Router, AppError>` which matches how `init_routers` in `bootstrap.rs` handles the consultation router (uses `?`).

---

### Task 3: Wire appointment router in `bootstrap.rs`

**Files:**
- Modify: `server/src/bootstrap.rs` (lines 41-47 for `AppRouters`, lines 189-241 for `init_routers`, lines 243-254 for `build_app`)

- [ ] **Step 1: Add `appointment` field to `AppRouters`**

In `bootstrap.rs`, add a field to the `AppRouters` struct (around line 41-47):

```rust
pub struct AppRouters {
    pub notification: Router,
    pub task: Router,
    pub consultation: Router,
    pub ranking: Router,
    pub timeslot: Router,
    pub appointment: Router,   // <-- ADD THIS
}
```

- [ ] **Step 2: Construct the appointment router in `init_routers`**

Inside `init_routers` (around line 189-241), after the existing router constructions, add:

```rust
    let appointment_router = module::appointment::router(cfg)?;
```

Then update the return expression to include `appointment: appointment_router,`.

The full updated `init_routers` function should look like:

```rust
pub async fn init_routers(cfg: &AppConfig, deps: &mut Dependencies) -> Result<AppRouters> {
    let (notification_router, notification_repo) =
        module::notification::router(deps.firestore.clone(), cfg);

    let task_router = module::webhook::task_routes(
        notification_repo,
        deps.fcm_service.clone(),
        deps.cloud_tasks_service.clone(),
        Arc::new(cfg.cloud_tasks.clone()),
    );

    let timeslot_repo: Arc<dyn module::timeslot::TimeslotRepo> =
        Arc::new(module::timeslot::TimeslotRepoImpl::new(deps.pg_pool.clone(), deps.redis_pool.clone()));

    let consultation_router = module::consultation::router(
        cfg.service.biz_apm_base_uri.clone(),
        deps.pg_pool.clone(),
        &cfg.paseto.summarization_key,
        cfg.service.biz_jade_service_base_uri.clone(),
        cfg.service.biz_apm_base_uri.clone(),
        deps.pubsub_publisher.clone(),
        cfg.pubsub.topics.consultations.clone(),
        timeslot_repo,
    )?;

    let ranking_router = module::ranking::router(
        deps.ranking_repo.clone(),
        deps.ranking_cache.clone(),
        deps.privilege_svc.clone(),
    );

    use crate::doctor_actor::repo::DoctorTimeslotRepoImpl;
    let doctor_timeslot_repo: Arc<dyn crate::doctor_actor::repo::DoctorTimeslotRepo> =
        Arc::new(DoctorTimeslotRepoImpl::new(deps.pg_pool.clone(), deps.redis_pool.clone()));

    let timeslot_router = module::timeslot::router(
        deps.pg_pool.clone(),
        cfg,
        deps.pubsub_publisher.clone(),
        doctor_timeslot_repo,
    ).await?;

    let appointment_router = module::appointment::router(cfg)?;

    Ok(AppRouters {
        notification: notification_router,
        task: task_router,
        consultation: consultation_router,
        ranking: ranking_router,
        timeslot: timeslot_router,
        appointment: appointment_router,
    })
}
```

- [ ] **Step 3: Mount the router in `build_app`**

In `build_app` (around line 243-254), add the `.nest(...)` line:

```rust
pub fn build_app(routers: AppRouters) -> Router {
    Router::new()
        .merge(SwaggerUi::new("/swagger").url("/api-docs/openapi.json", openapi::ApiDoc::openapi()))
        .merge(core::health_router())
        .nest("/notifications/v1", routers.notification)
        .nest("/webhook", routers.task)
        .nest("/consultation/v1", routers.consultation)
        .nest("/ranking/v1", routers.ranking)
        .nest("/timeslot/v1", routers.timeslot)
        .nest("/appointment/v1", routers.appointment)   // <-- ADD THIS
        .layer(middleware::from_fn(core::gcp_logging_middleware))
        .layer(CorsLayer::permissive())
}
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo check 2>&1 | head -60`
Expected: Clean compilation, or errors only in `handlers.rs` which we'll fix next.

---

### Task 4: Fix handler compilation issues

**Files:**
- Modify: `server/src/module/appointment/handlers.rs`

- [ ] **Step 1: Run cargo check and fix any issues**

The `today` computation in the handler is verbose. Replace it with a direct `jiff::civil::Date::today()` call:

```rust
    let today = jiff::civil::Date::today(jiff::tz::TimeZone::UTC);
```

This is a one-line replacement in the handler body — remove the `date(...)` construction and use the direct method.

Also verify:
- `use jiff::civil::date;` import is replaced with just using `Date::today()` directly
- All type references match what's defined in the client files and mapper

Run: `cargo check 2>&1 | head -60`
Expected: Clean compilation (warnings are OK).

---

### Task 5: Register in OpenAPI spec

**Files:**
- Modify: `server/src/openapi.rs`

- [ ] **Step 1: Add the handler path to `paths(...)`**

In the `#[openapi(paths(...))]` section (around line 52-85), add the appointment handler:

```rust
        // Timeslot endpoints
        crate::module::timeslot::handlers::get_available_timeslot,
        crate::module::timeslot::handlers::get_my_available_timeslots,
        // Appointment endpoints                   // <-- ADD THIS BLOCK
        crate::module::appointment::handlers::get_appointment_detail,
```

- [ ] **Step 2: Add schemas to `components(schemas(...))`**

In the `components(schemas(...))` section (around line 86-165), add the appointment response types:

```rust
            // Timeslot
            Timeslot,
            TimeslotStatus,
            GetAvailableTimeslotsQuery,
            GetAvailableTimeslotsResponse,
            crate::module::timeslot::handlers::MyAvailableQuery,
            crate::module::timeslot::handlers::MyAvailableResponse,
            crate::module::timeslot::handlers::DoctorTimeslotSchema,
            // Appointment                     // <-- ADD THIS BLOCK
            crate::module::appointment::models::ApiResponse,
            crate::module::appointment::models::SuccessBody,
            crate::module::appointment::models::AppointmentTime,
            crate::module::appointment::models::Patient,
            crate::module::appointment::models::Payment,
            crate::module::appointment::models::Coupon,
            crate::module::appointment::models::Prescreen,
```

- [ ] **Step 3: Add the appointment tag**

In the `tags(...)` section (around line 167-174), add:

```rust
        (name = "timeslot", description = "Doctor timeslot management"),
        (name = "appointment", description = "Doctor appointment detail"),   // <-- ADD THIS
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo check 2>&1 | head -60`
Expected: Clean compilation.

---

### Task 6: Run tests and verify

- [ ] **Step 1: Run the full test suite**

Run: `cargo test 2>&1`
Expected: All existing tests pass. The new handler has no dedicated integration tests yet (those were specified in the design spec but are out of scope for this route-wiring plan — the mapper already has 20+ unit tests covering all composition logic).

- [ ] **Step 2: Run clippy**

Run: `cargo clippy 2>&1 | head -40`
Expected: No errors. Warnings about unused imports should be fixed inline.

- [ ] **Step 3: Run fmt**

Run: `cargo fmt -- --check 2>&1`
Expected: No output (all formatted).

- [ ] **Step 4: Commit**

```bash
git add server/src/module/appointment/handlers.rs \
        server/src/module/appointment/mod.rs \
        server/src/bootstrap.rs \
        server/src/openapi.rs
git commit -m "feat(appointment): wire GET /appointment/v1/{bookingId} route with OpenAPI spec"
```

---

## Self-Review Checklist

1. **Spec coverage:** The design spec (`2026-04-09-appointment-detail-bff-design.md`) defines:
   - Endpoint `GET /appointment/v1/{bookingId}` with `DoctorIdentity` → Task 1 (handler)
   - `AppointmentState` with three upstream clients → Task 1 + Task 2
   - URL template startup validation → Task 2
   - Orchestration: consultation → parallel IAM + payment → compose → Task 1
   - Bootstrap wiring via `.nest("/appointment/v1", ...)` → Task 3
   - OpenAPI registration with tag → Task 5
   - All response types already have `ToSchema` derives (in models.rs) → Task 5 registers them

2. **Placeholder scan:** No TBDs, TODOs, or "implement later". All code is concrete.

3. **Type consistency:**
   - `AppointmentState` fields match the trait names: `ConsultationClientTrait`, `IamClientTrait`, `PaymentClientTrait`
   - `ConsultationLookup::Found/NotFound`, `IamLookup::Found/NotFound`, `PaymentLookup::Found/NotFound` match client definitions
   - `mapper::Templates` struct has `insurance` and `coupon` string fields — matches mapper.rs line 231-235
   - `mapper::compose` signature takes `(ConsultationDetail, MorDeeUserProfile, Option<PaymentDetail>, Templates, Date)` — matches mapper.rs line 243-248
   - `ApiResponse` enum variants `Success/AppointmentNotFound/PatientProfileNotFound` match models.rs line 12-18
   - `DoctorIdentity` extractor requires canonical `account_type == 2` for doctors; auth.rs also accepts `3` only for legacy compatibility
