# Doctor Ranking API Design

## Context

The legacy API (`legacy.api.tdh.bluewhale.space`) provides doctor listing and ranking endpoints consumed by the patient-facing app. We are building a replacement in the Rust doctor-app that:

- Serves the same response contract as legacy (with minor pagination improvement)
- Uses PostgreSQL as source of truth for doctor data
- Uses Redis (GCP Memorystore) for ranked list serving and profile caching
- Receives real-time availability updates via GCP Pub/Sub
- Replaces offset-based pagination with page-token cursor pagination to prevent drift

## API Endpoints

### 1. List Instant Doctors

```
GET /ranking/v1/doctors/instant?size=30&pageToken=<token>
```

Returns doctors with `instant_mode_enabled = true`, sorted by score descending. Excludes doctors currently in a consultation (marked unavailable via Pub/Sub events).

### 2. List Scheduled Doctors

```
GET /ranking/v1/doctors/scheduled?size=30&pageToken=<token>
```

Returns doctors with `schedule_mode_enabled = true`, sorted by score descending.

### 3. Get Doctor Profile

```
GET /ranking/v1/doctor/{doctor_uuid}
```

Returns a single doctor's full profile by UUID.

## Response Format

Matches legacy envelope structure:

```json
{
  "message": "DOCTOR_LISTING_SUCCEDDED",
  "returnType": "object",
  "data": {
    "doctors": [
      {
        "usid": "82e6b8ce-071c-4752-83b5-5cd552ef7ffb",
        "name": [
          { "langCode": "en-US", "name": "DoctorWoman GoodAdvice, M.D." }
        ],
        "profileImage": "https://storage.googleapis.com/...",
        "specialties": [
          { "id": 208, "name": "Mental Health", "langCode": "en-US" }
        ],
        "channels": [
          { "type": "Chat", "duration": 15, "price": 300, "currency": "THB" }
        ],
        "workPlace": [
          { "langCode": "en-US", "name": "Matrix" }
        ],
        "specialtyDesc": [
          { "langCode": "en-US", "name": "..." }
        ],
        "consultationCase": 361,
        "rating": 4.9,
        "availableLanguage": ["th-TH", "en-US"],
        "consultationFee": 450,
        "consultationDuration": 15,
        "associatePrivileges": [
          {
            "privilegeId": 123,
            "privilegeDisplayName": "Premium Health Plan",
            "providerName": "Insurance Company ABC",
            "companyLogoUrl": "https://...",
            "packageTypeName": "Gold"
          }
        ],
        "score": 85.5,
        "ranked": 1,
        "iRanked": 1
      }
    ],
    "pagingMetaData": {
      "size": 30,
      "total": 150,
      "nextPageToken": "eyJzY29yZSI6ODUuNSwidXVpZCI6ImFiYy0xMjMifQ=="
    }
  }
}
```

Single doctor response:

```json
{
  "message": "DOCTOR_PROFILE_SUCCEDDED",
  "returnType": "object",
  "data": { /* same doctor object as above */ }
}
```

### Field Mapping

| Legacy Field | Source |
|---|---|
| `usid` | `doctor.doctor_id` (UUID) |
| `name` | `doctor_name_i18n.firstname` + `lastname` (JSONB with lang keys) |
| `profileImage` | `doctor.profile_image_url` |
| `specialties` | `doctor_specialty` JOIN ref tables (specialty name from ref or department) |
| `channels` | `doctor_channel` (type, is_enabled). All channels share the same fee (`doctor_fee.fee_amount`) and default duration (`doctor_duration` where `is_default = true`). Legacy API uses uniform fee/duration across channel types. |
| `workPlace` | `doctor_workplace` JOIN `ref_workplaces`. `ref_workplaces.description` is wrapped as `[{ "langCode": "en-US", "name": "<description>" }]` (single-language, matches legacy). |
| `specialtyDesc` | `doctor.special_interest` (TEXT[]). Each array element is wrapped as `{ "langCode": "en-US", "name": "<element>" }` to match legacy response shape. |
| `consultationCase` | `doctor_case.case_amount` |
| `rating` | `doctor_rating.rating` (new table) |
| `availableLanguage` | `doctor.supported_languages` (mapped: `th` -> `th-TH`, `en` -> `en-US`) |
| `consultationFee` | `doctor_fee.fee_amount` |
| `consultationDuration` | `doctor_duration.duration_minutes` (new table, primary/default duration) |
| `associatePrivileges` | HTTP call to privilege-man internal API by specialty ID |
| `score` | `doctor_score.score` (new table) |
| `ranked` | `doctor_score.ranked` (new table) |
| `iRanked` | `doctor_score.i_ranked` (new table) |

### Removed Fields

| Legacy Field | Reason |
|---|---|
| `id` (integer) | Legacy internal ID, not needed |
| `workExperience` | Was hardcoded example data in legacy |
| `instantMeetStartAt/EndAt` | Derived from availability, not stored per-request |

## New PostgreSQL Tables

Migration file: `db/postgres/migrations/YYYYMMDDHHMMSS_doctor_ranking_tables.sql`

```sql
-- Doctor rating (follows doctor_case pattern)
CREATE TABLE doctor_rating (
    doctor_id UUID PRIMARY KEY REFERENCES doctor(doctor_id) ON DELETE CASCADE,
    rating NUMERIC(3,1) NOT NULL DEFAULT 0
        CHECK (rating >= 0 AND rating <= 5),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Doctor ranking score
-- ranked  = position in scheduled doctor list (1 = top)
-- i_ranked = position in instant doctor list (1 = top)
-- score   = numeric score used for sorting (higher = better)
CREATE TABLE doctor_score (
    doctor_id UUID PRIMARY KEY REFERENCES doctor(doctor_id) ON DELETE CASCADE,
    score NUMERIC(10,2) NOT NULL DEFAULT 0,
    ranked INTEGER NOT NULL DEFAULT 0,
    i_ranked INTEGER NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Composite index for cursor pagination (score DESC, doctor_id DESC)
CREATE INDEX idx_doctor_score_ranking ON doctor_score (score DESC, doctor_id DESC);

-- Doctor consultation duration options
CREATE TABLE doctor_duration (
    doctor_duration_id SERIAL PRIMARY KEY,
    doctor_id UUID NOT NULL REFERENCES doctor(doctor_id) ON DELETE CASCADE,
    duration_minutes INTEGER NOT NULL
        CHECK (duration_minutes IN (15, 30, 50)),
    is_default BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (doctor_id, duration_minutes)
);
```

## Page Token Pagination

### Token Format

Base64-encoded JSON cursor:

```json
{
  "s": 85.5,
  "u": "82e6b8ce-071c-4752-83b5-5cd552ef7ffb"
}
```

- `s` = last doctor's score (for sort position)
- `u` = last doctor's UUID (for tie-breaking)

### Query Logic

```sql
-- First page (no token)
SELECT ... FROM doctor d
JOIN doctor_score ds ON d.doctor_id = ds.doctor_id
JOIN doctor_availability da ON d.doctor_id = da.doctor_id
WHERE da.instant_mode_enabled = true
  AND d.approval_status = 'approved'
  AND d.is_active = true
ORDER BY ds.score DESC, d.doctor_id DESC
LIMIT :size;

-- Subsequent pages (with token)
SELECT ... FROM doctor d
JOIN doctor_score ds ON d.doctor_id = ds.doctor_id
JOIN doctor_availability da ON d.doctor_id = da.doctor_id
WHERE da.instant_mode_enabled = true
  AND d.approval_status = 'approved'
  AND d.is_active = true
  AND (ds.score, d.doctor_id) < (:last_score, :last_uuid)
ORDER BY ds.score DESC, d.doctor_id DESC
LIMIT :size;
```

### Token Generation

- If result count < requested size: `nextPageToken = null` (no more pages)
- Otherwise: encode last item's `(score, uuid)` into base64 JSON

### Why Not Offset Pagination

Offset pagination (`OFFSET 30`) suffers from:
- **Pagination drift**: if a doctor's score changes or a doctor is added/removed between pages, items can be skipped or duplicated
- **Performance**: `OFFSET N` scans N rows then discards them

Cursor pagination is positional and stable.

## Redis Architecture

Redis serves as a **fast ranked-ID lookup layer** and **profile cache**. Cursor-based pagination is handled by PostgreSQL (which supports composite `(score, uuid)` cursors natively). Redis provides fast cardinality counts and availability state.

### Data Structures

| Key Pattern | Type | Contents | TTL |
|---|---|---|---|
| `ranking:instant` | Sorted Set | member=doctor_uuid, score=doctor_score | None (managed) |
| `ranking:scheduled` | Sorted Set | member=doctor_uuid, score=doctor_score | None (managed) |
| `doctor:profile:{uuid}` | String (JSON) | Full doctor profile JSON | 5 min |
| `privilege:specialty:{id}` | String (JSON) | Privilege benefits JSON | 5 min |

Note: No separate `doctor:unavailable` keys. Unavailable doctors are removed from the sorted set directly (`ZREM` on session start, `ZADD` on session end) to avoid short pages from post-fetch filtering.

### Read Flow

```
GET /ranking/v1/doctors/instant?size=30&pageToken=xxx

1. Decode pageToken → (last_score, last_uuid)
2. Query PostgreSQL with cursor pagination:
   WHERE (ds.score, d.doctor_id) < (:last_score, :last_uuid)
   AND doctor is in Redis ranking:instant set (or use SQL availability filter)
   ORDER BY ds.score DESC, d.doctor_id DESC LIMIT :size
3. For each doctor in result:
   - Check doctor:profile:{uuid} in Redis
   - Cache hit → use cached JSON
   - Cache miss → build from query result, SET with 5min TTL
4. Get total from ZCARD ranking:instant (fast O(1))
5. Encode nextPageToken from last result's (score, uuid)
6. Return response
```

PostgreSQL handles the paginated query (composite cursor works natively). Redis provides the availability-filtered member set and fast total count.

### Write Flow (Warm-up)

On application startup (or via CLI command):

1. Query all active, approved doctors with scores from PostgreSQL
2. `ZADD ranking:instant` for instant-enabled doctors (exclude those currently in consultation)
3. `ZADD ranking:scheduled` for scheduled-enabled doctors
4. Pre-cache doctor profiles

### Availability Update Flow (Pub/Sub)

```
Consultation event received (session started):
  1. ZREM ranking:instant {uuid}  -- remove from instant set
  2. DEL doctor:profile:{uuid}    -- invalidate profile cache

Consultation event received (session ended):
  1. Fetch doctor score from PostgreSQL
  2. ZADD ranking:instant score {uuid}  -- re-add with score
  3. DEL doctor:profile:{uuid}

Availability toggle event received:
  1. Update doctor_availability in PostgreSQL
  2. If instant_mode_enabled changed:
     - true  → ZADD ranking:instant score uuid
     - false → ZREM ranking:instant uuid
  3. Same for schedule_mode_enabled → ranking:scheduled
  4. DEL doctor:profile:{uuid} (invalidate cache)
```

## Privilege Service Integration

### Endpoint

```
GET http://<privilege-man-host>/privilege/internal/v1/benefit/list?specialtyId={id}
```

Internal endpoint, no auth headers required.

### Response (mapped to legacy format)

From privilege-man:
```json
{
  "privilegeBenefits": [{
    "privilegeId": 123,
    "privilegeDisplayName": "Premium Health Plan",
    "providerName": "Insurance Company ABC",
    "companyLogoUrl": "https://...",
    "packageTypeName": "Gold"
  }]
}
```

Mapped to legacy `associatePrivileges` array format.

### Caching

Cache privilege responses in Redis (`privilege:specialty:{id}`) with 5-minute TTL. Privilege data rarely changes.

### Configuration

Add to `server/config/default.toml`:
```toml
[service]
privilege_service_base_uri = "http://localhost:8081/privilege"
```

Add to `.env`:
```
SERVICE__PRIVILEGE_SERVICE_BASE_URI=http://localhost:8081/privilege
```

## Pub/Sub Configuration

### Existing: Consultation Events

Already subscribed in `bootstrap.rs` via `spawn_consultation_subscriber()`. Add handler logic to detect session start/end and update Redis availability flags.

### New: Doctor Availability Events

New subscription for explicit availability toggles (doctor turns instant mode on/off).

Add to config:
```toml
[pubsub.subscriptions]
doctor_availability = "doctor-availability-sub"

[pubsub.topics]
doctor_availability = "doctor-availability-events"
```

Event payload:
```json
{
  "event_type": "DoctorAvailabilityChanged",
  "doctor_id": "82e6b8ce-071c-4752-83b5-5cd552ef7ffb",
  "instant_mode_enabled": true,
  "schedule_mode_enabled": false,
  "timestamp": "2026-03-30T10:00:00Z"
}
```

## Authentication

These are **patient-facing public listing endpoints** — no authentication required. No `DoctorIdentity`, `PatientHeaders`, or `BackofficeIdentity` extractors needed. The legacy API also serves these without auth headers.

## Error Handling

- **Malformed pageToken**: Return HTTP 400 `BadRequest("Invalid page token")`
- **Redis unavailable**: Fall back to PostgreSQL-only queries (degraded mode, no cache). Log warning.
- **Privilege service timeout**: Return doctor profile without `associatePrivileges` field (empty array). Log warning. Use 5s timeout.
- **Doctor not found** (single doctor endpoint): Return HTTP 200 with legacy-compatible empty response (matching `DOCTOR_PROFILE_SUCCEDDED` with null/zero fields), consistent with project response convention.

## Legacy Compatibility Notes

- `"DOCTOR_LISTING_SUCCEDDED"` and `"DOCTOR_PROFILE_SUCCEDDED"` are intentionally misspelled to match the legacy API contract.
- `pagingMetaData.total` uses `ZCARD` from Redis sorted set (O(1)). This is an approximate count that excludes currently-unavailable doctors since they are ZREM'd.

## Module Structure

```
server/src/module/ranking/
  mod.rs            - router() function, module wiring
  handlers.rs       - 3 Axum handlers with #[utoipa::path]
  repo.rs           - RankingRepoTrait + PostgreSQL implementation
  cache.rs          - RankingCacheTrait + Redis implementation
  models.rs         - DoctorRankingResponse, DoctorProfile, PageToken DTOs
  privilege.rs      - PrivilegeServiceTrait + reqwest HTTP client
  subscriber.rs     - Pub/Sub event handler for availability changes
```

### Dependencies to Add (server/Cargo.toml)

```toml
redis = { version = "0.27", features = ["tokio-comp", "connection-manager"] }
base64 = "0.22"
```

### Bootstrap Wiring

In `bootstrap.rs`:
1. Initialize Redis connection manager from config
2. Create `RankingCache` with Redis connection
3. Create `PrivilegeService` with HTTP client + base URI
4. Create `RankingRepo` with PgPool
5. Wire `ranking::router(repo, cache, privilege_svc)`
6. Nest at `/ranking/v1`
7. Spawn availability subscriber
8. Run warm-up task

## Docker Compose (Local Dev)

Create `docker-compose.yml` at project root:

```yaml
services:
  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"
    volumes:
      - redis-data:/data

volumes:
  redis-data:
```

Add to `.env`:
```
REDIS__URL=redis://localhost:6379
```

## Test Data

Migration includes ~35 test doctors with:
- Varied scores (10.0 to 99.0) for pagination testing
- Mix of instant/scheduled availability
- Multiple specialties and fee levels
- Some with ratings, some without

This ensures page token pagination can be verified across multiple pages with `size=10`.

## Configuration Summary

New config fields in `AppConfig`:

```rust
pub struct RedisConfig {
    pub url: String,  // env: REDIS__URL
}

// Added to ServiceConfig:
pub privilege_service_base_uri: String,  // env: SERVICE__PRIVILEGE_SERVICE_BASE_URI

// Added to PubsubSubscriptions:
pub doctor_availability: Option<String>,  // env: PUBSUB__SUBSCRIPTIONS__DOCTOR_AVAILABILITY
```

### Environment Variable Summary

| Env Var | Default | Purpose |
|---|---|---|
| `REDIS__URL` | `redis://localhost:6379` | Redis connection URL |
| `SERVICE__PRIVILEGE_SERVICE_BASE_URI` | `http://localhost:8081/privilege` | Privilege service internal API |
| `PUBSUB__SUBSCRIPTIONS__DOCTOR_AVAILABILITY` | `doctor-availability-sub` | Pub/Sub subscription for availability events |

## Verification Plan

1. **Unit tests**: Page token encode/decode, Redis cache operations (mock), response mapping
2. **Integration tests**: Full request cycle with `axum-test` TestServer, mock Redis + mock privilege service via `wiremock`
3. **Manual local testing**:
   - `docker compose up redis`
   - `cargo run` (server starts, warm-up populates Redis)
   - `curl localhost:8080/ranking/v1/doctors/instant?size=10` → verify first page with nextPageToken
   - Use nextPageToken to fetch page 2 → verify no duplicates/skips
   - `curl localhost:8080/ranking/v1/doctor/{uuid}` → verify single doctor profile
4. **Pub/Sub testing**: Use Pub/Sub emulator to send availability events, verify Redis updates
