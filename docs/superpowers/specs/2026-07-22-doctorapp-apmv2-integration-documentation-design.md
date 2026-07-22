# DoctorApp–APMv2 Integration Documentation Design

**Date:** 2026-07-22

## Goal

Make the relationship between DoctorApp and APMv2 discoverable from either
repository while maintaining only one authoritative integration document.

## Documentation Ownership

DoctorApp will own the canonical integration document because it owns the
`DoctorProfileApproved` source event and its `doctor-profile-events` AsyncAPI
contract. The document will live at:

```text
tdh-mordee-doctor-app/docs/contracts/doctorapp-apmv2-integration.md
```

APMv2 will contain a shorter consumer-facing reference at:

```text
tdh-biz-doctor-apmv2/docs/DOCTORAPP_INTEGRATION.md
```

The APMv2 document will explain its local responsibilities and point readers
to the canonical DoctorApp document and contract. It will not duplicate the
full payload schema, rollout instructions, or ownership matrix.

## Canonical Document Content

The DoctorApp document will cover:

- The system boundary: DoctorApp owns doctor profile and consultation
  configuration; APMv2 owns operational availability, appointment holds,
  occupancy, booking, and consultation execution.
- The synchronous HTTP path from DoctorApp to the APMv2
  `consultation-rs-api`.
- The asynchronous `doctor-profile-events` path from DoctorApp to
  `consultation-bg-rs`.
- The APMv2 runtime event path consumed by DoctorApp services through
  `biz-apm-published-events`.
- A contract ownership table containing the Backstage API entity, producer,
  consumer, and canonical specification path.
- Links to the existing doctor-profile outbox rollout and APM doctor
  projection rollout documents instead of copying their operational steps.
- The requirement that both catalog locations be registered in the same
  Backstage namespace for cross-repository `consumesApis` references to
  resolve.

The document will describe only active runtime contracts. Experimental or
historical Consultation Event V2 artifacts will not be presented as active
dependencies.

## Consumer Reference Content

The APMv2 reference will summarize:

- APMv2 consumes DoctorApp's `doctor-profile-events` projection source.
- APMv2 provides `consultation-rs-api` and `biz-apm-published-events`.
- The local projected copy is read-only and DoctorApp remains the authority
  for Doctor Consultation Configuration.
- The canonical DoctorApp integration document, AsyncAPI contract, and local
  rollout documentation locations.

Repository-relative links will be used for local documents. Cross-repository
links will use the GitHub `main` branch so they work in rendered repository
documentation and do not depend on sibling checkout locations.

## Discovery Links

Add a link to the canonical integration document from DoctorApp
`docs/CODEMAPS/integrations.md`. Add a link to the consumer reference from the
APMv2 root `README.md`. Existing sections and commands in both files remain
unchanged.

## Validation

Validation will:

1. Assert that all four documentation files contain the expected links.
2. Assert that the documented API entity names match both Backstage catalogs:
   `doctor-profile-events`, `consultation-rs-api`, and
   `biz-apm-published-events`.
3. Assert that every repository-relative Markdown link added by this change
   resolves to an existing file.
4. Scan the new documents for obsolete
   `specs/depends-on/doctor-profile-approved.asyncapi.yaml` references and
   experimental Consultation Event V2 claims.
5. Run `git diff --check` in both repositories.

No catalog, AsyncAPI, OpenAPI, runtime, deployment, or rollout behavior will
change.
