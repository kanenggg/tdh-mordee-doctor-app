# eKYC Service Pattern Design

**Date:** 2026-05-28
**Module:** `server/src/module/ekyc/`
**Status:** Draft

## Overview

Refactor the eKYC module to follow the service pattern established in `server/src/module/appointment/services.rs`. This removes the Firebase RTDB dependency and reuses the `ConsultationClientTrait` for fetching appointment/consultation data.

## Current State

```
GET /appointment/v1/{bookingId}/ekyc
  ↓
Handler parses booking_id → date
  ↓
PatientLookup.patient_account_id(doctor_id, date, booking_id) → RTDB
  ↓
EkycClient.fetch_by_account_id(patient_account_id) → Eagle
  ↓
EkycResponse
```

**Dependencies:**
- `FirebaseRepo` for appointment lookup
- Custom `PatientLookup` trait
- `EkycClient` for Eagle service calls

## Target State

```
GET /appointment/v1/{bookingId}/ekyc
  ↓
Handler delegates to EkycService
  ↓
EkycService.get_ekyc_by_booking_id(booking_id)
  ↓
ConsultationClient.get_appointment(booking_id) → ConsultationDetail
  ↓
Extract patient.account_id
  ↓
EkycClient.fetch_by_account_id(patient_account_id) → Eagle
  ↓
EkycResult → EkycResponse
```

**Dependencies:**
- `ConsultationClientTrait` (shared with appointment module)
- `EkycClient` for Eagle service calls
- `EkycService` for business logic

## Module Structure

```
ekyc/
  mod.rs           # Router wiring
  handlers.rs      # HTTP handlers (thin, delegate to service)
  service.rs       # EkycService (business logic)
```

**Removed:**
- `booking_id.rs` — Date parsing no longer needed (handled by consultation upstream)

## Types

### Service Result (Internal)

```rust
pub enum EkycResult {
    Found(EkycDetail),
    AppointmentNotFound,
    EkycNotAvailable,
}
```

### Found Case Payload

```rust
pub struct EkycDetail {
    pub document_image_url: String,
    pub liveness_image_url: String,
    pub full_name: String,
    pub birth_date: String,
    pub gender: String,
}
```

### API Response (External, unchanged)

```rust
#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "__type")]
pub enum EkycResponse {
    #[serde(rename_all = "camelCase")]
    EkycAvailable { /* ... */ },
    EkycNotAvailable,
    AppointmentNotFound,
}
```

## Service Implementation

### EkycService Struct

```rust
#[derive(Clone)]
pub struct EkycService {
    consultation: Arc<dyn ConsultationClientTrait>,
    ekyc: Arc<EkycClient>,
}
```

### Trait

```rust
#[async_trait]
pub trait EkycServiceTrait: Send + Sync {
    async fn get_ekyc_by_booking_id(&self, booking_id: &str) -> AppResult<EkycResult>;
}
```

### Business Logic

1. Call `consultation.get_appointment(booking_id)`
2. If `NotFound`, return `Ok(EkycResult::AppointmentNotFound)`
3. Extract `patient.account_id` from `ConsultationDetail`
4. Call `ekyc.fetch_by_account_id(patient_account_id)`
5. Convert `EkycInfo` to `EkycResult`

## Handler Changes

### EkycState (Simplified)

```rust
#[derive(Clone)]
pub struct EkycState {
    pub service: Arc<dyn EkycServiceTrait>,
}
```

### Handler (Thin Adapter)

- Remove `PatientLookup` trait
- Remove `parse_date_from_booking_id` call
- Delegate to `service.get_ekyc_by_booking_id()`
- Convert `EkycResult` → `EkycResponse`

## Router Changes

### mod.rs Signature

```rust
pub fn router(
    consultation: Arc<dyn ConsultationClientTrait>,
    eagle_base_uri: String,
) -> Router
```

**Import:**
```rust
use crate::module::appointment::external::ConsultationClientTrait;
```

### Bootstrap Wiring

**Before:**
```rust
let ekyc_router = module::ekyc::router(cfg, deps.firebase.clone());
```

**After:**
```rust
let ekyc_router = module::ekyc::router(
    consultation.clone(),
    cfg.service.eagle_base_uri.clone(),
);
```

where `consultation` is the same `Arc<dyn ConsultationClientTrait>` used by the appointment module.

## Error Handling

- `ConsultationLookup::NotFound` → `Ok(EkycResult::AppointmentNotFound)`
- `EkycInfo::NotAvailable` → `Ok(EkycResult::EkycNotAvailable)`
- Transport/upstream errors → `Err(AppError::UpstreamError(...))`

No changes to error types — existing `AppError` covers all cases.

## Testing

### Service Tests (in `service.rs`)

Following the pattern from `appointment/services.rs`:
1. Happy path: consultation found → eKYC available
2. Consultation not found → short-circuits eKYC call
3. eKYC not available when consultation found
4. Transport error handling

### Integration Test Update

Rename `router_with_lookup` → `router_with_service`:
```rust
#[cfg(test)]
pub fn router_with_service(service: Arc<dyn EkycServiceTrait>) -> Router {
    Router::new()
        .route("/{bookingId}/ekyc", get(handlers::get_appointment_ekyc))
        .with_state(EkycState { service })
}
```

## Files to Modify

1. `server/src/module/ekyc/mod.rs` — Router signature and wiring
2. `server/src/module/ekyc/handlers.rs` — Remove `PatientLookup`, update handler
3. `server/src/module/ekyc/service.rs` — Add `EkycService`, `EkycResult`, `EkycDetail`
4. `server/src/bootstrap.rs` — Update ekyc router initialization

## Dependencies Removed

- `FirebaseRepo` from ekyc module
- `PatientLookup` trait
- `booking_id.rs` file (date parsing no longer needed)
- Date parsing from booking_id (handled by consultation upstream)

## Dependencies Added

- `ConsultationClientTrait` from `module::appointment::external`
