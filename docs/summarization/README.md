# Consultation Summarization Module

After a teleconsultation ends, the doctor creates a summary: clinical notes, prescriptions, and optional follow-up. This module handles the full lifecycle from draft to submission.

## Architecture

```
server/src/module/consultation/summarization/
  handler.rs              Axum handlers (GET, save draft, submit)
  service.rs              Business logic, orchestrates repo + external calls
  repo.rs                 PostgreSQL persistence (trait + SummarizationRepoPsql)
  dto.rs                  Request/response types for HTTP layer
  models.rs               Domain types: SummaryNote, Prescription, FollowUpInfo
  encryptor.rs            Paseto v4 Local encryption for sensitive data at rest
  external_http_client.rs Traits for external services + stub implementations
  biz_apm_http_client.rs  HTTP client for biz-apm (save summary note)
  jade_http_client.rs     HTTP client for biz-jade (create prescription)
  follow_up_repo.rs       Follow-up timeslot reservation (trait + impl)
  follow_up_transform.rs  Transform client FollowUpInfo -> biz-apm FollowUp enum
  mod.rs                  Public exports
```

## Data Flow

```
                         +------------------+
  Mobile App  ---------> |    Handlers      |  Axum State: SummarizationState
                         +--------+---------+
                                  |
                         +--------v---------+
                         |    Service        |  Authorization, status checks, encryption
                         +--+---------+--+--+
                            |         |  |
               +------------+    +----+  +----------+
               |                 |                   |
      +--------v-------+  +-----v------+   +--------v---------+
      | SummarizationRepo |  | Jade HTTP  |   | BizApm HTTP      |
      | (PostgreSQL)     |  | Client     |   | Client           |
      +----------------+  +-----+------+   +--------+---------+
                                |                    |
                          biz-jade:9004        biz-apm:8080
                         /prescription/create  /e2e/v1/summary-note
```

## Endpoints

All routes are nested under `/consultation/v1`.

### GET /consultation/v1/summarization/{appointment_id}

Returns the current state of a summarization record.

| Response `__type` | Meaning |
|---|---|
| `PendingRecord` | No draft exists yet (or unauthorized access) |
| `SummarizationRecord` | Existing record with `status: "Draft"` or `"Submitted"` |

When status is `SummarizationRecord`, the response includes decrypted `summaryNote`, `prescription`, and `followUpInfo` fields.

### POST /consultation/v1/summarization/draft

Saves a partial or full draft. All content fields are optional for incremental saves.

| Response `__type` | Meaning |
|---|---|
| `SaveDraftResult.Success` | Draft saved |
| `SaveDraftResult.AlreadySubmitted` | Record was already submitted, cannot overwrite |
| `SaveDraftResult.Unauthorized` | Different doctor owns this record |

### POST /consultation/v1/summarization/submit

Submits the final summary. All fields required. Triggers external service calls.

| Response `__type` | Meaning |
|---|---|
| `SubmitResponse.Success` | Submitted successfully |
| `SubmitResponse.AlreadySubmitted` | Already submitted |
| `SubmitResponse.Unauthorized` | Different doctor owns this record |
| `SubmitResponse.PrescriptionServiceError` | Jade service call failed (includes `message`) |
| `SubmitResponse.ConsultationServiceError` | BizApm service call failed (includes `message`) |
| `SubmitResponse.TimeslotIsNotAavailable` | Follow-up timeslot not available |
| `SubmitResponse.TimeslotConflict` | Follow-up timeslot conflict |

## Submit Flow (Step by Step)

1. **Authorization check** - verify the calling doctor owns the record
2. **Status check** - reject if already submitted
3. **Prescription** - if items present, call biz-jade `POST /prescription/create` to get `prescriptionNo`
4. **Summary note** - call biz-apm `POST /e2e/v1/summary-note` with clinical data + follow-up info
5. **Encrypt & persist** - encrypt payload with Paseto v4, upsert with status `Submitted`
6. **Publish event** - publish `ConsultationSummarizedEvent` to Pub/Sub

If step 3 or 4 fails, the status stays `Draft` so the doctor can retry.

## Domain Types

### SummaryNote (draft - all fields optional)

```json
{
  "presentIllness": "Patient presents with...",
  "chiefComplaint": "Headache",
  "diagnosis": "Tension-type headache",
  "recommendations": "Rest, hydration",
  "icd10": [{ "code": "G44.2", "description": "Tension-type headache" }],
  "illnessDuration": { "value": 3, "unit": "days" },
  "noteToStaff": "Follow up in 1 week"
}
```

### Prescription

```json
{
  "drugAllergyInfo": {
    "__type": "HasDrugAllergies",
    "drugAllergies": [{ "id": 1, "displayText": "Penicillin" }]
  },
  "prescriptionItems": {
    "__type": "Prescription",
    "items": [{
      "medicineId": 1, "medicineName": "Paracetamol 500mg",
      "dose": { "value": 500, "unit": "mg" },
      "quantity": 10,
      "route": { "id": 1, "description": "Oral" },
      "frequency": { "id": 1, "description": "3 times a day" },
      "indication": { "id": 1, "description": "Pain relief" },
      "foodTiming": { "id": 1, "description": "After meals" },
      "mealInstruction": { "id": 1, "description": "Take with food" },
      "duration": { "value": 3, "unit": "days" },
      "cautions": null, "remark": "Max 4g/day",
      "noteToPatient": "Take with food"
    }]
  }
}
```

`drugAllergyInfo.__type`: `"HasDrugAllergies"` | `"NoDrugAllergies"`
`prescriptionItems.__type`: `"Prescription"` (with items) | `"NoPrescription"`

### FollowUpInfo

```json
// Schedule a follow-up
{
  "__type": "ScheduleAppointment",
  "followStartDatetime": 1717430400,
  "followEndDatetime": 1717432200,
  "visitType": "In-person",
  "noteToPatient": "Come back in 2 weeks",
  "noteToStaff": "Monitor BP"
}

// No follow-up needed
{
  "__type": "NoFollowUp",
  "noteToStaff": "No follow-up needed"
}
```

## Database

Table: `consultation_summarization` (PostgreSQL)

| Column | Type | Notes |
|---|---|---|
| `appointment_id` | VARCHAR(255) PK | |
| `doctor_account_id` | INTEGER | Indexed |
| `doctor_profile_id` | INTEGER | |
| `status` | ENUM `Draft` / `Submitted` | |
| `summary_note_encrypted` | TEXT | Paseto v4 token (XChaCha20-Poly1305) |
| `prescription_items` | JSONB | Plain JSON |
| `follow_up_info` | JSONB | Plain JSON |
| `created_at` / `updated_at` | TIMESTAMPTZ | |

Migration: `db/postgres/migrations/20260311000000_consultation_summarization.sql`

## Encryption

Sensitive clinical data (summary note, prescription, follow-up) is encrypted at rest using **Paseto v4 Local** (XChaCha20-Poly1305 symmetric encryption).

- Key: 32-byte hex string from `config.paseto.summarization_key` / env `PASETO__SUMMARIZATION_KEY`
- The three fields are combined into an `EncryptedPayload` struct, serialized to JSON, then encrypted as a single Paseto token stored in `summary_note_encrypted`
- Decryption happens in the service layer on GET requests

## External Service Contracts

### biz-jade (Prescription Service)

**Endpoint:** `POST {jade_base_uri}/prescription/create`
**OpenAPI:** `http://localhost:9004/q/openapi`

Request: `PrescriptionRequest` with `bookingId`, optional `bizUnitId`, `bizCenterId`, `patientId`, `items[]`
Response: `CreatePrescriptionResponse` tagged with `__type: "Success"`, returns `prescriptionNo`

### biz-apm (Summary Note Service)

**Endpoint:** `POST {biz_apm_base_uri}/e2e/v1/summary-note`
**OpenAPI:** `http://localhost:8080/docs/openapi.json`

Request: `SummarizationRequest` with `bookingId`, clinical fields, `followUp` (tagged enum: `AsNeeded` | `Appointment`)
Response: `SummarizationResult` tagged with `__type`:
- `SummarizationResult.Success` - includes `summaryNoteId`, `patientIdentity`, `bizUnitId`, `bizCenterId`
- `SummarizationResult.AlreadySubmitted` - same shape

Key detail: `FollowUpAppointment.consultationFee` must be `f64` (JSON number), not BigDecimal (string).

## Dependency Injection

All external dependencies are trait-based for testability:

| Trait | Production Impl | Test Stub |
|---|---|---|
| `SummarizationRepo` | `SummarizationRepoPsql` | Real DB via testcontainers |
| `JadeServiceTrait` | `JadeHttpClient` | `JadeServiceStub` |
| `ConsultationSummarizationServiceTrait` | `BizApmHttpClient` | `ConsultationSummarizationServiceStub` |
| `SummarizationPublisher` | `PubsubPublisher` | `SummarizationPublisherStub` (in test) |
| `FollowUpReservationRepo` | `FollowUpReservationRepoImpl` | `FollowUpReservationRepoStub` (in test) |

Wired in `server/src/module/consultation/mod.rs::router()` and called from `bootstrap.rs::init_routers()`.

## Configuration

| Env Var | Config Key | Used By |
|---|---|---|
| `PASETO__SUMMARIZATION_KEY` | `paseto.summarization_key` | Encryptor |
| `SERVICE__BIZ_JADE_SERVICE_BASE_URI` | `service.biz_jade_service_base_uri` | Jade HTTP client |
| `SERVICE__BIZ_APM_BASE_URI` | `service.biz_apm_base_uri` | BizApm HTTP client |
| `PUBSUB__TOPICS__CONSULTATIONS` | `pubsub.topics.consultations` | Event publisher |

## Testing

Integration tests: `server/tests/summarization_test.rs`
Fixtures: `server/tests/fixtures/summarization/`

Tests use testcontainers (PostgreSQL) with stub implementations for external services. No external HTTP calls during testing.

```bash
cargo test --test summarization_test           # All summarization tests
cargo test --lib follow_up_transform           # Follow-up transform unit tests
```

See also: `it/http/summary-note.http` for manual HTTP testing against a running server.
