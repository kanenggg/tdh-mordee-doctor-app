# PROJECT KNOWLEDGE BASE

**Generated/updated:** 2026-07-06

## OVERVIEW

Project: **TDH Doctor App** (`tdh-mordee-doctor-app`)

Healthcare telehealth API gateway for doctors, built in Rust. It manages appointments, consultations, notifications, onboarding, doctor profile/backoffice operations, EHR lookups, ranking, and timeslots. It proxies or integrates with upstream services such as auth/identity, registration, EHR, consultation, FCM, GCP Pub/Sub, Cloud Tasks, Firestore, Firebase RTDB, PostgreSQL, Redis, and GCS/KMS.

### Stack

- Rust 2021 Cargo workspace
- Axum 0.8 + Tokio 1
- SQLx 0.8/PostgreSQL, Firestore, Firebase Realtime Database, Redis (`deadpool-redis`)
- GCP: ADC auth, Pub/Sub, Cloud Tasks, FCM, Cloud Logging/Trace, KMS/GCS where configured
- OpenAPI via `utoipa` + Swagger UI
- Testing via `cargo nextest`/`cargo test`, `axum-test`, `wiremock`, `testcontainers`
- Time: prefer `jiff` for new date/time code; keep chrono only for protocol compatibility

## STRUCTURE

```text
Cargo.toml                         # Workspace: server, server-bg, crates/common, tdh-protocol/rust
server/                            # Main doctor API service (HTTP API gateway)
  config/default.toml              # Main service defaults; listens on 0.0.0.0:8080
  src/bootstrap.rs                 # Infra setup, dependency wiring, router nesting, graceful shutdown
  src/main.rs                      # Main server binary entrypoint
  src/openapi.rs                   # Central utoipa ApiDoc registration
  src/bin/                         # Utility binaries (notifications, schedule setup)
  src/core/                        # auth/extractors, errors, logging, telemetry, GCP auth, KMS, health
  src/model/                       # Shared server-domain models
  src/module/                      # Feature modules: appointment, consultation, onboarding, profile, etc.
  src/repo/                        # Generic Firestore/Firebase repos and models
  tests/                           # Main service integration tests
server-bg/                         # Background event/task service; listens on 0.0.0.0:8081
  config/default.toml
  src/module/doctor_calendar/      # Calendar update Pub/Sub processing
  src/module/doctor_notification/  # Doctor notification delivery/scheduling routes
  tests/                           # Background-service tests
crates/common/                     # Shared config/core/messaging/repo/notification/patient code
crates/tdh-protocol/               # Git submodule with protobuf-derived shared protocol types
db/postgres/
  schema/                          # Source-of-truth PostgreSQL schema files
  migrations/                      # Atlas-generated migrations
  atlas.hcl                        # Atlas migration config
  seeds/, initial/                 # Seed/initialization scripts
docs/CODEMAPS/                     # Architecture maps (frontend/backend/database/integrations/workers)
justfile                           # Common Docker/db/dev recipes
bacon.toml                         # Watch/check/test/run jobs
rust.Dockerfile                    # Rust Docker build image definition
```

## COMMANDS

Run commands from the repository root unless noted.

| Action | Command |
|---|---|
| Init protocol submodule | `just init-project` or `git submodule update --init crates/tdh-protocol` |
| Build workspace | `cargo build --workspace` |
| Build main service | `cargo build -p server` |
| Build background service | `cargo build -p server-bg` |
| Run main service | `cargo run -p server --bin server -- --config-dir ./server/config` or `just dev` |
| Run background service | `cargo run -p server-bg --bin server-bg -- --config-dir ./server-bg/config` |
| Test all | `cargo nextest run` (preferred) or `cargo test --workspace` |
| Test main service | `cargo nextest run -p server` or `cargo test -p server` |
| Test one integration file | `cargo nextest run --test webhook_publisher_test` |
| Lint | `cargo clippy --workspace -- -D warnings` |
| Format | `cargo fmt --all` |
| Watch/run via bacon | `bacon` (default `run-server-service`) |
| Docker build/deploy recipe | `just buildx-local <module_name>` / `just build-gcloud-docker <module_name>` |
| DB migration wrapper | `just db <command>` |
| Add blank migration file | `just add-migration postgres <migration_name>` |
| Pub/Sub local emulator helper | `just pubsub-emulator` |

Local development usually requires GCP Application Default Credentials:

```bash
gcloud auth application-default login
```

Local secrets/config overrides go in service-local `.env` files (for example `server/.env`) and environment variables use config-rs nested names such as `POSTGRES__DATABASE_URL`, `FIRESTORE__GCP_PROJECT_ID`, `SYS__HOST`, `SYS__PORT`.

## ARCHITECTURE & CONVENTIONS

### Main service module pattern

Feature code lives under `server/src/module/<name>/` and should follow this DDD-oriented layout:

- `mod.rs` declares submodules and exposes `pub fn router(...)`; it composes dependencies and delegates route declaration to handlers.
- `handler.rs`/`handlers.rs` contains Axum handlers returning `AppResult<impl IntoResponse>` and a `pub fn routes(svc: Arc<Service>) -> Router` that declares `.route(...)` paths and state.
- `service.rs` contains concrete business/application logic. Do **not** introduce service traits solely for unit mocks.
- `repo.rs` contains persistence boundaries (usually trait + implementation) for database/external data.
- `gateway.rs` or `gateway/` contains HTTP/RPC clients for upstream services.

Modules are wired in `server/src/bootstrap.rs` via `Router::nest(...)`. Current main route roots include:

Route path convention for upstream/new cross-service APIs is access-before-version: service-to-service `/internal/vN/...`, admin/backoffice `/admin/vN/...`, and public `/vN/...` (not `/public/vN`). Existing DoctorApp roots listed below are current compatibility roots unless a feature-specific migration explicitly changes them. Doctor Profile projection is published from the durable outbox; it has no direct APM HTTP sync endpoint.


```text
GET /health
/swagger
/api-docs/openapi.json
/notifications/v1
/consultation/v1
/ranking/v1
/profile
/timeslot
/appointment/v1          # includes merged ekyc routes
/onboarding/v1
/backoffice/v1
/ehr/v1
```

### Background service

`server-bg` is a separate workspace package/binary for background event and notification work. It builds an Axum app from handlers in:

- `server-bg/src/module/doctor_calendar`
- `server-bg/src/module/doctor_notification`

It reuses infrastructure from `crates/common` and has its own `server-bg/config/default.toml` (default port 8081).

### Shared code

`crates/common` holds cross-service infrastructure: config adapters, core logging/telemetry/error/GCP auth, Firestore/Firebase repos, Pub/Sub/Cloud Tasks helpers, notification repo, and patient service. Prefer adding truly shared infrastructure here instead of duplicating it in `server` and `server-bg`.

For TDH-wide domain/protocol types, prefer adding them to `tdh-protocol` (protobuf-derived submodule) rather than defining duplicate types in a service crate.

## CODING STANDARDS

- Rust edition 2021; use `cargo fmt --all` before handoff.
- Run `cargo clippy --workspace -- -D warnings` for lint-level validation when feasible.
- Prefer `anyhow` for setup/infrastructure errors and `thiserror` for typed domain/application errors.
- Handlers should return `AppResult<T>` and rely on `AppError` `IntoResponse` mapping.
- Domain-level "not found" compatibility cases should return **HTTP 200** with a typed JSON variant (for example `{ "__type": "AppointmentNotFound" }`), not HTTP 404. Reserve 4xx/5xx for HTTP-level/auth/server failures.
- Repositories may be traits where they represent real external boundaries. Avoid creating service traits only to enable mocks.
- For complex PostgreSQL upserts, multi-table writes, or multi-stage reads, prefer a PostgreSQL function called from Rust over a large SQL workflow in application code.
- Keep route declarations close to handlers (`handler::routes`), not in feature `mod.rs`.
- New OpenAPI-covered handlers should add `#[utoipa::path(...)]`, derive/register schemas, and update `server/src/openapi.rs`.

## DATA & MIGRATIONS

Data stores:

- **PostgreSQL**: main doctor/profile/fee/onboarding/timeslot/ranking data under `db/postgres/`.
- **Firestore**: generic CRUD/event-style documents via `FirestoreRepo`.
- **Firebase RTDB**: real-time appointment/consultation state via REST and ADC/secret configuration.
- **Redis**: cache/rate-limit/timeslot-related state.

PostgreSQL schema workflow:

1. Modify source schema in `db/postgres/schema/` first.
2. Use the Atlas workflow from `db/postgres/` / `just db ...` to generate SQL into `db/postgres/migrations/`.
3. Review generated migration SQL before committing.
4. Avoid hand-written migrations that drift from schema source unless intentionally reviewed.

## AUTHENTICATION & CONFIG GOTCHAS

- This service trusts the upstream gateway; it does **not** validate user tokens itself.
- Doctor/backoffice auth reads `tdh-sec-iam-user-identity` (JSON `UserIdentity`).
- `DoctorIdentity` expects canonical doctor `account_type == 2`; legacy `account_type == 3` may still be accepted in compatibility paths.
- `BackofficeIdentity` expects `account_type == 4`.
- Patient context uses `patient-account-id` and `patient-profile-id` headers.
- `rustls::crypto::aws_lc_rs::default_provider().install_default()` in service bootstrap is intentional. Do not remove it; it avoids rustls provider auto-detection failures when multiple providers are in the dependency tree.
- OpenTelemetry defaults to OTLP at `http://localhost:4317` when enabled.

## TESTING GUIDANCE

- Prefer integration tests in `server/tests/` and `server-bg/tests/` for routes, repositories, DB functions, and workflows.
- Use `axum-test` for HTTP route tests, `wiremock` for upstream HTTP services, and `testcontainers` for PostgreSQL/Redis where infrastructure behavior matters.
- Unit-test only complex pure business logic that can be tested without infrastructure.
- Useful examples: appointment, consultation, onboarding/profile, EHR, ranking, notifications, and webhook publisher tests in `server/tests/`.

## WHERE TO LOOK

- Main API wiring: `server/src/bootstrap.rs`
- Main config: `server/src/config.rs`, `server/config/default.toml`
- Background API wiring: `server-bg/src/lib.rs`, `server-bg/src/main.rs`
- Background config: `server-bg/src/config.rs`, `server-bg/config/default.toml`
- Shared infrastructure: `crates/common/src/`
- Error handling: `server/src/core/error.rs`, `crates/common/src/core/error.rs`
- OpenAPI: `server/src/openapi.rs`
- Database schema/migrations: `db/postgres/schema/`, `db/postgres/migrations/`
- Architecture docs: `docs/CODEMAPS/INDEX.md`
- Human/Claude context: `CLAUDE.md`

## NOTES

- The workspace currently has four members: `server`, `server-bg`, `crates/common`, and `crates/tdh-protocol/rust`.
- `tdh-protocol` is a submodule; initialize it before full workspace builds.
- Main service default port is 8080; background service default port is 8081.
- Use `jiff` first for new datetime handling.
- Preserve Scala/API compatibility response semantics unless explicitly changing the contract.
