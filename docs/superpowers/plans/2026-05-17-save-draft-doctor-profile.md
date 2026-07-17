# Save Draft Doctor Profile — Refactor + Partial-Save Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor the onboarding write path to call `save_doctor_profile_draft(...)` instead of `save_doctor_onboarding(...)`, renaming the method throughout the call chain, AND add partial-update support via `OnBoardingRequestPatch` so the frontend can omit unchanged fields.

**Architecture:** Six files change. Model first (regroup + Default + Patch derives), then repo (rename + 23-param SQL), then service (catalyst — applies patch onto default), then handler (renamed, accepts patch), then tests last. Each step produces a compilable state (verified with `cargo check`) before moving on. A pre-existing compile error from a `status` field removal is fixed in Task 0 before any new work starts.

**Tech Stack:** Rust, sqlx (PostgreSQL), Axum, serde_json, anyhow, `struct-patch 0.12.0`, `utoipa`

**Spec:** [`docs/superpowers/specs/2026-05-17-save-draft-doctor-profile-design.md`](../specs/2026-05-17-save-draft-doctor-profile-design.md) — see *Flow*, *Code Layout*, *Target PostgreSQL Function*, and Section 4 (*Authentication / Responses / HTTP error responses*).

---

## File Map

| File | Change |
|------|--------|
| `server/src/module/onboarding/repo.rs` | Remove `status` line from `From<OnBoardingRow>` (compile fix); rename trait method; replace SQL call with 23-param `save_doctor_profile_draft` |
| `server/src/module/onboarding/services.rs` | Remove broken `stub.status` call (compile fix); rename trait method; accept `OnBoardingRequestPatch` and resolve via `OnBoardingRequest::default()` |
| `server/src/model/ref_data.rs` | Add `Default` derives to 8 ref-data types |
| `server/src/model/onboarding.rs` | Regroup `work_place` into `SelectedWorkPlaceRequest`; add `Default` derives; `impl Default for Localized`; `#[derive(Patch)]` on `OnBoardingRequest`; update `From` impl |
| `server/src/module/onboarding/handlers.rs` | Fix `UserIdentity` import; rename handler to `save_doctor_profile_draft`; change input to `Json<OnBoardingRequestPatch>`; update utoipa annotation |
| `server/tests/onboarding_test.rs` | Update mock signature; fix `From` impl; fix test fixture; fix status assertions; add partial-save tests |

See spec *Code Layout* for the full module tree and layer responsibilities.

---

## API Contract (target behavior)

The endpoint must satisfy the following contract once all tasks are complete. Tests in Task 7 verify it.

**Endpoint:** `POST /onboarding/v1`

**Authentication:**
- Header `tdh-sec-iam-user-identity` (JSON-serialized `UserIdentity`)
- Extracted by `UserIdentity` (`crate::core::user_identity::UserIdentity`), NOT `DoctorIdentity` — the handler needs `user_profile_id`, which `DoctorIdentity` does not expose.

**Body:** `OnBoardingRequestPatch` — any field may be omitted. Missing fields are filled by `OnBoardingRequest::default()` at the service layer.

**Responses:**

| HTTP | When | Body |
|------|------|------|
| `200 OK` | Draft saved successfully | empty |
| `400 Bad Request` | Malformed JSON body (Axum `JsonRejection` — returned directly, not via `AppError`) | Axum default error text |
| `401 Unauthorized` | `tdh-sec-iam-user-identity` missing or fails to deserialize into `UserIdentity` | `AppError::Unauthorized` plain-text body |
| `500 Internal Server Error` | `serde_json::to_value` / `to_string` failure pre-SQL | `AppError::InternalError` plain-text body |
| `500 Internal Server Error` | `sqlx` failure on `SELECT save_doctor_profile_draft(...)` | `AppError::DatabaseError` plain-text body |

Error bodies are produced by `AppError::IntoResponse` in `server/src/core/error.rs:69`. No structured `__type` JSON variant for this endpoint.

---

## Task 0: Fix pre-existing compile errors from OnBoardingStub status removal

`OnBoardingStub` no longer has a `status` field (intentional model change), but two source files still reference it. This causes compile errors that block all subsequent work. If `cargo check` is clean on your branch, this task is a no-op — skip to Task 1.

**Files:**
- Modify: `server/src/module/onboarding/repo.rs`
- Modify: `server/src/module/onboarding/services.rs`

- [ ] **Step 1: Verify the errors**

```bash
cargo check 2>&1 | grep "error\["
```

Expected output (if the errors exist):
```
error[E0560]: struct `OnBoardingStub` has no field named `status`
error[E0609]: no field `status` on type `OnBoardingStub`
```

If no output, skip to Task 1.

- [ ] **Step 2: Fix `repo.rs` — remove the `status` line from `From<OnBoardingRow> for OnBoardingStub`**

In `server/src/module/onboarding/repo.rs`, find the `impl From<OnBoardingRow> for OnBoardingStub` block (around line 117). Remove the `status:` field initializer:

```rust
impl From<OnBoardingRow> for OnBoardingStub {
    fn from(row: OnBoardingRow) -> Self {
        let empty_loc = || Localized {
            th: String::new(),
            en: String::new(),
        };

        let first_name: Localized = serde_json::from_value(row.first_name).unwrap_or_else(|e| {
            warn!(error = %e, "failed to deserialize first_name");
            empty_loc()
        });
        let last_name: Localized = serde_json::from_value(row.last_name).unwrap_or_else(|e| {
            warn!(error = %e, "failed to deserialize last_name");
            empty_loc()
        });
        let specialty: Vec<Specialty> = serde_json::from_value::<Vec<Specialty>>(row.specialty)
            .unwrap_or_else(|e| {
                warn!(error = %e, "failed to deserialize specialty");
                vec![]
            })
            .into_iter()
            .map(Specialty::from)
            .collect();
        let additional_specialty: Vec<Specialty> = match row.additional_specialty {
            None => vec![],
            Some(v) => serde_json::from_value::<Vec<Specialty>>(v)
                .unwrap_or_else(|e| {
                    warn!(error = %e, "failed to deserialize additional_specialty");
                    vec![]
                })
                .into_iter()
                .map(Specialty::from)
                .collect(),
        };
        let special_interests: Vec<Localized> = serde_json::from_value(row.special_interests)
            .unwrap_or_else(|e| {
                warn!(error = %e, "failed to deserialize special_interests");
                vec![]
            });

        Self {
            profession: Profession {
                id: row.profession_id,
                name: empty_loc(),
                abbr: empty_loc(),
            },
            academic_position: AcademicPosition {
                id: row.academic_position_id,
                name: empty_loc(),
                abbr: empty_loc(),
            },
            citizen_id: row.citizen_id,
            first_name,
            last_name,
            address: Address {
                address_detail: row.address_detail,
                sub_district: SubDistrict {
                    id: row.sub_district_id,
                    name: empty_loc(),
                    district_id: row.district_id,
                    zip_code: String::new(),
                },
                district: District {
                    id: row.district_id,
                    name: empty_loc(),
                    province_id: row.province_id,
                },
                province: Province {
                    id: row.province_id,
                    name: empty_loc(),
                },
                postal_code: PostalCode {
                    id: row.postal_code_id,
                    description: String::new(),
                    district_id: row.district_id,
                },
            },
            work_place: SelectedWorkPlace {
                primary: row
                    .primary_workplace_ids
                    .into_iter()
                    .map(|id| WorkPlace {
                        id,
                        name: String::new(),
                    })
                    .collect(),
                additional: row
                    .additional_workplace_ids
                    .into_iter()
                    .map(|id| WorkPlace {
                        id,
                        name: String::new(),
                    })
                    .collect(),
            },
            education: Education {
                license_number: row.license_number,
                medical_school: MedicalSchool {
                    id: row.medical_school_id,
                    name: String::new(),
                },
                specialties: specialty,
                additional_specialties: additional_specialty,
                special_interests,
            },
            documents: Documents {
                profile_image_url: row.profile_image_url,
                id_card_image_url: row.id_card_image_url,
                book_bank_image_url: row.book_bank_image_url,
                med_license_image_url: row.med_license_image_url,
                certificate_image_urls: row.edu_certificate_image_urls,
            },
        }
    }
}
```

- [ ] **Step 3: Fix `services.rs` — remove the broken `stub.status` call in `submit_onboarding`**

In `server/src/module/onboarding/services.rs`, replace `submit_onboarding`:

```rust
async fn submit_onboarding(&self, doctor_id: i32) -> AppResult<()> {
    let stub = self
        .repo
        .get_onboarding(doctor_id)
        .await?
        .ok_or(Self::onboarding_not_found_error())?;

    self.validator.validate_onboarding_submission(&stub)?;

    self.repo
        .update_status(doctor_id, OnBoardingStatus::PendingApproval)
        .await?;

    Ok(())
}
```

- [ ] **Step 4: Verify compile errors are gone**

```bash
cargo check 2>&1 | grep "error\["
```

Expected: no output (or only errors that move to later tasks).

- [ ] **Step 5: Commit**

```bash
git add server/src/module/onboarding/repo.rs server/src/module/onboarding/services.rs
git commit -m "fix(onboarding): remove status field references after OnBoardingStub refactor"
```

---

## Task 1: Add Default derives to ref-data types

`OnBoardingRequest::default()` is the merge base used by the service. Every field type must implement `Default`. Start with the 8 ref-data types.

**Files:**
- Modify: `server/src/model/ref_data.rs`

- [ ] **Step 1: Add `Default` to the 8 types in `ref_data.rs`**

Replace the derive lines for each struct. Add `Default` to each:

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Profession {
    pub id: i32,
    pub name: Localized,
    pub abbr: Localized,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AcademicPosition {
    pub id: i32,
    pub name: Localized,
    pub abbr: Localized,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SubDistrict {
    pub id: i32,
    pub name: Localized,
    pub district_id: i32,
    pub zip_code: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct District {
    pub id: i32,
    pub name: Localized,
    pub province_id: i32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Province {
    pub id: i32,
    pub name: Localized,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PostalCode {
    pub id: i32,
    pub description: String,
    pub district_id: i32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkPlace {
    pub id: i32,
    pub name: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MedicalSchool {
    pub id: i32,
    pub name: String,
}
```

- [ ] **Step 2: Verify compile**

```bash
cargo check 2>&1 | grep "error\["
```

Expected: no new errors from this file. (`Localized` still missing `Default` is fine — fixed in Task 2.)

- [ ] **Step 3: Commit**

```bash
git add server/src/model/ref_data.rs
git commit -m "feat(onboarding): add Default derives to ref-data types"
```

---

## Task 2: Restructure `OnBoardingRequest` and add Default + Patch derives

Three layered changes to `model/onboarding.rs`: regroup `work_place`, add `Default` everywhere, add `impl Default for Localized`, add `#[derive(Patch)]`.

**Files:**
- Modify: `server/src/model/onboarding.rs`

- [ ] **Step 1: Add `use struct_patch::Patch;` import**

Add to the top of `server/src/model/onboarding.rs`:

```rust
use struct_patch::Patch;
```

- [ ] **Step 2: Add `impl Default for Localized`**

`Localized` lives in the `tdh-protocol` submodule and cannot be modified there. Add the impl locally, just before the `OnBoardingRequest` struct:

```rust
impl Default for Localized {
    fn default() -> Self {
        Self {
            th: String::new(),
            en: String::new(),
        }
    }
}
```

- [ ] **Step 3: Update `OnBoardingRequest` — regroup `work_place`, add Default + Patch derives**

Replace the `OnBoardingRequest` struct definition:

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema, Patch)]
#[patch_derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[patch(attribute(serde(rename_all = "camelCase")))]
#[serde(rename_all = "camelCase")]
pub struct OnBoardingRequest {
    pub profession: Profession,
    pub academic_position: AcademicPosition,
    pub citizen_id: String,
    pub first_name: Localized,
    pub last_name: Localized,
    pub address: Address,
    pub work_place: SelectedWorkPlaceRequest,
    pub education: EducationRequest,
    pub documents: Documents,
}
```

This generates `OnBoardingRequestPatch` with every field wrapped in `Option<T>` and `#[serde(rename_all = "camelCase")]` applied to the generated struct.

- [ ] **Step 4: Add `Default` to other owned types in this file**

Add `Default` to the derive list (field bodies unchanged):

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SelectedWorkPlaceRequest {
    pub primary: Vec<WorkPlace>,
    pub additional: Vec<WorkPlace>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EducationRequest { ... }

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Address { ... }

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Specialty { ... }

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Subspecialty { ... }

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Documents { ... }
```

- [ ] **Step 5: Update the `From<OnBoardingRequest> for OnBoardingStub` impl (cfg(test))**

Replace the `work_place` mapping inside the `From` impl:

```rust
#[cfg(test)]
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
                primary: input.work_place.primary,       // was: input.work_place
                additional: input.work_place.additional, // was: input.additional_work_place
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

(Drop the `status:` line if your `OnBoardingStub` had one before — already removed elsewhere.)

- [ ] **Step 6: Verify compile**

```bash
cargo check 2>&1 | grep "error\["
```

Expected: errors move out of `model/onboarding.rs`. If struct-patch rejects `#[patch(attribute(...))]` syntax, check the installed version's docs: `cargo doc --open -p struct-patch`.

- [ ] **Step 7: Verify the generated patch type exists**

```bash
grep -r "OnBoardingRequestPatch" server/src/ 2>/dev/null || echo "not yet referenced (expected)"
```

Expected: no references yet (Tasks 4–5 add them).

- [ ] **Step 8: Commit**

```bash
git add server/src/model/onboarding.rs
git commit -m "feat(onboarding): regroup work_place, derive Patch + Default, add Default for Localized"
```

---

## Task 3: Rename and rewrite repo method

**Files:**
- Modify: `server/src/module/onboarding/repo.rs`

- [ ] **Step 1: Update the `OnBoardingRepo` trait — rename and change signature**

Replace the `save_onboarding` declaration in the trait:

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

- [ ] **Step 2: Replace the `save_onboarding` impl with `save_draft_doctor_profile`**

Remove the entire old `save_onboarding` function body from `impl OnBoardingRepo for OnBoardingRepoImp` and add:

```rust
async fn save_draft_doctor_profile(
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
        request.education.specialties.iter()
            .chain(request.education.additional_specialties.iter())
            .collect::<Vec<_>>(),
    )
    .context("serialize specialty")?;
    let special_interests: Vec<String> = request.education.special_interests
        .iter()
        .map(|loc| serde_json::to_string(loc).context("serialize special_interest"))
        .collect::<Result<_, _>>()?;

    sqlx::query(
        r#"
        SELECT save_doctor_profile_draft(
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15,
            $16, $17, $18, $19, $20, $21, $22, $23
        )
        "#,
    )
    .bind(doctor_account_id)                           // $1  p_doctor_account_id
    .bind(doctor_profile_id)                           // $2  p_doctor_profile_id
    .bind(&request.citizen_id)                         // $3  p_citizen_id
    .bind(profession_json)                             // $4  p_profession
    .bind(academic_pos_json)                           // $5  p_academic_position
    .bind(first_name_json)                             // $6  p_first_name
    .bind(last_name_json)                              // $7  p_last_name
    .bind(&request.education.license_number)           // $8  p_license_number
    .bind(medical_school_json)                         // $9  p_medical_school
    .bind(specialty_json)                              // $10 p_specialty (merged)
    .bind(&special_interests)                          // $11 p_special_interests
    .bind(&request.address.address_detail)             // $12 p_address_detail
    .bind(sub_district_json)                           // $13 p_sub_district
    .bind(district_json)                               // $14 p_district
    .bind(province_json)                               // $15 p_province
    .bind(request.address.postal_code.id)              // $16 p_postal_code
    .bind(primary_wp_json)                             // $17 p_primary_workplace
    .bind(additional_wp_json)                          // $18 p_additional_workplace
    .bind(&request.documents.profile_image_url)        // $19 p_profile_image_url
    .bind(&request.documents.id_card_image_url)        // $20 p_id_card_image_url
    .bind(&request.documents.book_bank_image_url)      // $21 p_book_bank_image_url
    .bind(&request.documents.med_license_image_url)    // $22 p_medical_license_image_url
    .bind(&request.documents.certificate_image_urls)   // $23 p_education_certificate_image_urls
    .execute(&self.pool)
    .await?;

    Ok(())
}
```

- [ ] **Step 3: Verify repo compiles**

```bash
cargo check 2>&1 | grep "error\[" | head -20
```

Expected: errors move to `services.rs` (calls `save_onboarding` which no longer exists on the trait).

- [ ] **Step 4: Commit**

```bash
git add server/src/module/onboarding/repo.rs
git commit -m "refactor(onboarding): replace save_onboarding with save_draft_doctor_profile"
```

---

## Task 4: Update service trait and impl — accept patch, resolve onto default

The service is the Catalyst: it applies `OnBoardingRequestPatch` onto `OnBoardingRequest::default()` and passes the resolved request to the repo.

**Files:**
- Modify: `server/src/module/onboarding/services.rs`

- [ ] **Step 1: Update imports — add `OnBoardingRequestPatch`**

```rust
use crate::{
    core::error::{AppError, AppResult},
    model::onboarding::{OnBoarding, OnBoardingRequest, OnBoardingRequestPatch, OnBoardingStatus},
    module::onboarding::{OnBoardingRepo, OnboardingValidator},
};
```

- [ ] **Step 2: Update the `OnboardingServiceTrait` — rename method and accept patch**

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

- [ ] **Step 3: Update the impl — apply patch onto default, pass resolved request to repo**

```rust
async fn save_draft_doctor_profile(
    &self,
    doctor_account_id: i32,
    doctor_profile_id: i32,
    patch: OnBoardingRequestPatch,
) -> AppResult<()> {
    let mut request = OnBoardingRequest::default();
    request.apply(patch);
    self.repo
        .save_draft_doctor_profile(doctor_account_id, doctor_profile_id, &request)
        .await
}
```

- [ ] **Step 4: Verify service compiles**

```bash
cargo check 2>&1 | grep "error\[" | head -20
```

Expected: errors move to `handlers.rs` (calls `save_onboarding` which no longer exists on the trait).

- [ ] **Step 5: Commit**

```bash
git add server/src/module/onboarding/services.rs
git commit -m "feat(onboarding): service accepts OnBoardingRequestPatch and resolves against default"
```

---

## Task 5: Update handler — fix import, rename, accept patch

**Files:**
- Modify: `server/src/module/onboarding/handlers.rs`

- [ ] **Step 1: Fix the `UserIdentity` import**

At the top of `server/src/module/onboarding/handlers.rs`, replace:

```rust
// Remove:
use tdh_protocol::iam::user_identity::UserIdentity;

// Add:
use crate::core::user_identity::UserIdentity;
```

`FromRequestParts` is implemented for `crate::core::user_identity::UserIdentity` (in `core/auth.rs`) and only that type exposes `user_profile_id`.

- [ ] **Step 2: Add `OnBoardingRequestPatch` to the model import**

```rust
use crate::model::onboarding::{OnBoarding, OnBoardingRequest, OnBoardingRequestPatch};
```

- [ ] **Step 3: Rename and rewrite the save handler**

Replace the `save_onboarding` function:

```rust
#[utoipa::path(
    post,
    path = "/onboarding/v1",
    tag = "onboarding",
    request_body = OnBoardingRequestPatch,
    responses(
        (status = 200, description = "Onboarding info saved"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("TDH-SEC-IAM-USER-IDENTITY" = []))
)]
pub async fn save_doctor_profile_draft(
    State(state): State<OnboardingState>,
    identity: UserIdentity,
    Json(input): Json<OnBoardingRequestPatch>,
) -> AppResult<impl IntoResponse> {
    let doctor_account_id = identity.account_id;
    let doctor_profile_id = identity.user_profile_id;
    state
        .service
        .save_draft_doctor_profile(doctor_account_id, doctor_profile_id, input)
        .await?;
    Ok(StatusCode::OK.into_response())
}
```

- [ ] **Step 4: Update the router wiring in `mod.rs` if it references the old handler name**

```bash
grep -n "save_onboarding\|save_doctor_profile_draft" server/src/module/onboarding/mod.rs
```

If the route still points to `save_onboarding`, rename it to `save_doctor_profile_draft`.

- [ ] **Step 5: Verify handler compiles**

```bash
cargo check 2>&1 | grep "error\[" | head -20
```

Expected: no errors in `src/` — only test compilation errors remain.

- [ ] **Step 6: Visually verify auth contract**

Open `server/src/module/onboarding/handlers.rs` and confirm:
- `identity: UserIdentity` (NOT `DoctorIdentity`) — needed for `user_profile_id`
- Import is `use crate::core::user_identity::UserIdentity;` (NOT `tdh_protocol::iam::user_identity::UserIdentity`)
- Handler returns `StatusCode::OK` on success
- Errors propagate via `?` (no manual mapping — `AppError::IntoResponse` handles 401/500)

This matches the *API Contract* table above.

- [ ] **Step 7: Commit**

```bash
git add server/src/module/onboarding/handlers.rs server/src/module/onboarding/mod.rs
git commit -m "feat(onboarding): rename handler to save_doctor_profile_draft; accept OnBoardingRequestPatch"
```

---

## Task 6: Update tests — fixtures, mock, status assertions

**Files:**
- Modify: `server/tests/onboarding_test.rs`

- [ ] **Step 1: Verify current test compilation errors**

```bash
cargo test --test onboarding_test 2>&1 | grep "error\[" | head -30
```

Expected: errors about `save_onboarding` not found on `MockOnboardingRepo`, mismatched field names, and `stub.status` no longer valid.

- [ ] **Step 2: Fix imports**

Replace the import block at the top of `server/tests/onboarding_test.rs`:

```rust
use server::model::onboarding::{
    Address, Documents, Education, EducationRequest, OnBoardingRequest, OnBoardingStatus,
    OnBoardingStub, SelectedWorkPlace, SelectedWorkPlaceRequest, Specialty, Subspecialty,
};
use server::model::ref_data::{
    AcademicPosition, District, MedicalSchool, PostalCode, Profession, Province, SubDistrict,
    WorkPlace,
};
```

- [ ] **Step 3: Rename `save_onboarding` to `save_draft_doctor_profile` in `MockOnboardingRepo`**

```rust
async fn save_draft_doctor_profile(
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

Note: do NOT set `stub.status` — that field no longer exists on `OnBoardingStub`.

- [ ] **Step 4: Fix `create_valid_onboarding_request` to use current model types**

Replace the entire function:

```rust
fn create_valid_onboarding_request() -> OnBoardingRequest {
    let empty_loc = || Localized { th: String::new(), en: String::new() };

    OnBoardingRequest {
        profession: Profession { id: 1, name: empty_loc(), abbr: empty_loc() },
        academic_position: AcademicPosition { id: 1, name: empty_loc(), abbr: empty_loc() },
        citizen_id: "1234567890123".to_string(),
        first_name: localized("จอห์น", "John"),
        last_name: localized("โด", "Doe"),
        address: Address {
            address_detail: "123 Main St".to_string(),
            sub_district: SubDistrict {
                id: 100105,
                name: empty_loc(),
                district_id: 1001,
                zip_code: String::new(),
            },
            district: District { id: 1001, name: empty_loc(), province_id: 1 },
            province: Province { id: 1, name: empty_loc() },
            postal_code: PostalCode { id: 1, description: String::new(), district_id: 1001 },
        },
        work_place: SelectedWorkPlaceRequest {
            primary: vec![WorkPlace { id: 1, name: String::new() }],
            additional: vec![],
        },
        education: EducationRequest {
            license_number: "12345".to_string(),
            medical_school: MedicalSchool { id: 1, name: String::new() },
            specialties: vec![Specialty {
                id: 1,
                name: empty_loc(),
                subspecialty: Subspecialty {
                    id: 1,
                    name: empty_loc(),
                    medical_school: MedicalSchool { id: 1, name: String::new() },
                },
                medical_school: MedicalSchool { id: 1, name: String::new() },
            }],
            additional_specialties: vec![],
            special_interests: vec![localized("โรคโลหิตจาง", "Anemia")],
        },
        documents: Documents {
            profile_image_url: "https://example.com/profile.jpg".to_string(),
            id_card_image_url: "https://example.com/id.jpg".to_string(),
            book_bank_image_url: "https://example.com/bank.jpg".to_string(),
            med_license_image_url: "https://example.com/license.jpg".to_string(),
            certificate_image_urls: vec![],
        },
    }
}
```

- [ ] **Step 5: Fix the JSON payload in `post_ignores_extra_fields_like_description`**

Update the `workPlace` key in the inline JSON payload to use the grouped shape:

```rust
"workPlace": {
    "primary": [{ "id": 1, "description": "Asoke Skin Hospital" }],
    "additional": []
},
```

Remove the separate `"additionalWorkPlace"` key if present.

- [ ] **Step 6: Fix `save_always_sets_status_to_draft` — drop status body assertion**

`OnBoarding` no longer carries a `status` field in the HTTP response. Update to verify the save round-trip instead:

```rust
#[tokio::test]
async fn save_always_sets_status_to_draft() {
    let server = create_test_server();

    server
        .post("/")
        .add_header(AUTH_HEADER, doctor_identity(123))
        .json(&create_valid_onboarding_request())
        .await;

    let get_response = server
        .get("/")
        .add_header(AUTH_HEADER, doctor_identity(123))
        .await;

    assert_eq!(get_response.status_code(), StatusCode::OK);
    let body = get_response.text();
    assert!(body.contains("OnBoardingResponse"));
}
```

- [ ] **Step 7: Fix `submit_changes_status_to_pending_approval` — drop status body assertion**

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

- [ ] **Step 8: Run all onboarding tests**

```bash
cargo test --test onboarding_test -- --nocapture
```

Expected: existing tests pass. Partial-save coverage added in Task 7.

- [ ] **Step 9: Commit**

```bash
git add server/tests/onboarding_test.rs
git commit -m "test(onboarding): update mock, fixtures, and status assertions for save_draft_doctor_profile"
```

---

## Task 7: Add partial-save tests and auth-contract test

**Files:**
- Modify: `server/tests/onboarding_test.rs`

- [ ] **Step 1: Add 401 auth-contract test**

In `server/tests/onboarding_test.rs`, add a test that exercises the `UserIdentity` extractor's rejection path documented in the *API Contract*:

```rust
#[tokio::test]
async fn post_returns_401_when_identity_header_missing() {
    let server = create_test_server();

    let response = server
        .post("/")
        .json(&serde_json::json!({}))
        .await;

    assert_eq!(response.status_code(), StatusCode::UNAUTHORIZED);
}
```

If `create_test_server()` always injects the identity header, add a sibling helper (`create_test_server_no_identity()` or similar) that constructs the same router without the header — do not bypass the extractor itself; the production extractor must run.

- [ ] **Step 2: Add partial first-save test (only `citizenId`)**

```rust
#[tokio::test]
async fn partial_first_save_only_citizen_id_uses_defaults_for_rest() {
    let server = create_test_server();

    let payload = json!({ "citizenId": "1234567890123" });

    let response = server
        .post("/")
        .add_header(AUTH_HEADER, doctor_identity(789))
        .json(&payload)
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);

    let get_response = server
        .get("/")
        .add_header(AUTH_HEADER, doctor_identity(789))
        .await;

    assert_eq!(get_response.status_code(), StatusCode::OK);
    let body = get_response.text();
    assert!(body.contains("OnBoardingResponse"));
    assert!(body.contains("1234567890123"));
}
```

- [ ] **Step 3: Add omitted-section test (no `address`)**

```rust
#[tokio::test]
async fn omitted_address_section_saves_with_default_address() {
    let server = create_test_server();

    let payload = json!({
        "citizenId": "9999999999999",
        "firstName": { "th": "สมชาย", "en": "Somchai" },
        "lastName": { "th": "ใจดี", "en": "Jaidee" }
    });

    let response = server
        .post("/")
        .add_header(AUTH_HEADER, doctor_identity(456))
        .json(&payload)
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);

    let get_response = server
        .get("/")
        .add_header(AUTH_HEADER, doctor_identity(456))
        .await;

    assert_eq!(get_response.status_code(), StatusCode::OK);
    let body = get_response.text();
    assert!(body.contains("OnBoardingResponse"));
    assert!(body.contains("Somchai"));
}
```

- [ ] **Step 4: Run the new tests**

```bash
cargo test --test onboarding_test partial_first_save omitted_address post_returns_401 2>&1 | tail -15
```

Expected: all three pass.

- [ ] **Step 5: Run full onboarding test suite**

```bash
cargo test --test onboarding_test 2>&1 | tail -10
```

Expected: all tests pass.

- [ ] **Step 6: Run full repo test suite (regression check)**

```bash
cargo test
```

Expected: all tests pass, no regressions.

- [ ] **Step 7: Commit**

```bash
git add server/tests/onboarding_test.rs
git commit -m "test(onboarding): add 401 auth-contract test and partial-save coverage"
```
