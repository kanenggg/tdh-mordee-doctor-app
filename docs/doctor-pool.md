# Doctor Pool Service

Doctor Pool is a new workspace service for patient-facing doctor ranking and
personalized doctor search. It is separated from `server` so ranking policy,
pool indexes, and privilege eligibility can evolve independently from the
doctor-app API gateway.

## Current Implementation

- Crate path: `doctor-pool/`
- Binary: `cargo run -p doctor-pool`
- Default port: `0.0.0.0:8081` (`PORT` overrides it)
- Local adapters: in-memory ranking, doctor projection, and privilege
  eligibility read models
- Production adapters: Postgres projection reads and Redis sorted-set ranking
- Startup warm-up: loads active doctors from `doctor_profile` into Redis when
  `DATABASE_URL` and `REDIS_URL` are configured

## API Contract

```text
GET /health
GET /doctor-pool/v1/doctors/instant?size=30&pageToken=<token>
GET /doctor-pool/v1/doctors/scheduled?size=30&pageToken=<token>
GET /doctor-pool/v1/doctors/{doctorUuid}
```

Privilege filters are optional:

```text
GET /doctor-pool/v1/doctors/instant
GET /doctor-pool/v1/doctors/instant?privilegeId=9001
GET /doctor-pool/v1/doctors/instant?privilegeId=9001&privilegeId=9002
GET /doctor-pool/v1/doctors/instant?privilegeIds=9001,9002
```

No privilege IDs means the pool is not privilege-filtered. One or more
privilege IDs means strict eligibility filtering by department.

## Internal Update API

Doctor-pool exposes internal update routes so upstream event consumers or
service integrations can keep the pool current after onboarding, booking, score,
profile, availability, and privilege changes.

```text
POST /doctor-pool/v1/internal/doctors
POST /doctor-pool/v1/internal/doctors/{doctorUuid}/instant-availability
POST /doctor-pool/v1/internal/privileges/{privilegeId}/departments
```

`POST /internal/doctors` upserts the doctor projection and Redis ranked-set
membership:

```json
{
  "doctorId": "00000000-0000-0000-0000-000000000001",
  "displayName": "Doctor Name",
  "score": 100,
  "departmentId": 10,
  "instantModeEnabled": true,
  "scheduleModeEnabled": true
}
```

`POST /internal/doctors/{doctorUuid}/instant-availability` removes or re-adds a
doctor in the instant pool, which covers booking/session occupancy:

```json
{ "available": false }
```

`POST /internal/privileges/{privilegeId}/departments` refreshes personalized
search eligibility:

```json
{ "departmentIds": [10, 20] }
```

## Ranking And Filtering

Ranking still applies for all requests:

- Instant search uses the instant doctor pool.
- Scheduled search uses the scheduled doctor pool.
- Results sort by `score DESC`, then `doctor_id DESC`.
- Cursor pagination uses the last returned doctor's score and UUID.
- Privilege filtering happens before pagination.
- Multiple privilege IDs use union semantics across eligible departments.

Example: if `privilegeId=9001` maps to departments `{10, 20}` and
`privilegeId=9002` maps to `{30}`, the request is eligible for departments
`{10, 20, 30}`. Doctors outside those departments are excluded.

## SOLID Boundaries

- `DoctorPoolService`: orchestrates search, eligibility filtering, and
  pagination.
- `RankingIndex`: returns ordered doctor IDs for instant/scheduled pools.
- `DoctorProjectionRepo`: returns doctor display/search projections.
- `EligibilityReadModel`: returns department IDs for privilege IDs.

The domain service depends on these ports, not on Redis, PostgreSQL, or HTTP
clients directly. Production storage adapters should implement the ports without
changing search behavior.

## Doctor Profile Source

Doctor-pool reads doctor display/search profile data from `doctor_profile`
only. `doctor_profile_draft` is not a runtime source. Its column shape is used
only as the reference template for mock data before inserting those mock rows
into `doctor_profile`.

The profile path maps `doctor_account_id` to the seeded UUID template so it can
reuse existing score and availability rows. Example: `doctor_account_id = 12`
maps to
`a0000012-0012-4000-8000-000000000012`.

Mock data seed:

- `doctor-pool/mock/doctor_profile_mock.sql` inserts draft-shaped mock profile
  rows into `doctor_profile`.

Projection fields:

- `displayName`: `first_name` + `last_name`, preferring `en` and falling back to
  `th`
- `departmentId`: first specialty object ID from the profile JSON
- `score`: `doctor_score.score` when present, otherwise `0`
- `instantModeEnabled` / `scheduleModeEnabled`: `doctor_availability` values
  when present, otherwise `true` for mock visibility

## Redis Production Design

Redis should be used as the production ranking/cache layer to improve response
time and reduce database load.

Recommended keys:

| Key | Type | Purpose |
|---|---|---|
| `doctor_pool:instant` | Sorted set | Instant doctor IDs scored by ranking score |
| `doctor_pool:scheduled` | Sorted set | Scheduled doctor IDs scored by ranking score |
| `doctor_pool:projection:{doctorUuid}` | String/JSON | Future cached doctor search projection |
| `doctor_pool:privilege_departments:{privilegeId}` | String/JSON or set | Future cached eligible department IDs |

Implemented update flow:

- Warm up Redis from PostgreSQL/read models on service startup or deployment.
- Update sorted sets when doctor score, approval, active status, or availability
  changes via the internal doctor upsert route.
- Remove a doctor from `doctor_pool:instant` when they enter an instant
  consultation; re-add them after the session ends if still eligible via the
  instant availability route.
- Refresh privilege departments through the internal privilege departments
  route.

If Redis is unavailable, production adapters should either fail closed with a
clear 5xx or use a controlled database fallback, depending on latency and load
requirements.

## Configuration

| Env Var | Purpose |
|---|---|
| `PORT` | HTTP port, default `8081` |
| `DATABASE_URL` | Postgres connection URL for doctor projections and warm-up |
| `REDIS_URL` | Redis connection URL for ranked pool sorted sets |
| `POSTGRES_MAX_CONNECTIONS` | Optional Postgres pool size, default `10` |
| `PRIVILEGE_DEPARTMENT_MAP_JSON` | Optional initial `privilegeId -> departmentId[]` JSON map |

If `DATABASE_URL` or `REDIS_URL` is absent, doctor-pool starts with empty
in-memory data for local development.
