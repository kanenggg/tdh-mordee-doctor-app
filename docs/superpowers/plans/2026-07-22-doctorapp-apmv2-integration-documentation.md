# DoctorApp–APMv2 Integration Documentation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add one canonical DoctorApp–APMv2 integration guide and a concise APMv2 consumer reference that are discoverable from both repositories.

**Architecture:** DoctorApp owns the full integration guide beside its producer contract documentation. APMv2 owns only a consumer-facing summary and links across repositories by stable GitHub `main` URLs, while repository-local navigation uses relative Markdown links.

**Tech Stack:** Markdown, Backstage entity references, Ruby link-validation scripts, Git

## Global Constraints

- Keep one authoritative integration document in DoctorApp; do not duplicate its full content in APMv2.
- Treat document bodies embedded in this implementation plan as non-authoritative creation-time snapshots; maintain only the delivered files after execution.
- Document only active runtime contracts: `doctor-profile-events`, `consultation-rs-api`, and `biz-apm-published-events`.
- Do not present experimental Consultation Event V2 artifacts as active dependencies.
- Do not change catalogs, AsyncAPI, OpenAPI, runtime, deployment, or rollout behavior.
- Use repository-relative links for local files and GitHub `main` links for cross-repository files.
- Preserve unrelated user changes in both worktrees.

---

### Task 1: Add the canonical DoctorApp integration guide

**Files:**
- Create: `tdh-mordee-doctor-app/docs/contracts/doctorapp-apmv2-integration.md`
- Modify: `tdh-mordee-doctor-app/docs/CODEMAPS/integrations.md`

**Interfaces:**
- Consumes: DoctorApp `doctor-profile-events`, APMv2 `consultation-rs-api`, APMv2 `biz-apm-published-events`, and existing rollout documentation.
- Produces: Canonical cross-system documentation linked from the DoctorApp integrations codemap.

- [ ] **Step 1: Run the pre-change discovery assertion and verify it fails**

Run from `/Users/thanawat/Documents/GitHub`:

```bash
test -f tdh-mordee-doctor-app/docs/contracts/doctorapp-apmv2-integration.md && rg -q 'doctorapp-apmv2-integration\.md' tdh-mordee-doctor-app/docs/CODEMAPS/integrations.md
```

Expected: non-zero exit because the canonical guide does not exist.

- [ ] **Step 2: Create the canonical guide**

Create `tdh-mordee-doctor-app/docs/contracts/doctorapp-apmv2-integration.md`
with this content:

> **Creation-time snapshot:** The code block below records the initial file
> creation instructions. After execution it is non-authoritative and must not
> be maintained; the delivered file is the only canonical integration guide.

```markdown
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
```

- [ ] **Step 3: Add the DoctorApp discovery link**

Insert this section after the opening Architecture diagram in
`tdh-mordee-doctor-app/docs/CODEMAPS/integrations.md`:

```markdown
## Cross-system integration guides

- [DoctorApp–APMv2 integration](../contracts/doctorapp-apmv2-integration.md) — canonical ownership, API, and event-contract overview.
```

- [ ] **Step 4: Validate DoctorApp links and terminology**

```bash
ruby -e 'files=%w[tdh-mordee-doctor-app/docs/contracts/doctorapp-apmv2-integration.md tdh-mordee-doctor-app/docs/CODEMAPS/integrations.md]; files.each { |file| text=File.read(file); text.scan(/\[[^\]]+\]\(([^)]+)\)/).flatten.reject { |link| link.start_with?("http") || link.start_with?("#") }.each { |link| path=File.expand_path(link.split("#",2).first, File.dirname(file)); abort "broken link: #{file} -> #{link}" unless File.file?(path) } }; puts "DoctorApp local documentation links valid"'
rg -q 'doctor-profile-events' tdh-mordee-doctor-app/docs/contracts/doctorapp-apmv2-integration.md
rg -q 'consultation-rs-api' tdh-mordee-doctor-app/docs/contracts/doctorapp-apmv2-integration.md
rg -q 'biz-apm-published-events' tdh-mordee-doctor-app/docs/contracts/doctorapp-apmv2-integration.md
! rg -n 'biz-apm-consultation-event-v2|specs/depends-on/doctor-profile-approved' tdh-mordee-doctor-app/docs/contracts/doctorapp-apmv2-integration.md
git -C tdh-mordee-doctor-app diff --check
```

Expected: `DoctorApp local documentation links valid`; all commands exit 0.

- [ ] **Step 5: Commit the canonical guide**

```bash
git -C tdh-mordee-doctor-app add docs/contracts/doctorapp-apmv2-integration.md docs/CODEMAPS/integrations.md
git -C tdh-mordee-doctor-app commit -m "docs: add DoctorApp APM integration guide"
```

Expected: successful commit.

### Task 2: Add the APMv2 consumer reference and final validation

**Files:**
- Create: `tdh-biz-doctor-apmv2/docs/DOCTORAPP_INTEGRATION.md`
- Modify: `tdh-biz-doctor-apmv2/README.md`

**Interfaces:**
- Consumes: Canonical DoctorApp integration guide and DoctorApp-owned AsyncAPI contract from Task 1.
- Produces: APMv2-local consumer documentation discoverable from its root README.

- [ ] **Step 1: Run the pre-change APMv2 discovery assertion and verify it fails**

```bash
test -f tdh-biz-doctor-apmv2/docs/DOCTORAPP_INTEGRATION.md && rg -q 'docs/DOCTORAPP_INTEGRATION\.md' tdh-biz-doctor-apmv2/README.md
```

Expected: non-zero exit because the consumer reference does not exist.

- [ ] **Step 2: Create the APMv2 consumer reference**

Create `tdh-biz-doctor-apmv2/docs/DOCTORAPP_INTEGRATION.md` with this
content:

> **Creation-time snapshot:** The code block below records the initial file
> creation instructions. After execution it is non-authoritative and must not
> be maintained; maintain only the delivered APMv2 consumer reference.

```markdown
# DoctorApp integration

APMv2 integrates with `tdh-mordee-doctor-app` through active HTTP and Pub/Sub
contracts. The
[canonical cross-system guide](https://github.com/kanenggg/tdh-mordee-doctor-app/blob/main/docs/contracts/doctorapp-apmv2-integration.md)
is maintained by DoctorApp.

## APMv2 responsibilities

- `consultation-rs` provides `consultation-rs-api`.
- APMv2 services provide `biz-apm-published-events`.
- `consultation-bg-rs` consumes DoctorApp's `doctor-profile-events` and stores
  a read-only doctor identity and consultation-configuration projection.
- APMv2 owns Doctor Operational Availability, Appointment Holds, Doctor
  Occupancy, booking, and consultation execution.

DoctorApp remains the only editable authority for Doctor Profile and Doctor
Consultation Configuration. APMv2 must not treat its projection as an
independent configuration source.

## Contract and rollout references

- [DoctorApp-owned `doctor-profile-events` AsyncAPI](https://github.com/kanenggg/tdh-mordee-doctor-app/blob/main/specs/provides/doctor-profile-approved.asyncapi.yaml)
- [APMv2 Consultation OpenAPI](../specs/provides/consultation-rs.yaml)
- [APMv2 runtime events AsyncAPI](../specs/provides/biz-apm-published-events.asyncapi.yaml)
- [Doctor projection rollout](plans/DOCTOR_PROJECTION_SYNC_ROLLOUT.md)
- [APMv2 Backstage catalog](../catalog-info.yaml)
```

- [ ] **Step 3: Add README discovery**

Append this section to `tdh-biz-doctor-apmv2/README.md`:

```markdown
## Architecture documentation

- [DoctorApp integration](docs/DOCTORAPP_INTEGRATION.md) — cross-repository ownership and active API/event contracts.
```

- [ ] **Step 4: Run final documentation and catalog validation**

```bash
ruby -e 'files=%w[tdh-mordee-doctor-app/docs/contracts/doctorapp-apmv2-integration.md tdh-mordee-doctor-app/docs/CODEMAPS/integrations.md tdh-biz-doctor-apmv2/docs/DOCTORAPP_INTEGRATION.md tdh-biz-doctor-apmv2/README.md]; files.each { |file| text=File.read(file); text.scan(/\[[^\]]+\]\(([^)]+)\)/).flatten.reject { |link| link.start_with?("http") || link.start_with?("#") }.each { |link| path=File.expand_path(link.split("#",2).first, File.dirname(file)); abort "broken link: #{file} -> #{link}" unless File.file?(path) } }; puts "all local documentation links valid"'
ruby -e 'require "yaml"; docs=%w[tdh-mordee-doctor-app/catalog-info.yaml tdh-biz-doctor-apmv2/catalog-info.yaml].flat_map { |file| YAML.load_stream(File.read(file)).compact }; names=docs.select { |doc| doc["kind"] == "API" }.map { |doc| doc.dig("metadata", "name") }; required=%w[doctor-profile-events consultation-rs-api biz-apm-published-events]; abort "documented API missing from catalogs: #{required - names}" unless (required - names).empty?; puts "documented API names match catalogs"'
! rg -n 'biz-apm-consultation-event-v2|specs/depends-on/doctor-profile-approved' tdh-mordee-doctor-app/docs/contracts/doctorapp-apmv2-integration.md tdh-biz-doctor-apmv2/docs/DOCTORAPP_INTEGRATION.md
git -C tdh-mordee-doctor-app diff --check
git -C tdh-biz-doctor-apmv2 diff --check
```

Expected: `all local documentation links valid` and `documented API names match catalogs`; all commands exit 0.

- [ ] **Step 5: Commit the APMv2 reference**

```bash
git -C tdh-biz-doctor-apmv2 add docs/DOCTORAPP_INTEGRATION.md README.md
git -C tdh-biz-doctor-apmv2 commit -m "docs: add DoctorApp integration reference"
```

Expected: successful commit and clean worktrees in both repositories.
