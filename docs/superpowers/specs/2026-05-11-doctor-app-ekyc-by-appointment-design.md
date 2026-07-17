# Doctor App — Fetch Patient eKYC by Appointment

**Status:** Draft
**Date:** 2026-05-11
**Owner:** mordee

## Goal

Expose a new endpoint in `tdh-mordee-doctor-app` that lets the Doctor App
client fetch the patient's eKYC images (document + liveness/selfie) for a
given appointment. The doctor must be authenticated; the appointment must
belong to that doctor.

## Endpoint

```
GET /appointment/v1/{bookingId}/ekyc
Header: tdh-sec-iam-user-identity   (extracted by DoctorIdentity)
```

### Path parameters

| Name        | Type   | Notes                                         |
| ----------- | ------ | --------------------------------------------- |
| `bookingId` | string | First 6 chars are `yymmdd` (e.g. `260511...`) |

### Responses (HTTP 200, typed-variant convention)

```jsonc
// eKYC found
{ "__type": "EkycAvailable",
  "documentImageUrl": "https://...",
  "livenessImageUrl": "https://..." }

// Patient has no eKYC record on file
{ "__type": "EkycNotAvailable" }

// Appointment not found in RTDB for (doctorId, date, bookingId)
{ "__type": "AppointmentNotFound" }
```

### HTTP error responses

| Status | When                                                       |
| ------ | ---------------------------------------------------------- |
| 400    | `bookingId` is shorter than 6 chars or prefix is not yymmdd |
| 401    | Missing/invalid `tdh-sec-iam-user-identity` header         |
| 502    | Eagle/RTDB call failed after retries                        |

## Flow

1. Extract `doctorAccountId` from `DoctorIdentity` (`account_type == 2`).
2. Parse `bookingId[0..6]` → `YYYY-MM-DD` (assume 20yy). Reject 400 on
   parse failure.
3. RTDB GET
   `/appointments/{doctorAccountId}/{date}/{bookingId}/patientAccountId`
   via existing `FirebaseRepo::get`.
   - Result `None` → return `AppointmentNotFound`.
   - Result must deserialize as `i32`; otherwise 500.
4. Call eagle `GET {service.eagle_base_uri}/v1/user/kyc-info/account/{patientAccountId}`.
   - Eagle returns `GetKycUserInfoResponse` (`__type` = `ValidKycUserInfo` |
     `NoKycUserInfo`).
   - Map `ValidKycUserInfo` → `EkycAvailable` with
     `documentImageUrl = ekyc_session_result.document_image_url`,
     `livenessImageUrl = ekyc_session_result.selfie_image_url`.
   - Map `NoKycUserInfo` → `EkycNotAvailable`.
5. Eagle 5xx / network / timeout → `AppError::InternalError` (HTTP 502 via
   `IntoResponse`).

## New code layout

```
server/src/module/ekyc/
  mod.rs        # router(cfg, firebase) -> Router
  handlers.rs   # GET /{bookingId}/ekyc handler + EkycState
  service.rs    # EkycClient (reqwest) + DTO types for eagle response
  booking_id.rs # parse_date_from_booking_id(&str) -> AppResult<String>
```

Mounted in `bootstrap.rs` alongside the existing appointment router:

```rust
.nest("/appointment/v1", module::ekyc::router(&cfg, repos.firebase.clone()))
```

(The new sub-router only declares `/{bookingId}/ekyc`; existing
`appointment` router keeps `/{bookingId}`. Axum allows merging two
routers under the same nest prefix when paths don't collide; if it does,
fall back to a single appointment router that also owns the eKYC
endpoint.)

## Configuration

Add to `ServiceConfig` in `server/src/config/mod.rs`:

```rust
pub eagle_base_uri: String,
```

`server/config/default.toml`, `local.toml`, `example.local.toml`:

```toml
[service]
eagle_base_uri = "http://localhost:9100"
```

Production override via env: `SERVICE__EAGLE_BASE_URI`.

## EkycClient

- `reqwest::Client` with 10s timeout (mirror `PatientService`).
- Method: `async fn fetch_by_account_id(&self, account_id: i32) -> AppResult<EkycInfo>`
  where `EkycInfo` is one of `Available { document_url, liveness_url }`
  / `NotAvailable`.
- DTO mirrors eagle's `GetKycUserInfoResponse` enum with
  `#[serde(tag = "__type")]`. Only the fields we need are deserialized.

## OpenAPI

- Add `#[utoipa::path(get, path = "/appointment/v1/{bookingId}/ekyc", ...)]`
  to the handler.
- New `ToSchema` types: `EkycResponse` (tagged enum with the three variants).
- Register in `server/src/openapi.rs` under `paths(...)` and `schemas(...)`.

## Testing

`server/tests/ekyc_test.rs` using `axum-test` + `wiremock`:

1. **Happy path:** RTDB returns `42`; eagle returns ValidKycUserInfo →
   200 `EkycAvailable` with both URLs.
2. **No eKYC:** RTDB returns `42`; eagle returns NoKycUserInfo → 200
   `EkycNotAvailable`.
3. **Unknown appointment:** RTDB returns null → 200 `AppointmentNotFound`.
4. **Bad bookingId:** `"abc"` → 400.
5. **Eagle 5xx:** wiremock 503 → 502.

`FirebaseRepo` is concrete (no trait), so the test will inject a small
trait abstraction `PatientLookup` over the RTDB read with two impls:
prod (wraps `FirebaseRepo`) and a `MockPatientLookup` for tests. This
keeps the test boundary tight without rewriting `FirebaseRepo`.

Unit test for `parse_date_from_booking_id` covering: valid `260511...`,
short string, non-digit prefix, invalid month/day.

## Out of scope (YAGNI)

- Image proxying / re-signing — client fetches eagle-hosted URLs directly.
- Caching of eKYC responses.
- Pagination, history, multiple eKYC sessions per patient.
- Liveness vs document URL fallback when one is missing — return what
  eagle returns; empty strings allowed.

## Risks / open questions

- Eagle URLs may be GCS signed URLs with limited TTL; client must fetch
  promptly. Document this in API description.
- Authorization: relying on RTDB path scoped by `doctorAccountId` to
  enforce that doctor owns the appointment. If a doctor probes with
  another doctor's bookingId, RTDB returns null → `AppointmentNotFound`,
  which is acceptable.
