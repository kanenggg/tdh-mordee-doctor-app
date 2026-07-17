# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

This is a multi-language Protocol Buffers repository for TDH (Telehealth Doctor Hub) services. Proto files in `protos/` are the single source of truth, generating type-safe code for Scala, Rust, Python, and TypeScript. All services use identical types with compile-time safety through discriminated unions (oneof).

## Common Commands

```bash
# Generate code for all languages (primary development command)
make gen          # or: buf generate

# Verify proto definitions
make lint         # buf lint - checks proto style
make build        # buf build - compiles protos
make break        # buf breaking --against '.git#branch=main'

# Run tests
make test-all              # All languages
make test-rust             # cd rust && cargo test
make test-scala            # cd scala && sbt test
make test-python           # cd python && python -m pytest tests/
make test-typescript       # cd typescript && npm test

# Clean generated files
make clean

# Publish to GCP Artifact Registry (requires gcloud auth)
make publish-all
make publish-scala
make publish-rust
make publish-python
make publish-typescript
```

## Architecture

### Code Generation Flow

```
protos/*.proto (source of truth)
     ↓
buf generate (reads buf.gen.yaml)
     ↓
┌─────────────┬─────────────┬─────────────┬─────────────┐
│  Scala      │  Rust       │  Python     │ TypeScript  │
│  ScalaPB    │  tonic+     │  betterproto│ protobuf-ts │
│  (sealed    │  prost      │             │             │
│  traits)    │  (enums)    │             │             │
└─────────────┴─────────────┴─────────────┴─────────────┘
```

### Directory Structure

- `protos/` - Proto definitions organized by domain
  - `common/` - Shared types (ConsultationEvent, PatientIdentity, UserIdentity, enums)
  - `onboarding/` - Doctor onboarding types
  - `appointment/` - Appointment status types
  - `notification/` - Notification payloads
- `buf.gen.yaml` - Code generation config (plugins, output dirs)
- `buf.yaml` - Breaking changes config (FILE mode, allows ENUM/FIELD deletion during v0)
- `scripts/generate.sh` - Code generation script using protoc

### Generated Code Locations

| Language | Output Directory | Build Config |
|----------|------------------|--------------|
| Scala    | `scala/src/generated/` | `scala/build.sbt` |
| Rust     | `rust/src/` | `rust/build.rs` + `Cargo.toml` |
| Python   | `python/tdh_protocol/` | `python/pyproject.toml` |
| TypeScript | `typescript/src/` | `typescript/package.json` |

## Proto Design Patterns

### Discriminated Unions with `oneof`

All domain events use `oneof` for type-safe exhaustive matching across languages:

```protobuf
message ConsultationEvent {
  oneof event {
    TimeslotReserved timeslot_reserved = 1;
    SessionCreated session_created = 2;
    DoctorJoined doctor_joined = 3;
    // ... more variants
  }
}
```

This generates:
- **Scala**: Sealed trait hierarchy with exhaustive pattern matching
- **Rust**: Enum with variants
- **TypeScript**: Discriminated union with `case` + `value` fields

### Naming Conventions

- **Files**: `lowercase_with_underscores.proto`
- **Packages**: `tdh.protocol.<domain>` (common, onboarding, appointment, notification)
- **Messages**: `PascalCase`
- **Fields**: `snake_case`
- **Enums**: `PascalCase` with values in `SCREAMING_SNAKE_CASE`

### Import Structure

Proto files import from `common/` without package prefix:
```protobuf
import "common/patient.proto";
import "common/consulting.proto";
```

## Backward Compatibility

**Safe (non-breaking) changes:**
- Add new field
- Add new message
- Add new enum value
- Add new oneof variant
- Deprecate field (don't remove)

**Breaking changes (require major version bump):**
- Remove field or change field number
- Change field type
- Remove enum value
- Rename package/message

Breaking changes detected by `buf breaking` against `main` branch.

## Versioning

- `0.1.0` - Current version (v0 = development phase, breaking changes allowed)
- Version defined in: `scala/build.sbt`, `rust/Cargo.toml`, `python/pyproject.toml`, `typescript/package.json`
- All language packages must be version bumped together

## Publishing

All publishing configuration is centralized in `.publish-config` at the repository root. This file defines:

- GCP project: `tdg-dh-truehealth-core-nonprod`
- Region: `asia-southeast1`
- Registry base name: `tdh-protocol`

**Individual registries:**
- Maven (Scala/Java): `tdh-protocol-maven`
- Cargo (Rust): `tdh-protocol-cargo` (raw format)
- PyPI (Python): `tdh-protocol-python`
- npm (TypeScript): `tdh-protocol-npm`

### Local Overrides

For local development with different registry settings:
1. Copy `.publish-config.local.example` to `.publish-config.local`
2. Customize the variables
3. The `.publish-config.local` file is gitignored

### Authentication

All publishing requires `gcloud auth print-access-token` for authentication.
