# Backstage OpenAPI Sync Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Automatically generate, version, and refresh the TDH Doctor OpenAPI specification in Backstage after relevant changes reach `main`.

**Architecture:** A small Rust binary serializes the existing `ApiDoc` without booting application infrastructure. GitHub Actions validates and commits the generated JSON, then parses repository catalog entities and asks Backstage to refresh them after the new file is present on `main`.

**Tech Stack:** Rust 2021, `utoipa`, `anyhow`, GitHub Actions, Python 3.12, PyYAML, Backstage Catalog API

## Global Constraints

- Preserve existing user changes in `.github/workflows/ci.yml`, `catalog-info.yaml`, `server/Cargo.toml`, and `server/src/bin/export_openapi.rs` while completing them.
- Keep Backstage credentials exclusively in `BACKSTAGE_URL` and `BACKSTAGE_TOKEN` GitHub Actions secrets.
- Run synchronization only for pushes to `main` or authorized manual dispatches.
- Commit only `server/openapi.json`; no deployment belongs in this workflow.
- A Backstage 404 is a warning; other non-2xx refresh responses fail after all entities are attempted.
- Generated commits must not trigger an infinite workflow loop.

---

### Task 1: Deterministic OpenAPI Exporter

**Files:**
- Modify: `server/Cargo.toml`
- Create: `server/src/bin/export_openapi.rs`
- Create: `server/openapi.json`

**Interfaces:**
- Consumes: `server::openapi::ApiDoc` implementing `utoipa::OpenApi`
- Produces: `cargo run --locked -p server --bin export_openapi -- <output-path>`

- [ ] **Step 1: Verify the exporter is absent from the committed baseline**

Run `git show HEAD^:server/Cargo.toml | rg 'export_openapi'`.
Expected: exit 1. The working tree may already contain the intended uncommitted implementation.

- [ ] **Step 2: Register and implement the exporter**

Add to `server/Cargo.toml`:

```toml
[[bin]]
name = "export_openapi"
path = "src/bin/export_openapi.rs"
```

Create `server/src/bin/export_openapi.rs`:

```rust
use std::{env, fs, path::PathBuf};

use anyhow::{Context, Result};
use server::openapi::ApiDoc;
use utoipa::OpenApi;

fn main() -> Result<()> {
    let output = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("server/openapi.json"));

    let document = ApiDoc::openapi()
        .to_pretty_json()
        .context("failed to serialize the OpenAPI document")?;

    fs::write(&output, document)
        .with_context(|| format!("failed to write OpenAPI document to {}", output.display()))?;

    println!("wrote OpenAPI document to {}", output.display());
    Ok(())
}
```

- [ ] **Step 3: Generate and validate locally**

```bash
cargo run --locked -p server --bin export_openapi -- server/openapi.json
python3 -c 'import json; s=json.load(open("server/openapi.json")); assert s.get("openapi"); assert s.get("info", {}).get("title"); assert s.get("paths"); print(len(s["paths"]))'
```

Expected: exporter exits 0 and Python prints a positive path count.

- [ ] **Step 4: Format and compile-check**

Run `cargo fmt --all -- --check` and `cargo check --locked -p server --bin export_openapi`.
Expected: both exit 0.

- [ ] **Step 5: Commit**

```bash
git add server/Cargo.toml server/src/bin/export_openapi.rs server/openapi.json
git commit -m "feat(openapi): add deterministic specification exporter"
```

### Task 2: Backstage Catalog Definition

**Files:**
- Modify: `catalog-info.yaml`
- Test: `.github/scripts/parse_catalog.py`

**Interfaces:**
- Consumes: generated `server/openapi.json`
- Produces: Backstage API definition `{"$text": "./server/openapi.json"}`

- [ ] **Step 1: Demonstrate the previous placeholder**

Run `git show HEAD^:catalog-info.yaml | rg -n 'paths: \{\}'`.
Expected: the inline empty-path placeholder is printed.

- [ ] **Step 2: Point the entity to the generated file**

```yaml
  definition:
    $text: ./server/openapi.json
```

- [ ] **Step 3: Assert the catalog reference resolves**

```bash
python3 .github/scripts/parse_catalog.py catalog-info.yaml > /tmp/tdh-doctor-catalog.json
python3 -c 'import json; d=json.load(open("/tmp/tdh-doctor-catalog.json")); assert not d["missing_files"]; api=[e for e in d["entities"] if e["entityRef"].startswith("api:")]; assert api; assert api[0]["definition"]["mode"] == "$text"; print(api[0]["definition"]["resolved_path"])'
```

Expected: output ends in `/server/openapi.json`.

- [ ] **Step 4: Commit**

```bash
git add catalog-info.yaml
git commit -m "docs(backstage): source API definition from generated spec"
```

### Task 3: Automatic Generation and Backstage Refresh Workflow

**Files:**
- Modify: `.github/workflows/ci.yml`

**Interfaces:**
- Consumes: exporter, parser, `BACKSTAGE_URL`, and `BACKSTAGE_TOKEN`
- Produces: a bot commit for changed spec followed by Backstage refresh requests

- [ ] **Step 1: Complete triggers and permissions**

Use these paths in addition to `main` and `workflow_dispatch`:

```yaml
paths:
  - '**/catalog-info.yaml'
  - 'server/Cargo.toml'
  - 'server/src/**/*.rs'
  - 'crates/tdh-protocol/**'
  - '.github/scripts/parse_catalog.py'
  - '.github/workflows/ci.yml'
```

Set `permissions.contents: write`. Do not add `server/openapi.json`, so the generated commit cannot retrigger the workflow.

- [ ] **Step 2: Complete generation, validation, and publication**

Checkout recursive submodules, install stable Rust, restore `Swatinem/rust-cache`, run the Task 1 CLI, then validate `openapi`, `info.title`, and non-empty `paths`. Commit only `server/openapi.json` when `git status --porcelain -- server/openapi.json` is non-empty, using `docs(openapi): update generated specification [skip ci]`.

- [ ] **Step 3: Keep catalog checks and refresh semantics**

Parse all catalog files with `.github/scripts/parse_catalog.py`. Fail before refresh if referenced files are missing. Treat 2xx as success, 404 as an unregistered warning, and other responses as failures after all entities have been attempted.

- [ ] **Step 4: Make the always-run summary safe**

Before parsing `/tmp/catalog_result.json`, add:

```bash
if [ ! -f /tmp/catalog_result.json ]; then
  echo "### 📋 Backstage Catalog Sync" >> "$GITHUB_STEP_SUMMARY"
  echo >> "$GITHUB_STEP_SUMMARY"
  echo "Catalog parsing did not complete." >> "$GITHUB_STEP_SUMMARY"
  exit 0
fi
```

- [ ] **Step 5: Validate workflow structure and invariants**

```bash
python3 - <<'PY'
from pathlib import Path
import yaml
workflow = yaml.safe_load(Path('.github/workflows/ci.yml').read_text())
trigger = workflow.get('on', workflow.get(True))
assert trigger['push']['branches'] == ['main']
assert 'server/openapi.json' not in trigger['push']['paths']
assert workflow['permissions']['contents'] == 'write'
assert workflow['jobs']['sync-catalog']['steps']
text = Path('.github/workflows/ci.yml').read_text()
for marker in ['submodules: recursive', 'git add server/openapi.json',
               'secrets.BACKSTAGE_URL', 'secrets.BACKSTAGE_TOKEN']:
    assert marker in text
print('workflow YAML and invariants are valid')
PY
```

Expected: exits 0 and prints `workflow YAML and invariants are valid`.

- [ ] **Step 6: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: synchronize generated OpenAPI with Backstage"
```

### Task 4: End-to-End Local Verification

**Files:**
- Verify: `.github/workflows/ci.yml`
- Verify: `.github/scripts/parse_catalog.py`
- Verify: `catalog-info.yaml`
- Verify: `server/Cargo.toml`
- Verify: `server/src/bin/export_openapi.rs`
- Verify: `server/openapi.json`

**Interfaces:**
- Consumes: all previous outputs
- Produces: local evidence without contacting production Backstage

- [ ] **Step 1: Prove generation is deterministic**

Run the exporter, then `git diff --exit-code -- server/openapi.json`.
Expected: both exit 0 and the generated file has no diff.

- [ ] **Step 2: Run catalog discovery as CI does**

```bash
FILES=$(find . -name 'catalog-info.yaml' -not -path '*/node_modules/*' | sort | tr '\n' ' ')
python3 .github/scripts/parse_catalog.py $FILES > /tmp/tdh-doctor-catalog-result.json
python3 -c 'import json; d=json.load(open("/tmp/tdh-doctor-catalog-result.json")); assert d["entities"]; assert not d["missing_files"]; print(len(d["entities"]), "entities, 0 missing files")'
```

Expected: a positive entity count and `0 missing files`.

- [ ] **Step 3: Run quality checks**

```bash
cargo fmt --all -- --check
cargo check --locked -p server --bin export_openapi
git diff --check
```

Expected: all exit 0.

- [ ] **Step 4: Review scope**

Run `git status --short` and `git log -4 --oneline`.
Expected: no unintended files changed. Do not call real Backstage locally because it requires production secrets and causes external state changes.

