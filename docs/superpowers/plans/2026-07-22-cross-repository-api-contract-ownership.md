# Cross-Repository API Contract Ownership Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make DoctorApp the sole Backstage owner of `doctor-profile-events` while DoctorApp and APMv2 reference each other's owned contracts as consumers.

**Architecture:** Contract definitions live with their runtime producer. DoctorApp will provide the `DoctorProfileApproved` AsyncAPI entity, APMv2 will consume it, and DoctorApp will consume the APMv2-owned Consultation HTTP and runtime event APIs by Backstage entity reference.

**Tech Stack:** Backstage catalog YAML, AsyncAPI 3.1, Ruby YAML parser, Git

## Global Constraints

- Do not change Rust runtime behavior, Pub/Sub topics, event payloads, deployment configuration, or rollout flags.
- Register exactly one `api:default/doctor-profile-events` entity across both repositories.
- Keep `consultation-rs-api` and `biz-apm-published-events` owned and defined by APMv2.
- Use unqualified API entity names within the Backstage `default` namespace.
- Do not retain a consumer snapshot of `doctor-profile-approved.asyncapi.yaml` in APMv2.
- Preserve unrelated user changes in both worktrees.

---

### Task 1: Transfer the Doctor Profile event contract to DoctorApp

**Files:**
- Create: `tdh-mordee-doctor-app/specs/provides/doctor-profile-approved.asyncapi.yaml`
- Modify: `tdh-mordee-doctor-app/catalog-info.yaml`
- Delete: `tdh-biz-doctor-apmv2/specs/depends-on/doctor-profile-approved.asyncapi.yaml`

**Interfaces:**
- Consumes: Existing APMv2 dependency contract and DoctorApp outbox topic `doctor-profile.approved`.
- Produces: Backstage API entity `api:default/doctor-profile-events` with an AsyncAPI 3.1 definition.

- [ ] **Step 1: Run the pre-change ownership assertion and verify it fails the desired state**

Run from `/Users/thanawat/Documents/GitHub`:

```bash
ruby -e 'require "yaml"; files=%w[tdh-biz-doctor-apmv2/catalog-info.yaml tdh-mordee-doctor-app/catalog-info.yaml]; docs=files.flat_map { |f| YAML.load_stream(File.read(f)).compact }; owners=docs.select { |d| d["kind"] == "API" && d.dig("metadata", "name") == "doctor-profile-events" }; abort "expected one DoctorApp owner" unless owners.length == 1 && owners[0].dig("spec", "definition", "$text") == "./specs/provides/doctor-profile-approved.asyncapi.yaml"'
```

Expected: non-zero exit with `expected one DoctorApp owner`, because the only current entity is defined by APMv2.

- [ ] **Step 2: Move the contract without changing its payload schema**

Use `apply_patch` to add
`tdh-mordee-doctor-app/specs/provides/doctor-profile-approved.asyncapi.yaml`
with the exact content currently in
`tdh-biz-doctor-apmv2/specs/depends-on/doctor-profile-approved.asyncapi.yaml`,
then delete the APMv2 source file in the same patch. Do not alter AsyncAPI
version, channel address, required fields, schemas, or examples during the
move.

- [ ] **Step 3: Register the API in DoctorApp and remove the duplicate registrar from APMv2**

Add to both DoctorApp service components:

```yaml
  providesApis:
    - doctor-profile-events
```

For `tdh-mordee-doctor-app`, append `doctor-profile-events` to the existing
`providesApis`. For `tdh-mordee-doctor-bg`, add a new `providesApis` block.
Then add this DoctorApp catalog document before the Resource documents:

```yaml
---
apiVersion: backstage.io/v1alpha1
kind: API
metadata:
  name: doctor-profile-events
  title: Doctor Profile Events
  description: Doctor profile events published for downstream projections.
  tags:
    - asyncapi
    - gcp-pubsub
    - doctor-profile
spec:
  type: asyncapi
  lifecycle: production
  owner: user:default/p-bank
  system: doctor
  definition:
    $text: ./specs/provides/doctor-profile-approved.asyncapi.yaml
```

Remove only the complete `kind: API` document named `doctor-profile-events`
from `tdh-biz-doctor-apmv2/catalog-info.yaml`. Keep
`consultation-bg-rs.spec.consumesApis: [doctor-profile-events]` unchanged.

- [ ] **Step 4: Run the ownership assertion and schema parse**

```bash
ruby -e 'require "yaml"; files=%w[tdh-biz-doctor-apmv2/catalog-info.yaml tdh-mordee-doctor-app/catalog-info.yaml]; docs=files.flat_map { |f| YAML.load_stream(File.read(f)).compact }; owners=docs.select { |d| d["kind"] == "API" && d.dig("metadata", "name") == "doctor-profile-events" }; abort "expected one DoctorApp owner" unless owners.length == 1 && owners[0].dig("spec", "definition", "$text") == "./specs/provides/doctor-profile-approved.asyncapi.yaml"; YAML.load_file("tdh-mordee-doctor-app/specs/provides/doctor-profile-approved.asyncapi.yaml"); puts "ownership and AsyncAPI YAML valid"'
```

Expected: exit 0 and `ownership and AsyncAPI YAML valid`.

- [ ] **Step 5: Commit the independently valid ownership transfer in each repository**

```bash
git -C tdh-mordee-doctor-app add catalog-info.yaml specs/provides/doctor-profile-approved.asyncapi.yaml
git -C tdh-mordee-doctor-app commit -m "docs: own doctor profile event contract"
git -C tdh-biz-doctor-apmv2 add catalog-info.yaml specs/depends-on/doctor-profile-approved.asyncapi.yaml
git -C tdh-biz-doctor-apmv2 commit -m "docs: reference DoctorApp event contract"
```

Expected: one successful commit per repository.

### Task 2: Add cross-repository consumer relations

**Files:**
- Modify: `tdh-mordee-doctor-app/catalog-info.yaml`

**Interfaces:**
- Consumes: APMv2-owned `api:default/consultation-rs-api` and `api:default/biz-apm-published-events`.
- Produces: Backstage `consumesApis` relations on both DoctorApp components.

- [ ] **Step 1: Run the pre-change relation assertion and verify it fails**

```bash
ruby -e 'require "yaml"; docs=YAML.load_stream(File.read("tdh-mordee-doctor-app/catalog-info.yaml")).compact; app=docs.find { |d| d["kind"] == "Component" && d.dig("metadata", "name") == "tdh-mordee-doctor-app" }; bg=docs.find { |d| d["kind"] == "Component" && d.dig("metadata", "name") == "tdh-mordee-doctor-bg" }; abort "missing API relations" unless app.dig("spec", "consumesApis") == %w[consultation-rs-api biz-apm-published-events] && bg.dig("spec", "consumesApis") == ["biz-apm-published-events"]'
```

Expected: non-zero exit with `missing API relations`.

- [ ] **Step 2: Add the minimal consumer relations**

Add to `tdh-mordee-doctor-app`:

```yaml
  consumesApis:
    - consultation-rs-api
    - biz-apm-published-events
```

Add to `tdh-mordee-doctor-bg`:

```yaml
  consumesApis:
    - biz-apm-published-events
```

Do not register local API documents for either external name.

- [ ] **Step 3: Run the relation assertion and verify it passes**

Run the Ruby command from Step 1 again.

Expected: exit 0 with no output.

- [ ] **Step 4: Commit the consumer relations**

```bash
git -C tdh-mordee-doctor-app add catalog-info.yaml
git -C tdh-mordee-doctor-app commit -m "docs: link APM consultation contracts"
```

Expected: successful commit.

### Task 3: Update active ownership documentation and verify both catalogs

**Files:**
- Modify: `tdh-biz-doctor-apmv2/docs/plans/DOCTOR_PROJECTION_SYNC_ROLLOUT.md`

**Interfaces:**
- Consumes: Canonical DoctorApp contract path and Backstage API name from Task 1.
- Produces: Active rollout documentation with no reference to the deleted APMv2 snapshot.

- [ ] **Step 1: Verify the active rollout document still contains the obsolete path**

```bash
rg -n 'specs/depends-on/doctor-profile-approved\.asyncapi\.yaml' tdh-biz-doctor-apmv2/docs/plans/DOCTOR_PROJECTION_SYNC_ROLLOUT.md
```

Expected: one matching line describing the consumer baseline.

- [ ] **Step 2: Replace the obsolete reference**

Change the final sentence on the consumer-baseline bullet to:

```markdown
The canonical contract is the DoctorApp-owned Backstage API `doctor-profile-events`, defined at `tdh-mordee-doctor-app/specs/provides/doctor-profile-approved.asyncapi.yaml`.
```

Keep the existing required-field description unchanged.

- [ ] **Step 3: Run final structural verification**

```bash
ruby -e 'require "yaml"; roots={"tdh-biz-doctor-apmv2"=>"catalog-info.yaml", "tdh-mordee-doctor-app"=>"catalog-info.yaml"}; docs=roots.flat_map { |root,file| YAML.load_stream(File.read(File.join(root,file))).compact.map { |d| [root,d] } }; apis=docs.select { |_,d| d["kind"] == "API" }.map { |root,d| [d.dig("metadata","name"),root,d.dig("spec","definition","$text")] }; required=%w[consultation-rs-api biz-apm-published-events tdh-mordee-doctor-api doctor-profile-events]; actual=apis.map(&:first); abort "missing APIs: #{required - actual}" unless (required - actual).empty?; abort "duplicate API names" unless actual.uniq.length == actual.length; apis.each { |name,root,ref| next unless ref&.start_with?("./"); path=File.expand_path(ref, root); abort "missing definition for #{name}: #{path}" unless File.file?(path); YAML.load_file(path) }; puts "catalog APIs and local definitions valid"'
git -C tdh-mordee-doctor-app diff --check
git -C tdh-biz-doctor-apmv2 diff --check
```

Expected: `catalog APIs and local definitions valid`, followed by both Git checks exiting 0.

- [ ] **Step 4: Verify relations and removed-path cleanup**

```bash
ruby -e 'require "yaml"; doctor=YAML.load_stream(File.read("tdh-mordee-doctor-app/catalog-info.yaml")).compact; apm=YAML.load_stream(File.read("tdh-biz-doctor-apmv2/catalog-info.yaml")).compact; app=doctor.find { |d| d.dig("metadata","name") == "tdh-mordee-doctor-app" }; bg=doctor.find { |d| d.dig("metadata","name") == "tdh-mordee-doctor-bg" }; apmbg=apm.find { |d| d.dig("metadata","name") == "consultation-bg-rs" }; abort "DoctorApp relations wrong" unless app.dig("spec","providesApis").include?("doctor-profile-events") && app.dig("spec","consumesApis") == %w[consultation-rs-api biz-apm-published-events]; abort "DoctorApp BG relations wrong" unless bg.dig("spec","providesApis") == ["doctor-profile-events"] && bg.dig("spec","consumesApis") == ["biz-apm-published-events"]; abort "APM relation wrong" unless apmbg.dig("spec","consumesApis") == ["doctor-profile-events"]; puts "cross-repository relations valid"'
test ! -e tdh-biz-doctor-apmv2/specs/depends-on/doctor-profile-approved.asyncapi.yaml
! rg -n 'specs/depends-on/doctor-profile-approved\.asyncapi\.yaml' tdh-biz-doctor-apmv2/docs/plans/DOCTOR_PROJECTION_SYNC_ROLLOUT.md
```

Expected: `cross-repository relations valid`; all commands exit 0.

- [ ] **Step 5: Commit the active documentation update**

```bash
git -C tdh-biz-doctor-apmv2 add docs/plans/DOCTOR_PROJECTION_SYNC_ROLLOUT.md
git -C tdh-biz-doctor-apmv2 commit -m "docs: point projection rollout to DoctorApp contract"
```

Expected: successful commit and clean worktrees in both repositories.
