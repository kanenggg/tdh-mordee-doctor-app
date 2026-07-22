# Cross-Repository API Contract Ownership Design

**Date:** 2026-07-22

## Goal

Make the Backstage catalogs in `tdh-mordee-doctor-app` and
`tdh-biz-doctor-apmv2` describe their real HTTP and event integrations without
duplicating ownership of the same API contract.

## Current State

DoctorApp is the runtime producer and source of truth for the
`DoctorProfileApproved` event published to `doctor-profile.approved`. APMv2
consumes that event to maintain its doctor identity and consultation
configuration projection. Despite that ownership, APMv2 currently stores and
registers the `doctor-profile-events` AsyncAPI definition.

APMv2 owns the Consultation HTTP API and its published consultation events.
DoctorApp calls the Consultation HTTP API, publishes consultation workflow
events, and consumes consultation events in its background service. The
DoctorApp catalog does not currently declare those cross-repository API
relations.

## Considered Approaches

### Canonical producer ownership

Store each contract in the repository that produces and owns it. Consumers
refer to the Backstage API entity by name. This avoids duplicate entities and
contract drift and matches the runtime domain ownership.

This is the selected approach.

### Consumer snapshots

Keep a copy of each external contract under every consumer's `depends-on`
directory. This makes local validation convenient but creates multiple files
that can diverge without additional synchronization tooling.

### Dedicated contracts repository

Move all cross-service contracts to a separate repository. This can centralize
governance but adds a new repository and release workflow that is unnecessary
for the current two-repository relationship.

## Design

### DoctorApp-owned contract

Move the canonical `DoctorProfileApproved` AsyncAPI definition to:

```text
tdh-mordee-doctor-app/specs/provides/doctor-profile-approved.asyncapi.yaml
```

Register one Backstage API entity named `doctor-profile-events` in the
DoctorApp `catalog-info.yaml`. Its definition uses a relative `$text` reference
to the canonical AsyncAPI file. Both DoctorApp components may provide the API
because the main service commits the event to its outbox and the background
service performs Pub/Sub delivery; ownership remains DoctorApp's.

The APMv2 catalog must not register another `doctor-profile-events` API entity
or embed another `$text` definition. `consultation-bg-rs` continues to declare
`doctor-profile-events` in `consumesApis`.

### APMv2-owned contracts

APMv2 remains the owner and registrar of:

- `consultation-rs-api`, the Consultation HTTP OpenAPI contract.
- `biz-apm-published-events`, the active runtime AsyncAPI contract.

DoctorApp declares those API names only through `consumesApis`:

- `tdh-mordee-doctor-app` consumes `consultation-rs-api` and
  `biz-apm-published-events`.
- `tdh-mordee-doctor-bg` consumes `biz-apm-published-events`.

The catalogs use unqualified entity names because all entities are registered
in Backstage's `default` namespace. Cross-repository resolution therefore
requires both catalog locations to be registered in the same Backstage
instance.

### File migration

After the DoctorApp copy and catalog registration exist, remove
`tdh-biz-doctor-apmv2/specs/depends-on/doctor-profile-approved.asyncapi.yaml`.
Update active documentation that points to the removed local file so it names
the DoctorApp-owned Backstage API entity or canonical repository path instead.
Historical implementation plans and dated design records remain unchanged.

## Validation

Validation will:

1. Parse both multi-document `catalog-info.yaml` files.
2. Parse the moved AsyncAPI YAML file.
3. Assert there is exactly one `doctor-profile-events` API entity across the
   two catalogs and that its `$text` target exists.
4. Assert the expected `providesApis` and `consumesApis` relationships.
5. Scan active APMv2 documentation for references to the removed contract
   path.
6. Run `git diff --check` in both repositories.

No runtime Rust, Pub/Sub topic, event payload, deployment, or rollout behavior
changes as part of this work.
