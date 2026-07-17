# Appointment Detail BFF Aggregator — Design

**Date:** 2026-04-09
**Owner:** doctor-app (this repo)
**Status:** Draft for review

## Goal

Add a single doctor-facing endpoint that, given a `bookingId`, returns
everything the "appointment detail" screen needs (patient identity,
prescreen symptoms, payer/insurance, payment info) in one round-trip from
the doctor mobile app. The endpoint is a thin BFF aggregator on top of
three upstream services — it owns no persistent state.

The screen this endpoint powers shows: patient name / patient id / age /
gender / payer / insurance condition / consultation channel / primary
symptom / period of sickness / drug allergy / attachments / appointment
time and number.

**Out of scope for v1:** Body Analyzer (BMI, weight). The consultation
contract explicitly lists those as health-record service responsibilities.
A follow-up ticket will integrate the EHR/health-record service. The
frontend will hide or placeholder the Body Analyzer section in the
meantime.

## Upstream services

| Service | Endpoint | Source contract |
| --- | --- | --- |
| consultation-rs | `GET /internal/v1/appointment/{bookingId}` | `tdh-biz-doctor-apmv2/docs/plans/APPOINTMENT_DETAIL_API.md` |
| iam gatekeeper | `GET /iam/v1/internal/profile/by-account/{accountId}` | `tdh-sec-iam/.../InternalGetProfileByAccountId.scala` |
| payment | `GET /payment/transactions/{paymentTransactionId}` | `tdh-biz-payment/docs/api-contract-internal-payment-transaction-info.md` |

All three are internal-only, return HTTP 200 always, and use a top-level
`__type` discriminator for outcomes.

## Endpoint

```
GET /appointment/v1/{bookingId}
Header: tdh-sec-iam-user-identity   (DoctorIdentity extractor)
```

- Auth: `DoctorIdentity` (returns 401 missing header / 403 wrong account
  type — same as the rest of the doctor-app endpoints).
- Mounted in `bootstrap.rs::init_routers` via
  `.nest("/appointment/v1", module::appointment::router(cfg, ...))`.
- Replaces the contents of the existing `module/appointment` (the
  Firestore CRUD scaffold there is unmounted and unused — see "Module
  layout" below).

## Response

The response is a discriminated union on `__type`. HTTP status is always
200 for these variants.

### Variant: `Success`

```json
{
  "__type": "Success",
  "bookingId": "BK20220227810949",
  "appointmentNo": "20220227810949",
  "appointmentTime": { "startTime": 1645940400, "endTime": 1645941300 },
  "appointmentDate": "2022-02-27",
  "status": "BOOKED",
  "bookingType": "Schedule",
  "consultationChannel": "video",

  "patient": {
    "accountId": 124236,
    "profileId": 200,
    "fullName": "Mrs.Bunyang Lopez",
    "dateOfBirth": "1957-03-22",
    "age": 45,
    "gender": "Female"
  },

  "payment": {
    "paymentTxId": 1042,
    "paymentTxRefId": "PT-20260408-9F3A2C",
    "payerName": "AIA",
    "hasInsurance": true,
    "insuranceConditionUrl": "https://static.tdh.com/insurance/aia.html",
    "amount": 1500.00
  },

  "coupon": {
    "campaignName": "New Year Sale 2026",
    "conditionUrl": "https://static.tdh.com/coupon/new-year-sale-2026.html"
  },

  "prescreen": {
    "symptom": "...",
    "duration": 7,
    "durationUnit": "day",
    "attachments": ["att-ref-001", "att-ref-002"],
    "allergies": ["Amoxicillin"]
  }
}
```

### Variant: `AppointmentNotFound`

```json
{ "__type": "AppointmentNotFound" }
```

Returned when the consultation upstream returns its own
`appointmentNotFound` variant.

### Variant: `PatientProfileNotFound`

```json
{ "__type": "PatientProfileNotFound" }
```

Returned when the consultation booking exists, but IAM returns
`AccountNotFound` or `ProfileNotFound` for the patient identity. This is
a data-integrity case — surface it explicitly so the UI shows a clear
error state and the issue is investigable from logs, instead of
half-rendering the card.

### Field reference

| Field | Type | Notes |
| --- | --- | --- |
| `bookingId` | `string` | Echo of path param. |
| `appointmentNo` | `string` | If `bookingId` starts with `"BK"`, strip the two-character prefix; otherwise pass `bookingId` through unchanged. Used by the UI's `#20220227810949` rendering. |
| `appointmentTime.startTime` | `i64` | Epoch seconds (UTC) from consultation. |
| `appointmentTime.endTime` | `i64` | Epoch seconds (UTC) from consultation. |
| `appointmentDate` | `string` | `YYYY-MM-DD` derived from `startTime` (UTC) using `jiff`. |
| `status` | `enum` | FHIR appointment status passed through from consultation (`PROPOSED` / `PENDING` / `BOOKED` / `ARRIVED` / `FULFILLED` / `CANCELLED` / `NOSHOW` / `ENTERED_IN_ERROR`). |
| `bookingType` | `enum` | `Instant` \| `Schedule` \| `FollowUp`. |
| `consultationChannel` | `enum` | `video` \| `voice` \| `chat`. |
| `patient.accountId` | `i32` | From consultation. |
| `patient.profileId` | `i32` | From consultation. |
| `patient.fullName` | `string \| null` | `"{firstName} {lastName}"` from IAM `MorDeeUserProfileV1`, trimmed; if both names are missing the field is `null`. |
| `patient.dateOfBirth` | `string \| null` | `YYYY-MM-DD` from IAM `MorDeeUserProfileV1.dateOfBirth`. |
| `patient.age` | `i32 \| null` | Years between `dateOfBirth` and "today" (UTC), via `jiff::civil::Date`. `null` if `dateOfBirth` is missing. |
| `patient.gender` | `string \| null` | Pass-through from IAM. |
| `payment` | `Payment \| null` | `null` when payment-svc returns `NotFound` (booking not yet paid — soft-missing, render as `—`). |
| `payment.paymentTxId` | `i64` | From consultation (also echoed by payment-svc for sanity-check). |
| `payment.paymentTxRefId` | `string` | From payment-svc. |
| `payment.payerName` | `string` | See "Payer mapping" table below. |
| `payment.hasInsurance` | `bool` | True iff the upstream `selectedChannelResult` carries an `Insurance*` channel (v1, v2, or v3) on the coverage side. |
| `payment.insuranceConditionUrl` | `string \| null` | URL to a static HTML page that the mobile app opens in an in-app browser to show the insurance terms and conditions. Built from a template + insurer key (see "Insurance condition URL" below). `null` when `hasInsurance` is `false`, or when `hasInsurance` is `true` but the upstream payload is missing the insurer key needed to build the URL. |
| `payment.amount` | `decimal` | Pass-through from payment-svc `detail.amount` (THB, up to 2 decimal places). |
| `coupon` | `Coupon \| null` | Top-level field. `null` when (a) payment-svc returned `NotFound` (no successful payment yet), or (b) the upstream `couponProtocol` was `null` (no coupon applied), or (c) the upstream `campaignName` is missing or empty (nothing useful to show). The internal redemption code, tenant ids, and discount math from the upstream `couponProtocol` are intentionally **not** exposed. |
| `coupon.campaignName` | `string` | Display name of the coupon campaign, passed through unchanged from upstream `couponProtocol.campaignName` (works for both `Coupon` and `LegacyCoupon` variants). |
| `coupon.conditionUrl` | `string \| null` | URL to a static HTML page that the mobile app opens in an in-app browser to show the coupon terms and conditions. Built from a config-driven template + a slugified `campaignName` (see "Coupon condition URL" below). `null` when the slug rule produces an empty string (e.g. an all-non-alphanumeric `campaignName`). |
| `prescreen.symptom` | `string` | Free-text primary problem. |
| `prescreen.duration` | `i32` | Period of sickness, paired with `durationUnit`. |
| `prescreen.durationUnit` | `string` | E.g. `"day"`. |
| `prescreen.attachments` | `string[]` | Opaque attachment refs (URL resolution is a separate concern). |
| `prescreen.allergies` | `string[]` | Drug allergies. |

### Payer mapping

The payment service's `selectedChannelResult` is a 3-shape outer union ×
~10-shape inner union. We collapse it to the lean `payerName` /
`hasInsurance` pair:

| Upstream `selectedChannelResult.__type` | Inner channel | `payerName` | `hasInsurance` |
| --- | --- | --- | --- |
| `null` (zero-amount free flow) | — | `"Free"` | `false` |
| `SelectedChannelResult.SelfPayChannel` | any `PaymentChannelResult.{Card,PromptPay,TrueMoney,CardSchedule}` | `"Self pay"` | `false` |
| `SelectedChannelResult.CoverageChannel` | `PaymentChannelResult.Insurance` / `InsuranceV2` / `InsuranceV3` | First non-empty of: `providerName` (v3 only), then `insuranceNameI18n.en`, then `insurerCode` uppercased (v1/v2 only), then the literal `"Insurance"` | `true` |
| `SelectedChannelResult.CoverageChannel` | `PaymentChannelResult.EmployeeBenefit` / `EmployeeBenefitV2` | `companyName` if non-empty, else `"Employee Benefit"` | `false` |
| `SelectedChannelResult.CoverageChannel` | `PaymentChannelResult.CampaignLocation` | `"Campaign"` | `false` |
| `SelectedChannelResult.CoverageAndSelfPayChannel` | (use `coverageChannel` per the rows above) | as per coverage row | `true` iff coverage row is insurance |
| Unknown / future variant | — | `"Self pay"` (defensive fallback) | `false` |

Unknown variants are logged at `warn!` with the raw `__type` so we
notice when payment-svc adds something new, but they never panic the
handler.

### Insurance condition URL

When `hasInsurance` is `true`, the BFF builds `insuranceConditionUrl`
from a config-driven template. The template lives in `AppConfig` (see
"Config additions" below) and contains a single `{insurerKey}`
placeholder, e.g.:

```
https://static.tdh.com/insurance/{insurerKey}.html
```

The `insurerKey` is derived from the upstream insurance channel:

| Upstream channel `__type` | `insurerKey` source |
| --- | --- |
| `PaymentChannelResult.Insurance` (v1) | lowercase `insurerCode` |
| `PaymentChannelResult.InsuranceV2` | lowercase `insurerCode` |
| `PaymentChannelResult.InsuranceV3` | lowercase `providerAbbreviation` (v3 has no `insurerCode`) |

The lookup is case-insensitive on the way out (always lowercased) so
the static-asset bucket can use a single canonical filename per
insurer regardless of how the upstream casing varies.

If `hasInsurance` is `true` but the chosen source is missing or empty
(e.g. an `InsuranceV3` payload with `providerAbbreviation = null`), the
BFF logs a `warn!` with the upstream channel `__type` plus
`paymentTxId` and returns `insuranceConditionUrl: null` rather than
falling back to a wrong URL. The handler still succeeds — the UI just
hides the "View details" affordance for that one record.

`insuranceConditionUrl` is always `null` when `hasInsurance` is
`false`, regardless of the template config.

### Coupon condition URL

When the upstream `couponProtocol` is non-null **and** carries a
non-empty `campaignName` (true on both `CouponProtocol.Coupon` and
`CouponProtocol.LegacyCoupon` variants), the BFF builds
`coupon.conditionUrl` from a second config-driven template containing
a single `{couponKey}` placeholder, e.g.:

```
https://static.tdh.com/coupon/{couponKey}.html
```

The `couponKey` is a slug derived from `campaignName`:

1. Lowercase
2. Replace each run of non-`[a-z0-9]` characters with a single `-`
3. Trim leading/trailing `-`

Examples:

| `campaignName` | `couponKey` |
| --- | --- |
| `"New Year Sale 2026"` | `"new-year-sale-2026"` |
| `"50% OFF — Doctor's Day!"` | `"50-off-doctor-s-day"` |
| `"  TDH_Promo  "` | `"tdh-promo"` |

**Edge cases:**

- Upstream `couponProtocol` is `null` (no coupon applied) → top-level
  `coupon` is `null`. No log.
- Upstream `couponProtocol` is non-null but `campaignName` is missing or
  empty → top-level `coupon` is `null`, BFF logs a `warn!` with the
  upstream coupon `__type` and `paymentTxId` so we notice broken
  upstream data. The handler still succeeds.
- Upstream `campaignName` is non-empty but slugifies to the empty
  string (e.g. only punctuation or only Thai characters that the
  ASCII-only slug rule strips) → `coupon` is populated with
  `campaignName`, but `coupon.conditionUrl` is `null`. Logged at
  `warn!`. The UI still renders the campaign name, just hides the
  "View terms" affordance.

The slugifier is a small private helper local to the new module — not a
generic project-wide abstraction. We do **not** pull in a slug crate;
the rule above is short enough to write in ~10 lines.

## Orchestration

```
1. Consultation: GET /internal/v1/appointment/{bookingId}
   - if appointmentNotFound, short-circuit and return AppointmentNotFound
   - if success, extract patient.accountId and paymentTxId
2. Parallel via tokio::try_join!:
   a. IAM:     GET /iam/v1/internal/profile/by-account/{patient.accountId}
   b. Payment: GET /payment/transactions/{paymentTxId}
3. Map all three results into the response shape.
```

`tokio::try_join!` because IAM and payment are independent once we have
the consultation response. Each call still gets its own tracing span and
logs.

## Retry policy

Each of the three upstream calls wraps its `reqwest::send().await` in a
single transparent retry on transient transport failure: network error,
connect timeout, request timeout, or HTTP 5xx from the underlying
transport. **No retry** on:

- 4xx responses
- The upstream's own discriminated `NotFound` / `appointmentNotFound` /
  `Error` / `UnexpectedError` variants (those are domain results, not
  transport failures).

Implemented as a small `retry_once` helper local to the new module's
client files. Not a generic project-wide abstraction — YAGNI.

## Variant → BFF behaviour matrix

| Upstream | Variant / failure mode | BFF response |
| --- | --- | --- |
| consultation | `success` | continue |
| consultation | `appointmentNotFound` | `200 { "__type": "AppointmentNotFound" }` |
| consultation | network or 5xx (after 1 retry) | `502 { "error": "Upstream service unavailable: consultation" }` |
| iam | `Success` | populate `patient` sub-object |
| iam | `AccountNotFound` / `ProfileNotFound` | `200 { "__type": "PatientProfileNotFound" }` |
| iam | `Error` (decryption failure, unknown user data type, etc.) | `502 { "error": "Upstream service unavailable: iam" }` |
| iam | network or 5xx (after 1 retry) | `502 { "error": "Upstream service unavailable: iam" }` |
| payment | `Success` | populate `payment` sub-object |
| payment | `NotFound` | `payment: null` (soft-missing — booking not yet paid) |
| payment | `UnexpectedError` | `502 { "error": "Upstream service unavailable: payment" }` |
| payment | network or 5xx (after 1 retry) | `502 { "error": "Upstream service unavailable: payment" }` |

## Error response shape

HTTP 502 responses use the existing `AppError::IntoResponse` body shape
in `server/src/core/error.rs:84-86`:

```json
{ "error": "Upstream service unavailable: payment" }
```

A new `AppError::UpstreamError(String)` variant is added; the inner
string carries the upstream service name (`"consultation"` / `"iam"` /
`"payment"`).

## Observability

- The handler is wrapped in
  `tracing::info_span!("appointment_detail", booking_id = %bid, doctor_account_id = ...)`.
  The existing `gcp_logging_middleware` already produces request_id +
  Cloud Trace correlation; this span just adds the domain fields so logs
  group by booking.
- Each upstream call has its own child span:
  `consultation.get_appointment_detail`,
  `iam.get_profile_by_account`,
  `payment.get_transaction_info` — each carrying the relevant id field.
- Failures (incl. retries) emit `tracing::warn!` with the upstream
  service, the error string, and the retry attempt number (so a single
  flap is visible vs. a sustained outage).
- Successful aggregations emit one `tracing::info!` line at handler exit
  with `booking_id`, `patient.account_id`, `payment.tx_id`,
  `has_insurance`, `total_ms`. PII (name, DOB, allergies, attachments)
  is **never** logged — only ids and structural booleans.
- Spans flow through to GCP Cloud Logging via the existing OTLP exporter
  configured in `bootstrap.rs::init_telemetry` and the `GcpLogFormatter`
  in `server/src/core/logging.rs`.

## Module layout

The existing `server/src/module/appointment` is a Firestore-backed CRUD
scaffold ported from the old Scala contract. It's not mounted in
`bootstrap.rs::init_routers` (that block currently only wires
`notification`, `task`, `consultation`, `ranking`, `timeslot`), and
nothing else in the codebase depends on it. We replace its contents:

```
server/src/module/appointment/
├── mod.rs                    # router(cfg) → Router; constructs the three clients from cfg, wires AppointmentState
├── handlers.rs               # GET handler, DoctorIdentity extractor, utoipa annotations
├── models.rs                 # ApiResponse enum (Success/AppointmentNotFound/PatientProfileNotFound)
│                             # Patient, Payment, Prescreen sub-structs (the BFF response shape)
├── consultation_client.rs    # GET /internal/v1/appointment/{id} client + DTOs + retry_once
├── iam_client.rs             # IAM internal profile client + DTOs + retry_once
├── payment_client.rs         # Payment transaction client + DTOs + retry_once (incl. selectedChannelResult union types)
└── mapper.rs                 # consultation+iam+payment → ApiResponse, including the payer-mapping table above
```

The old `repo.rs` (Firestore-backed) is removed. The old request/response
types in `handlers.rs` and `models.rs` are replaced — they aren't
imported anywhere outside the module.

## Config additions

Three new URIs added to `ServiceConfig` in `server/src/config/mod.rs`:

```rust
pub consultation_internal_base_uri: String,   // CONSULTATION__INTERNAL_BASE_URI
pub iam_gatekeeper_base_uri: String,          // IAM__GATEKEEPER_BASE_URI
pub payment_internal_base_uri: String,        // PAYMENT__INTERNAL_BASE_URI
```

Two new config structs are added to `AppConfig` for the static T&C
URL templates:

```rust
#[derive(Debug, Deserialize, Clone)]
pub struct InsuranceConfig {
    /// URL template for the insurance terms-and-conditions HTML page.
    /// Must contain a single `{insurerKey}` placeholder.
    /// e.g. "https://static.tdh.com/insurance/{insurerKey}.html"
    pub condition_url_template: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CouponConfig {
    /// URL template for the coupon terms-and-conditions HTML page.
    /// Must contain a single `{couponKey}` placeholder.
    /// e.g. "https://static.tdh.com/coupon/{couponKey}.html"
    pub condition_url_template: String,
}
```

`AppConfig` gains `pub insurance: InsuranceConfig` and
`pub coupon: CouponConfig`. Env overrides:
`INSURANCE__CONDITION_URL_TEMPLATE` and
`COUPON__CONDITION_URL_TEMPLATE`. Matching keys added to
`server/config/default.toml`.

The BFF validates at startup (in `init_repos_and_services`) that each
template contains exactly one occurrence of its expected placeholder
(`{insurerKey}` for insurance, `{couponKey}` for coupon); an invalid
template fails fast rather than producing broken URLs at runtime.

Each upstream client builds its `reqwest::Client` with
`Duration::from_secs(5)` per-request timeout, matching the existing
`PrivilegeService` pattern in
`server/src/module/ranking/privilege.rs:23`.

## OpenAPI

- Handler annotated with `#[utoipa::path(get, path = "/appointment/v1/{bookingId}", ...)]`
- All response types derive `ToSchema`
- Handler path registered in `server/src/openapi.rs` under the
  `paths(...)` mod
- Tag: `appointment`

## Tests

Integration tests live in `server/tests/appointment_detail_test.rs`,
using `axum-test::TestServer` for the BFF and `wiremock` for the three
upstream services (both already dev-deps per CLAUDE.md):

1. Happy path — insurance v1 with `insurerCode = "AIA"` →
   `payerName: "AIA"`, `insuranceConditionUrl` built from template with
   `{insurerKey}` = `"aia"` (lowercased)
2. Happy path — insurance v3 with `providerAbbreviation = "ACME"` →
   `insuranceConditionUrl` built from template with `{insurerKey}` =
   `"acme"`
3. Happy path — self pay (PromptPay) → `payerName: "Self pay"`,
   `insuranceConditionUrl: null`, `coupon: null`
4. Happy path — split coverage + self pay (insurance + PromptPay) →
   `payerName` from coverage row, `hasInsurance: true`, URL built
5. Happy path — payment with non-null `couponProtocol` and
   `campaignName = "New Year Sale 2026"` → top-level `coupon` is
   `{ campaignName: "New Year Sale 2026", conditionUrl: "...new-year-sale-2026.html" }`.
   Test asserts that the upstream coupon code, tenant ids, discount
   math, etc. are **not** present in the BFF response (regression
   guard against the opaque pass-through coming back)
5a. Coupon with `campaignName = "50% OFF — Doctor's Day!"` →
    `couponKey = "50-off-doctor-s-day"` (verifies the slug rule on
    punctuation, em-dash, apostrophe, and multiple spaces)
5b. `couponProtocol` non-null but `campaignName = null` → top-level
    `coupon` is `null`, warn log captured
5c. `couponProtocol` non-null but `campaignName = "   "` → top-level
    `coupon` is `null`, warn log captured (same as 5b)
5d. `couponProtocol` non-null with non-empty `campaignName` that
    slugifies to empty (e.g. `"!!!"`) → `coupon.campaignName = "!!!"`,
    `coupon.conditionUrl = null`, warn log captured
6. Insurance v3 with `providerAbbreviation = null` →
   `hasInsurance: true`, `insuranceConditionUrl: null`, warn log
   captured (handler still 200 success)
7. Consultation `appointmentNotFound` → BFF `AppointmentNotFound`
8. IAM `AccountNotFound` → BFF `PatientProfileNotFound`
9. IAM `ProfileNotFound` → BFF `PatientProfileNotFound`
10. Payment `NotFound` → BFF Success with `payment: null` and
    `coupon: null`
11. Payment `UnexpectedError` → BFF 502, and the test asserts that
    payment-svc was called exactly once (proves we do not retry domain
    variants)
12. Network failure on IAM on the first attempt, succeeds on retry → BFF
    Success (proves transparent transport-level retry works)
13. Unknown payment channel `__type` → `payerName: "Self pay"`,
    `hasInsurance: false`, no panic
14. Age computed correctly across a leap-year DOB
15. Missing `dateOfBirth` in IAM profile → `age: null`, `dateOfBirth: null`
16. Missing both first and last name in IAM profile → `fullName: null`
17. Missing `tdh-sec-iam-user-identity` header → 401
18. Header present but not a doctor account type (`account_type != 2` for canonical doctor identities) → 403
19. Startup validation: `insurance.condition_url_template` missing the
    `{insurerKey}` placeholder → server fails to start with a clear
    error
20. Startup validation: `coupon.condition_url_template` missing the
    `{couponKey}` placeholder → server fails to start with a clear
    error

## Out of scope (call out explicitly)

- Body Analyzer (BMI, weight) — needs EHR/health-record integration,
  separate ticket
- Attachment URL resolution (the response returns opaque refs only)
- Past visit / lab results / summary note tabs (separate endpoints)
- Caching of any of the three upstream responses
- Any retry budget beyond a single transparent retry per upstream call
- Removing or repurposing the existing-but-unused
  `module/appointment/repo.rs` and Firestore wiring beyond what's
  strictly needed to land this endpoint
