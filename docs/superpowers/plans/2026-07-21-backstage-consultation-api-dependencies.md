# Backstage Consultation API Dependencies Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Register the Doctor App and Doctor Background Service relationships to the externally owned consultation HTTP API and Consultation Event V2 API in Backstage.

**Architecture:** Update only the two existing `Component` entities in `catalog-info.yaml` with cross-repository `consumesApis` references. Keep external API definitions in their owning catalog, and document that the V2 relation represents an experimental rollout contract while runtime remains V1-compatible.

**Tech Stack:** Backstage Catalog `backstage.io/v1alpha1`, multi-document YAML, Ruby/Psych validation, Git whitespace validation

## Global Constraints

- Modify only `catalog-info.yaml`; do not copy externally owned OpenAPI or AsyncAPI definitions into this repository.
- Preserve the existing local correction of the Doctor API title to `TDH MorDee Doctor API`.
- Use unqualified same-namespace API references: `consultation-rs-api` and `biz-apm-consultation-event-v2`.
- Do not change Pub/Sub topics, subscriptions, Rust payload models, rollout flags, or external API ownership.
- State explicitly that the V2 catalog relation is experimental and does not imply completion of the runtime V2 migration.

---

### Task 1: Add consultation API relations to the Doctor components

**Files:**
- Modify: `catalog-info.yaml:1-83`
- Test: inline Ruby/Psych assertions against `catalog-info.yaml`

**Interfaces:**
- Consumes: Backstage API entities `api:default/consultation-rs-api` and `api:default/biz-apm-consultation-event-v2`, registered by the external source catalog.
- Produces: `tdh-mordee-doctor-app.spec.consumesApis` and `tdh-mordee-doctor-bg.spec.consumesApis` catalog relations.

- [ ] **Step 1: Run the relation assertion and verify the current catalog fails it**

```bash
ruby -ryaml -e 'docs=YAML.load_stream(File.read("catalog-info.yaml")); entities=docs.compact.to_h { |e| [e.dig("metadata", "name"), e] }; abort "doctor app consumesApis mismatch" unless entities.fetch("tdh-mordee-doctor-app").dig("spec", "consumesApis") == ["consultation-rs-api", "biz-apm-consultation-event-v2"]; abort "doctor bg consumesApis mismatch" unless entities.fetch("tdh-mordee-doctor-bg").dig("spec", "consumesApis") == ["biz-apm-consultation-event-v2"]'
```

Expected: non-zero exit with `doctor app consumesApis mismatch` because the relations are absent.

- [ ] **Step 2: Add the API relations and rollout metadata**

Set the main component description and API fields to:

```yaml
description: >-
  Doctor-facing telehealth API gateway for appointments, consultations,
  notifications, onboarding, doctor profiles, EHR, ranking, and timeslots.
  The Consultation Event V2 API relation represents an experimental rollout
  contract; runtime event handling remains V1-compatible until rollout is
  explicitly enabled.
```

```yaml
providesApis:
  - tdh-mordee-doctor-api
consumesApis:
  - consultation-rs-api
  - biz-apm-consultation-event-v2
```

Set the background component description and relation to:

```yaml
description: >-
  Background service for doctor calendar events and scheduled doctor
  notifications. The Consultation Event V2 API relation represents the
  experimental rollout contract while runtime consumers retain V1
  compatibility.
```

```yaml
system: doctor
consumesApis:
  - biz-apm-consultation-event-v2
dependsOn:
```

Preserve the existing API title correction exactly:

```yaml
title: TDH MorDee Doctor API
```

- [ ] **Step 3: Run structural and relation validation**

```bash
ruby -ryaml -e 'docs=YAML.load_stream(File.read("catalog-info.yaml")); entities=docs.compact.to_h { |e| [e.dig("metadata", "name"), e] }; abort "doctor app consumesApis mismatch" unless entities.fetch("tdh-mordee-doctor-app").dig("spec", "consumesApis") == ["consultation-rs-api", "biz-apm-consultation-event-v2"]; abort "doctor bg consumesApis mismatch" unless entities.fetch("tdh-mordee-doctor-bg").dig("spec", "consumesApis") == ["biz-apm-consultation-event-v2"]; abort "doctor API title changed" unless entities.fetch("tdh-mordee-doctor-api").dig("metadata", "title") == "TDH MorDee Doctor API"; puts "catalog relations valid"'
```

Expected: exit 0 with `catalog relations valid`.

- [ ] **Step 4: Check formatting and review the scoped diff**

```bash
git diff --check -- catalog-info.yaml
git diff -- catalog-info.yaml
```

Expected: `git diff --check` exits 0. The diff contains only the preserved title correction, two component description additions, and two `consumesApis` additions.

- [ ] **Step 5: Commit the catalog update**

```bash
git add catalog-info.yaml
git commit -m "docs: link consultation APIs in Backstage catalog"
```
