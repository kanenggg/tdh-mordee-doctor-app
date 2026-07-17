# Get & Submit Doctor Profile Draft — Refactor Design

## Goal

Refactor `get_onboarding` and `submit_onboarding` to align with two new PostgreSQL functions (`get_doctor_profile_draft`, `submit_doctor_profile_draft`). Rename application-layer identifiers to match DB nomenclature. Extend the GET response to include `status`. Wire submit to accept a full draft payload from the frontend and atomically set `PendingApproval` in SQL.

## Architecture

The refactor touches five layers: model, repo, service, handlers, and tests. The Patch + Filler pattern (already in place for save) is extended to submit. The `OnBoardingStub` / `OnBoarding` type split is preserved for serialization safety; `OnBoardingStub` gains a `status` field.

The DB functions drive the structural changes:
- `get_doctor_profile_draft` now returns full jsonb objects for all reference-data fields (previously IDs only), and returns `status`.
- `submit_doctor_profile_draft` takes the same 23 data parameters as `save_doctor_profile_draft`, hardcodes `status = PendingApproval`, and atomically inserts into `doctor_profile_transaction`.

**Tech stack:** Rust / Axum / sqlx / PostgreSQL / serde_json

---

## Section 1: Model (`model/onboarding.rs`)

`OnBoardingStub` gets `status: OnBoardingStatus` added back. `OnBoarding` (the HTTP response type) also gets `status: OnBoardingStatus`. `From<OnBoardingStub> for OnBoarding` passes status through.

`OnBoardingRequest`, `OnBoardingRequestPatch`, and `apply` are unchanged.

**Structural note:** Because the new SQL stores and retrieves full jsonb objects for reference-data fields, `OnBoardingStub` now carries populated `name`/`abbr` fields on `Profession`, `AcademicPosition`, etc. rather than empty strings. The `OnBoardingStub` (no `Serialize`) / `OnBoarding` (has `Serialize`) split is kept for type safety.

---

## Section 2: Repo (`module/onboarding/repo.rs`)

### `OnBoardingRow` — rewritten

Column mapping from new `get_doctor_profile_draft` SQL:

| New SQL column | Rust field type | Notes |
|---|---|---|
| `citizen_id` | `String` | unchanged |
| `profession` | `serde_json::Value` | jsonb → `Profession` |
| `academic_position` | `serde_json::Value` | jsonb → `AcademicPosition` |
| `first_name` | `serde_json::Value` | jsonb → `Localized` |
| `last_name` | `serde_json::Value` | jsonb → `Localized` |
| `license_number` | `String` | unchanged |
| `medical_school` | `serde_json::Value` | jsonb → `MedicalSchool` |
| `specialty` | `serde_json::Value` | jsonb → `Vec<Specialty>` (merged; `additional_specialties` always `[]` on read) |
| `special_interests` | `serde_json::Value` | jsonb → `Vec<Localized>` |
| `address_detail` | `String` | unchanged |
| `sub_district` | `serde_json::Value` | jsonb → `SubDistrict` |
| `district` | `serde_json::Value` | jsonb → `District` |
| `province` | `serde_json::Value` | jsonb → `Province` |
| `postal_code` | `i32` | → `PostalCode { id, description: "", district_id: district.id }` |
| `primary_workplace` | `serde_json::Value` | jsonb → `Vec<WorkPlace>` |
| `additional_workplace` | `serde_json::Value` | jsonb → `Vec<WorkPlace>` |
| `profile_image_url` | `String` | unchanged |
| `id_card_image_url` | `String` | unchanged |
| `book_bank_image_url` | `String` | unchanged |
| `medical_license_image_url` | `String` | unchanged |
| `education_license_image_url` | `Vec<String>` | unchanged |
| `status` | `OnBoardingStatusDb` | now returned |

Removed columns (no longer in SQL): `profession_id`, `academic_position_id`, `sub_district_id`, `district_id`, `province_id`, `postal_code_id`, `medical_school_id`, `primary_workplace_ids`, `additional_workplace_ids`, `additional_specialty`, `status_reason`.

**`postal_code` mapping:** `district_id` is taken from the already-decoded `district.id`, not defaulted to zero:
```rust
PostalCode {
    id: row.postal_code,
    description: String::new(),
    district_id: district.id,
}
```

**`status_reason` absent:** `Rejected { reason }` and `Deactivated { reason }` default to `reason: String::new()`.

### `OnBoardingRepo` trait changes

- `update_status` **removed** — submit now handles the status transition atomically in SQL.
- New method added:
  ```rust
  async fn submit_doctor_profile_draft(
      &self,
      doctor_account_id: i32,
      doctor_profile_id: i32,
      request: &OnBoardingRequest,
  ) -> AppResult<()>;
  ```

### `submit_doctor_profile_draft` impl

Identical 23-parameter binding sequence as `save_draft_doctor_profile`, but calls `submit_doctor_profile_draft($1…$23)` instead of `save_doctor_profile_draft($1…$23)`.

---

## Section 3: Service (`module/onboarding/services.rs`)

`submit_onboarding(doctor_id)` is replaced by:

```rust
async fn submit_doctor_profile_draft(
    &self,
    doctor_account_id: i32,
    doctor_profile_id: i32,
    patch: OnBoardingRequestPatch,
) -> AppResult<()>;
```

**Implementation flow (Patch + Filler):**

1. Apply `patch` over `OnBoardingRequest::default()` — Filler fills any absent fields with zero values.
2. Convert resolved `OnBoardingRequest` → `OnBoardingStub` via the existing `From` impl.
3. Run `validator.validate_onboarding_submission(&stub)` — all current validation rules unchanged.
4. Call `repo.submit_doctor_profile_draft(doctor_account_id, doctor_profile_id, &request)`.

No DB read. No `update_status` call. SQL handles `PendingApproval` + `doctor_profile_transaction` insert atomically.

`get_onboarding` and `save_draft_doctor_profile` are unchanged.

---

## Section 4: Handlers & routing (`handlers.rs`, `mod.rs`)

### Renamed handler functions

| Old name | New name |
|---|---|
| `get_onboarding` | `get_doctor_profile_draft` |
| `save_onboarding` | `save_doctor_profile_draft` |
| `submit_onboarding` | `submit_doctor_profile_draft` |

### `submit_doctor_profile_draft` handler

Now accepts a request body:

```rust
pub async fn submit_doctor_profile_draft(
    State(state): State<OnboardingState>,
    identity: UserIdentity,
    Json(input): Json<OnBoardingRequestPatch>,
) -> AppResult<impl IntoResponse> {
    state.service
        .submit_doctor_profile_draft(identity.account_id, identity.user_profile_id, input)
        .await?;
    Ok(StatusCode::OK.into_response())
}
```

### Routes in `mod.rs` — unchanged

`GET /`, `POST /`, `POST /submit` — only the handler function references are updated.

---

## Section 5: Tests (`tests/onboarding_test.rs`)

### Mock `OnBoardingRepo`

- `update_status` removed.
- `submit_doctor_profile_draft` added — stores the resolved `OnBoardingRequest` into the in-memory store (identical to save mock), so GET-after-submit assertions continue to work.

### Test changes

| Test | Change |
|---|---|
| `submit_changes_status_to_pending_approval` | Add request body (`create_valid_onboarding_request()`) to submit POST |
| `submit_returns_error_when_onboarding_not_found` | **Removed** — with new design, submit never pre-reads from DB; replace with a test that sends missing required fields and expects `400` |
| `submit_validates_required_documents` | Add request body to submit POST |
| `status_transition_from_approved_to_draft_is_invalid` | Unchanged |
| Handler rename references | `save_onboarding` → `save_doctor_profile_draft`, `submit_onboarding` → `submit_doctor_profile_draft`, `get_onboarding` → `get_doctor_profile_draft` in `create_test_server()` routes |

---

## What is NOT changed

- `OnBoardingRequest`, `OnBoardingRequestPatch`, `apply` — unchanged.
- `OnboardingValidator` interface and all validation rules — unchanged.
- `validate_onboarding_status_transition` — kept (used by backoffice module and tests).
- `OnBoardingStatusDb` and `to_domain` / `from_domain` conversions — unchanged except `to_domain` no longer receives a `reason` parameter for the `get` path (always passes `None`).
- HTTP routes and URL paths — unchanged.
- `save_draft_doctor_profile` throughout the stack — unchanged.
- `module/user/handlers.rs` `From<OnBoardingStub> for UserProfile` — unchanged; silently drops the new `status` field by design (UserProfile is documented to mirror the Scala shape that excludes `status`).
