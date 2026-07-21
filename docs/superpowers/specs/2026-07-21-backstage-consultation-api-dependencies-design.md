# Backstage Consultation API Dependencies Design

**Date:** 2026-07-21

## Goal

Make the TDH MorDee Doctor Backstage catalog show its integration with the
consultation service and the Biz APM Consultation Event V2 contract without
duplicating API entities owned by another repository.

## Current State

The repository registers two service components:

- `tdh-mordee-doctor-app`, the doctor-facing HTTP service.
- `tdh-mordee-doctor-bg`, the background event-processing service.

The main service calls the consultation service through `consultation_base_uri`
and publishes a consultation summary event. The background service consumes
consultation events for calendar and notification workflows. Neither
relationship is currently represented through `consumesApis` in
`catalog-info.yaml`.

The externally registered API entities are:

- `consultation-rs-api`, an OpenAPI contract provided by
  `tdh-biz-doctor-apmv2`.
- `biz-apm-consultation-event-v2`, an experimental AsyncAPI contract consumed
  by `tdh-biz-doctor-apmv2` and associated with the consultation event stream.

Runtime code in this repository still retains V1-compatible event models and
the `consultations` topic configuration. Catalog references to V2 therefore
describe the intended integration and rollout dependency, not proof that the
runtime migration is complete.

## Design

Update only `catalog-info.yaml`; do not copy the externally owned OpenAPI or
AsyncAPI definitions into this repository.

Add the following API relations:

- `tdh-mordee-doctor-app` consumes `consultation-rs-api` because it calls the
  consultation HTTP service.
- `tdh-mordee-doctor-app` consumes `biz-apm-consultation-event-v2` because it
  participates in the consultation event flow as a publisher.
- `tdh-mordee-doctor-bg` consumes `biz-apm-consultation-event-v2` because it
  processes consultation events for doctor calendar and notification flows.

Use unqualified API names in `consumesApis`, consistent with Backstage's
same-namespace entity-reference syntax. Backstage resolves them as
`api:default/consultation-rs-api` and
`api:default/biz-apm-consultation-event-v2`.

Add concise component metadata text explaining that the V2 relation is an
experimental rollout contract while runtime V1 compatibility remains. This
prevents the catalog relation from being mistaken for a completed runtime
migration.

## Ownership and Source of Truth

The API entities remain owned and defined by their source catalog:

- `consultation-rs-api`: `user:default/p-bank`
- `biz-apm-consultation-event-v2`: `user:default/n-kan`

This repository contains references only. It must not register duplicate API
entities or duplicate `$text` definitions, avoiding conflicting ownership and
specification drift.

## Validation

After editing the catalog:

1. Parse every YAML document in `catalog-info.yaml`.
2. Confirm the two component entities contain the intended `consumesApis`
   values and no duplicate entries.
3. Run `git diff --check`.
4. Review the final diff to ensure the existing local title correction is
   preserved and no unrelated catalog entities changed.

Validation can verify local structure and references, but resolution of the
cross-repository API names ultimately requires those API entities to be
registered in the same Backstage catalog and namespace.

## Out of Scope

- Implementing or enabling the Consultation Event V2 Rust payload model.
- Changing Pub/Sub topics, subscriptions, headers, or rollout flags.
- Copying external OpenAPI or AsyncAPI files into this repository.
- Changing ownership of externally registered API entities.
