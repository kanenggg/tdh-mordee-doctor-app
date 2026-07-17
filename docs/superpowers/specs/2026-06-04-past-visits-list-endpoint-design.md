# Past Visits List Endpoint Design

**Date:** 2026-06-04
**Status:** Draft
**Module:** Appointment

## Context

The existing `GET /appointment/v1/{bookingId}/past-visit` endpoint returns a single past visit detail for a specific booking. This is used when viewing details of one particular past appointment.

This design adds a new endpoint that returns a **list** of all past visits for a patient, accessed directly by patient account ID. This is primarily for the doctor app to display a patient's complete medical history before a consultation.

**Route Design Decision:** The endpoint uses `/by-patient/{patientAccountId}/past-visits` instead of `/{patientAccountId}/past-visits` to avoid routing conflicts with the existing `/{bookingId}` route. Both patterns would be single-segment wildcards in Axum, causing ambiguity. The `/by-patient/` prefix clearly distinguishes patient-based queries from booking-based queries.

## Requirements

### Functional Requirements

1. Return all past visits for a given patient account ID
2. No pagination - return complete list
3. Use same data source as existing endpoint (Qolphin service)
4. Support `DoctorIdentity` authentication

### Non-Functional Requirements

1. Follow existing module patterns (handler → service → external client)
2. Consistent error handling (HTTP 200 with typed errors)
3. OpenAPI documentation
4. Integration test coverage

## Design

### Endpoint Definition

```
GET /appointment/v1/by-patient/{patientAccountId}/past-visits
```

**Path Parameters:**
- `patientAccountId` (string): Patient's account ID (will be parsed to i32 for Qolphin)

**Response:** JSON object with `pastVisits` array

### Architecture

```
Handler (handlers.rs)
    │
    ├─ Extract: DoctorIdentity, Path(patientAccountId)
    │
    └─ Service.get_patient_past_visits_list(patientAccountId)
           │
           └─ QolphinClient.get_past_visits(patientAccountId)
                  │
                  └─ External Qolphin Service
```

### Component Details

#### 1. Router (`module/appointment/mod.rs`)

Add new route to existing router:
```rust
let router = Router::new()
    .route("/{bookingId}", get(handlers::get_appointment_detail))
    .route("/{bookingId}/past-visit", get(handlers::get_past_visit))
    .route("/by-patient/{patientAccountId}/past-visits", get(handlers::get_past_visits_list))
    .with_state(state);
```

**Note:** The `/by-patient/` prefix avoids routing conflicts with `/{bookingId}`.

#### 2. Handler (`module/appointment/handlers.rs`)

```rust
/// `GET /appointment/v1/by-patient/{patientAccountId}/past-visits`
#[utoipa::path(
    get,
    path = "/appointment/v1/by-patient/{patientAccountId}/past-visits",
    tag = "appointment",
    params(
        ("patientAccountId" = String, Path, description = "Patient account ID")
    ),
    responses(
        (status = 200, description = "Success", body = PastVisitsListResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden (account_type != 2|3)"),
        (status = 502, description = "Upstream service unavailable"),
    ),
    security(("TDH-SEC-IAM-USER-IDENTITY" = []))
)]
#[instrument(
    name = "past_visits_list",
    skip(state),
    fields(patient_account_id = %patient_account_id, doctor_account_id = %doctor_identity.doctor_account_id)
)]
pub async fn get_past_visits_list(
    State(state): State<AppointmentState>,
    doctor_identity: DoctorIdentity,
    Path(patient_account_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    // Parse String to i32 for Qolphin client
    let patient_id_i32: i32 = patient_account_id.parse()
        .map_err(|_| AppError::BadRequest(format!("Invalid patientAccountId: {}", patient_account_id)))?;

    let past_visits = state.service.get_patient_past_visits_list(patient_id_i32).await?;

    Ok(Json(PastVisitsListResponse { past_visits }))
}
```

**Key change:** The path parameter is `String` but must be parsed to `i32` for the Qolphin client.

#### 3. Service (`module/appointment/services.rs`)

Add to `AppointmentServiceTrait`:
```rust
async fn get_patient_past_visits_list(
    &self,
    patient_account_id: i32,
) -> AppResult<Vec<serde_json::Value>>;
```

Implementation - direct Qolphin call:
```rust
async fn get_patient_past_visits_list(
    &self,
    patient_account_id: i32,
) -> AppResult<Vec<serde_json::Value>> {
    self.qolphin.get_past_visits(patient_account_id).await
}
```

**Note:** Returns raw `Vec<serde_json::Value>` from Qolphin, matching existing passthrough pattern.

#### 4. Qolphin Client (`module/appointment/external/qolphin_client.rs`)

**No changes needed.** The existing `get_past_visits(user_account_id: i32)` method already provides the required functionality. It returns `Vec<serde_json::Value>` representing the raw Qolphin response.

#### 5. Response Model (`module/appointment/models.rs`)

**Reuse existing types.** Add a simple response wrapper:

```rust
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PastVisitsListResponse {
    /// Passthrough array from qolphin (all past visits for the patient).
    #[schema(value_type = Vec<Object>)]
    pub past_visits: Vec<serde_json::Value>,
}
```

**Response Format:**
```json
{
  "pastVisits": [
    {
      "__type": "PastVisit",
      "bookingId": "string",
      "consultationStartTime": 1234567890,
      "consultationEndTime": 1234567890,
      "doctorInfo": {
        "doctorName": "Dr. Example",
        "doctorSpecialty": "General Practice",
        "doctorImageUrl": "https://..."
      }
    }
  ]
}
```

**Note:** This reuses the existing passthrough pattern from `PastVisitsResponse`. The raw JSON array from Qolphin is passed through without intermediate parsing.

#### 6. OpenAPI Registration (`server/src/openapi.rs`)

The `#[utoipa::path]` attribute is already added to the handler function (see Handler section above).

Add the handler path to the `paths!` macro in `/server/src/openapi.rs`:

```rust
paths!(
    // ... existing paths ...
    crate::module::appointment::handlers::get_past_visits_list,
)
```

### Authentication

**Extractor:** `DoctorIdentity` from `core/user_identity.rs`

- Reads `tdh-sec-iam-user-identity` header
- Validates `account_type == 2` (doctor) or `account_type == 3` (legacy)
- No additional authorization - authenticated doctors can view any patient's history

### Error Handling

| Scenario | Response Type | HTTP Status |
|----------|---------------|-------------|
| Success (visits found) | `PastVisitsListResponse` with array | 200 |
| No past visits | `PastVisitsListResponse` with empty array | 200 |
| Qolphin timeout/error | `AppError::UpstreamError` → 502 | 502 |
| Invalid patientAccountId (non-numeric) | `AppError::BadRequest` → 400 | 400 |

**Note:** Unlike the existing `/{bookingId}/past-visit` endpoint which returns domain errors as HTTP 200 with typed variants, this endpoint uses standard HTTP status codes for simplicity since it directly calls Qolphin without consultation lookup.

## Testing

### Integration Test (`server/tests/appointment_test.rs`)

Create new test file following the project's test patterns:

```rust
use axum_test::TestServer;
use serde_json::json;

#[tokio::test]
async fn test_get_past_visits_list_returns_visits() {
    // Given
    let server = build_test_server();
    let patient_id = "124236";

    // When
    let response = server
        .get(&format!("/appointment/v1/by-patient/{}/past-visits", patient_id))
        .add_header("tdh-sec-iam-user-identity", doctor_identity_header())
        .await;

    // Then
    response.assert_status_ok();
    let body: serde_json::Value = response.json();
    assert!(body["pastVisits"].is_array());
}

#[tokio::test]
async fn test_get_past_visits_list_with_invalid_patient_id() {
    // Given
    let server = build_test_server();

    // When
    let response = server
        .get("/appointment/v1/by-patient/invalid/past-visits")
        .add_header("tdh-sec-iam-user-identity", doctor_identity_header())
        .await;

    // Then
    response.assert_status_bad_request();
}
```

### Test Cases

1. **Happy path:** Patient with past visits returns non-empty list
2. **Empty result:** Patient with no history returns empty array
3. **Invalid patientAccountId:** Non-numeric ID returns 400 BadRequest
4. **Authentication:** Missing/invalid header returns 401 (from extractor)

## Implementation Checklist

- [ ] Add `PastVisitsListResponse` model to `models.rs`
- [ ] Add `get_patient_past_visits_list(i32)` to `AppointmentServiceTrait` in `services.rs`
- [ ] Implement method in `AppointmentService`
- [ ] Add `get_past_visits_list()` handler to `handlers.rs` with `#[utoipa::path]` attribute
- [ ] Add route `/by-patient/{patientAccountId}/past-visits` in `mod.rs`
- [ ] Register handler path in `/server/src/openapi.rs` `paths!` macro
- [ ] Create `/server/tests/appointment_test.rs` with integration tests

## Files to Modify

| File | Changes |
|------|---------|
| `server/src/module/appointment/models.rs` | Add `PastVisitsListResponse` |
| `server/src/module/appointment/services.rs` | Add `get_patient_past_visits_list()` to trait + implementation |
| `server/src/module/appointment/handlers.rs` | Add `get_past_visits_list()` handler |
| `server/src/module/appointment/mod.rs` | Add route |
| `server/src/openapi.rs` | Register path in `paths!` macro |
| `server/tests/appointment_test.rs` | **Create new test file** with integration tests |

**No changes needed:**
- `server/src/module/appointment/external/qolphin_client.rs` - reuse existing `get_past_visits(i32)`

## Dependencies

### External Services
- **Qolphin Service:** Must be accessible for `GET /v1/internal/past-visit?userAccountId={id}`

### Internal Dependencies
- Existing `DoctorIdentity` extractor
- Existing `QolphinClient` infrastructure
- Existing error handling patterns
