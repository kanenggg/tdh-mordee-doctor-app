# eKYC Service Pattern Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor the eKYC module to follow the service pattern established in `appointment/services.rs`, replacing Firebase RTDB lookup with ConsultationClient.

**Architecture:** Extract `ConsultationClient` from appointment router, share with ekyc router. Ekyc handler delegates to `EkycService` which calls ConsultationClient then EkycClient.

**Tech Stack:** Rust, Axum, async-trait, Arc<dyn Trait> pattern

**Reference:** `docs/superpowers/specs/2026-05-28-ekyc-service-pattern-design.md`

---

## Chunk 1: Refactor Appointment Router to Expose ConsultationClient

The consultation client is currently created inside `appointment::router()`. We need to extract it so ekyc can use the same instance.

### Task 1: Update Appointment Router Signature

**Files:**
- Modify: `server/src/module/appointment/mod.rs:24-54`

- [ ] **Step 1: Change router return type**

Current signature:
```rust
pub fn router(cfg: &AppConfig) -> Result<Router, AppError>
```

New signature:
```rust
pub fn router(cfg: &AppConfig) -> Result<(Router, Arc<dyn ConsultationClientTrait>), AppError>
```

Find line 24 and change:
```rust
pub fn router(cfg: &AppConfig) -> Result<(Router, Arc<dyn ConsultationClientTrait>), AppError> {
```

Also add the import at top of file if not present:
```rust
use self::external::ConsultationClientTrait;
```

- [ ] **Step 2: Return tuple with router and consultation client**

At line 51-54, change from:
```rust
Ok(Router::new()
    .route("/{bookingId}", get(handlers::get_appointment_detail))
    .with_state(state))
```

To:
```rust
let consultation = Arc::new(ConsultationClient::new(
    cfg.service.consultation_internal_base_uri.clone(),
));

let service: Arc<dyn AppointmentServiceTrait> = Arc::new(AppointmentService::new(
    consultation.clone(),
    Arc::new(IamClient::new(cfg.service.iam_gatekeeper_base_uri.clone())),
    Arc::new(PaymentClient::new(
        cfg.service.payment_internal_base_uri.clone(),
    )),
));

let state = handlers::AppointmentState {
    service,
    insurance_template: insurance_tpl,
    coupon_template: coupon_tpl,
};

let router = Router::new()
    .route("/{bookingId}", get(handlers::get_appointment_detail))
    .with_state(state);

Ok((router, consultation))
```

- [ ] **Step 3: Update bootstrap.rs to handle tuple**

File: `server/src/bootstrap.rs:236-237`

Change from:
```rust
let appointment_router =
    module::appointment::router(cfg)?.merge(module::ekyc::router(cfg, deps.firebase.clone()));
```

To:
```rust
let (appointment_router, consultation) =
    module::appointment::router(cfg)?;

let ekyc_router = module::ekyc::router(
    consultation.clone(),
    cfg.service.eagle_base_uri.clone(),
);

let appointment_router = appointment_router.merge(ekyc_router);
```

- [ ] **Step 4: Remove unused Firebase import from ekyc module (cleanup in mod.rs)**

File: `server/src/module/ekyc/mod.rs:1-14`

Remove the FirebaseRepo import and PatientLookup reference:
```rust
use std::sync::Arc;

use axum::{routing::get, Router};

use crate::config::AppConfig;
use crate::core::error::AppError;

use self::service::EkycClient;
```

Remove lines referencing FirebaseRepo and PatientLookup.

- [ ] **Step 5: Run cargo check**

```bash
cargo check
```

Expected: Compilation errors (ekyc module still expects old signature)

- [ ] **Step 6: Commit**

```bash
git add server/src/module/appointment/mod.rs server/src/bootstrap.rs
git commit -m "refactor(appointment): extract ConsultationClient for sharing with ekyc"
```

---

## Chunk 2: Implement EkycService and Types

Create the service layer with business logic and types.

### Task 2: Add Service Types and EkycService

**Files:**
- Modify: `server/src/module/ekyc/service.rs`

- [ ] **Step 1: Add imports at top of service.rs**

Add to existing imports:
```rust
use std::sync::Arc;

use async_trait::async_trait;

use crate::module::appointment::external::ConsultationClientTrait;
use crate::module::appointment::external::{ConsultationDetail, ConsultationLookup};
```

- [ ] **Step 2: Add EkycDetail struct**

After the `EkycInfo` enum definition (around line 19), add:

```rust
/// Public detail returned when eKYC is found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EkycDetail {
    pub document_image_url: String,
    pub liveness_image_url: String,
    pub full_name: String,
    pub birth_date: String,
    pub gender: String,
}
```

- [ ] **Step 3: Add EkycResult enum**

After `EkycDetail`, add:

```rust
/// Result of eKYC lookup by booking ID.
#[derive(Debug)]
pub enum EkycResult {
    Found(EkycDetail),
    AppointmentNotFound,
    EkycNotAvailable,
}
```

- [ ] **Step 4: Add EkycServiceTrait trait**

After `EkycResult`, add:

```rust
#[async_trait]
pub trait EkycServiceTrait: Send + Sync {
    async fn get_ekyc_by_booking_id(&self, booking_id: &str) -> AppResult<EkycResult>;
}
```

- [ ] **Step 5: Add EkycService struct**

After the trait, add:

```rust
#[derive(Clone)]
pub struct EkycService {
    consultation: Arc<dyn ConsultationClientTrait>,
    ekyc: Arc<EkycClient>,
}
```

- [ ] **Step 6: Add EkycService::new method**

```rust
impl EkycService {
    pub fn new(
        consultation: Arc<dyn ConsultationClientTrait>,
        ekyc: Arc<EkycClient>,
    ) -> Self {
        Self { consultation, ekyc }
    }
}
```

- [ ] **Step 7: Implement EkycServiceTrait for EkycService**

```rust
#[async_trait]
impl EkycServiceTrait for EkycService {
    async fn get_ekyc_by_booking_id(&self, booking_id: &str) -> AppResult<EkycResult> {
        // 1. Get consultation detail
        let consultation = match self.consultation.get_appointment(booking_id).await? {
            ConsultationLookup::Found(detail) => detail,
            ConsultationLookup::NotFound => {
                return Ok(EkycResult::AppointmentNotFound);
            }
        };

        // 2. Extract patient account_id
        let patient_account_id = consultation.patient.account_id;

        // 3. Fetch eKYC info
        let ekyc_info = self.ekyc.fetch_by_account_id(patient_account_id).await?;

        // 4. Convert to EkycResult
        Ok(match ekyc_info {
            EkycInfo::Available {
                document_url,
                liveness_url,
                full_name,
                birth_date,
                gender,
            } => EkycResult::Found(EkycDetail {
                document_image_url: document_url,
                liveness_image_url: liveness_url,
                full_name,
                birth_date,
                gender,
            }),
            EkycInfo::NotAvailable => EkycResult::EkycNotAvailable,
        })
    }
}
```

- [ ] **Step 8: Run cargo check**

```bash
cargo check
```

Expected: Errors in handlers.rs and mod.rs (will fix in next chunk)

- [ ] **Step 9: Commit**

```bash
git add server/src/module/ekyc/service.rs
git commit -m "feat(ekyc): add EkycService with ConsultationClient integration"
```

---

## Chunk 3: Update Handlers and State

Remove PatientLookup trait, simplify handler to delegate to service.

### Task 3: Update Handlers

**Files:**
- Modify: `server/src/module/ekyc/handlers.rs`

- [ ] **Step 1: Remove PatientLookup trait and impl**

Delete lines 19-63 (the `PatientLookup` trait and `impl PatientLookup for FirebaseRepo`).

- [ ] **Step 2: Update EkycState struct**

Change from (around line 66):
```rust
#[derive(Clone)]
pub struct EkycState {
    pub lookup: Arc<dyn PatientLookup>,
    pub ekyc: Arc<EkycClient>,
}
```

To:
```rust
#[derive(Clone)]
pub struct EkycState {
    pub service: Arc<dyn EkycServiceTrait>,
}
```

- [ ] **Step 3: Add imports for new types**

At top of file, add:
```rust
use super::service::{EkycServiceTrait, EkycResult};
```

Remove the import for `PatientLookup`.

- [ ] **Step 4: Update handler function**

Change the handler body (lines 110-150) to:
```rust
pub async fn get_appointment_ekyc(
    State(state): State<EkycState>,
    identity: DoctorIdentity,
    Path(booking_id): Path<String>,
) -> AppResult<Json<EkycResponse>> {
    let result = state.service.get_ekyc_by_booking_id(&booking_id).await?;

    Ok(Json(match result {
        EkycResult::Found(detail) => EkycResponse::EkycAvailable {
            document_image_url: detail.document_image_url,
            liveness_image_url: detail.liveness_image_url,
            full_name: detail.full_name,
            birth_date: detail.birth_date,
            gender: detail.gender,
        },
        EkycResult::AppointmentNotFound => EkycResponse::AppointmentNotFound,
        EkycResult::EkycNotAvailable => EkycResponse::EkycNotAvailable,
    }))
}
```

Remove the unused `booking_id::parse_date_from_booking_id` import and call.

- [ ] **Step 5: Remove booking_id module import**

Remove this line from imports:
```rust
use super::booking_id::parse_date_from_booking_id;
```

- [ ] **Step 6: Run cargo check**

```bash
cargo check
```

Expected: Errors in mod.rs (will fix in next chunk)

- [ ] **Step 7: Commit**

```bash
git add server/src/module/ekyc/handlers.rs
git commit -m "refactor(ekyc): simplify handler to delegate to EkycService"
```

---

## Chunk 4: Update Module Router

Update mod.rs to use new dependencies and remove old test helper.

### Task 4: Update Ekyc Module Router

**Files:**
- Modify: `server/src/module/ekyc/mod.rs`
- Delete: `server/src/module/ekyc/booking_id.rs`

- [ ] **Step 1: Update mod.rs imports**

Change imports at top to:
```rust
pub mod handlers;
pub mod service;

use std::sync::Arc;

use axum::{routing::get, Router};

use crate::module::appointment::external::ConsultationClientTrait;

use self::handlers::EkycState;
use self::service::{EkycClient, EkycService, EkycServiceTrait};
```

- [ ] **Step 3: Update router function signature**

Change from:
```rust
pub fn router(cfg: &AppConfig, firebase: FirebaseRepo) -> Router {
    let lookup: Arc<dyn PatientLookup> = Arc::new(firebase);
    let ekyc = Arc::new(EkycClient::new(cfg.service.eagle_base_uri.clone()));
    router_with_state(EkycState { lookup, ekyc })
}
```

To:
```rust
pub fn router(
    consultation: Arc<dyn ConsultationClientTrait>,
    eagle_base_uri: String,
) -> Router {
    let ekyc_client = Arc::new(EkycClient::new(eagle_base_uri));
    let service = Arc::new(EkycService::new(consultation, ekyc_client));

    Router::new()
        .route("/{bookingId}/ekyc", get(handlers::get_appointment_ekyc))
        .with_state(EkycState { service })
}
```

- [ ] **Step 4: Remove old test helper functions**

Delete `router_with_lookup` and `router_with_state` functions (if they exist after the above changes).

- [ ] **Step 5: Add test-only constructor**

Add at end of file:
```rust
#[cfg(test)]
pub fn router_with_service(service: Arc<dyn EkycServiceTrait>) -> Router {
    Router::new()
        .route("/{bookingId}/ekyc", get(handlers::get_appointment_ekyc))
        .with_state(EkycState { service })
}
```

- [ ] **Step 6: Delete booking_id.rs file**

```bash
rm server/src/module/ekyc/booking_id.rs
```

- [ ] **Step 7: Check for test file references**

Verify no test files import `booking_id` or `PatientLookup`:

```bash
grep -r "booking_id\|PatientLookup" server/tests/ || echo "No references found"
```

- [ ] **Step 8: Run cargo check**

```bash
cargo check
```

Expected: No errors

- [ ] **Step 9: Run tests**

```bash
cargo test
```

Expected: Tests pass (ekyc client tests still work)

- [ ] **Step 10: Commit**

```bash
git add server/src/module/ekyc/
git commit -m "refactor(ekyc): update router to use ConsultationClient"
```

---

## Chunk 5: Update OpenAPI Registration

Ensure ekyc endpoint is registered in OpenAPI docs.

### Task 5: Verify OpenAPI Registration

**Files:**
- Check: `server/src/openapi.rs`

- [ ] **Step 1: Verify ekyc path is registered**

Check that `/appointment/v1/{bookingId}/ekyc` is registered in openapi.rs.

The handler already has `#[utoipa::path(...)]` annotation, so it should already be registered. Verify by grepping:

```bash
grep -n "ekyc" server/src/openapi.rs
```

If not found, add it to the appropriate paths module.

- [ ] **Step 2: Run cargo build**

```bash
cargo build
```

Expected: Successful build

- [ ] **Step 3: Commit if changes made**

```bash
git add server/src/openapi.rs
git commit -m "docs(ekyc): verify OpenAPI path registration"
```

---

## Verification Steps

After completing all chunks:

- [ ] **Run all tests**
```bash
cargo test
```

- [ ] **Run clippy**
```bash
cargo clippy
```

- [ ] **Check formatting**
```bash
cargo fmt --check
```

- [ ] **Verify route exists**

Start server and hit the endpoint:
```bash
cargo run
curl http://localhost:8080/appointment/v1/BK20260227810949/ekyc
```

---

## Summary of Changes

**Files modified:**
1. `server/src/module/appointment/mod.rs` - Return consultation client
2. `server/src/bootstrap.rs` - Wire consultation client to ekyc
3. `server/src/module/ekyc/service.rs` - Add EkycService and types
4. `server/src/module/ekyc/handlers.rs` - Simplify to delegate to service
5. `server/src/module/ekyc/mod.rs` - Update router signature
6. `server/src/module/ekyc/booking_id.rs` - DELETED

**Dependencies removed:**
- `FirebaseRepo` from ekyc module
- `PatientLookup` trait
- `booking_id.rs` file

**Dependencies added:**
- `ConsultationClientTrait` from appointment module
- `EkycService` with service pattern
