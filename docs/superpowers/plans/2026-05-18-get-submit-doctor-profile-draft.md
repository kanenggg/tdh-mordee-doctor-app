# Get & Submit Doctor Profile Draft — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **IMPORTANT:** Do NOT `git add` or `git commit` at any task boundary. A single commit is made after Task 5 when all tests pass.

**Goal:** Refactor `get_onboarding` and `submit_onboarding` to align with `get_doctor_profile_draft` and `submit_doctor_profile_draft` SQL functions; include `status` in the GET response; rename application-layer identifiers to match DB nomenclature.

**Architecture:** `OnBoardingRow` is rewritten to match new SQL columns (full jsonb objects instead of bare IDs); `submit_doctor_profile_draft` applies the Patch+Filler pattern (same as save) and calls the SQL function that atomically sets `PendingApproval` and inserts a `doctor_profile_transaction` row; `update_status` is removed from the onboarding repo. Validation is unchanged — runs against the resolved `OnBoardingRequest` converted to an `OnBoardingStub`.

**Tech Stack:** Rust / Axum / sqlx (PostgreSQL) / serde_json / utoipa

---

## File map

| File | Change |
|---|---|
| `server/src/model/onboarding.rs` | Add `status` to `OnBoardingStub` and `OnBoarding`; update two `From` impls |
| `server/src/module/onboarding/repo.rs` | Rewrite `OnBoardingRow`; rewrite `From<OnBoardingRow>`; remove `update_status`; add `submit_doctor_profile_draft` |
| `server/src/module/onboarding/services.rs` | Remove `submit_onboarding`; add `submit_doctor_profile_draft` |
| `server/src/module/onboarding/handlers.rs` | Rename three handlers; add body to submit handler |
| `server/src/module/onboarding/mod.rs` | Update route handler references |
| `server/src/openapi.rs` | Rename three handler path references |
| `server/tests/onboarding_test.rs` | Update mock; rename routes; update/replace tests |

---

## Task 0: Add `status` to `OnBoardingStub` and `OnBoarding`

**Files:**
- Modify: `server/src/model/onboarding.rs`

- [ ] **Step 1: Add `status` to `OnBoardingStub`**

In `server/src/model/onboarding.rs`, replace:

```rust
#[derive(Debug, Clone)]
pub struct OnBoardingStub {
    pub citizen_id: String,
    pub profession: Profession,
    pub academic_position: AcademicPosition,
    pub first_name: Localized,
    pub last_name: Localized,
    pub address: Address,
    pub work_place: SelectedWorkPlace,
    pub education: Education,
    pub documents: Documents,
}
```

with:

```rust
#[derive(Debug, Clone)]
pub struct OnBoardingStub {
    pub citizen_id: String,
    pub profession: Profession,
    pub academic_position: AcademicPosition,
    pub first_name: Localized,
    pub last_name: Localized,
    pub address: Address,
    pub work_place: SelectedWorkPlace,
    pub education: Education,
    pub documents: Documents,
    pub status: OnBoardingStatus,
}
```

- [ ] **Step 2: Add `status` to `OnBoarding`**

In `server/src/model/onboarding.rs`, replace:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OnBoarding {
    pub profession: Profession,
    pub academic_position: AcademicPosition,
    pub citizen_id: String,
    pub first_name: Localized,
    pub last_name: Localized,
    pub address: Address,
    pub work_place: SelectedWorkPlace,
    pub education: Education,
    pub documents: Documents,
}
```

with:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OnBoarding {
    pub profession: Profession,
    pub academic_position: AcademicPosition,
    pub citizen_id: String,
    pub first_name: Localized,
    pub last_name: Localized,
    pub address: Address,
    pub work_place: SelectedWorkPlace,
    pub education: Education,
    pub documents: Documents,
    pub status: OnBoardingStatus,
}
```

- [ ] **Step 3: Update `From<OnBoardingStub> for OnBoarding`**

Replace:

```rust
impl From<OnBoardingStub> for OnBoarding {
    fn from(stub: OnBoardingStub) -> Self {
        OnBoarding {
            profession: stub.profession,
            academic_position: stub.academic_position,
            citizen_id: stub.citizen_id,
            first_name: stub.first_name,
            last_name: stub.last_name,
            address: stub.address,
            work_place: stub.work_place,
            education: stub.education,
            documents: stub.documents,
        }
    }
}
```

with:

```rust
impl From<OnBoardingStub> for OnBoarding {
    fn from(stub: OnBoardingStub) -> Self {
        OnBoarding {
            profession: stub.profession,
            academic_position: stub.academic_position,
            citizen_id: stub.citizen_id,
            first_name: stub.first_name,
            last_name: stub.last_name,
            address: stub.address,
            work_place: stub.work_place,
            education: stub.education,
            documents: stub.documents,
            status: stub.status,
        }
    }
}
```

- [ ] **Step 4: Update `From<OnBoardingRequest> for OnBoardingStub`**

Replace:

```rust
impl From<OnBoardingRequest> for OnBoardingStub {
    fn from(input: OnBoardingRequest) -> Self {
        OnBoardingStub {
            profession: input.profession,
            academic_position: input.academic_position,
            citizen_id: input.citizen_id,
            first_name: input.first_name,
            last_name: input.last_name,
            address: input.address,
            work_place: SelectedWorkPlace {
                primary: input.work_place.primary,
                additional: input.work_place.additional,
            },
            education: Education {
                license_number: input.education.license_number,
                medical_school: input.education.medical_school,
                specialties: input.education.specialties,
                additional_specialties: input.education.additional_specialties,
                special_interests: input.education.special_interests,
            },
            documents: input.documents,
        }
    }
}
```

with:

```rust
impl From<OnBoardingRequest> for OnBoardingStub {
    fn from(input: OnBoardingRequest) -> Self {
        OnBoardingStub {
            profession: input.profession,
            academic_position: input.academic_position,
            citizen_id: input.citizen_id,
            first_name: input.first_name,
            last_name: input.last_name,
            address: input.address,
            work_place: SelectedWorkPlace {
                primary: input.work_place.primary,
                additional: input.work_place.additional,
            },
            education: Education {
                license_number: input.education.license_number,
                medical_school: input.education.medical_school,
                specialties: input.education.specialties,
                additional_specialties: input.education.additional_specialties,
                special_interests: input.education.special_interests,
            },
            documents: input.documents,
            status: OnBoardingStatus::Draft,
        }
    }
}
```

- [ ] **Step 5: Verify**

Run: `cargo check --lib`

Expected: compile error E0063 `missing field 'status'` in `repo.rs` `From<OnBoardingRow>` — this is correct, it will be fixed in Task 1. All other errors at this point indicate a mistake in the steps above.

---

## Task 1: Rewrite `OnBoardingRow` and `From<OnBoardingRow> for OnBoardingStub`

**Files:**
- Modify: `server/src/module/onboarding/repo.rs`

The new `get_doctor_profile_draft` SQL returns full jsonb objects for all reference-data fields and includes `status`. The old row struct used bare integer IDs.

- [ ] **Step 1: Replace the `OnBoardingRow` struct**

In `server/src/module/onboarding/repo.rs`, replace the entire `OnBoardingRow` struct (the `#[derive(FromRow)]` block and all its fields) with:

```rust
/// Raw database row returned by get_doctor_profile_draft().
/// Field names must match the SQL function's TABLE column names exactly.
#[derive(FromRow)]
struct OnBoardingRow {
    citizen_id: String,
    profession: serde_json::Value,
    academic_position: serde_json::Value,
    first_name: serde_json::Value,
    last_name: serde_json::Value,
    license_number: String,
    medical_school: serde_json::Value,
    specialty: serde_json::Value,
    special_interests: serde_json::Value,
    address_detail: String,
    sub_district: serde_json::Value,
    district: serde_json::Value,
    province: serde_json::Value,
    postal_code: i32,
    primary_workplace: serde_json::Value,
    additional_workplace: serde_json::Value,
    profile_image_url: String,
    id_card_image_url: String,
    book_bank_image_url: String,
    medical_license_image_url: String,
    education_license_image_url: Vec<String>,
    status: OnBoardingStatusDb,
}
```

- [ ] **Step 2: Replace `From<OnBoardingRow> for OnBoardingStub`**

Replace the entire `impl From<OnBoardingRow> for OnBoardingStub` block with:

```rust
impl From<OnBoardingRow> for OnBoardingStub {
    fn from(row: OnBoardingRow) -> Self {
        let empty_loc = || Localized { th: String::new(), en: String::new() };

        let first_name: Localized = serde_json::from_value(row.first_name)
            .unwrap_or_else(|e| { warn!(error = %e, "failed to deserialize first_name"); empty_loc() });
        let last_name: Localized = serde_json::from_value(row.last_name)
            .unwrap_or_else(|e| { warn!(error = %e, "failed to deserialize last_name"); empty_loc() });
        let profession: Profession = serde_json::from_value(row.profession)
            .unwrap_or_else(|e| { warn!(error = %e, "failed to deserialize profession"); Profession::default() });
        let academic_position: AcademicPosition = serde_json::from_value(row.academic_position)
            .unwrap_or_else(|e| { warn!(error = %e, "failed to deserialize academic_position"); AcademicPosition::default() });
        let medical_school: MedicalSchool = serde_json::from_value(row.medical_school)
            .unwrap_or_else(|e| { warn!(error = %e, "failed to deserialize medical_school"); MedicalSchool::default() });
        let sub_district: SubDistrict = serde_json::from_value(row.sub_district)
            .unwrap_or_else(|e| { warn!(error = %e, "failed to deserialize sub_district"); SubDistrict::default() });
        let district: District = serde_json::from_value(row.district)
            .unwrap_or_else(|e| { warn!(error = %e, "failed to deserialize district"); District::default() });
        let province: Province = serde_json::from_value(row.province)
            .unwrap_or_else(|e| { warn!(error = %e, "failed to deserialize province"); Province::default() });
        let primary_workplace: Vec<WorkPlace> = serde_json::from_value(row.primary_workplace)
            .unwrap_or_else(|e| { warn!(error = %e, "failed to deserialize primary_workplace"); vec![] });
        let additional_workplace: Vec<WorkPlace> = serde_json::from_value(row.additional_workplace)
            .unwrap_or_else(|e| { warn!(error = %e, "failed to deserialize additional_workplace"); vec![] });
        let specialties: Vec<Specialty> = serde_json::from_value(row.specialty)
            .unwrap_or_else(|e| { warn!(error = %e, "failed to deserialize specialty"); vec![] });
        let special_interests: Vec<Localized> = serde_json::from_value(row.special_interests)
            .unwrap_or_else(|e| { warn!(error = %e, "failed to deserialize special_interests"); vec![] });
        let postal_code = PostalCode {
            id: row.postal_code,
            description: String::new(),
            district_id: district.id,
        };

        Self {
            citizen_id: row.citizen_id,
            profession,
            academic_position,
            first_name,
            last_name,
            address: Address {
                address_detail: row.address_detail,
                sub_district,
                district,
                province,
                postal_code,
            },
            work_place: SelectedWorkPlace {
                primary: primary_workplace,
                additional: additional_workplace,
            },
            education: Education {
                license_number: row.license_number,
                medical_school,
                specialties,
                additional_specialties: vec![],
                special_interests,
            },
            documents: Documents {
                profile_image_url: row.profile_image_url,
                id_card_image_url: row.id_card_image_url,
                book_bank_image_url: row.book_bank_image_url,
                med_license_image_url: row.medical_license_image_url,
                certificate_image_urls: row.education_license_image_url,
            },
            status: row.status.to_domain(None),
        }
    }
}
```

- [ ] **Step 3: Verify**

Run: `cargo check --lib`

Expected: compile errors about `update_status` in `services.rs` (the service still calls it). No errors in `model/` or `repo.rs` row mapping at this point.

---

## Task 2: Update `OnBoardingRepo` trait — remove `update_status`, add `submit_doctor_profile_draft`

**Files:**
- Modify: `server/src/module/onboarding/repo.rs`

- [ ] **Step 1: Update the `OnBoardingRepo` trait**

Replace:

```rust
#[async_trait]
pub trait OnBoardingRepo: Send + Sync {
    async fn get_onboarding(&self, doctor_id: i32) -> AppResult<Option<OnBoardingStub>>;
    async fn save_draft_doctor_profile(
        &self,
        doctor_account_id: i32,
        doctor_profile_id: i32,
        request: &OnBoardingRequest,
    ) -> AppResult<()>;
    async fn update_status(&self, doctor_id: i32, status: OnBoardingStatus) -> AppResult<()>;
}
```

with:

```rust
#[async_trait]
pub trait OnBoardingRepo: Send + Sync {
    async fn get_onboarding(&self, doctor_id: i32) -> AppResult<Option<OnBoardingStub>>;
    async fn save_draft_doctor_profile(
        &self,
        doctor_account_id: i32,
        doctor_profile_id: i32,
        request: &OnBoardingRequest,
    ) -> AppResult<()>;
    async fn submit_doctor_profile_draft(
        &self,
        doctor_account_id: i32,
        doctor_profile_id: i32,
        request: &OnBoardingRequest,
    ) -> AppResult<()>;
}
```

- [ ] **Step 2: Remove `update_status` from `OnBoardingRepoImp`**

Delete the entire `update_status` impl block from the `impl OnBoardingRepo for OnBoardingRepoImp` section:

```rust
    async fn update_status(&self, doctor_id: i32, status: OnBoardingStatus) -> AppResult<()> {
        let (status, status_reason) = OnBoardingStatusDb::from_domain(status);

        sqlx::query(
            r#"
            SELECT update_doctor_onboarding_status($1, $2, $3)
            "#,
        )
        .bind(doctor_id)
        .bind(&status)
        .bind(&status_reason)
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::InternalError(format!("Database error: {}", e)))?;

        Ok(())
    }
```

Also delete the `from_domain` method from `impl OnBoardingStatusDb` (it was only used by `update_status`):

```rust
    pub fn from_domain(status: OnBoardingStatus) -> (Self, Option<String>) {
        match status {
            OnBoardingStatus::Draft => (Self::Draft, None),
            OnBoardingStatus::PendingApproval => (Self::PendingApproval, None),
            OnBoardingStatus::CancelledByUser => (Self::CancelledByUser, None),
            OnBoardingStatus::Approved => (Self::Approved, None),
            OnBoardingStatus::Rejected { reason } => (Self::Rejected, Some(reason)),
            OnBoardingStatus::Deactivated { reason } => (Self::Deactivated, Some(reason)),
        }
    }
```

- [ ] **Step 3: Add `submit_doctor_profile_draft` to `OnBoardingRepoImp`**

Inside the `impl OnBoardingRepo for OnBoardingRepoImp` block, add after `save_draft_doctor_profile`:

```rust
    async fn submit_doctor_profile_draft(
        &self,
        doctor_account_id: i32,
        doctor_profile_id: i32,
        request: &OnBoardingRequest,
    ) -> AppResult<()> {
        use anyhow::Context;

        let profession_json = serde_json::to_value(&request.profession)
            .context("serialize profession")?;
        let academic_pos_json = serde_json::to_value(&request.academic_position)
            .context("serialize academic_position")?;
        let first_name_json = serde_json::to_value(&request.first_name)
            .context("serialize first_name")?;
        let last_name_json = serde_json::to_value(&request.last_name)
            .context("serialize last_name")?;
        let medical_school_json = serde_json::to_value(&request.education.medical_school)
            .context("serialize medical_school")?;
        let sub_district_json = serde_json::to_value(&request.address.sub_district)
            .context("serialize sub_district")?;
        let district_json = serde_json::to_value(&request.address.district)
            .context("serialize district")?;
        let province_json = serde_json::to_value(&request.address.province)
            .context("serialize province")?;
        let primary_wp_json = serde_json::to_value(&request.work_place.primary)
            .context("serialize primary_workplace")?;
        let additional_wp_json = serde_json::to_value(&request.work_place.additional)
            .context("serialize additional_workplace")?;
        let specialty_json = serde_json::to_value(
            request
                .education
                .specialties
                .iter()
                .chain(request.education.additional_specialties.iter())
                .collect::<Vec<_>>(),
        )
        .context("serialize specialty")?;
        let special_interests: Vec<String> = request
            .education
            .special_interests
            .iter()
            .map(|loc| serde_json::to_string(loc).context("serialize special_interest"))
            .collect::<Result<_, _>>()?;

        sqlx::query(
            r#"
            SELECT submit_doctor_profile_draft(
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15,
                $16, $17, $18, $19, $20, $21, $22, $23
            )
            "#,
        )
        .bind(doctor_account_id)                         // $1  p_doctor_account_id
        .bind(doctor_profile_id)                         // $2  p_doctor_profile_id
        .bind(&request.citizen_id)                       // $3  p_citizen_id
        .bind(profession_json)                           // $4  p_profession
        .bind(academic_pos_json)                         // $5  p_academic_position
        .bind(first_name_json)                           // $6  p_first_name
        .bind(last_name_json)                            // $7  p_last_name
        .bind(&request.education.license_number)         // $8  p_license_number
        .bind(medical_school_json)                       // $9  p_medical_school
        .bind(specialty_json)                            // $10 p_specialty (merged)
        .bind(&special_interests)                        // $11 p_special_interests
        .bind(&request.address.address_detail)           // $12 p_address_detail
        .bind(sub_district_json)                         // $13 p_sub_district
        .bind(district_json)                             // $14 p_district
        .bind(province_json)                             // $15 p_province
        .bind(request.address.postal_code.id)            // $16 p_postal_code
        .bind(primary_wp_json)                           // $17 p_primary_workplace
        .bind(additional_wp_json)                        // $18 p_additional_workplace
        .bind(&request.documents.profile_image_url)      // $19 p_profile_image_url
        .bind(&request.documents.id_card_image_url)      // $20 p_id_card_image_url
        .bind(&request.documents.book_bank_image_url)    // $21 p_book_bank_image_url
        .bind(&request.documents.med_license_image_url)  // $22 p_medical_license_image_url
        .bind(&request.documents.certificate_image_urls) // $23 p_education_certificate_image_urls
        .execute(&self.pool)
        .await?;

        Ok(())
    }
```

- [ ] **Step 4: Verify**

Run: `cargo check --lib`

Expected: compile errors in `services.rs` about `submit_onboarding` / `update_status` still being referenced. No errors in `repo.rs`.

---

## Task 3: Replace `submit_onboarding` with `submit_doctor_profile_draft` in the service

**Files:**
- Modify: `server/src/module/onboarding/services.rs`

- [ ] **Step 1: Update `OnboardingServiceTrait`**

Replace:

```rust
#[async_trait]
pub trait OnboardingServiceTrait: Send + Sync {
    async fn get_onboarding(&self, doctor_id: i32) -> AppResult<Option<OnBoarding>>;
    async fn save_draft_doctor_profile(
        &self,
        doctor_account_id: i32,
        doctor_profile_id: i32,
        patch: OnBoardingRequestPatch,
    ) -> AppResult<()>;
    async fn submit_onboarding(&self, doctor_id: i32) -> AppResult<()>;
}
```

with:

```rust
#[async_trait]
pub trait OnboardingServiceTrait: Send + Sync {
    async fn get_onboarding(&self, doctor_id: i32) -> AppResult<Option<OnBoarding>>;
    async fn save_draft_doctor_profile(
        &self,
        doctor_account_id: i32,
        doctor_profile_id: i32,
        patch: OnBoardingRequestPatch,
    ) -> AppResult<()>;
    async fn submit_doctor_profile_draft(
        &self,
        doctor_account_id: i32,
        doctor_profile_id: i32,
        patch: OnBoardingRequestPatch,
    ) -> AppResult<()>;
}
```

- [ ] **Step 2: Remove `onboarding_not_found_error` helper**

Delete the `onboarding_not_found_error` function from `impl OnboardingService`:

```rust
    fn onboarding_not_found_error() -> AppError {
        AppError::BadRequest(
            "Onboarding information not found. Please save your information first.".to_string(),
        )
    }
```

- [ ] **Step 3: Replace `submit_onboarding` impl with `submit_doctor_profile_draft`**

Remove the entire `submit_onboarding` async fn from `impl OnboardingServiceTrait for OnboardingService` and add:

```rust
    async fn submit_doctor_profile_draft(
        &self,
        doctor_account_id: i32,
        doctor_profile_id: i32,
        patch: OnBoardingRequestPatch,
    ) -> AppResult<()> {
        let mut request = OnBoardingRequest::default();
        request.apply(patch);
        let stub = OnBoardingStub::from(request.clone());
        self.validator.validate_onboarding_submission(&stub)?;
        self.repo
            .submit_doctor_profile_draft(doctor_account_id, doctor_profile_id, &request)
            .await?;
        Ok(())
    }
```

- [ ] **Step 4: Update imports**

The import at the top of `services.rs` currently includes `OnBoardingStatus` and `AppError`. After Step 2 removed `onboarding_not_found_error`, no code in this file constructs `AppError` directly (`AppResult` is the alias `Result<T, AppError>` and doesn't require importing the error type). `OnBoardingStatus` is no longer used either. The updated import block:

```rust
use crate::{
    core::error::AppResult,
    model::onboarding::{OnBoarding, OnBoardingRequest, OnBoardingRequestPatch, OnBoardingStub},
    module::onboarding::{OnBoardingRepo, OnboardingValidator},
};
```

- [ ] **Step 5: Verify**

Run: `cargo check --lib`

Expected: compile errors in `handlers.rs` about `submit_onboarding` not existing. No errors in `services.rs`.

---

## Task 4: Rename handlers, update submit handler, update routes and OpenAPI

**Files:**
- Modify: `server/src/module/onboarding/handlers.rs`
- Modify: `server/src/module/onboarding/mod.rs`
- Modify: `server/src/openapi.rs`

- [ ] **Step 1: Rename `get_onboarding` to `get_doctor_profile_draft`**

In `server/src/module/onboarding/handlers.rs`, rename the function `get_onboarding` to `get_doctor_profile_draft`. The body and `#[utoipa::path]` attribute are unchanged.

- [ ] **Step 2: Rename `save_onboarding` to `save_doctor_profile_draft`**

Rename the function `save_onboarding` to `save_doctor_profile_draft`. Body and attribute unchanged.

- [ ] **Step 3: Replace `submit_onboarding` with `submit_doctor_profile_draft`**

Replace the entire `submit_onboarding` handler with:

```rust
#[utoipa::path(
    post,
    path = "/onboarding/v1/submit",
    tag = "onboarding",
    request_body = OnBoardingRequestPatch,
    responses(
        (status = 200, description = "Submitted for approval"),
        (status = 400, description = "Validation failed"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("TDH-SEC-IAM-USER-IDENTITY" = []))
)]
pub async fn submit_doctor_profile_draft(
    State(state): State<OnboardingState>,
    identity: UserIdentity,
    Json(input): Json<OnBoardingRequestPatch>,
) -> AppResult<impl IntoResponse> {
    state
        .service
        .submit_doctor_profile_draft(identity.account_id, identity.user_profile_id, input)
        .await?;
    Ok(StatusCode::OK.into_response())
}
```

- [ ] **Step 4: Update routes in `mod.rs`**

In `server/src/module/onboarding/mod.rs`, replace:

```rust
    let r = Router::new()
        .route(
            "/",
            get(handlers::get_onboarding).post(handlers::save_onboarding),
        )
        .route("/submit", post(handlers::submit_onboarding))
        .with_state(state);
```

with:

```rust
    let r = Router::new()
        .route(
            "/",
            get(handlers::get_doctor_profile_draft).post(handlers::save_doctor_profile_draft),
        )
        .route("/submit", post(handlers::submit_doctor_profile_draft))
        .with_state(state);
```

- [ ] **Step 5: Update handler path references in `openapi.rs`**

In `server/src/openapi.rs`, replace:

```rust
        crate::module::onboarding::handlers::get_onboarding,
        crate::module::onboarding::handlers::save_onboarding,
        crate::module::onboarding::handlers::submit_onboarding,
```

with:

```rust
        crate::module::onboarding::handlers::get_doctor_profile_draft,
        crate::module::onboarding::handlers::save_doctor_profile_draft,
        crate::module::onboarding::handlers::submit_doctor_profile_draft,
```

- [ ] **Step 6: Verify**

Run: `cargo check --lib`

Expected: `Finished` with warnings only — no errors. The library itself should compile cleanly. (Do NOT run `cargo check` or `cargo check --tests` here — the integration test crate still references old mock and route names and will fail until Task 5.)

---

## Task 5: Update integration tests

**Files:**
- Modify: `server/tests/onboarding_test.rs`

The mock `MockOnboardingRepo` still implements the old trait (with `update_status`, without `submit_doctor_profile_draft`). The test server routes still reference old handler names. Several tests need body updates or replacement.

- [ ] **Step 1: Remove `update_status` from the mock, add `submit_doctor_profile_draft`**

In the `impl OnBoardingRepo for MockOnboardingRepo` block, replace the `update_status` method:

```rust
    async fn update_status(&self, doctor_id: i32, _status: OnBoardingStatus) -> AppResult<()> {
        let store = self.store.lock().unwrap();
        let key = doctor_id.to_string();
        if store.contains_key(&key) {
            Ok(())
        } else {
            Err(AppError::BadRequest(format!(
                "Onboarding not found for doctor: {}",
                doctor_id
            )))
        }
    }
```

with:

```rust
    async fn submit_doctor_profile_draft(
        &self,
        doctor_account_id: i32,
        _doctor_profile_id: i32,
        request: &OnBoardingRequest,
    ) -> AppResult<()> {
        let mut store = self.store.lock().unwrap();
        let stub = OnBoardingStub::from(request.clone());
        store.insert(doctor_account_id.to_string(), stub);
        Ok(())
    }
```

- [ ] **Step 2: Verify `OnBoardingStatus` import is retained**

The test file's import block includes `OnBoardingStatus` (previously used in the `update_status` mock). It is still needed for the `status_transition_from_approved_to_draft_is_invalid` test. No change required — just confirm it remains in the import block after Step 1.

- [ ] **Step 3: Update `create_test_server` route references**

Replace:

```rust
    let app = Router::new()
        .route(
            "/",
            axum::routing::get(onboarding::handlers::get_onboarding)
                .post(onboarding::handlers::save_onboarding),
        )
        .route(
            "/submit",
            axum::routing::post(onboarding::handlers::submit_onboarding),
        )
        .with_state(state);
```

with:

```rust
    let app = Router::new()
        .route(
            "/",
            axum::routing::get(onboarding::handlers::get_doctor_profile_draft)
                .post(onboarding::handlers::save_doctor_profile_draft),
        )
        .route(
            "/submit",
            axum::routing::post(onboarding::handlers::submit_doctor_profile_draft),
        )
        .with_state(state);
```

- [ ] **Step 4: Update `save_always_sets_status_to_draft` — add status assertion**

Since `OnBoarding` now includes `status`, the GET response body will contain `"Draft"`. Update the assertion:

Replace:

```rust
    let body = get_response.text();
    assert!(body.contains("OnBoardingResponse"));
    assert!(!body.contains("Approved"));
```

with:

```rust
    let body = get_response.text();
    assert!(body.contains("OnBoardingResponse"));
    assert!(body.contains(r#""__type":"Draft""#));
    assert!(!body.contains("Approved"));
```

- [ ] **Step 5: Update `submit_changes_status_to_pending_approval` — add request body**

Submit now requires a body. Replace:

```rust
#[tokio::test]
async fn submit_changes_status_to_pending_approval() {
    let server = create_test_server();

    server
        .post("/")
        .add_header(AUTH_HEADER, doctor_identity(123))
        .json(&create_valid_onboarding_request())
        .await;

    let response = server
        .post("/submit")
        .add_header(AUTH_HEADER, doctor_identity(123))
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);
}
```

with:

```rust
#[tokio::test]
async fn submit_changes_status_to_pending_approval() {
    let server = create_test_server();

    let response = server
        .post("/submit")
        .add_header(AUTH_HEADER, doctor_identity(123))
        .json(&create_valid_onboarding_request())
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);
}
```

- [ ] **Step 6: Replace `submit_returns_error_when_onboarding_not_found` with `submit_validates_required_fields_before_saving`**

Delete the old test:

```rust
#[tokio::test]
async fn submit_returns_error_when_onboarding_not_found() {
    let server = create_test_server();

    let response = server
        .post("/submit")
        .add_header(AUTH_HEADER, doctor_identity(999))
        .await;

    assert_eq!(response.status_code(), StatusCode::BAD_REQUEST);
    let body = response.text();
    assert!(body.contains("not found"));
}
```

Replace with:

```rust
#[tokio::test]
async fn submit_validates_required_fields_before_saving() {
    let server = create_test_server();

    // Empty patch: all fields default — citizen_id is "" (not 13 chars), fails validation
    let response = server
        .post("/submit")
        .add_header(AUTH_HEADER, doctor_identity(999))
        .json(&json!({}))
        .await;

    assert_eq!(response.status_code(), StatusCode::BAD_REQUEST);
}
```

- [ ] **Step 7: Update `submit_validates_required_documents` — add request body**

Replace:

```rust
#[tokio::test]
async fn submit_validates_required_documents() {
    let server = create_test_server();
    let mut input = create_valid_onboarding_request();
    input.documents.profile_image_url = String::new();

    server
        .post("/")
        .add_header(AUTH_HEADER, doctor_identity(123))
        .json(&input)
        .await;

    let response = server
        .post("/submit")
        .add_header(AUTH_HEADER, doctor_identity(123))
        .await;

    assert_eq!(response.status_code(), StatusCode::BAD_REQUEST);
    let body = response.text();
    assert!(body.contains("profile_image_url") || body.contains("profile"));
}
```

with:

```rust
#[tokio::test]
async fn submit_validates_required_documents() {
    let server = create_test_server();
    let mut input = create_valid_onboarding_request();
    input.documents.profile_image_url = String::new();

    let response = server
        .post("/submit")
        .add_header(AUTH_HEADER, doctor_identity(123))
        .json(&input)
        .await;

    assert_eq!(response.status_code(), StatusCode::BAD_REQUEST);
    let body = response.text();
    assert!(body.contains("profile_image_url") || body.contains("profile"));
}
```

- [ ] **Step 8: Run all tests**

Run: `cargo test --test onboarding_test -- --nocapture`

Expected output:
```
running 13 tests
test get_onboarding_returns_not_found_for_new_doctor ... ok
test post_saves_draft_onboarding ... ok
test save_always_sets_status_to_draft ... ok
test post_ignores_extra_fields_like_description ... ok
test submit_changes_status_to_pending_approval ... ok
test submit_validates_required_fields_before_saving ... ok
test submit_validates_required_documents ... ok
test validation_requires_all_documents ... ok
test validation_limits_certificates_to_six ... ok
test validation_allows_six_certificates ... ok
test status_transition_from_approved_to_draft_is_invalid ... ok
test partial_first_save_only_citizen_id_uses_defaults_for_rest ... ok
test omitted_address_section_saves_with_default_address ... ok

test result: ok. 13 passed; 0 failed; 0 ignored
```

- [ ] **Step 9: Commit all changes**

After all 13 tests pass, commit everything in one batch:

```bash
git add \
  server/src/model/onboarding.rs \
  server/src/module/onboarding/repo.rs \
  server/src/module/onboarding/services.rs \
  server/src/module/onboarding/handlers.rs \
  server/src/module/onboarding/mod.rs \
  server/src/openapi.rs \
  server/tests/onboarding_test.rs \
  docs/superpowers/specs/2026-05-18-get-submit-doctor-profile-draft-design.md \
  docs/superpowers/plans/2026-05-18-get-submit-doctor-profile-draft.md

git commit -m "feat(onboarding): align get/submit with new DB functions, add status to GET response"
```
