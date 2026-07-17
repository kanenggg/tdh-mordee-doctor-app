# Design: Align consultation V2 clients to the new upstream OpenAPI spec

- **Date:** 2026-05-20
- **Author:** mordee.tdh@gmail.com (with Claude Code)
- **Source of truth:** `https://consult.api.tdh.bluewhale.space/docs/openapi.json` (Consultation API v0.1.0, OpenAPI 3.1.0)

## Problem

The doctor-app gateway proxies to the upstream consultation v2 service. Two HTTP clients
mirror that service:

- `server/src/module/consultation/services.rs` — `ConsultationService` (doctor session ops)
- `server/src/module/consultation/confirmed_appointment_client.rs` — `ConfirmedAppointmentClient`

The upstream spec has changed: endpoint paths now take `booking_id` in the path
(`/v2/consultation/{booking_id}/...`), response bodies use new tagged unions
(`GetDoctorSessionInfoResult`, `EndSessionResult`, `AddConsultationScreenshot`), facial
verification is now a multipart upload, and the payment-channel variants changed
(`SelfPay`/`InsuranceCoverage` removed). The clients must be brought in line with the spec.

## Scope

In scope:

- Update `ConsultationService` upstream calls + response parsing.
- Update `ConfirmedAppointmentClient` request shape (payment channels).

Out of scope (explicitly):

- No changes to `routes.rs`, `handlers`, or `openapi.rs`. The gateway's **outward contract
  stays byte-for-byte identical**.
- Patient-id-verify-match / miss-match endpoints are **not** added this round.
- New `/v2/appointment/reserve`, `/v2/internal/create-appointment`,
  `/internal/v1/appointment/{bookingId}` clients are **not** added this round.

## Design

### Principle: map back to existing types

`ConsultationService`'s public method signatures and public return types
(`GetSessionInfoResult`, `EndSessionResult`, `SessionInfo`, `SessionChannel`,
`FaceVerificationRequest`) stay unchanged so `routes.rs` and `openapi.rs` need no edits.
Only the internal upstream call changes: new URL, new request encoding, and private
`#[derive(Deserialize)]` mirror types for the upstream payloads that are then mapped
back to the existing public types.

Response parsing reads the body for any non-5xx status (the spec returns the typed result
union in 200/401/404/409 bodies); 5xx → `AppError`. If the body fails to parse as the
expected union → `AppError`.

### Component 1 — `ConsultationService` (`services.rs`)

#### `end_session`

- Upstream: `POST {base}/v2/consultation/end-session/{booking_id}` (no JSON body).
- Mapping (`EndSessionResult.__type` → public `EndSessionResult`):
  - `EndSession.Success` → `SessionEnded`
  - `EndSession.SessionNotFound` → `SessionNotFound`
  - `EndSession.Unauthorized` → `Unauthorized`

#### `get_session_info`

- Upstream: `GET {base}/v2/consultation/session-info/{booking_id}`.
- Mapping (`GetDoctorSessionInfoResult.__type` → public `GetSessionInfoResult`):
  - `GetDoctorSessionInfo.SessionReady` → `SessionInformation(SessionInfo)`
  - `GetDoctorSessionInfo.SessionNotFound` → `SessionNotFound`
  - `GetDoctorSessionInfo.SessionIsFinished` → `SessionIsFinished`
  - `GetDoctorSessionInfo.SessionIsNotReady` → `SessionIsNotReady`
  - `GetDoctorSessionInfo.ProviderIsOutOfService` → `SessionIsNotReady` (closest existing
    variant; provider down ⇒ session not ready)
  - `getdoctorsessioninfo.unauthorized` (lowercase in spec, matched verbatim) → `Unauthorized`

- `SessionReady` → `SessionInfo` field mapping:
  - `sessionStartTime` → `session_start_time`
  - `sessionEndTime` → `session_end_time`
  - `isFacialVerified` → `is_facial_verified`
  - `is_patient_identity_verified` → **always `false`** (the new spec has no
    "verified" field, only `isRequiredPatientVerification`; we do not conflate the two).
  - `sessionInfo` (`ProviderSessionInfo`) → `session_channel` (`SessionChannel`):
    - `twilio` (`TwilioSessionInfo`) → `{ channel_type: "twilio", session_name: Some(sessionName),
      session_chat_name: sessionChatName, session_token: sessionToken }`
    - `tokBox` (`TokBoxSessionInfo`) → best-effort `{ channel_type: "tokBox",
      session_name: Some(sessionId), session_chat_name: None, session_token: sessionToken }`.
      `conferenceProviderId` and `appointmentNo` are **dropped** — the existing
      `SessionChannel` has no field for them. (Accepted limitation under "map back to
      existing types"; revisit if tokBox is adopted in production.)

#### `submit_face_verification`

- Upstream: `POST {base}/v2/consultation/facial-upload/{booking_id}`, `multipart/form-data`,
  single text part named `image` containing `FaceVerificationRequest.image` **verbatim**
  (no base64 decode — preserves current forwarding behavior).
- Public signature stays `AppResult<()>`.
- Mapping (`AddConsultationScreenshot.__type`):
  - `AddConsultationScreenshot.UploadSuccess` → `Ok(())`
  - `AddConsultationScreenshot.ScreenshotAlreadyUploaded` → `Ok(())` (idempotent)
  - `AddConsultationScreenshot.Unauthorized` → `Err(AppError::Unauthorized)`
  - `AddConsultationScreenshot.ConsultationNotFound` → `Err(AppError::UpstreamError(..))`

#### Identity / headers

- `X-Request-Id` forwarding kept.
- New spec endpoints take only `booking_id` in the path (auth via ApiKeyAuth upstream), so
  `doctor_account_id` is **no longer sent upstream**. It stays in the method signatures
  (so `routes.rs` is untouched) and is used only in tracing/log fields.

### Component 2 — `ConfirmedAppointmentClient` (`confirmed_appointment_client.rs`)

Already targets `POST /v2/internal/create-confirmed-appointment`. Changes:

- Replace the `PaymentChannel` enum. Old `SelfPay` / `InsuranceCoverage` are removed from
  the spec. New variants (kept for documentation / future use, annotated `#[allow(dead_code)]`):
  `Insurance { binding_id: i64, privilege_id: i32 }`, `EmployeeBenefit`, `CampaignLocation`,
  `Campaign`, `Card { id: String }`, `PromptPay { id: String }`, `TrueMoney { id: String }`.
  Tagged with `#[serde(tag = "__type")]`.
- **Emit `paymentChannels: []`** (empty array). The source `ConsultationBookedEvent` carries
  only `payment_module_id` with no channel breakdown, so no real channel can be constructed.
  Remove the `PAYMENT_CHANNEL_VARIANTS` const and the random-selection logic.
- `prescreen` (`ConsultationPreScreen`, snake_case `duration_unit`) already matches the spec's
  `ConsultationPreScreen` — no change.
- Lorem symptom / allergy / attachment stub data left untouched (out of scope).
- All other request fields (`bizUnitId`, `bizCenterId`, `tenantId`, `patientId`, `doctorId`,
  `bookingType`, `consultationChannel`, `consultDuration`, `parentAppointmentId`) already match.

## Testing

- `ConfirmedAppointmentClient`: update the two existing serde-shape unit tests so the
  `paymentChannels` length assertion is `0` and no `__type` channel is asserted.
- `ConsultationService`: add unit tests for each `__type → public` mapping, driven by
  `serde_json::from_value` over representative upstream bodies (Success / NotFound /
  Unauthorized for each endpoint; twilio + tokBox for SessionReady; verify
  `is_patient_identity_verified == false`).

## Risks / accepted limitations

1. **tokBox field loss** — `conferenceProviderId` / `appointmentNo` not surfaced through the
   existing `SessionChannel`. Acceptable while gateway contract is frozen.
2. **`is_patient_identity_verified` always false** — gateway clients no longer see a true
   verification flag; upstream stopped reporting it.
3. **Empty `paymentChannels`** — if upstream rejects an empty list with 4xx, we will need to
   pick a default channel variant. Flagged for monitoring after deploy.
4. **doctor_account_id no longer forwarded upstream** — relies on upstream authorizing via
   `booking_id` + ApiKeyAuth. If upstream needs the doctor identity, a forwarding header
   must be added.
