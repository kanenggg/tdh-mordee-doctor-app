# DoctorApp–APMv2 integration

This document is the canonical overview of the active runtime integration
between `tdh-mordee-doctor-app` (DoctorApp) and
`tdh-biz-doctor-apmv2` (APMv2). Payload details and operational rollout steps
remain in the linked contract and rollout documents.

## System ownership

| Capability | Authority | Consumer or projection |
|---|---|---|
| Doctor Profile and Doctor Consultation Configuration | DoctorApp | APMv2 keeps a read-only projection |
| Doctor Operational Availability, Appointment Holds, Doctor Occupancy, booking, and consultation execution | APMv2 | DoctorApp calls or consumes APMv2 contracts |

DoctorApp is the only editable source for consultation channels, languages,
duration, fee, currency, and profile eligibility. APMv2 uses the projected
configuration for eligibility and scheduling decisions but does not edit or
republish it as a source fact.

## Active integration paths

### Consultation HTTP API

DoctorApp calls APMv2 through the Backstage API entity
`consultation-rs-api`. APMv2 owns its
[OpenAPI definition](https://github.com/kanenggg/tdh-biz-doctor-apmv2/blob/main/specs/provides/consultation-rs.yaml).

### Doctor profile projection

DoctorApp publishes `DoctorProfileApproved` on `doctor-profile.approved`.
APMv2 `consultation-bg-rs` consumes the Backstage API entity
`doctor-profile-events` and stores a read-only projection.

- [Registered AsyncAPI contract](../../specs/provides/doctor-profile-approved.asyncapi.yaml)
- [Detailed Doctor Profile event documentation](doctor-profile-events.md)
- [DoctorApp outbox rollout](doctor-profile-outbox-rollout.md)
- [APMv2 projection rollout](https://github.com/kanenggg/tdh-biz-doctor-apmv2/blob/main/docs/plans/DOCTOR_PROJECTION_SYNC_ROLLOUT.md)

Delivery is at least once. Consumers use the domain `eventId` for idempotency
and apply the versioning rules defined by the producer contract.

### APMv2 runtime events

APMv2 publishes its active runtime events through the Backstage API entity
`biz-apm-published-events`. DoctorApp services consume that entity for active
calendar, notification, and consultation workflows. APMv2 owns the
[AsyncAPI definition](https://github.com/kanenggg/tdh-biz-doctor-apmv2/blob/main/specs/provides/biz-apm-published-events.asyncapi.yaml).

## Contract ownership

| Backstage API entity | Producer and owner | Consumer | Canonical specification |
|---|---|---|---|
| `doctor-profile-events` | DoctorApp | APMv2 `consultation-bg-rs` | [AsyncAPI](../../specs/provides/doctor-profile-approved.asyncapi.yaml) |
| `consultation-rs-api` | APMv2 `consultation-rs` | DoctorApp | [OpenAPI](https://github.com/kanenggg/tdh-biz-doctor-apmv2/blob/main/specs/provides/consultation-rs.yaml) |
| `biz-apm-published-events` | APMv2 services | DoctorApp and DoctorApp background service | [AsyncAPI](https://github.com/kanenggg/tdh-biz-doctor-apmv2/blob/main/specs/provides/biz-apm-published-events.asyncapi.yaml) |

The API entities and component relations are registered in each repository's
[`catalog-info.yaml`](../../catalog-info.yaml). Both catalog locations must be
registered in the same Backstage namespace for unqualified cross-repository
`providesApis` and `consumesApis` references to resolve.
