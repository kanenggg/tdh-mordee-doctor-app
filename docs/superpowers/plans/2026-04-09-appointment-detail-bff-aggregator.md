# Appointment Detail BFF Aggregator Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `GET /appointment/v1/{bookingId}` to the doctor-app — a thin BFF aggregator that fetches the consultation booking, the patient's IAM profile, and the payment record in parallel, then composes them into a single response that powers the doctor's "appointment detail" screen.

**Architecture:** Replace the contents of the existing-but-unmounted `module/appointment` (Firestore CRUD scaffold) with three thin upstream HTTP clients (consultation, IAM, payment), one mapper that collapses the payment service's tagged-union `selectedChannelResult` into a lean payer struct, and a single Axum handler that does `tokio::try_join!` after the consultation call returns. New `InsuranceConfig` and `CouponConfig` carry T&C URL templates.

**Tech Stack:** Rust, Axum 0.8, reqwest 0.12, tokio 1, jiff 0.2 (date math), serde / serde_json, utoipa 5 (OpenAPI), tracing (GCP-correlated logs), wiremock + axum-test (integration tests).

**Spec:** `docs/superpowers/specs/2026-04-09-appointment-detail-bff-design.md` — read it before starting; this plan implements that spec exactly.

---

## File Map

| File | Action | Responsibility |
|------|--------|----------------|
| `server/src/module/appointment/mod.rs` | Replace | New `router(cfg)` that constructs the three clients and `AppointmentState`; module exports |
| `server/src/module/appointment/handlers.rs` | Replace | Single GET handler with `DoctorIdentity` extractor + `#[utoipa::path]` annotation |
| `server/src/module/appointment/models.rs` | Replace | BFF response shape: `ApiResponse` enum, `Patient`, `Payment`, `Coupon`, `Prescreen` structs |
| `server/src/module/appointment/consultation_client.rs` | Create | `ConsultationClient` + upstream DTO + `retry_once` |
| `server/src/module/appointment/iam_client.rs` | Create | `IamClient` + upstream DTO (`MorDeeUserProfileV1` shape) + `retry_once` |
| `server/src/module/appointment/payment_client.rs` | Create | `PaymentClient` + upstream DTOs incl. `selectedChannelResult` tagged union + `retry_once` |
| `server/src/module/appointment/mapper.rs` | Create | Pure functions: `derive_appointment_no`, `compute_age`, `slugify_campaign`, `extract_payer`, `build_insurance_url`, `build_coupon_url`, plus the top-level `compose` |
| `server/src/module/appointment/repo.rs` | Delete | Old Firestore CRUD scaffold — unused, replaced by HTTP clients |
| `server/src/config/mod.rs` | Modify | Add three URIs to `ServiceConfig`; add new `InsuranceConfig`, `CouponConfig`; add to `AppConfig` |
| `server/src/core/error.rs` | Modify | Add `AppError::UpstreamError(String)` variant returning HTTP 502 |
| `server/src/bootstrap.rs` | Modify | Wire `module::appointment::router(&cfg)` into `init_routers`; validate URL templates at startup |
| `server/src/module/mod.rs` | (no change) | `pub mod appointment;` already present |
| `server/src/openapi.rs` | Modify | Register handler path and response schemas |
| `server/config/default.toml` | Modify | Add the new config keys with placeholder local values |
| `server/tests/appointment_detail_test.rs` | Create | All 20 integration tests using `wiremock` for upstream services and `axum-test` for the BFF |

---

## Task 1: Delete the old Firestore scaffold

The existing `module/appointment/repo.rs` is a Firestore-backed CRUD scaffold ported from the old Scala contract. It's not mounted in `bootstrap.rs::init_routers` and nothing else in the codebase imports `AppointmentRepoTrait`. We delete it before doing anything else so we have a clean slate.

**Files:**
- Delete: `server/src/module/appointment/repo.rs`
- Modify: `server/src/module/appointment/mod.rs`

- [ ] **Step 1: Verify nothing imports the old repo trait**

Run:
```bash
grep -rn "AppointmentRepoTrait\|appointment::repo" /Users/peelz/Workspace/doctor-apm/tdh-mordee-doctor-app/server/src /Users/peelz/Workspace/doctor-apm/tdh-mordee-doctor-app/server/tests 2>/dev/null
```
Expected: only matches inside `server/src/module/appointment/` itself (mod.rs, handlers.rs). If anything else matches, stop and investigate before continuing.

- [ ] **Step 2: Delete the old repo file**

```bash
rm /Users/peelz/Workspace/doctor-apm/tdh-mordee-doctor-app/server/src/module/appointment/repo.rs
```

- [ ] **Step 3: Stub out mod.rs so the workspace still compiles**

Replace the entire contents of `server/src/module/appointment/mod.rs` with a minimal placeholder. Tasks 6 and 11 will fill it in.

```rust
//! BFF aggregator for the doctor "appointment detail" screen.
//!
//! This module is being rewritten — see
//! docs/superpowers/specs/2026-04-09-appointment-detail-bff-design.md
//! and docs/superpowers/plans/2026-04-09-appointment-detail-bff-aggregator.md.
```

- [ ] **Step 4: Stub out handlers.rs and models.rs**

Replace the entire contents of `server/src/module/appointment/handlers.rs` with:
```rust
//! Replaced by the BFF aggregator — see mod.rs.
```

Replace the entire contents of `server/src/module/appointment/models.rs` with:
```rust
//! Replaced by the BFF aggregator — see mod.rs.
```

- [ ] **Step 5: Run cargo check to confirm the workspace still compiles**

Run from `/Users/peelz/Workspace/doctor-apm/tdh-mordee-doctor-app`:
```bash
cargo check -p server
```
Expected: no errors. Warnings about unused files are OK.

- [ ] **Step 6: Commit**

```bash
git add server/src/module/appointment/
git commit -m "chore: clear out old appointment Firestore scaffold

Removes the unmounted Firestore CRUD module so the new BFF aggregator
can be built on a clean slate. See spec
docs/superpowers/specs/2026-04-09-appointment-detail-bff-design.md."
```

---

## Task 2: Add `AppError::UpstreamError` variant

The handler needs to return HTTP 502 when any upstream fails after retry. Add a new error variant.

**Files:**
- Modify: `server/src/core/error.rs`

- [ ] **Step 1: Read the current `AppError` enum**

Read `server/src/core/error.rs` end-to-end so you understand the existing enum, the `IntoResponse` impl, and the body shape (`{ "error": "..." }`).

- [ ] **Step 2: Add the new variant**

In `server/src/core/error.rs`, add this variant inside the `AppError` enum, just above the `#[error(transparent)]` `ReqwestError` line:

```rust
    #[error("Upstream service unavailable: {0}")]
    UpstreamError(String),
```

- [ ] **Step 3: Add the matching arm in `IntoResponse`**

In the same file, inside `impl IntoResponse for AppError`, add this match arm just above the `AppError::ReqwestError(e)` arm:

```rust
            AppError::UpstreamError(svc) => (
                StatusCode::BAD_GATEWAY,
                format!("Upstream service unavailable: {}", svc),
            ),
```

- [ ] **Step 4: Compile**

```bash
cargo check -p server
```
Expected: no errors.

- [ ] **Step 5: Commit**

```bash
git add server/src/core/error.rs
git commit -m "feat(error): add UpstreamError(String) variant returning 502

Used by the appointment-detail BFF aggregator for transparent transport
failures from any of the three upstream services."
```

---

## Task 3: Add config structs and `default.toml` keys

Add the three new service URIs and the two URL-template config structs.

**Files:**
- Modify: `server/src/config/mod.rs`
- Modify: `server/config/default.toml`

- [ ] **Step 1: Add the three URIs to `ServiceConfig`**

In `server/src/config/mod.rs`, locate the `ServiceConfig` struct (around line 17) and add three new fields after `biz_apm_base_uri`:

```rust
    pub consultation_internal_base_uri: String,
    pub iam_gatekeeper_base_uri: String,
    pub payment_internal_base_uri: String,
```

- [ ] **Step 2: Add `InsuranceConfig` and `CouponConfig` structs**

In `server/src/config/mod.rs`, just before the `AppConfig` struct definition (around line 374), add:

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

- [ ] **Step 3: Add the structs to `AppConfig`**

In the same file, add these two fields to the `AppConfig` struct (alphabetically grouped near the other domain configs):

```rust
    pub insurance: InsuranceConfig,
    pub coupon: CouponConfig,
```

- [ ] **Step 4: Add the matching keys to `default.toml`**

In `server/config/default.toml`, find the `[service]` block and append three new lines after `biz_apm_base_uri`:

```toml
consultation_internal_base_uri = "http://localhost:9100"
iam_gatekeeper_base_uri = "http://localhost:9101"
payment_internal_base_uri = "http://localhost:9102"
```

Then add two new top-level blocks at the end of the file:

```toml
[insurance]
# Override with INSURANCE__CONDITION_URL_TEMPLATE in production.
# Must contain a single {insurerKey} placeholder.
condition_url_template = "https://static.tdh.example/insurance/{insurerKey}.html"

[coupon]
# Override with COUPON__CONDITION_URL_TEMPLATE in production.
# Must contain a single {couponKey} placeholder.
condition_url_template = "https://static.tdh.example/coupon/{couponKey}.html"
```

- [ ] **Step 5: Compile**

```bash
cargo check -p server
```
Expected: no errors.

- [ ] **Step 6: Commit**

```bash
git add server/src/config/mod.rs server/config/default.toml
git commit -m "feat(config): add upstream URIs and T&C URL templates

Adds three internal-service base URIs and two URL-template configs
(InsuranceConfig, CouponConfig) for the appointment-detail BFF
aggregator."
```

---

## Task 4: Implement `models.rs` — BFF response shape

Define the response types as a discriminated union mirroring the spec's "Response" section. We define them now (without the handler) so the upstream-client tasks can build against the final wire shape.

**Files:**
- Modify (replace): `server/src/module/appointment/models.rs`
- Modify: `server/src/module/appointment/mod.rs`

- [ ] **Step 1: Wire `pub mod models;` into `mod.rs`**

Replace the comment-only stub from Task 1 in `server/src/module/appointment/mod.rs` with:

```rust
//! BFF aggregator for the doctor "appointment detail" screen.
//!
//! This module is being rewritten — see
//! docs/superpowers/specs/2026-04-09-appointment-detail-bff-design.md
//! and docs/superpowers/plans/2026-04-09-appointment-detail-bff-aggregator.md.

pub mod models;
```

(Subsequent tasks will add `pub mod mapper;`, `pub mod consultation_client;`, etc. as those files are created.)

- [ ] **Step 2: Replace `models.rs` with the response shape**

Replace the entire contents of `server/src/module/appointment/models.rs` with:

```rust
//! Response types for `GET /appointment/v1/{bookingId}`.
//!
//! Mirrors the discriminated-union shape in
//! docs/superpowers/specs/2026-04-09-appointment-detail-bff-design.md.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Top-level discriminated response for the appointment-detail endpoint.
#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "__type")]
pub enum ApiResponse {
    #[serde(rename = "Success")]
    Success(Box<SuccessBody>),
    #[serde(rename = "AppointmentNotFound")]
    AppointmentNotFound,
    #[serde(rename = "PatientProfileNotFound")]
    PatientProfileNotFound,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SuccessBody {
    pub booking_id: String,
    pub appointment_no: String,
    pub appointment_time: AppointmentTime,
    /// `YYYY-MM-DD` derived from `appointment_time.start_time` (UTC).
    pub appointment_date: String,
    /// FHIR appointment status, passed through from consultation.
    pub status: String,
    /// `Instant` | `Schedule` | `FollowUp`.
    pub booking_type: String,
    /// `video` | `voice` | `chat`.
    pub consultation_channel: String,
    pub patient: Patient,
    /// `null` when payment-svc returned `NotFound` (no successful payment yet).
    pub payment: Option<Payment>,
    /// `null` when (a) payment is null, (b) upstream couponProtocol is null,
    /// or (c) upstream campaignName is missing or empty.
    pub coupon: Option<Coupon>,
    pub prescreen: Prescreen,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AppointmentTime {
    pub start_time: i64,
    pub end_time: i64,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Patient {
    pub account_id: i32,
    pub profile_id: i32,
    pub full_name: Option<String>,
    pub date_of_birth: Option<String>,
    pub age: Option<i32>,
    pub gender: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Payment {
    pub payment_tx_id: i64,
    pub payment_tx_ref_id: String,
    pub payer_name: String,
    pub has_insurance: bool,
    pub insurance_condition_url: Option<String>,
    /// THB total. Serialized as a JSON number.
    pub amount: serde_json::Number,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Coupon {
    pub campaign_name: String,
    pub condition_url: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Prescreen {
    pub symptom: String,
    pub duration: i32,
    pub duration_unit: String,
    pub attachments: Vec<String>,
    pub allergies: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appointment_not_found_serializes_with_discriminator_only() {
        let v = ApiResponse::AppointmentNotFound;
        let s = serde_json::to_string(&v).unwrap();
        assert_eq!(s, r#"{"__type":"AppointmentNotFound"}"#);
    }

    #[test]
    fn patient_profile_not_found_serializes_with_discriminator_only() {
        let v = ApiResponse::PatientProfileNotFound;
        let s = serde_json::to_string(&v).unwrap();
        assert_eq!(s, r#"{"__type":"PatientProfileNotFound"}"#);
    }

    #[test]
    fn success_serializes_camel_case_fields() {
        let v = ApiResponse::Success(Box::new(SuccessBody {
            booking_id: "BK20220227810949".to_string(),
            appointment_no: "20220227810949".to_string(),
            appointment_time: AppointmentTime {
                start_time: 1645940400,
                end_time: 1645941300,
            },
            appointment_date: "2022-02-27".to_string(),
            status: "BOOKED".to_string(),
            booking_type: "Schedule".to_string(),
            consultation_channel: "video".to_string(),
            patient: Patient {
                account_id: 124236,
                profile_id: 200,
                full_name: Some("Mrs.Bunyang Lopez".to_string()),
                date_of_birth: Some("1957-03-22".to_string()),
                age: Some(45),
                gender: Some("Female".to_string()),
            },
            payment: None,
            coupon: None,
            prescreen: Prescreen {
                symptom: "headache".to_string(),
                duration: 7,
                duration_unit: "day".to_string(),
                attachments: vec![],
                allergies: vec![],
            },
        }));
        let json: serde_json::Value = serde_json::to_value(&v).unwrap();
        // Spot-check key field names use camelCase, not snake_case.
        assert_eq!(json["__type"], "Success");
        assert_eq!(json["bookingId"], "BK20220227810949");
        assert_eq!(json["appointmentNo"], "20220227810949");
        assert_eq!(json["appointmentTime"]["startTime"], 1645940400);
        assert_eq!(json["patient"]["fullName"], "Mrs.Bunyang Lopez");
        assert_eq!(json["payment"], serde_json::Value::Null);
        assert_eq!(json["coupon"], serde_json::Value::Null);
    }
}
```

- [ ] **Step 3: Run the unit tests**

```bash
cargo test -p server --lib module::appointment::models
```
Expected: 3 tests pass.

- [ ] **Step 4: Commit**

```bash
git add server/src/module/appointment/mod.rs server/src/module/appointment/models.rs
git commit -m "feat(appointment): add BFF response types

Discriminated-union ApiResponse with Success / AppointmentNotFound /
PatientProfileNotFound variants. Sub-structs Patient, Payment, Coupon,
Prescreen mirror the spec wire shape (camelCase via rename_all).
Includes serde unit tests as a regression guard against snake_case
leakage."
```

---

## Task 5: Implement `mapper.rs` — pure helper functions

The mapper holds the pure business logic so it can be unit-tested without spinning up wiremock servers. Each helper has its own focused tests.

**Files:**
- Create: `server/src/module/appointment/mapper.rs`

- [ ] **Step 1: Write the failing tests for `derive_appointment_no`**

Create `server/src/module/appointment/mapper.rs` with:

```rust
//! Pure helper functions used by the BFF handler to compose its
//! response from the three upstream payloads.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appointment_no_strips_bk_prefix() {
        assert_eq!(derive_appointment_no("BK20220227810949"), "20220227810949");
    }

    #[test]
    fn appointment_no_passes_through_when_no_bk_prefix() {
        assert_eq!(derive_appointment_no("XX20220227"), "XX20220227");
        assert_eq!(derive_appointment_no("20220227"), "20220227");
    }

    #[test]
    fn appointment_no_handles_short_strings() {
        assert_eq!(derive_appointment_no("BK"), "");
        assert_eq!(derive_appointment_no("B"), "B");
        assert_eq!(derive_appointment_no(""), "");
    }
}
```

- [ ] **Step 2: Run the failing tests**

```bash
cargo test -p server --lib module::appointment::mapper::tests::appointment_no
```
Expected: compile error / not-found error for `derive_appointment_no`.

- [ ] **Step 3: Implement `derive_appointment_no`**

Add at the top of `server/src/module/appointment/mapper.rs`, above the `#[cfg(test)] mod tests` block:

```rust
/// Strip the leading two-character `"BK"` booking prefix if present;
/// otherwise pass the input through unchanged.
pub fn derive_appointment_no(booking_id: &str) -> String {
    if let Some(rest) = booking_id.strip_prefix("BK") {
        rest.to_string()
    } else {
        booking_id.to_string()
    }
}
```

- [ ] **Step 4: Run the tests**

```bash
cargo test -p server --lib module::appointment::mapper::tests::appointment_no
```
Expected: 3 pass.

- [ ] **Step 5: Write failing tests for `compute_age`**

Append to the `tests` mod in `mapper.rs`:

```rust
    use jiff::civil::date;

    #[test]
    fn age_basic() {
        let dob = date(1957, 3, 22);
        let today = date(2002, 4, 1);
        assert_eq!(compute_age(dob, today), 45);
    }

    #[test]
    fn age_birthday_not_yet_reached() {
        let dob = date(1957, 3, 22);
        let today = date(2002, 3, 21);
        assert_eq!(compute_age(dob, today), 44);
    }

    #[test]
    fn age_birthday_today() {
        let dob = date(1957, 3, 22);
        let today = date(2002, 3, 22);
        assert_eq!(compute_age(dob, today), 45);
    }

    #[test]
    fn age_leap_year_dob() {
        // Born on a leap day. On a non-leap year, the birthday is treated as
        // March 1 (i.e. one day after Feb 28). Compute strictly by month/day.
        let dob = date(1996, 2, 29);
        // Feb 28, 2026 — birthday not yet reached.
        assert_eq!(compute_age(dob, date(2026, 2, 28)), 29);
        // Mar 1, 2026 — birthday reached.
        assert_eq!(compute_age(dob, date(2026, 3, 1)), 30);
        // Feb 29, 2024 (leap year) — birthday reached.
        assert_eq!(compute_age(dob, date(2024, 2, 29)), 28);
    }
```

- [ ] **Step 6: Run the failing tests**

```bash
cargo test -p server --lib module::appointment::mapper::tests::age
```
Expected: compile error for `compute_age`.

- [ ] **Step 7: Implement `compute_age`**

Add to `server/src/module/appointment/mapper.rs` (above the `tests` mod):

```rust
use jiff::civil::Date;

/// Compute integer age in completed years.
///
/// Returns `today.year - dob.year`, minus 1 if today's `(month, day)` is
/// before the dob's `(month, day)`. Strict month/day comparison handles
/// leap-day birthdays correctly: a Feb-29 dob "reaches" age N+1 on
/// March 1 of non-leap years.
pub fn compute_age(dob: Date, today: Date) -> i32 {
    let mut age = today.year() as i32 - dob.year() as i32;
    let today_md = (today.month(), today.day());
    let dob_md = (dob.month(), dob.day());
    if today_md < dob_md {
        age -= 1;
    }
    age
}
```

- [ ] **Step 8: Run the age tests**

```bash
cargo test -p server --lib module::appointment::mapper::tests::age
```
Expected: 4 pass.

- [ ] **Step 9: Write failing tests for `slugify_campaign`**

Append to the `tests` mod:

```rust
    #[test]
    fn slug_basic() {
        assert_eq!(slugify_campaign("New Year Sale 2026"), "new-year-sale-2026");
    }

    #[test]
    fn slug_punctuation_em_dash_apostrophe() {
        assert_eq!(
            slugify_campaign("50% OFF — Doctor's Day!"),
            "50-off-doctor-s-day"
        );
    }

    #[test]
    fn slug_underscores_and_padding() {
        assert_eq!(slugify_campaign("  TDH_Promo  "), "tdh-promo");
    }

    #[test]
    fn slug_only_punctuation_returns_empty() {
        assert_eq!(slugify_campaign("!!!"), "");
        assert_eq!(slugify_campaign("---"), "");
    }

    #[test]
    fn slug_thai_characters_strip_to_empty() {
        assert_eq!(slugify_campaign("โปรปีใหม่"), "");
    }

    #[test]
    fn slug_collapses_multiple_separators() {
        assert_eq!(slugify_campaign("a   b___c"), "a-b-c");
    }
```

- [ ] **Step 10: Run failing tests**

```bash
cargo test -p server --lib module::appointment::mapper::tests::slug
```
Expected: compile error for `slugify_campaign`.

- [ ] **Step 11: Implement `slugify_campaign`**

Add to `mapper.rs`:

```rust
/// Slugify a campaign name to fit a `{couponKey}` URL placeholder.
///
/// 1. Lowercase
/// 2. Replace each run of non-`[a-z0-9]` characters with a single `-`
/// 3. Trim leading/trailing `-`
///
/// Returns an empty string if no `[a-z0-9]` characters remain.
pub fn slugify_campaign(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_dash = true; // start "true" so leading separators don't emit
    for ch in s.chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() {
            out.push(lower);
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    // Trim trailing dash if any.
    if out.ends_with('-') {
        out.pop();
    }
    out
}
```

- [ ] **Step 12: Run the slug tests**

```bash
cargo test -p server --lib module::appointment::mapper::tests::slug
```
Expected: 6 pass.

- [ ] **Step 13: Write failing tests for `build_url_from_template`**

Append to the `tests` mod:

```rust
    #[test]
    fn url_template_substitutes_placeholder() {
        let url = build_url_from_template(
            "https://static.tdh.com/insurance/{insurerKey}.html",
            "{insurerKey}",
            "aia",
        );
        assert_eq!(url, "https://static.tdh.com/insurance/aia.html");
    }

    #[test]
    fn url_template_lowercases_key() {
        let url = build_url_from_template(
            "https://static.tdh.com/insurance/{insurerKey}.html",
            "{insurerKey}",
            "AIA",
        );
        assert_eq!(url, "https://static.tdh.com/insurance/aia.html");
    }

    #[test]
    fn url_template_validation_accepts_exact_one_placeholder() {
        assert!(validate_url_template(
            "https://x/{insurerKey}.html",
            "{insurerKey}"
        )
        .is_ok());
    }

    #[test]
    fn url_template_validation_rejects_missing_placeholder() {
        assert!(validate_url_template("https://x/foo.html", "{insurerKey}").is_err());
    }

    #[test]
    fn url_template_validation_rejects_duplicate_placeholder() {
        assert!(validate_url_template(
            "https://x/{insurerKey}/{insurerKey}.html",
            "{insurerKey}"
        )
        .is_err());
    }
```

- [ ] **Step 14: Run failing tests**

```bash
cargo test -p server --lib module::appointment::mapper::tests::url_template
```
Expected: compile error.

- [ ] **Step 15: Implement `build_url_from_template` and `validate_url_template`**

Add to `mapper.rs`:

```rust
/// Substitute a single `{key}` placeholder in `template` with a
/// lowercased `key_value`. Caller is responsible for having validated
/// the template at startup via [`validate_url_template`].
pub fn build_url_from_template(template: &str, placeholder: &str, key_value: &str) -> String {
    template.replace(placeholder, &key_value.to_ascii_lowercase())
}

/// Validate that `template` contains the `placeholder` literal exactly
/// once. Returns the template back unchanged on success, or a
/// human-readable error message on failure. Used at startup so an
/// invalid template fails fast instead of producing broken URLs at
/// request time.
pub fn validate_url_template<'a>(
    template: &'a str,
    placeholder: &str,
) -> Result<&'a str, String> {
    let count = template.matches(placeholder).count();
    match count {
        1 => Ok(template),
        0 => Err(format!(
            "URL template {:?} is missing the required placeholder {:?}",
            template, placeholder
        )),
        n => Err(format!(
            "URL template {:?} contains the placeholder {:?} {} times; expected exactly 1",
            template, placeholder, n
        )),
    }
}
```

- [ ] **Step 16: Run the url_template tests**

```bash
cargo test -p server --lib module::appointment::mapper::tests::url_template
```
Expected: 5 pass.

- [ ] **Step 17: Wire `mapper.rs` into the module**

Append a new `pub mod mapper;` line to `server/src/module/appointment/mod.rs` (Task 4 already added `pub mod models;`):

```rust
pub mod mapper;
```

- [ ] **Step 18: Run all mapper tests**

```bash
cargo test -p server --lib module::appointment::mapper
```
Expected: 18 pass (3 appointment_no + 4 age + 6 slug + 5 url_template).

- [ ] **Step 19: Commit**

```bash
git add server/src/module/appointment/mapper.rs server/src/module/appointment/mod.rs
git commit -m "feat(appointment): add pure mapper helpers (TDD)

- derive_appointment_no: strip BK prefix
- compute_age: integer years, leap-day correct
- slugify_campaign: lowercase + non-alnum runs collapsed
- build_url_from_template + validate_url_template
All test-first; 18 tests passing."
```

---

## Task 6: Implement `consultation_client.rs`

The thinnest of the three clients. Calls `GET /internal/v1/appointment/{bookingId}` and returns either an upstream success DTO or `AppointmentNotFound`. Includes a single transparent retry on transport failures.

**Files:**
- Create: `server/src/module/appointment/consultation_client.rs`

- [ ] **Step 1: Create the file with the upstream DTO and the trait**

Create `server/src/module/appointment/consultation_client.rs` with:

```rust
//! Client for consultation-rs `GET /internal/v1/appointment/{bookingId}`.
//!
//! See spec:
//! docs/superpowers/specs/2026-04-09-appointment-detail-bff-design.md.

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;
use tracing::{debug, warn};

use crate::core::error::{AppError, AppResult};

/// Outcome of a consultation lookup. Wraps the upstream discriminated
/// union (`success` | `appointmentNotFound`).
#[derive(Debug)]
pub enum ConsultationLookup {
    Found(ConsultationDetail),
    NotFound,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsultationDetail {
    pub booking_id: String,
    pub appointment_time: ConsultationAppointmentTime,
    pub status: String,
    pub booking_type: String,
    pub consultation_channel: String,
    pub patient: ConsultationIdentity,
    #[allow(dead_code)]
    pub doctor: ConsultationIdentity,
    pub prescreen: ConsultationPrescreen,
    pub payment_tx_id: i64,
    pub payment_tx_ref_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsultationAppointmentTime {
    pub start_time: i64,
    pub end_time: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsultationIdentity {
    pub account_id: i32,
    pub profile_id: i32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsultationPrescreen {
    pub symptom: String,
    pub duration: i32,
    pub duration_unit: String,
    #[serde(default)]
    pub attachments: Vec<String>,
    #[serde(default)]
    pub allergies: Vec<String>,
}

/// Raw upstream wire envelope on `__type`. Internal — never leaks out.
#[derive(Debug, Deserialize)]
#[serde(tag = "__type")]
enum WireEnvelope {
    #[serde(rename = "success")]
    Success(ConsultationDetail),
    #[serde(rename = "appointmentNotFound")]
    AppointmentNotFound,
}

#[async_trait]
pub trait ConsultationClientTrait: Send + Sync {
    async fn get_appointment(&self, booking_id: &str) -> AppResult<ConsultationLookup>;
}

#[derive(Clone)]
pub struct ConsultationClient {
    client: Client,
    base_uri: String,
}

impl ConsultationClient {
    pub fn new(base_uri: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .expect("failed to build consultation HTTP client"),
            base_uri,
        }
    }
}

#[async_trait]
impl ConsultationClientTrait for ConsultationClient {
    #[tracing::instrument(name = "consultation.get_appointment_detail", skip(self), fields(booking_id = %booking_id))]
    async fn get_appointment(&self, booking_id: &str) -> AppResult<ConsultationLookup> {
        let url = format!("{}/internal/v1/appointment/{}", self.base_uri, booking_id);
        debug!(%url, "calling consultation upstream");

        // One transparent retry on transport failure.
        let resp = match send_with_retry(&self.client, &url).await {
            Ok(r) => r,
            Err(e) => {
                warn!(error = %e, "consultation upstream failed after retry");
                return Err(AppError::UpstreamError("consultation".to_string()));
            }
        };

        let envelope: WireEnvelope = resp.json().await.map_err(|e| {
            warn!(error = %e, "consultation upstream returned malformed JSON");
            AppError::UpstreamError("consultation".to_string())
        })?;

        Ok(match envelope {
            WireEnvelope::Success(d) => ConsultationLookup::Found(d),
            WireEnvelope::AppointmentNotFound => ConsultationLookup::NotFound,
        })
    }
}

/// Send a GET request once, retry once on transient transport failure
/// (network error or HTTP 5xx). No retry on 4xx or successful response.
async fn send_with_retry(client: &Client, url: &str) -> Result<reqwest::Response, reqwest::Error> {
    match client.get(url).send().await {
        Ok(r) if r.status().is_server_error() => {
            warn!(status = %r.status(), %url, attempt = 1, "transport 5xx, retrying");
            client.get(url).send().await?.error_for_status()
        }
        Ok(r) => r.error_for_status(),
        Err(e) if e.is_connect() || e.is_timeout() || e.is_request() => {
            warn!(error = %e, %url, attempt = 1, "transport error, retrying");
            client.get(url).send().await?.error_for_status()
        }
        Err(e) => Err(e),
    }
}
```

- [ ] **Step 2: Add `pub mod consultation_client;` to `mod.rs`**

In `server/src/module/appointment/mod.rs`, add:

```rust
pub mod consultation_client;
```

- [ ] **Step 3: Compile**

```bash
cargo check -p server
```
Expected: no errors. Warnings about unused code in `ConsultationDetail.doctor` are OK (silenced via `#[allow(dead_code)]`).

- [ ] **Step 4: Commit**

```bash
git add server/src/module/appointment/consultation_client.rs server/src/module/appointment/mod.rs
git commit -m "feat(appointment): add consultation_client with retry-once

Calls GET /internal/v1/appointment/{bookingId}, returns
ConsultationLookup::{Found, NotFound}, retries once on network
or 5xx errors, surfaces as AppError::UpstreamError on final
failure. Tracing span for the upstream call."
```

---

## Task 7: Implement `iam_client.rs`

Calls IAM `GET /iam/v1/internal/profile/by-account/{accountId}` and parses the discriminated `Result` (`Success` / `AccountNotFound` / `ProfileNotFound` / `Error`). Returns a typed enum so the handler can branch cleanly.

**Files:**
- Create: `server/src/module/appointment/iam_client.rs`

- [ ] **Step 1: Create the file with the DTO + trait + impl**

Create `server/src/module/appointment/iam_client.rs` with:

```rust
//! Client for IAM gatekeeper
//! `GET /iam/v1/internal/profile/by-account/{accountId}`.
//!
//! Upstream returns a discriminated union on `__type` with a `RawJson`
//! profile. We deserialize the profile as `MorDeeUserProfileV1` (the
//! patient profile shape), since the doctor app's appointment-detail
//! screen is patient-facing. See spec:
//! docs/superpowers/specs/2026-04-09-appointment-detail-bff-design.md.

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;
use tracing::{debug, warn};

use crate::core::error::{AppError, AppResult};

/// Outcome of an IAM profile lookup.
#[derive(Debug)]
pub enum IamLookup {
    Found(MorDeeUserProfile),
    NotFound, // covers AccountNotFound and ProfileNotFound
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MorDeeUserProfile {
    #[serde(default)]
    pub first_name: Option<String>,
    #[serde(default)]
    pub last_name: Option<String>,
    #[serde(default)]
    pub gender: Option<String>,
    #[serde(default)]
    pub date_of_birth: Option<String>,
    // Other IAM profile fields (email, phoneNumber, imageUrl) are
    // intentionally not modeled — we don't need them and don't want
    // to log them.
}

/// Raw wire envelope. The `__type` discriminator carries dot-namespaced
/// names per the upstream Scala definition.
#[derive(Debug, Deserialize)]
#[serde(tag = "__type")]
enum WireEnvelope {
    #[serde(rename = "InternalGetProfileByAccountId.Result.Success")]
    Success { profile: MorDeeUserProfile },
    #[serde(rename = "InternalGetProfileByAccountId.Result.AccountNotFound")]
    AccountNotFound { #[allow(dead_code)] msg: String },
    #[serde(rename = "InternalGetProfileByAccountId.Result.ProfileNotFound")]
    ProfileNotFound { #[allow(dead_code)] msg: String },
    #[serde(rename = "InternalGetProfileByAccountId.Result.Error")]
    Error { msg: String },
}

#[async_trait]
pub trait IamClientTrait: Send + Sync {
    async fn get_profile_by_account(&self, account_id: i32) -> AppResult<IamLookup>;
}

#[derive(Clone)]
pub struct IamClient {
    client: Client,
    base_uri: String,
}

impl IamClient {
    pub fn new(base_uri: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .expect("failed to build IAM HTTP client"),
            base_uri,
        }
    }
}

#[async_trait]
impl IamClientTrait for IamClient {
    #[tracing::instrument(name = "iam.get_profile_by_account", skip(self), fields(account_id = account_id))]
    async fn get_profile_by_account(&self, account_id: i32) -> AppResult<IamLookup> {
        let url = format!(
            "{}/iam/v1/internal/profile/by-account/{}",
            self.base_uri, account_id
        );
        debug!(%url, "calling IAM upstream");

        let resp = match send_with_retry(&self.client, &url).await {
            Ok(r) => r,
            Err(e) => {
                warn!(error = %e, "iam upstream failed after retry");
                return Err(AppError::UpstreamError("iam".to_string()));
            }
        };

        let envelope: WireEnvelope = resp.json().await.map_err(|e| {
            warn!(error = %e, "iam upstream returned malformed JSON");
            AppError::UpstreamError("iam".to_string())
        })?;

        Ok(match envelope {
            WireEnvelope::Success { profile } => IamLookup::Found(profile),
            WireEnvelope::AccountNotFound { .. } | WireEnvelope::ProfileNotFound { .. } => {
                IamLookup::NotFound
            }
            WireEnvelope::Error { msg } => {
                warn!(upstream_msg = %msg, "iam upstream Error variant");
                return Err(AppError::UpstreamError("iam".to_string()));
            }
        })
    }
}

async fn send_with_retry(client: &Client, url: &str) -> Result<reqwest::Response, reqwest::Error> {
    match client.get(url).send().await {
        Ok(r) if r.status().is_server_error() => {
            warn!(status = %r.status(), %url, attempt = 1, "transport 5xx, retrying");
            client.get(url).send().await?.error_for_status()
        }
        Ok(r) => r.error_for_status(),
        Err(e) if e.is_connect() || e.is_timeout() || e.is_request() => {
            warn!(error = %e, %url, attempt = 1, "transport error, retrying");
            client.get(url).send().await?.error_for_status()
        }
        Err(e) => Err(e),
    }
}
```

- [ ] **Step 2: Add to `mod.rs`**

```rust
pub mod iam_client;
```

- [ ] **Step 3: Compile**

```bash
cargo check -p server
```
Expected: no errors.

- [ ] **Step 4: Commit**

```bash
git add server/src/module/appointment/iam_client.rs server/src/module/appointment/mod.rs
git commit -m "feat(appointment): add iam_client with retry-once

Calls IAM internal/profile/by-account/{accountId}, parses the
discriminated Result envelope, returns IamLookup::{Found, NotFound}
or AppError::UpstreamError on transport failure or upstream Error
variant."
```

---

## Task 8: Implement `payment_client.rs`

The biggest of the three clients. Models the full `selectedChannelResult` tagged union so the mapper can switch on it without re-parsing JSON. The `couponProtocol` field is kept opaque (`serde_json::Value`) since the mapper only reads `campaignName` from it.

**Files:**
- Create: `server/src/module/appointment/payment_client.rs`

- [ ] **Step 1: Create the file**

Create `server/src/module/appointment/payment_client.rs` with:

```rust
//! Client for the payment service
//! `GET /payment/transactions/{paymentTransactionId}`.
//!
//! Models the full `selectedChannelResult` tagged union so the mapper
//! can switch on its variants without re-parsing JSON. See spec:
//! docs/superpowers/specs/2026-04-09-appointment-detail-bff-design.md.

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use std::time::Duration;
use tracing::{debug, warn};

use crate::core::error::{AppError, AppResult};

#[derive(Debug)]
pub enum PaymentLookup {
    Found(PaymentDetail),
    NotFound,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentDetail {
    pub payment_transaction_id: i64,
    pub payment_transaction_ref_id: String,
    pub amount: serde_json::Number,
    /// May be null only on zero-amount free flows.
    #[serde(default)]
    pub selected_channel_result: Option<SelectedChannelResult>,
    /// Opaque pass-through; only `campaignName` is read by the mapper.
    #[serde(default)]
    pub coupon_protocol: Option<JsonValue>,
}

/// Top-level discriminator for which payment channel(s) covered the
/// transaction. The mapper MUST dispatch on the outer variant first
/// (SelfPay/Coverage/CoverageAndSelfPay) to determine `has_insurance`,
/// and only then look at the inner `PaymentChannel` for the display
/// name. Inverting that order would misclassify a Coverage payment
/// whose inner channel happens to be `PaymentChannel::Unknown`.
#[derive(Debug, Deserialize)]
#[serde(tag = "__type")]
pub enum SelectedChannelResult {
    #[serde(rename = "SelectedChannelResult.SelfPayChannel")]
    SelfPay { channel: PaymentChannel },
    #[serde(rename = "SelectedChannelResult.CoverageChannel")]
    Coverage { channel: PaymentChannel },
    #[serde(rename = "SelectedChannelResult.CoverageAndSelfPayChannel")]
    CoverageAndSelfPay {
        #[serde(rename = "coverageChannel")]
        coverage_channel: PaymentChannel,
        #[allow(dead_code)]
        #[serde(rename = "selfPayChannel")]
        self_pay_channel: PaymentChannel,
    },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "__type")]
pub enum PaymentChannel {
    // Self-pay variants — we only need to know they're self-pay.
    #[serde(rename = "PaymentChannelResult.PromptPay")]
    PromptPay,
    #[serde(rename = "PaymentChannelResult.TrueMoney")]
    TrueMoney,
    #[serde(rename = "PaymentChannelResult.Card")]
    Card,
    #[serde(rename = "PaymentChannelResult.CardSchedule")]
    CardSchedule,

    // Coverage — insurance v1/v2 share these fields.
    #[serde(rename = "PaymentChannelResult.Insurance")]
    Insurance {
        #[serde(default, rename = "insurerCode")]
        insurer_code: Option<String>,
        #[serde(default, rename = "insuranceNameI18n")]
        insurance_name_i18n: Option<I18nMap>,
    },
    #[serde(rename = "PaymentChannelResult.InsuranceV2")]
    InsuranceV2 {
        #[serde(default, rename = "insurerCode")]
        insurer_code: Option<String>,
        #[serde(default, rename = "insuranceNameI18n")]
        insurance_name_i18n: Option<I18nMap>,
    },
    // Coverage — insurance v3 has provider* instead of insurer*.
    #[serde(rename = "PaymentChannelResult.InsuranceV3")]
    InsuranceV3 {
        #[serde(default, rename = "providerName")]
        provider_name: Option<String>,
        #[serde(default, rename = "providerAbbreviation")]
        provider_abbreviation: Option<String>,
        #[serde(default, rename = "insuranceNameI18n")]
        insurance_name_i18n: Option<I18nMap>,
    },

    // Coverage — employee benefit (not insurance).
    #[serde(rename = "PaymentChannelResult.EmployeeBenefit")]
    EmployeeBenefit {
        #[serde(default, rename = "companyName")]
        company_name: Option<String>,
    },
    #[serde(rename = "PaymentChannelResult.EmployeeBenefitV2")]
    EmployeeBenefitV2 {
        #[serde(default, rename = "companyName")]
        company_name: Option<String>,
    },

    // Coverage — campaign.
    #[serde(rename = "PaymentChannelResult.CampaignLocation")]
    CampaignLocation,

    /// Catch-all for variants we don't model. Mapper falls back to "Self pay".
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
pub struct I18nMap {
    #[serde(default)]
    pub en: Option<String>,
    // Other locales (th, etc.) intentionally unmodeled.
}

#[derive(Debug, Deserialize)]
#[serde(tag = "__type")]
enum WireEnvelope {
    #[serde(rename = "Success")]
    Success { detail: PaymentDetail },
    #[serde(rename = "NotFound")]
    NotFound,
    #[serde(rename = "UnexpectedError")]
    UnexpectedError,
}

#[async_trait]
pub trait PaymentClientTrait: Send + Sync {
    async fn get_payment(&self, payment_tx_id: i64) -> AppResult<PaymentLookup>;
}

#[derive(Clone)]
pub struct PaymentClient {
    client: Client,
    base_uri: String,
}

impl PaymentClient {
    pub fn new(base_uri: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .expect("failed to build payment HTTP client"),
            base_uri,
        }
    }
}

#[async_trait]
impl PaymentClientTrait for PaymentClient {
    #[tracing::instrument(name = "payment.get_transaction_info", skip(self), fields(payment_tx_id = payment_tx_id))]
    async fn get_payment(&self, payment_tx_id: i64) -> AppResult<PaymentLookup> {
        let url = format!("{}/payment/transactions/{}", self.base_uri, payment_tx_id);
        debug!(%url, "calling payment upstream");

        let resp = match send_with_retry(&self.client, &url).await {
            Ok(r) => r,
            Err(e) => {
                warn!(error = %e, "payment upstream failed after retry");
                return Err(AppError::UpstreamError("payment".to_string()));
            }
        };

        let envelope: WireEnvelope = resp.json().await.map_err(|e| {
            warn!(error = %e, "payment upstream returned malformed JSON");
            AppError::UpstreamError("payment".to_string())
        })?;

        Ok(match envelope {
            WireEnvelope::Success { detail } => PaymentLookup::Found(detail),
            WireEnvelope::NotFound => PaymentLookup::NotFound,
            WireEnvelope::UnexpectedError => {
                warn!("payment upstream returned UnexpectedError");
                return Err(AppError::UpstreamError("payment".to_string()));
            }
        })
    }
}

async fn send_with_retry(client: &Client, url: &str) -> Result<reqwest::Response, reqwest::Error> {
    match client.get(url).send().await {
        Ok(r) if r.status().is_server_error() => {
            warn!(status = %r.status(), %url, attempt = 1, "transport 5xx, retrying");
            client.get(url).send().await?.error_for_status()
        }
        Ok(r) => r.error_for_status(),
        Err(e) if e.is_connect() || e.is_timeout() || e.is_request() => {
            warn!(error = %e, %url, attempt = 1, "transport error, retrying");
            client.get(url).send().await?.error_for_status()
        }
        Err(e) => Err(e),
    }
}
```

- [ ] **Step 2: Add to `mod.rs`**

```rust
pub mod payment_client;
```

- [ ] **Step 3: Compile**

```bash
cargo check -p server
```
Expected: no errors.

- [ ] **Step 4: Commit**

```bash
git add server/src/module/appointment/payment_client.rs server/src/module/appointment/mod.rs
git commit -m "feat(appointment): add payment_client with retry-once

Models the upstream selectedChannelResult tagged union (SelfPay /
Coverage / split) and PaymentChannel inner union (Card / PromptPay /
Insurance v1/v2/v3 / EmployeeBenefit / Campaign). Unknown channel
types fall through to PaymentChannel::Unknown so the mapper can
defensively default. couponProtocol kept as opaque serde_json::Value."
```

---

## Task 9: Implement `mapper.rs` — payer extraction

Now that the upstream DTOs exist, add the payer-mapping logic to `mapper.rs` (with unit tests). This stays as a pure function so it's easy to test in isolation.

**Files:**
- Modify: `server/src/module/appointment/mapper.rs`

- [ ] **Step 1: Write failing tests for `extract_payer`**

Append to the `tests` mod in `mapper.rs`:

```rust
    use crate::module::appointment::payment_client::{
        I18nMap, PaymentChannel, SelectedChannelResult,
    };

    #[test]
    fn payer_self_pay_promptpay() {
        let scr = Some(SelectedChannelResult::SelfPay {
            channel: PaymentChannel::PromptPay,
        });
        let p = extract_payer(scr.as_ref());
        assert_eq!(p.payer_name, "Self pay");
        assert!(!p.has_insurance);
        assert!(p.insurer_key.is_none());
    }

    #[test]
    fn payer_self_pay_card() {
        let scr = Some(SelectedChannelResult::SelfPay {
            channel: PaymentChannel::Card,
        });
        let p = extract_payer(scr.as_ref());
        assert_eq!(p.payer_name, "Self pay");
        assert!(!p.has_insurance);
    }

    #[test]
    fn payer_insurance_v1_uses_insurer_code() {
        let scr = Some(SelectedChannelResult::Coverage {
            channel: PaymentChannel::Insurance {
                insurer_code: Some("AIA".to_string()),
                insurance_name_i18n: None,
            },
        });
        let p = extract_payer(scr.as_ref());
        assert_eq!(p.payer_name, "AIA");
        assert!(p.has_insurance);
        assert_eq!(p.insurer_key.as_deref(), Some("AIA"));
    }

    #[test]
    fn payer_insurance_v1_prefers_i18n_over_code() {
        let scr = Some(SelectedChannelResult::Coverage {
            channel: PaymentChannel::Insurance {
                insurer_code: Some("AIA".to_string()),
                insurance_name_i18n: Some(I18nMap {
                    en: Some("AIA Health".to_string()),
                }),
            },
        });
        let p = extract_payer(scr.as_ref());
        assert_eq!(p.payer_name, "AIA Health");
        assert!(p.has_insurance);
        assert_eq!(p.insurer_key.as_deref(), Some("AIA"));
    }

    #[test]
    fn payer_insurance_v3_uses_provider_name() {
        let scr = Some(SelectedChannelResult::Coverage {
            channel: PaymentChannel::InsuranceV3 {
                provider_name: Some("ACME Insurance".to_string()),
                provider_abbreviation: Some("ACME".to_string()),
                insurance_name_i18n: None,
            },
        });
        let p = extract_payer(scr.as_ref());
        assert_eq!(p.payer_name, "ACME Insurance");
        assert!(p.has_insurance);
        assert_eq!(p.insurer_key.as_deref(), Some("ACME"));
    }

    #[test]
    fn payer_insurance_v3_falls_back_to_abbreviation_when_no_provider_name() {
        let scr = Some(SelectedChannelResult::Coverage {
            channel: PaymentChannel::InsuranceV3 {
                provider_name: None,
                provider_abbreviation: Some("ACME".to_string()),
                insurance_name_i18n: None,
            },
        });
        let p = extract_payer(scr.as_ref());
        assert_eq!(p.payer_name, "ACME");
        assert!(p.has_insurance);
        assert_eq!(p.insurer_key.as_deref(), Some("ACME"));
    }

    #[test]
    fn payer_insurance_v3_no_key_returns_none() {
        let scr = Some(SelectedChannelResult::Coverage {
            channel: PaymentChannel::InsuranceV3 {
                provider_name: None,
                provider_abbreviation: None,
                insurance_name_i18n: None,
            },
        });
        let p = extract_payer(scr.as_ref());
        assert_eq!(p.payer_name, "Insurance");
        assert!(p.has_insurance);
        assert!(p.insurer_key.is_none());
    }

    #[test]
    fn payer_employee_benefit_not_insurance() {
        let scr = Some(SelectedChannelResult::Coverage {
            channel: PaymentChannel::EmployeeBenefit {
                company_name: Some("Acme Corp".to_string()),
            },
        });
        let p = extract_payer(scr.as_ref());
        assert_eq!(p.payer_name, "Acme Corp");
        assert!(!p.has_insurance);
        assert!(p.insurer_key.is_none());
    }

    #[test]
    fn payer_employee_benefit_no_company_name() {
        let scr = Some(SelectedChannelResult::Coverage {
            channel: PaymentChannel::EmployeeBenefit { company_name: None },
        });
        let p = extract_payer(scr.as_ref());
        assert_eq!(p.payer_name, "Employee Benefit");
        assert!(!p.has_insurance);
    }

    #[test]
    fn payer_campaign_location() {
        let scr = Some(SelectedChannelResult::Coverage {
            channel: PaymentChannel::CampaignLocation,
        });
        let p = extract_payer(scr.as_ref());
        assert_eq!(p.payer_name, "Campaign");
        assert!(!p.has_insurance);
    }

    #[test]
    fn payer_split_uses_coverage_channel() {
        let scr = Some(SelectedChannelResult::CoverageAndSelfPay {
            coverage_channel: PaymentChannel::Insurance {
                insurer_code: Some("AIA".to_string()),
                insurance_name_i18n: None,
            },
            self_pay_channel: PaymentChannel::PromptPay,
        });
        let p = extract_payer(scr.as_ref());
        assert_eq!(p.payer_name, "AIA");
        assert!(p.has_insurance);
    }

    #[test]
    fn payer_null_selected_channel_is_free() {
        let p = extract_payer(None);
        assert_eq!(p.payer_name, "Free");
        assert!(!p.has_insurance);
    }

    #[test]
    fn payer_unknown_channel_falls_back_to_self_pay() {
        let scr = Some(SelectedChannelResult::Coverage {
            channel: PaymentChannel::Unknown,
        });
        let p = extract_payer(scr.as_ref());
        assert_eq!(p.payer_name, "Self pay");
        assert!(!p.has_insurance);
    }
```

- [ ] **Step 2: Run failing tests**

```bash
cargo test -p server --lib module::appointment::mapper::tests::payer
```
Expected: compile error for `extract_payer`, `PayerInfo`.

- [ ] **Step 3: Implement `extract_payer`**

Add to `mapper.rs` (above the `tests` mod):

```rust
use crate::module::appointment::payment_client::{PaymentChannel, SelectedChannelResult};

/// Lean view of the payer pulled out of the upstream
/// `selectedChannelResult` payload. Used by the handler to populate
/// `Payment.payer_name`, `Payment.has_insurance`, and to feed
/// `build_insurance_url`.
pub struct PayerInfo {
    pub payer_name: String,
    pub has_insurance: bool,
    /// The raw (un-lowercased) key the URL builder should slot into
    /// `{insurerKey}`. `None` when `has_insurance` is false, OR when
    /// it's true but no usable key field is present (e.g. an
    /// `InsuranceV3` payload missing both `provider_abbreviation` and
    /// `provider_name`).
    pub insurer_key: Option<String>,
}

pub fn extract_payer(scr: Option<&SelectedChannelResult>) -> PayerInfo {
    match scr {
        None => PayerInfo {
            payer_name: "Free".to_string(),
            has_insurance: false,
            insurer_key: None,
        },
        Some(SelectedChannelResult::SelfPay { .. }) => PayerInfo {
            payer_name: "Self pay".to_string(),
            has_insurance: false,
            insurer_key: None,
        },
        Some(SelectedChannelResult::Coverage { channel })
        | Some(SelectedChannelResult::CoverageAndSelfPay {
            coverage_channel: channel,
            ..
        }) => from_coverage_channel(channel),
    }
}

fn from_coverage_channel(channel: &PaymentChannel) -> PayerInfo {
    match channel {
        PaymentChannel::Insurance {
            insurer_code,
            insurance_name_i18n,
        }
        | PaymentChannel::InsuranceV2 {
            insurer_code,
            insurance_name_i18n,
        } => {
            let i18n_en = insurance_name_i18n.as_ref().and_then(|m| m.en.clone());
            let payer_name = first_non_empty([i18n_en, insurer_code.clone()])
                .unwrap_or_else(|| "Insurance".to_string());
            PayerInfo {
                payer_name,
                has_insurance: true,
                insurer_key: insurer_code.clone(),
            }
        }
        PaymentChannel::InsuranceV3 {
            provider_name,
            provider_abbreviation,
            insurance_name_i18n,
        } => {
            let i18n_en = insurance_name_i18n.as_ref().and_then(|m| m.en.clone());
            let payer_name = first_non_empty([
                provider_name.clone(),
                i18n_en,
                provider_abbreviation.clone(),
            ])
            .unwrap_or_else(|| "Insurance".to_string());
            PayerInfo {
                payer_name,
                has_insurance: true,
                insurer_key: provider_abbreviation.clone(),
            }
        }
        PaymentChannel::EmployeeBenefit { company_name }
        | PaymentChannel::EmployeeBenefitV2 { company_name } => PayerInfo {
            payer_name: company_name
                .clone()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| "Employee Benefit".to_string()),
            has_insurance: false,
            insurer_key: None,
        },
        PaymentChannel::CampaignLocation => PayerInfo {
            payer_name: "Campaign".to_string(),
            has_insurance: false,
            insurer_key: None,
        },
        // Self-pay channels appearing under Coverage (shouldn't happen
        // in practice but we're defensive) and the Unknown catch-all
        // both fall through to self pay.
        PaymentChannel::Card
        | PaymentChannel::PromptPay
        | PaymentChannel::TrueMoney
        | PaymentChannel::CardSchedule
        | PaymentChannel::Unknown => PayerInfo {
            payer_name: "Self pay".to_string(),
            has_insurance: false,
            insurer_key: None,
        },
    }
}

fn first_non_empty<I: IntoIterator<Item = Option<String>>>(opts: I) -> Option<String> {
    opts.into_iter()
        .flatten()
        .find(|s| !s.trim().is_empty())
        .map(|s| s.trim().to_string())
}
```

- [ ] **Step 4: Run the payer tests**

```bash
cargo test -p server --lib module::appointment::mapper::tests::payer
```
Expected: 13 pass.

- [ ] **Step 5: Run all mapper tests**

```bash
cargo test -p server --lib module::appointment::mapper
```
Expected: 31 pass (18 from Task 5 + 13 new payer tests).

- [ ] **Step 6: Commit**

```bash
git add server/src/module/appointment/mapper.rs
git commit -m "feat(appointment): add extract_payer mapping logic (TDD)

Collapses upstream selectedChannelResult tagged union into a lean
PayerInfo { payer_name, has_insurance, insurer_key }. Handles
Insurance v1/v2 (insurerCode), v3 (providerAbbreviation), employee
benefit, campaign, split coverage+selfpay, free flow, and unknown
fallback. 13 new tests."
```

---

## Task 10: Implement `mapper.rs` — top-level `compose`

The final mapper helper. Glues the three upstream payloads + the URL templates into a `SuccessBody`. Pure function — handler will call it inside the spawn-and-join.

**Files:**
- Modify: `server/src/module/appointment/mapper.rs`

- [ ] **Step 1: Write failing test for `compose` (happy path, insurance)**

Append to the `tests` mod in `mapper.rs`:

```rust
    use crate::module::appointment::consultation_client::{
        ConsultationAppointmentTime, ConsultationDetail, ConsultationIdentity,
        ConsultationPrescreen,
    };
    use crate::module::appointment::iam_client::MorDeeUserProfile;
    use crate::module::appointment::payment_client::PaymentDetail;
    use serde_json::json;

    fn fixture_consultation() -> ConsultationDetail {
        ConsultationDetail {
            booking_id: "BK20220227810949".to_string(),
            appointment_time: ConsultationAppointmentTime {
                start_time: 1645940400, // 2022-02-27 03:00 UTC
                end_time: 1645941300,
            },
            status: "BOOKED".to_string(),
            booking_type: "Schedule".to_string(),
            consultation_channel: "video".to_string(),
            patient: ConsultationIdentity {
                account_id: 124236,
                profile_id: 200,
            },
            doctor: ConsultationIdentity {
                account_id: 300,
                profile_id: 400,
            },
            prescreen: ConsultationPrescreen {
                symptom: "headache".to_string(),
                duration: 7,
                duration_unit: "day".to_string(),
                attachments: vec!["att-001".to_string()],
                allergies: vec!["Amoxicillin".to_string()],
            },
            payment_tx_id: 1042,
            payment_tx_ref_id: "PT-2026-001".to_string(),
        }
    }

    fn fixture_iam_profile() -> MorDeeUserProfile {
        MorDeeUserProfile {
            first_name: Some("Mrs.Bunyang".to_string()),
            last_name: Some("Lopez".to_string()),
            gender: Some("Female".to_string()),
            date_of_birth: Some("1957-03-22".to_string()),
        }
    }

    fn fixture_payment_insurance_v1() -> PaymentDetail {
        PaymentDetail {
            payment_transaction_id: 1042,
            payment_transaction_ref_id: "PT-2026-001".to_string(),
            amount: serde_json::Number::from_f64(1500.0).unwrap(),
            selected_channel_result: Some(SelectedChannelResult::Coverage {
                channel: PaymentChannel::Insurance {
                    insurer_code: Some("AIA".to_string()),
                    insurance_name_i18n: None,
                },
            }),
            coupon_protocol: None,
        }
    }

    fn fixture_templates() -> Templates {
        Templates {
            insurance: "https://static.tdh.com/insurance/{insurerKey}.html",
            coupon: "https://static.tdh.com/coupon/{couponKey}.html",
        }
    }

    fn today_2002() -> Date {
        date(2002, 4, 1)
    }

    #[test]
    fn compose_happy_insurance_v1() {
        let body = compose(
            fixture_consultation(),
            fixture_iam_profile(),
            Some(fixture_payment_insurance_v1()),
            fixture_templates(),
            today_2002(),
        );
        assert_eq!(body.booking_id, "BK20220227810949");
        assert_eq!(body.appointment_no, "20220227810949");
        assert_eq!(body.appointment_date, "2022-02-27");
        assert_eq!(body.status, "BOOKED");
        assert_eq!(body.consultation_channel, "video");

        assert_eq!(body.patient.account_id, 124236);
        assert_eq!(body.patient.full_name.as_deref(), Some("Mrs.Bunyang Lopez"));
        assert_eq!(body.patient.age, Some(45));
        assert_eq!(body.patient.gender.as_deref(), Some("Female"));

        let payment = body.payment.expect("payment populated");
        assert_eq!(payment.payment_tx_id, 1042);
        assert_eq!(payment.payer_name, "AIA");
        assert!(payment.has_insurance);
        assert_eq!(
            payment.insurance_condition_url.as_deref(),
            Some("https://static.tdh.com/insurance/aia.html")
        );

        assert!(body.coupon.is_none());
    }

    #[test]
    fn compose_payment_none_payment_field_null() {
        let body = compose(
            fixture_consultation(),
            fixture_iam_profile(),
            None, // payment-svc returned NotFound
            fixture_templates(),
            today_2002(),
        );
        assert!(body.payment.is_none());
        assert!(body.coupon.is_none());
    }

    #[test]
    fn compose_full_name_null_when_both_missing() {
        let mut profile = fixture_iam_profile();
        profile.first_name = None;
        profile.last_name = None;
        let body = compose(
            fixture_consultation(),
            profile,
            Some(fixture_payment_insurance_v1()),
            fixture_templates(),
            today_2002(),
        );
        assert!(body.patient.full_name.is_none());
    }

    #[test]
    fn compose_age_null_when_dob_missing() {
        let mut profile = fixture_iam_profile();
        profile.date_of_birth = None;
        let body = compose(
            fixture_consultation(),
            profile,
            Some(fixture_payment_insurance_v1()),
            fixture_templates(),
            today_2002(),
        );
        assert!(body.patient.date_of_birth.is_none());
        assert!(body.patient.age.is_none());
    }

    #[test]
    fn compose_insurance_v3_no_key_url_null() {
        let mut payment = fixture_payment_insurance_v1();
        payment.selected_channel_result = Some(SelectedChannelResult::Coverage {
            channel: PaymentChannel::InsuranceV3 {
                provider_name: None,
                provider_abbreviation: None,
                insurance_name_i18n: None,
            },
        });
        let body = compose(
            fixture_consultation(),
            fixture_iam_profile(),
            Some(payment),
            fixture_templates(),
            today_2002(),
        );
        let payment = body.payment.unwrap();
        assert_eq!(payment.payer_name, "Insurance");
        assert!(payment.has_insurance);
        assert!(payment.insurance_condition_url.is_none());
    }

    #[test]
    fn compose_coupon_happy_path() {
        let mut payment = fixture_payment_insurance_v1();
        payment.coupon_protocol = Some(json!({
            "__type": "CouponProtocol.Coupon",
            "campaignName": "New Year Sale 2026",
            "coupon": "XMAS2026",
            "couponCampaignId": 99
        }));
        let body = compose(
            fixture_consultation(),
            fixture_iam_profile(),
            Some(payment),
            fixture_templates(),
            today_2002(),
        );
        let coupon = body.coupon.unwrap();
        assert_eq!(coupon.campaign_name, "New Year Sale 2026");
        assert_eq!(
            coupon.condition_url.as_deref(),
            Some("https://static.tdh.com/coupon/new-year-sale-2026.html")
        );
    }

    #[test]
    fn compose_coupon_missing_campaign_name_yields_null() {
        let mut payment = fixture_payment_insurance_v1();
        payment.coupon_protocol = Some(json!({
            "__type": "CouponProtocol.Coupon",
            "coupon": "XMAS2026"
            // no campaignName
        }));
        let body = compose(
            fixture_consultation(),
            fixture_iam_profile(),
            Some(payment),
            fixture_templates(),
            today_2002(),
        );
        assert!(body.coupon.is_none());
    }

    #[test]
    fn compose_coupon_empty_campaign_name_yields_null() {
        let mut payment = fixture_payment_insurance_v1();
        payment.coupon_protocol = Some(json!({
            "__type": "CouponProtocol.Coupon",
            "campaignName": "   "
        }));
        let body = compose(
            fixture_consultation(),
            fixture_iam_profile(),
            Some(payment),
            fixture_templates(),
            today_2002(),
        );
        assert!(body.coupon.is_none());
    }

    #[test]
    fn compose_coupon_slug_to_empty_keeps_name_drops_url() {
        let mut payment = fixture_payment_insurance_v1();
        payment.coupon_protocol = Some(json!({
            "__type": "CouponProtocol.Coupon",
            "campaignName": "!!!"
        }));
        let body = compose(
            fixture_consultation(),
            fixture_iam_profile(),
            Some(payment),
            fixture_templates(),
            today_2002(),
        );
        let coupon = body.coupon.unwrap();
        assert_eq!(coupon.campaign_name, "!!!");
        assert!(coupon.condition_url.is_none());
    }
```

- [ ] **Step 2: Run failing tests**

```bash
cargo test -p server --lib module::appointment::mapper::tests::compose
```
Expected: compile error.

- [ ] **Step 3: Implement `Templates`, `compose`, and helpers**

Add to `mapper.rs`:

```rust
use crate::module::appointment::consultation_client::ConsultationDetail;
use crate::module::appointment::iam_client::MorDeeUserProfile;
use crate::module::appointment::models::{
    AppointmentTime, Coupon, Patient, Payment, Prescreen, SuccessBody,
};
use crate::module::appointment::payment_client::PaymentDetail;
use jiff::Timestamp;

/// Borrowed view over the two URL templates the handler reads from
/// config. Passed by value through the mapper so the helpers stay
/// dependency-injected and trivially unit-testable.
#[derive(Clone, Copy)]
pub struct Templates<'a> {
    pub insurance: &'a str,
    pub coupon: &'a str,
}

const INSURANCE_PLACEHOLDER: &str = "{insurerKey}";
const COUPON_PLACEHOLDER: &str = "{couponKey}";

/// Compose the BFF response body from the three upstream payloads and
/// the URL templates. `today` is injected so the function is
/// deterministic and the age computation can be unit-tested.
pub fn compose(
    consultation: ConsultationDetail,
    profile: MorDeeUserProfile,
    payment: Option<PaymentDetail>,
    templates: Templates<'_>,
    today: Date,
) -> SuccessBody {
    let appointment_no = derive_appointment_no(&consultation.booking_id);
    let appointment_date = utc_date_string(consultation.appointment_time.start_time);

    let dob_parsed = profile
        .date_of_birth
        .as_ref()
        .and_then(|s| s.parse::<Date>().ok());
    let age = dob_parsed.map(|d| compute_age(d, today));

    let full_name = build_full_name(profile.first_name.as_deref(), profile.last_name.as_deref());

    let patient = Patient {
        account_id: consultation.patient.account_id,
        profile_id: consultation.patient.profile_id,
        full_name,
        date_of_birth: profile.date_of_birth,
        age,
        gender: profile.gender,
    };

    let (payment_obj, coupon_obj) = match payment {
        None => (None, None),
        Some(p) => {
            let payer = extract_payer(p.selected_channel_result.as_ref());
            let insurance_condition_url = if payer.has_insurance {
                payer
                    .insurer_key
                    .as_deref()
                    .filter(|k| !k.is_empty())
                    .map(|k| build_url_from_template(templates.insurance, INSURANCE_PLACEHOLDER, k))
            } else {
                None
            };
            let coupon = extract_coupon(p.coupon_protocol.as_ref(), templates.coupon);
            let payment = Payment {
                payment_tx_id: p.payment_transaction_id,
                payment_tx_ref_id: p.payment_transaction_ref_id,
                payer_name: payer.payer_name,
                has_insurance: payer.has_insurance,
                insurance_condition_url,
                amount: p.amount,
            };
            (Some(payment), coupon)
        }
    };

    SuccessBody {
        booking_id: consultation.booking_id,
        appointment_no,
        appointment_time: AppointmentTime {
            start_time: consultation.appointment_time.start_time,
            end_time: consultation.appointment_time.end_time,
        },
        appointment_date,
        status: consultation.status,
        booking_type: consultation.booking_type,
        consultation_channel: consultation.consultation_channel,
        patient,
        payment: payment_obj,
        coupon: coupon_obj,
        prescreen: Prescreen {
            symptom: consultation.prescreen.symptom,
            duration: consultation.prescreen.duration,
            duration_unit: consultation.prescreen.duration_unit,
            attachments: consultation.prescreen.attachments,
            allergies: consultation.prescreen.allergies,
        },
    }
}

fn build_full_name(first: Option<&str>, last: Option<&str>) -> Option<String> {
    let f = first.map(str::trim).filter(|s| !s.is_empty());
    let l = last.map(str::trim).filter(|s| !s.is_empty());
    match (f, l) {
        (Some(f), Some(l)) => Some(format!("{} {}", f, l)),
        (Some(f), None) => Some(f.to_string()),
        (None, Some(l)) => Some(l.to_string()),
        (None, None) => None,
    }
}

fn utc_date_string(epoch_seconds: i64) -> String {
    // Mirrors the project's existing jiff pattern in
    // module/webhook/pubsub_handler.rs:
    //   Timestamp::now().to_zoned(TimeZone::UTC)
    // `Timestamp::from_second` is the only fallible step.
    Timestamp::from_second(epoch_seconds)
        .map(|ts| {
            let zdt = ts.to_zoned(jiff::tz::TimeZone::UTC);
            format!(
                "{:04}-{:02}-{:02}",
                zdt.year(),
                zdt.month() as u8,
                zdt.day()
            )
        })
        .unwrap_or_default()
}

fn extract_coupon(
    coupon_protocol: Option<&serde_json::Value>,
    coupon_template: &str,
) -> Option<Coupon> {
    let proto = coupon_protocol?;
    let raw_name = proto.get("campaignName").and_then(|v| v.as_str())?;
    let trimmed = raw_name.trim();
    if trimmed.is_empty() {
        tracing::warn!(
            upstream_type = ?proto.get("__type"),
            "coupon campaignName missing or empty",
        );
        return None;
    }
    let slug = slugify_campaign(trimmed);
    let condition_url = if slug.is_empty() {
        tracing::warn!(
            campaign_name = trimmed,
            "coupon campaignName slugifies to empty; conditionUrl null",
        );
        None
    } else {
        Some(build_url_from_template(
            coupon_template,
            COUPON_PLACEHOLDER,
            &slug,
        ))
    };
    Some(Coupon {
        campaign_name: trimmed.to_string(),
        condition_url,
    })
}
```

Also add this small unit test for `build_full_name` to the `tests` mod (just to lock in the trim/empty behaviour the spec calls out):

```rust
    #[test]
    fn full_name_first_only() {
        let body = compose(
            {
                let mut c = fixture_consultation();
                c.patient.account_id = 1;
                c
            },
            MorDeeUserProfile {
                first_name: Some("Solo".to_string()),
                last_name: None,
                gender: None,
                date_of_birth: None,
            },
            None,
            fixture_templates(),
            today_2002(),
        );
        assert_eq!(body.patient.full_name.as_deref(), Some("Solo"));
    }
```

- [ ] **Step 4: Run all mapper tests**

```bash
cargo test -p server --lib module::appointment::mapper
```
Expected: 41 pass (31 from Task 9 + 9 compose + 1 full_name_first_only).

- [ ] **Step 5: Commit**

```bash
git add server/src/module/appointment/mapper.rs
git commit -m "feat(appointment): add compose() and extract_coupon() (TDD)

Glues consultation + iam profile + payment + URL templates into the
final SuccessBody. Date is injected so age math is deterministic in
tests. Coupon is extracted to { campaignName, conditionUrl } via
slugify_campaign; missing or empty campaignName collapses the whole
coupon to None. 10 new tests."
```

---

## Task 11: Implement `handlers.rs` and wire `mod.rs`

The Axum handler that ties the three clients together via `tokio::try_join!`. Pure orchestration — no business logic (lives in the mapper).

**Files:**
- Replace: `server/src/module/appointment/handlers.rs`
- Replace: `server/src/module/appointment/mod.rs`

- [ ] **Step 1: Replace `handlers.rs`**

Replace the entire contents of `server/src/module/appointment/handlers.rs` with:

```rust
//! GET /appointment/v1/{bookingId} — BFF aggregator handler.

use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use jiff::{tz::TimeZone, Timestamp};
use std::sync::Arc;
use tracing::info;

use crate::core::auth::DoctorIdentity;
use crate::core::error::AppResult;
use crate::module::appointment::consultation_client::{ConsultationClientTrait, ConsultationLookup};
use crate::module::appointment::iam_client::{IamClientTrait, IamLookup};
use crate::module::appointment::mapper::{compose, Templates};
use crate::module::appointment::models::{ApiResponse, SuccessBody};
use crate::module::appointment::payment_client::{PaymentClientTrait, PaymentLookup};

#[derive(Clone)]
pub struct AppointmentState {
    pub consultation: Arc<dyn ConsultationClientTrait>,
    pub iam: Arc<dyn IamClientTrait>,
    pub payment: Arc<dyn PaymentClientTrait>,
    pub insurance_template: Arc<String>,
    pub coupon_template: Arc<String>,
}

#[utoipa::path(
    get,
    path = "/appointment/v1/{bookingId}",
    tag = "appointment",
    params(
        ("bookingId" = String, Path, description = "Appointment booking id (e.g., BK20220227810949)")
    ),
    responses(
        (status = 200, description = "Success / AppointmentNotFound / PatientProfileNotFound", body = ApiResponse),
        (status = 401, description = "Unauthorized — missing or malformed identity header"),
        (status = 403, description = "Forbidden — caller is not a doctor"),
        (status = 502, description = "Upstream service unavailable")
    ),
    security(("TDH-SEC-IAM-USER-IDENTITY" = []))
)]
#[tracing::instrument(
    name = "appointment_detail",
    skip(state, _doctor),
    fields(booking_id = %booking_id, doctor_account_id = _doctor.doctor_account_id)
)]
pub async fn get_appointment_detail(
    State(state): State<AppointmentState>,
    _doctor: DoctorIdentity,
    Path(booking_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    // Step 1: consultation. Short-circuit on appointmentNotFound.
    let consultation = match state.consultation.get_appointment(&booking_id).await? {
        ConsultationLookup::Found(d) => d,
        ConsultationLookup::NotFound => {
            return Ok(Json(ApiResponse::AppointmentNotFound).into_response());
        }
    };

    let patient_account_id = consultation.patient.account_id;
    let payment_tx_id = consultation.payment_tx_id;

    // Step 2: IAM + payment in parallel.
    let (iam_result, payment_result) = tokio::try_join!(
        state.iam.get_profile_by_account(patient_account_id),
        state.payment.get_payment(payment_tx_id),
    )?;

    // Step 3: handle IAM not-found.
    let profile = match iam_result {
        IamLookup::Found(p) => p,
        IamLookup::NotFound => {
            return Ok(Json(ApiResponse::PatientProfileNotFound).into_response());
        }
    };

    // Step 4: payment NotFound is soft — payment field is null.
    let payment_detail = match payment_result {
        PaymentLookup::Found(p) => Some(p),
        PaymentLookup::NotFound => None,
    };

    let templates = Templates {
        insurance: state.insurance_template.as_str(),
        coupon: state.coupon_template.as_str(),
    };
    let today = today_utc();

    let body: SuccessBody = compose(consultation, profile, payment_detail, templates, today);

    info!(
        booking_id = %body.booking_id,
        patient_account_id,
        payment_tx_id = body.payment.as_ref().map(|p| p.payment_tx_id),
        has_insurance = body.payment.as_ref().map(|p| p.has_insurance).unwrap_or(false),
        "appointment_detail composed"
    );

    Ok(Json(ApiResponse::Success(Box::new(body))).into_response())
}

fn today_utc() -> jiff::civil::Date {
    // Matches the existing jiff usage in module/webhook/pubsub_handler.rs:
    // Timestamp::now().to_zoned(TimeZone::UTC) is infallible.
    Timestamp::now().to_zoned(TimeZone::UTC).date()
}
```

- [ ] **Step 2: Replace `mod.rs`**

Replace the entire contents of `server/src/module/appointment/mod.rs` with:

```rust
//! BFF aggregator for the doctor "appointment detail" screen.
//!
//! Exposes `GET /appointment/v1/{bookingId}` mounted via `router(...)`.
//! See spec:
//! docs/superpowers/specs/2026-04-09-appointment-detail-bff-design.md.

pub mod consultation_client;
pub mod handlers;
pub mod iam_client;
pub mod mapper;
pub mod models;
pub mod payment_client;

use axum::{routing::get, Router};
use std::sync::Arc;

use crate::config::AppConfig;
use crate::module::appointment::consultation_client::{ConsultationClient, ConsultationClientTrait};
use crate::module::appointment::handlers::{get_appointment_detail, AppointmentState};
use crate::module::appointment::iam_client::{IamClient, IamClientTrait};
use crate::module::appointment::mapper::validate_url_template;
use crate::module::appointment::payment_client::{PaymentClient, PaymentClientTrait};

const INSURANCE_PLACEHOLDER: &str = "{insurerKey}";
const COUPON_PLACEHOLDER: &str = "{couponKey}";

pub fn router(cfg: &AppConfig) -> anyhow::Result<Router> {
    // Validate templates at startup so an invalid template fails fast
    // instead of producing broken URLs at request time.
    validate_url_template(
        &cfg.insurance.condition_url_template,
        INSURANCE_PLACEHOLDER,
    )
    .map_err(|e| anyhow::anyhow!("insurance.condition_url_template invalid: {}", e))?;
    validate_url_template(&cfg.coupon.condition_url_template, COUPON_PLACEHOLDER)
        .map_err(|e| anyhow::anyhow!("coupon.condition_url_template invalid: {}", e))?;

    let consultation: Arc<dyn ConsultationClientTrait> = Arc::new(ConsultationClient::new(
        cfg.service.consultation_internal_base_uri.clone(),
    ));
    let iam: Arc<dyn IamClientTrait> = Arc::new(IamClient::new(
        cfg.service.iam_gatekeeper_base_uri.clone(),
    ));
    let payment: Arc<dyn PaymentClientTrait> = Arc::new(PaymentClient::new(
        cfg.service.payment_internal_base_uri.clone(),
    ));

    let state = AppointmentState {
        consultation,
        iam,
        payment,
        insurance_template: Arc::new(cfg.insurance.condition_url_template.clone()),
        coupon_template: Arc::new(cfg.coupon.condition_url_template.clone()),
    };

    Ok(Router::new()
        .route("/{booking_id}", get(get_appointment_detail))
        .with_state(state))
}
```

- [ ] **Step 3: Compile**

```bash
cargo check -p server
```
Expected: no errors.

- [ ] **Step 4: Commit**

```bash
git add server/src/module/appointment/handlers.rs server/src/module/appointment/mod.rs
git commit -m "feat(appointment): add handler and router

GET /appointment/v1/{bookingId} handler that calls consultation,
short-circuits on AppointmentNotFound, then fans out IAM + payment
in parallel via tokio::try_join, hands the three payloads to
compose(), and emits a single tracing info log on success. Router
validates URL templates at startup."
```

---

## Task 12: Wire the new module into `bootstrap.rs`

Mount the new router under `/appointment/v1`. The CLAUDE.md project guide explicitly lists this prefix, but it's not currently in `init_routers`.

**Files:**
- Modify: `server/src/bootstrap.rs`

- [ ] **Step 1: Read the current `init_routers`**

Read `server/src/bootstrap.rs` from the `pub async fn init_routers` line through the end of `build_app`. Make sure you understand how the existing `consultation`, `ranking`, etc. routers get plumbed through `AppRouters`.

- [ ] **Step 2: Add `appointment: Router` to `AppRouters`**

In `server/src/bootstrap.rs`, locate the `pub struct AppRouters` (around line 41) and add a new field:

```rust
pub appointment: Router,
```

- [ ] **Step 3: Construct the appointment router inside `init_routers`**

Inside `init_routers`, just before the final `Ok(AppRouters { ... })` block, add:

```rust
    let appointment_router = module::appointment::router(cfg)?;
```

- [ ] **Step 4: Add `appointment` to the returned `AppRouters` struct literal**

Update the `Ok(AppRouters { ... })` block to include the new field:

```rust
    Ok(
        AppRouters {
            notification: notification_router,
            task: task_router,
            consultation: consultation_router,
            ranking: ranking_router,
            timeslot: timeslot_router,
            appointment: appointment_router,
        },
    )
```

- [ ] **Step 5: Mount it in `build_app`**

In `pub fn build_app`, add a `.nest("/appointment/v1", routers.appointment)` line. Place it next to the other `.nest` calls (order doesn't matter for routing but match the existing style):

```rust
        .nest("/appointment/v1", routers.appointment)
```

- [ ] **Step 6: Compile and run the existing test suite to make sure nothing else broke**

```bash
cargo check -p server
cargo test -p server --lib
```
Expected: no errors. All existing lib tests pass plus the appointment mapper/models tests from earlier tasks.

- [ ] **Step 7: Commit**

```bash
git add server/src/bootstrap.rs
git commit -m "feat(bootstrap): mount appointment BFF router at /appointment/v1

Wires the new aggregator into init_routers and build_app. URL
template validation runs at startup; an invalid template aborts
boot rather than producing broken URLs at request time."
```

---

## Task 13: Register the new endpoint in `openapi.rs`

So the Swagger UI shows the new route and types.

**Files:**
- Modify: `server/src/openapi.rs`

- [ ] **Step 1: Read the existing imports and `paths(...)` block**

Read `server/src/openapi.rs` end-to-end so you can see exactly where the existing `paths(...)` and component types get registered.

- [ ] **Step 2: Add the import for the new types**

At the top of `server/src/openapi.rs`, add:

```rust
use crate::module::appointment::handlers::get_appointment_detail;
use crate::module::appointment::models::{
    ApiResponse as AppointmentDetailResponse, AppointmentTime as AppointmentDetailTime,
    Coupon as AppointmentCoupon, Patient as AppointmentPatient, Payment as AppointmentPayment,
    Prescreen as AppointmentPrescreen, SuccessBody as AppointmentSuccessBody,
};
```

(The `as` aliases avoid name collisions with any existing `Appointment*` types in the openapi imports — verify by skimming the existing imports first; if there are no collisions, drop the aliases.)

- [ ] **Step 3: Register the handler in the `paths(...)` block**

Inside `#[openapi(paths(...))]`, add a line for the new handler. Match the style of the existing entries:

```rust
        crate::module::appointment::handlers::get_appointment_detail,
```

- [ ] **Step 4: Register the schemas in the `components(schemas(...))` block**

Find the `components(schemas(...))` macro section and add the new schema types:

```rust
            crate::module::appointment::models::ApiResponse,
            crate::module::appointment::models::SuccessBody,
            crate::module::appointment::models::AppointmentTime,
            crate::module::appointment::models::Patient,
            crate::module::appointment::models::Payment,
            crate::module::appointment::models::Coupon,
            crate::module::appointment::models::Prescreen,
```

- [ ] **Step 5: Compile**

```bash
cargo check -p server
```
Expected: no errors. If utoipa complains about a missing schema for any field type, add it to the `schemas(...)` block.

- [ ] **Step 6: Commit**

```bash
git add server/src/openapi.rs
git commit -m "docs(openapi): register appointment-detail handler and schemas"
```

---

## Task 14: Integration tests — happy paths

End-to-end tests that wire `wiremock` for the three upstream services and `axum-test` for the BFF. We split the tests across multiple tasks (14 / 15) so each commit is reviewable.

**Files:**
- Create: `server/tests/appointment_detail_test.rs`

- [ ] **Step 1: Create the test file with shared helpers**

Create `server/tests/appointment_detail_test.rs` with the shared scaffolding. We instantiate the three real client structs (not mocks) pointing at three `wiremock::MockServer` instances — this exercises the actual HTTP serialization paths.

```rust
//! Integration tests for GET /appointment/v1/{bookingId}.
//!
//! Uses three wiremock servers (one each for consultation, IAM, and
//! payment) plus axum-test::TestServer for the BFF. Each test
//! constructs the AppointmentState manually with real client structs,
//! so we exercise the full HTTP serde path on both sides of every
//! call.

use axum::{routing::get, Router};
use axum_test::TestServer;
use serde_json::{json, Value};
use std::sync::Arc;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

use server::module::appointment::consultation_client::{ConsultationClient, ConsultationClientTrait};
use server::module::appointment::handlers::{get_appointment_detail, AppointmentState};
use server::module::appointment::iam_client::{IamClient, IamClientTrait};
use server::module::appointment::payment_client::{PaymentClient, PaymentClientTrait};

const DOCTOR_HEADER: &str = "tdh-sec-iam-user-identity";
const DOCTOR_HEADER_VALUE: &str =
    r#"{"tenantId":1,"accountId":300,"accountType":2,"userProfileId":400,"userMainProfileId":400,"oidcUserId":null,"legacyData":null}"#;

const INSURANCE_TEMPLATE: &str = "https://static.tdh.example/insurance/{insurerKey}.html";
const COUPON_TEMPLATE: &str = "https://static.tdh.example/coupon/{couponKey}.html";

struct Servers {
    consultation: MockServer,
    iam: MockServer,
    payment: MockServer,
}

async fn start_servers() -> Servers {
    Servers {
        consultation: MockServer::start().await,
        iam: MockServer::start().await,
        payment: MockServer::start().await,
    }
}

async fn build_test_server(servers: &Servers) -> TestServer {
    let consultation: Arc<dyn ConsultationClientTrait> =
        Arc::new(ConsultationClient::new(servers.consultation.uri()));
    let iam: Arc<dyn IamClientTrait> = Arc::new(IamClient::new(servers.iam.uri()));
    let payment: Arc<dyn PaymentClientTrait> = Arc::new(PaymentClient::new(servers.payment.uri()));

    let state = AppointmentState {
        consultation,
        iam,
        payment,
        insurance_template: Arc::new(INSURANCE_TEMPLATE.to_string()),
        coupon_template: Arc::new(COUPON_TEMPLATE.to_string()),
    };
    let router = Router::new()
        .route("/appointment/v1/{booking_id}", get(get_appointment_detail))
        .with_state(state);
    TestServer::new(router).expect("test server starts")
}

fn consultation_success_body() -> Value {
    json!({
        "__type": "success",
        "bookingId": "BK20220227810949",
        "appointmentTime": { "startTime": 1645940400, "endTime": 1645941300 },
        "status": "BOOKED",
        "bookingType": "Schedule",
        "consultationChannel": "video",
        "patient": { "accountId": 124236, "profileId": 200 },
        "doctor":  { "accountId": 300,    "profileId": 400 },
        "prescreen": {
            "symptom": "headache",
            "duration": 7,
            "durationUnit": "day",
            "attachments": ["att-001", "att-002"],
            "allergies": ["Amoxicillin"]
        },
        "paymentTxId": 1042,
        "paymentTxRefId": "PT-2026-001"
    })
}

fn iam_success_body() -> Value {
    json!({
        "__type": "InternalGetProfileByAccountId.Result.Success",
        "profile": {
            "firstName": "Mrs.Bunyang",
            "lastName": "Lopez",
            "gender": "Female",
            "dateOfBirth": "1957-03-22"
        }
    })
}

fn payment_success_body_insurance_v1() -> Value {
    json!({
        "__type": "Success",
        "detail": {
            "paymentTransactionId": 1042,
            "paymentTransactionRefId": "PT-2026-001",
            "amount": 1500.00,
            "selectedChannelResult": {
                "__type": "SelectedChannelResult.CoverageChannel",
                "channel": {
                    "__type": "PaymentChannelResult.Insurance",
                    "insurerCode": "AIA"
                }
            },
            "couponProtocol": null
        }
    })
}

async fn mock_consultation(servers: &Servers, body: Value) {
    Mock::given(method("GET"))
        .and(path("/internal/v1/appointment/BK20220227810949"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(&servers.consultation)
        .await;
}

async fn mock_iam(servers: &Servers, body: Value) {
    Mock::given(method("GET"))
        .and(path("/iam/v1/internal/profile/by-account/124236"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(&servers.iam)
        .await;
}

async fn mock_payment(servers: &Servers, body: Value) {
    Mock::given(method("GET"))
        .and(path("/payment/transactions/1042"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(&servers.payment)
        .await;
}

#[tokio::test]
async fn happy_path_insurance_v1() {
    let servers = start_servers().await;
    mock_consultation(&servers, consultation_success_body()).await;
    mock_iam(&servers, iam_success_body()).await;
    mock_payment(&servers, payment_success_body_insurance_v1()).await;

    let app = build_test_server(&servers).await;
    let resp = app
        .get("/appointment/v1/BK20220227810949")
        .add_header(DOCTOR_HEADER, DOCTOR_HEADER_VALUE)
        .await;
    resp.assert_status_ok();
    let body: Value = resp.json();
    assert_eq!(body["__type"], "Success");
    assert_eq!(body["bookingId"], "BK20220227810949");
    assert_eq!(body["appointmentNo"], "20220227810949");
    assert_eq!(body["appointmentDate"], "2022-02-27");
    assert_eq!(body["patient"]["fullName"], "Mrs.Bunyang Lopez");
    assert_eq!(body["payment"]["payerName"], "AIA");
    assert_eq!(body["payment"]["hasInsurance"], true);
    assert_eq!(
        body["payment"]["insuranceConditionUrl"],
        "https://static.tdh.example/insurance/aia.html"
    );
    assert!(body["coupon"].is_null());
}

#[tokio::test]
async fn happy_path_insurance_v3_with_provider_abbreviation() {
    let servers = start_servers().await;
    mock_consultation(&servers, consultation_success_body()).await;
    mock_iam(&servers, iam_success_body()).await;
    let mut payment_body = payment_success_body_insurance_v1();
    payment_body["detail"]["selectedChannelResult"]["channel"] = json!({
        "__type": "PaymentChannelResult.InsuranceV3",
        "providerName": "ACME Insurance",
        "providerAbbreviation": "ACME"
    });
    mock_payment(&servers, payment_body).await;

    let app = build_test_server(&servers).await;
    let resp = app
        .get("/appointment/v1/BK20220227810949")
        .add_header(DOCTOR_HEADER, DOCTOR_HEADER_VALUE)
        .await;
    resp.assert_status_ok();
    let body: Value = resp.json();
    assert_eq!(body["payment"]["payerName"], "ACME Insurance");
    assert_eq!(
        body["payment"]["insuranceConditionUrl"],
        "https://static.tdh.example/insurance/acme.html"
    );
}

#[tokio::test]
async fn happy_path_self_pay_promptpay() {
    let servers = start_servers().await;
    mock_consultation(&servers, consultation_success_body()).await;
    mock_iam(&servers, iam_success_body()).await;
    let mut payment_body = payment_success_body_insurance_v1();
    payment_body["detail"]["selectedChannelResult"] = json!({
        "__type": "SelectedChannelResult.SelfPayChannel",
        "channel": { "__type": "PaymentChannelResult.PromptPay" }
    });
    mock_payment(&servers, payment_body).await;

    let app = build_test_server(&servers).await;
    let resp = app
        .get("/appointment/v1/BK20220227810949")
        .add_header(DOCTOR_HEADER, DOCTOR_HEADER_VALUE)
        .await;
    resp.assert_status_ok();
    let body: Value = resp.json();
    assert_eq!(body["payment"]["payerName"], "Self pay");
    assert_eq!(body["payment"]["hasInsurance"], false);
    assert!(body["payment"]["insuranceConditionUrl"].is_null());
    assert!(body["coupon"].is_null());
}

#[tokio::test]
async fn happy_path_split_coverage_and_self_pay() {
    let servers = start_servers().await;
    mock_consultation(&servers, consultation_success_body()).await;
    mock_iam(&servers, iam_success_body()).await;
    let mut payment_body = payment_success_body_insurance_v1();
    payment_body["detail"]["selectedChannelResult"] = json!({
        "__type": "SelectedChannelResult.CoverageAndSelfPayChannel",
        "coverageChannel": {
            "__type": "PaymentChannelResult.Insurance",
            "insurerCode": "AIA"
        },
        "selfPayChannel": {
            "__type": "PaymentChannelResult.PromptPay"
        }
    });
    mock_payment(&servers, payment_body).await;

    let app = build_test_server(&servers).await;
    let resp = app
        .get("/appointment/v1/BK20220227810949")
        .add_header(DOCTOR_HEADER, DOCTOR_HEADER_VALUE)
        .await;
    resp.assert_status_ok();
    let body: Value = resp.json();
    assert_eq!(body["payment"]["payerName"], "AIA");
    assert_eq!(body["payment"]["hasInsurance"], true);
}

#[tokio::test]
async fn happy_path_with_coupon() {
    let servers = start_servers().await;
    mock_consultation(&servers, consultation_success_body()).await;
    mock_iam(&servers, iam_success_body()).await;
    let mut payment_body = payment_success_body_insurance_v1();
    payment_body["detail"]["couponProtocol"] = json!({
        "__type": "CouponProtocol.Coupon",
        "campaignName": "New Year Sale 2026",
        "coupon": "XMAS2026",
        "couponCampaignId": 99
    });
    mock_payment(&servers, payment_body).await;

    let app = build_test_server(&servers).await;
    let resp = app
        .get("/appointment/v1/BK20220227810949")
        .add_header(DOCTOR_HEADER, DOCTOR_HEADER_VALUE)
        .await;
    resp.assert_status_ok();
    let body: Value = resp.json();
    assert_eq!(body["coupon"]["campaignName"], "New Year Sale 2026");
    assert_eq!(
        body["coupon"]["conditionUrl"],
        "https://static.tdh.example/coupon/new-year-sale-2026.html"
    );
    // Regression guard: internal coupon code MUST NOT leak.
    let body_string = serde_json::to_string(&body).unwrap();
    assert!(
        !body_string.contains("XMAS2026"),
        "internal coupon code leaked into BFF response"
    );
    assert!(
        !body_string.contains("couponCampaignId"),
        "internal couponCampaignId leaked into BFF response"
    );
}
```

- [ ] **Step 2: Run the happy path tests**

```bash
cargo test --test appointment_detail_test happy_path
```
Expected: 5 tests pass.

- [ ] **Step 3: Commit**

```bash
git add server/tests/appointment_detail_test.rs
git commit -m "test(appointment): add happy-path integration tests

5 tests covering insurance v1, insurance v3, self pay, split
coverage+selfpay, and coupon happy path. The coupon test asserts
the internal coupon code does NOT leak into the response."
```

---

## Task 15: Integration tests — error and edge cases

Add the remaining error/edge cases to the same test file.

**Files:**
- Modify: `server/tests/appointment_detail_test.rs`

- [ ] **Step 1: Add appointment-not-found and patient-not-found tests**

Append to `server/tests/appointment_detail_test.rs`:

```rust
#[tokio::test]
async fn consultation_appointment_not_found() {
    let servers = start_servers().await;
    mock_consultation(&servers, json!({ "__type": "appointmentNotFound" })).await;
    // IAM and payment must NOT be hit when consultation short-circuits.
    let app = build_test_server(&servers).await;
    let resp = app
        .get("/appointment/v1/BK20220227810949")
        .add_header(DOCTOR_HEADER, DOCTOR_HEADER_VALUE)
        .await;
    resp.assert_status_ok();
    let body: Value = resp.json();
    assert_eq!(body["__type"], "AppointmentNotFound");
}

#[tokio::test]
async fn iam_account_not_found_returns_patient_profile_not_found() {
    let servers = start_servers().await;
    mock_consultation(&servers, consultation_success_body()).await;
    mock_iam(
        &servers,
        json!({
            "__type": "InternalGetProfileByAccountId.Result.AccountNotFound",
            "msg": "no such account"
        }),
    )
    .await;
    mock_payment(&servers, payment_success_body_insurance_v1()).await;

    let app = build_test_server(&servers).await;
    let resp = app
        .get("/appointment/v1/BK20220227810949")
        .add_header(DOCTOR_HEADER, DOCTOR_HEADER_VALUE)
        .await;
    resp.assert_status_ok();
    let body: Value = resp.json();
    assert_eq!(body["__type"], "PatientProfileNotFound");
}

#[tokio::test]
async fn iam_profile_not_found_returns_patient_profile_not_found() {
    let servers = start_servers().await;
    mock_consultation(&servers, consultation_success_body()).await;
    mock_iam(
        &servers,
        json!({
            "__type": "InternalGetProfileByAccountId.Result.ProfileNotFound",
            "msg": "no such profile"
        }),
    )
    .await;
    mock_payment(&servers, payment_success_body_insurance_v1()).await;

    let app = build_test_server(&servers).await;
    let resp = app
        .get("/appointment/v1/BK20220227810949")
        .add_header(DOCTOR_HEADER, DOCTOR_HEADER_VALUE)
        .await;
    resp.assert_status_ok();
    let body: Value = resp.json();
    assert_eq!(body["__type"], "PatientProfileNotFound");
}

#[tokio::test]
async fn payment_not_found_renders_payment_field_null() {
    let servers = start_servers().await;
    mock_consultation(&servers, consultation_success_body()).await;
    mock_iam(&servers, iam_success_body()).await;
    mock_payment(&servers, json!({ "__type": "NotFound" })).await;

    let app = build_test_server(&servers).await;
    let resp = app
        .get("/appointment/v1/BK20220227810949")
        .add_header(DOCTOR_HEADER, DOCTOR_HEADER_VALUE)
        .await;
    resp.assert_status_ok();
    let body: Value = resp.json();
    assert_eq!(body["__type"], "Success");
    assert!(body["payment"].is_null());
    assert!(body["coupon"].is_null());
}

#[tokio::test]
async fn payment_unexpected_error_returns_502_no_retry() {
    let servers = start_servers().await;
    mock_consultation(&servers, consultation_success_body()).await;
    mock_iam(&servers, iam_success_body()).await;

    // Use expect(1) to assert payment-svc was called exactly once
    // (proves we do NOT retry domain variants).
    Mock::given(method("GET"))
        .and(path("/payment/transactions/1042"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "__type": "UnexpectedError" })))
        .expect(1)
        .mount(&servers.payment)
        .await;

    let app = build_test_server(&servers).await;
    let resp = app
        .get("/appointment/v1/BK20220227810949")
        .add_header(DOCTOR_HEADER, DOCTOR_HEADER_VALUE)
        .await;
    resp.assert_status(axum::http::StatusCode::BAD_GATEWAY);
}
```

- [ ] **Step 2: Add the network-retry test**

We use a stateful mock that returns 503 on the first call and 200 on the second to verify retry-once works on network 5xx.

Append to the test file:

```rust
#[tokio::test]
async fn iam_5xx_first_then_success_on_retry() {
    let servers = start_servers().await;
    mock_consultation(&servers, consultation_success_body()).await;

    // First mount: respond once with 503 (highest priority by mount order).
    Mock::given(method("GET"))
        .and(path("/iam/v1/internal/profile/by-account/124236"))
        .respond_with(ResponseTemplate::new(503))
        .up_to_n_times(1)
        .mount(&servers.iam)
        .await;
    // Second mount: catches the retry attempt with a 200.
    Mock::given(method("GET"))
        .and(path("/iam/v1/internal/profile/by-account/124236"))
        .respond_with(ResponseTemplate::new(200).set_body_json(iam_success_body()))
        .mount(&servers.iam)
        .await;

    mock_payment(&servers, payment_success_body_insurance_v1()).await;

    let app = build_test_server(&servers).await;
    let resp = app
        .get("/appointment/v1/BK20220227810949")
        .add_header(DOCTOR_HEADER, DOCTOR_HEADER_VALUE)
        .await;
    resp.assert_status_ok();
    let body: Value = resp.json();
    assert_eq!(body["__type"], "Success");
    assert_eq!(body["patient"]["fullName"], "Mrs.Bunyang Lopez");
}
```

- [ ] **Step 3: Add the unknown-channel-falls-back test**

Append:

```rust
#[tokio::test]
async fn unknown_payment_channel_falls_back_to_self_pay() {
    let servers = start_servers().await;
    mock_consultation(&servers, consultation_success_body()).await;
    mock_iam(&servers, iam_success_body()).await;
    let mut payment_body = payment_success_body_insurance_v1();
    payment_body["detail"]["selectedChannelResult"] = json!({
        "__type": "SelectedChannelResult.CoverageChannel",
        "channel": {
            "__type": "PaymentChannelResult.SomeFutureChannelType",
            "weirdField": 42
        }
    });
    mock_payment(&servers, payment_body).await;

    let app = build_test_server(&servers).await;
    let resp = app
        .get("/appointment/v1/BK20220227810949")
        .add_header(DOCTOR_HEADER, DOCTOR_HEADER_VALUE)
        .await;
    resp.assert_status_ok();
    let body: Value = resp.json();
    assert_eq!(body["__type"], "Success");
    assert_eq!(body["payment"]["payerName"], "Self pay");
    assert_eq!(body["payment"]["hasInsurance"], false);
}
```

- [ ] **Step 4: Add IAM-profile-fields-missing tests**

Append:

```rust
#[tokio::test]
async fn iam_profile_missing_dob_yields_null_age() {
    let servers = start_servers().await;
    mock_consultation(&servers, consultation_success_body()).await;
    mock_iam(
        &servers,
        json!({
            "__type": "InternalGetProfileByAccountId.Result.Success",
            "profile": {
                "firstName": "Mrs.Bunyang",
                "lastName": "Lopez",
                "gender": "Female"
                // dateOfBirth omitted
            }
        }),
    )
    .await;
    mock_payment(&servers, payment_success_body_insurance_v1()).await;

    let app = build_test_server(&servers).await;
    let resp = app
        .get("/appointment/v1/BK20220227810949")
        .add_header(DOCTOR_HEADER, DOCTOR_HEADER_VALUE)
        .await;
    resp.assert_status_ok();
    let body: Value = resp.json();
    assert!(body["patient"]["dateOfBirth"].is_null());
    assert!(body["patient"]["age"].is_null());
}

#[tokio::test]
async fn iam_profile_missing_both_names_yields_null_full_name() {
    let servers = start_servers().await;
    mock_consultation(&servers, consultation_success_body()).await;
    mock_iam(
        &servers,
        json!({
            "__type": "InternalGetProfileByAccountId.Result.Success",
            "profile": {
                "gender": "Female",
                "dateOfBirth": "1957-03-22"
            }
        }),
    )
    .await;
    mock_payment(&servers, payment_success_body_insurance_v1()).await;

    let app = build_test_server(&servers).await;
    let resp = app
        .get("/appointment/v1/BK20220227810949")
        .add_header(DOCTOR_HEADER, DOCTOR_HEADER_VALUE)
        .await;
    resp.assert_status_ok();
    let body: Value = resp.json();
    assert!(body["patient"]["fullName"].is_null());
}
```

- [ ] **Step 5: Add auth tests (401 and 403)**

Append:

```rust
#[tokio::test]
async fn missing_identity_header_returns_401() {
    let servers = start_servers().await;
    let app = build_test_server(&servers).await;
    let resp = app.get("/appointment/v1/BK20220227810949").await;
    resp.assert_status(axum::http::StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn non_doctor_account_type_returns_403() {
    // accountType=0 is a regular patient, not 2 (doctor).
    let patient_header =
        r#"{"tenantId":1,"accountId":300,"accountType":0,"userProfileId":400,"userMainProfileId":400,"oidcUserId":null,"legacyData":null}"#;
    let servers = start_servers().await;
    let app = build_test_server(&servers).await;
    let resp = app
        .get("/appointment/v1/BK20220227810949")
        .add_header(DOCTOR_HEADER, patient_header)
        .await;
    resp.assert_status(axum::http::StatusCode::FORBIDDEN);
}
```

- [ ] **Step 6: Run the full appointment test file**

```bash
cargo test --test appointment_detail_test
```
Expected: all tests pass (5 happy path from Task 14 + 5 not-found/error + 1 retry + 1 unknown channel + 2 missing-field + 2 auth = 16 tests).

- [ ] **Step 7: Commit**

```bash
git add server/tests/appointment_detail_test.rs
git commit -m "test(appointment): add error, retry, edge case, and auth tests

- Consultation appointmentNotFound short-circuits
- IAM AccountNotFound / ProfileNotFound -> PatientProfileNotFound
- Payment NotFound -> Success with payment: null
- Payment UnexpectedError -> 502, asserts no retry of domain variants
- IAM 5xx then success on retry -> Success
- Unknown payment channel -> defensive Self pay fallback
- Missing dateOfBirth / both names -> null fields
- Missing or non-doctor identity header -> 401 / 403"
```

---

## Task 16: Coupon edge-case integration tests

Cover the coupon-specific edge cases in the same integration test file.

**Files:**
- Modify: `server/tests/appointment_detail_test.rs`

- [ ] **Step 1: Add the four coupon edge-case tests**

Append to the file:

```rust
#[tokio::test]
async fn coupon_campaign_name_with_punctuation_slugifies_correctly() {
    let servers = start_servers().await;
    mock_consultation(&servers, consultation_success_body()).await;
    mock_iam(&servers, iam_success_body()).await;
    let mut payment_body = payment_success_body_insurance_v1();
    payment_body["detail"]["couponProtocol"] = json!({
        "__type": "CouponProtocol.Coupon",
        "campaignName": "50% OFF — Doctor's Day!"
    });
    mock_payment(&servers, payment_body).await;

    let app = build_test_server(&servers).await;
    let resp = app
        .get("/appointment/v1/BK20220227810949")
        .add_header(DOCTOR_HEADER, DOCTOR_HEADER_VALUE)
        .await;
    resp.assert_status_ok();
    let body: Value = resp.json();
    assert_eq!(body["coupon"]["campaignName"], "50% OFF — Doctor's Day!");
    assert_eq!(
        body["coupon"]["conditionUrl"],
        "https://static.tdh.example/coupon/50-off-doctor-s-day.html"
    );
}

#[tokio::test]
async fn coupon_missing_campaign_name_collapses_to_null() {
    let servers = start_servers().await;
    mock_consultation(&servers, consultation_success_body()).await;
    mock_iam(&servers, iam_success_body()).await;
    let mut payment_body = payment_success_body_insurance_v1();
    payment_body["detail"]["couponProtocol"] = json!({
        "__type": "CouponProtocol.Coupon",
        "coupon": "XMAS2026"
        // no campaignName
    });
    mock_payment(&servers, payment_body).await;

    let app = build_test_server(&servers).await;
    let resp = app
        .get("/appointment/v1/BK20220227810949")
        .add_header(DOCTOR_HEADER, DOCTOR_HEADER_VALUE)
        .await;
    resp.assert_status_ok();
    let body: Value = resp.json();
    assert!(body["coupon"].is_null());
}

#[tokio::test]
async fn coupon_whitespace_campaign_name_collapses_to_null() {
    let servers = start_servers().await;
    mock_consultation(&servers, consultation_success_body()).await;
    mock_iam(&servers, iam_success_body()).await;
    let mut payment_body = payment_success_body_insurance_v1();
    payment_body["detail"]["couponProtocol"] = json!({
        "__type": "CouponProtocol.Coupon",
        "campaignName": "   "
    });
    mock_payment(&servers, payment_body).await;

    let app = build_test_server(&servers).await;
    let resp = app
        .get("/appointment/v1/BK20220227810949")
        .add_header(DOCTOR_HEADER, DOCTOR_HEADER_VALUE)
        .await;
    resp.assert_status_ok();
    let body: Value = resp.json();
    assert!(body["coupon"].is_null());
}

#[tokio::test]
async fn coupon_slugifies_to_empty_keeps_name_null_url() {
    let servers = start_servers().await;
    mock_consultation(&servers, consultation_success_body()).await;
    mock_iam(&servers, iam_success_body()).await;
    let mut payment_body = payment_success_body_insurance_v1();
    payment_body["detail"]["couponProtocol"] = json!({
        "__type": "CouponProtocol.Coupon",
        "campaignName": "!!!"
    });
    mock_payment(&servers, payment_body).await;

    let app = build_test_server(&servers).await;
    let resp = app
        .get("/appointment/v1/BK20220227810949")
        .add_header(DOCTOR_HEADER, DOCTOR_HEADER_VALUE)
        .await;
    resp.assert_status_ok();
    let body: Value = resp.json();
    assert_eq!(body["coupon"]["campaignName"], "!!!");
    assert!(body["coupon"]["conditionUrl"].is_null());
}
```

- [ ] **Step 2: Add the v3-no-key edge case integration test**

Append:

```rust
#[tokio::test]
async fn insurance_v3_no_key_yields_null_url() {
    let servers = start_servers().await;
    mock_consultation(&servers, consultation_success_body()).await;
    mock_iam(&servers, iam_success_body()).await;
    let mut payment_body = payment_success_body_insurance_v1();
    payment_body["detail"]["selectedChannelResult"]["channel"] = json!({
        "__type": "PaymentChannelResult.InsuranceV3"
        // no providerName, no providerAbbreviation
    });
    mock_payment(&servers, payment_body).await;

    let app = build_test_server(&servers).await;
    let resp = app
        .get("/appointment/v1/BK20220227810949")
        .add_header(DOCTOR_HEADER, DOCTOR_HEADER_VALUE)
        .await;
    resp.assert_status_ok();
    let body: Value = resp.json();
    assert_eq!(body["__type"], "Success");
    assert_eq!(body["payment"]["payerName"], "Insurance");
    assert_eq!(body["payment"]["hasInsurance"], true);
    assert!(body["payment"]["insuranceConditionUrl"].is_null());
}
```

- [ ] **Step 3: Run the full file**

```bash
cargo test --test appointment_detail_test
```
Expected: 21 tests pass (16 from Task 15 + 4 coupon + 1 insurance v3 no key).

- [ ] **Step 4: Commit**

```bash
git add server/tests/appointment_detail_test.rs
git commit -m "test(appointment): add coupon edge cases and insurance v3 no-key"
```

---

## Task 17: Startup-validation unit test

Two tiny tests that the router constructor rejects bad URL templates.

**Files:**
- Modify: `server/src/module/appointment/mod.rs`

- [ ] **Step 1: Add a `tests` mod with two failing-fast checks**

Append to the bottom of `server/src/module/appointment/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::validate_url_template;

    #[test]
    fn insurance_template_missing_placeholder_is_rejected() {
        let err = validate_url_template("https://static.tdh.com/insurance/foo.html", "{insurerKey}")
            .unwrap_err();
        assert!(err.contains("missing the required placeholder"));
        assert!(err.contains("{insurerKey}"));
    }

    #[test]
    fn coupon_template_missing_placeholder_is_rejected() {
        let err =
            validate_url_template("https://static.tdh.com/coupon/foo.html", "{couponKey}")
                .unwrap_err();
        assert!(err.contains("missing the required placeholder"));
        assert!(err.contains("{couponKey}"));
    }
}
```

- [ ] **Step 2: Run the new tests**

```bash
cargo test -p server --lib module::appointment::tests
```
Expected: 2 pass.

- [ ] **Step 3: Commit**

```bash
git add server/src/module/appointment/mod.rs
git commit -m "test(appointment): add startup template-validation tests"
```

---

## Task 18: Final sweep — full build, fmt, clippy, all tests

Quick verification that everything is green before declaring done.

**Files:** none

- [ ] **Step 1: cargo fmt**

```bash
cargo fmt -p server
```

- [ ] **Step 2: cargo clippy**

```bash
cargo clippy -p server -- -D warnings
```
Expected: no warnings. If clippy complains about anything in the new appointment module, fix it before committing.

- [ ] **Step 3: Full test suite (lib + integration)**

```bash
cargo test -p server
```
Expected: all existing tests still pass, plus the appointment unit tests (~43 in `mapper`, 3 in `models`, 2 in `mod`) and the integration test file (~21 tests).

- [ ] **Step 4: cargo build --release sanity check**

```bash
cargo build -p server --release
```
Expected: clean release build.

- [ ] **Step 5: If any changes were made by fmt or clippy, commit them**

```bash
git add -u
git commit -m "chore(appointment): fmt + clippy fixes from final sweep" || true
```

(The `|| true` lets this step pass cleanly if there's nothing to commit.)

- [ ] **Step 6: Verify the route is mounted via cargo run + curl smoke test**

In one terminal:
```bash
cargo run -p server
```

Wait until the log shows `Server listening on 0.0.0.0:8080`.

In another terminal:
```bash
curl -i http://localhost:8080/appointment/v1/BK20220227810949
```
Expected: HTTP 401 (we didn't pass the identity header, but the route exists). NOT 404. Stop the server with Ctrl+C.

If you get 404, the route isn't mounted — re-check Task 12.

---

## Spec coverage map

| Spec section | Implementing task(s) |
|---|---|
| Endpoint + auth (`DoctorIdentity`) | 11, 15 (auth tests) |
| Response shape (`ApiResponse` / `Success` / `AppointmentNotFound` / `PatientProfileNotFound`) | 4, 11, 14, 15 |
| Field reference table | 4 (types), 5 + 9 + 10 (derivations), 14 (assertions) |
| Payer mapping table | 9, 14, 15, 16 |
| Insurance condition URL | 5 (`build_url_from_template`), 10 (`compose`), 14, 16 |
| Coupon condition URL + slugifier | 5 (`slugify_campaign`), 10 (`extract_coupon`), 14, 16 |
| Orchestration (consultation → parallel IAM+payment) | 11, 14 |
| Retry policy | 6, 7, 8 (`send_with_retry` in each client), 15 |
| Variant → BFF behaviour matrix | 6, 7, 8, 11, 14, 15 |
| Error response shape | 2, 15 |
| Observability (spans, logs, no PII in logs) | 6, 7, 8, 11 |
| Module layout | 1, 4–11 |
| Config additions | 3, 11 |
| OpenAPI | 13 |
| Tests (1–20) | 4, 5, 9, 10, 14, 15, 16, 17 |
| Out of scope (Body Analyzer omitted) | (no task — explicitly skipped per spec) |

---

**End of plan.** When all 18 tasks are checked off, the BFF aggregator endpoint is shipped end-to-end.
