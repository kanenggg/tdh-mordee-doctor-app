# Unified Catalog CI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the catalog/OpenAPI workflow and executable CI logic byte-identical in DoctorApp and APMv2 while expressing generator differences only in declarative configuration.

**Architecture:** A shared parser handles every Backstage definition form. A shared `catalog_ci.py` runner reads `.github/catalog-ci.yaml`, runs each repository's configured generator without a shell, validates artifacts and catalogs, and renders summaries; one identical workflow orchestrates it in both repositories.

**Tech Stack:** Python 3 standard library, PyYAML, `unittest`, GitHub Actions, Rust OpenAPI generators

## Global Constraints

- `.github/workflows/ci.yml`, `.github/scripts/catalog_ci.py`, `.github/scripts/parse_catalog.py`, and `.github/scripts/test_catalog_ci.py` must be byte-identical across both repositories.
- Repository differences belong only in `.github/catalog-ci.yaml`.
- Support `$text`, `$json`, `$openapi`, URL, inline mapping/string/list, no-definition, multi-document, and non-API entity patterns.
- Commands are YAML arrays executed without a shell.
- Missing secrets skip Backstage registration/refresh with a warning; configured Backstage failures fail CI.
- Do not force-push or change Backstage entities, API contracts, runtime code, or Rust workspace interfaces.
- Embedded code in this plan is a non-authoritative creation-time snapshot; maintain delivered files after execution.
- Work on `main` only with explicit user consent and preserve unrelated changes.

---

### Task 1: Test and harden the byte-identical catalog parser

**Files:**
- Modify: `tdh-mordee-doctor-app/.github/scripts/parse_catalog.py`
- Modify: `tdh-biz-doctor-apmv2/.github/scripts/parse_catalog.py`
- Create: `tdh-mordee-doctor-app/.github/scripts/test_catalog_ci.py`
- Create: `tdh-biz-doctor-apmv2/.github/scripts/test_catalog_ci.py`

**Interfaces:**
- Produces: `entity_ref(entity)`, `parse_definition(definition, catalog_path, ref, repository_root)`, and `parse_file(catalog_path, repository_root)`.
- Rejects: local definition paths that resolve outside `repository_root`.

- [ ] **Step 1: Add parser tests in DoctorApp**

Create a `unittest.TestCase` using `tempfile.TemporaryDirectory` that imports
`parse_catalog` from the script directory. A table-driven
`test_definition_patterns` must assert the exact modes for `$text`, `$json`,
`$openapi`, inline mapping, inline string, inline list, and `None` as listed in
the design. Add focused tests named
`test_url_text_and_json_do_not_require_local_files`,
`test_missing_local_file_is_reported`,
`test_local_path_outside_repository_is_rejected`,
`test_multi_document_catalog_accepts_non_api_definition`, and
`test_malformed_yaml_returns_warning`. Assert the complete mode and path
dictionaries, missing-file entries, entity refs, and warning messages rather
than truthiness. Copy the test file unchanged to APMv2 after Task 2 adds runner
tests.

- [ ] **Step 2: Run the focused test and verify RED**

```bash
python3 tdh-mordee-doctor-app/.github/scripts/test_catalog_ci.py -v
```

Expected: FAIL because the current parser does not accept `repository_root`
and does not reject escaping paths.

- [ ] **Step 3: Implement repository-bound local path resolution**

Add:

```python
def resolve_local_path(raw_path: str, catalog_path: Path, repository_root: Path) -> Path:
    resolved = (catalog_path.parent / raw_path).resolve()
    try:
        resolved.relative_to(repository_root.resolve())
    except ValueError as error:
        raise ValueError(f"local definition escapes repository: {raw_path}") from error
    return resolved
```

Pass `repository_root` through `parse_definition` and `parse_file`. For `$text`
and `$json`, catch `ValueError`, return definition details with mode/path, and
append a warning containing the entity ref and exception text. In `main`, use
`Path.cwd().resolve()` as the repository root. Preserve all existing modes and
JSON output keys.

- [ ] **Step 4: Copy the parser and verify GREEN**

Copy the resulting parser byte-for-byte to APMv2, then run:

```bash
python3 tdh-mordee-doctor-app/.github/scripts/test_catalog_ci.py -v
cmp tdh-mordee-doctor-app/.github/scripts/parse_catalog.py tdh-biz-doctor-apmv2/.github/scripts/parse_catalog.py
```

Expected: all parser tests PASS and `cmp` exits 0.

### Task 2: Build the shared configuration-driven runner

**Files:**
- Create: both `.github/scripts/catalog_ci.py`
- Extend: both `.github/scripts/test_catalog_ci.py`
- Create: both `.github/catalog-ci.yaml`

**Interfaces:**
- `load_config(path, repository_root) -> Config`
- `generate_openapi(config) -> None`
- `validate_openapi(config) -> None`
- `artifact_paths(config) -> tuple[Path, ...]`
- `artifacts_changed(config) -> bool`
- `catalog_result(repository_root, output_path) -> dict`
- `render_summary(result_path) -> str`
- CLI subcommands: `generate-openapi`, `validate-openapi`, `artifact-paths`, `artifacts-changed`, `catalog-result`, `summary`.

- [ ] **Step 1: Add runner RED tests**

Extend the shared test file with tests named
`test_load_config_accepts_version_one_and_array_command`,
`test_load_config_rejects_unknown_keys`,
`test_load_config_rejects_wrong_version`,
`test_load_config_rejects_empty_or_string_command`,
`test_load_config_rejects_duplicate_or_escaping_artifact_paths`,
`test_load_config_rejects_unsupported_format`,
`test_validate_openapi_accepts_json_and_yaml`,
`test_validate_openapi_rejects_missing_openapi_info_title_or_empty_paths`,
`test_catalog_result_finds_nested_catalogs_and_fails_warnings`, and
`test_summary_handles_valid_missing_and_malformed_results`. Every rejection
test must assert the exact exception message. Use temporary repositories and
`unittest.mock.patch("subprocess.run")` for generation tests. Do not invoke
Cargo from unit tests.

- [ ] **Step 2: Run and verify RED**

```bash
python3 tdh-mordee-doctor-app/.github/scripts/test_catalog_ci.py -v
```

Expected: import failure for missing `catalog_ci.py`.

- [ ] **Step 3: Implement the runner**

Use frozen dataclasses:

```python
@dataclass(frozen=True)
class Artifact:
    path: Path
    format: str
    required_key: str

@dataclass(frozen=True)
class Config:
    repository_root: Path
    command: tuple[str, ...]
    artifacts: tuple[Artifact, ...]
```

Configuration validation must allow only `version`, `openapi`, `generate`,
`command`, `artifacts`, `path`, `format`, and `required_key` at their exact
levels. `generate_openapi` calls `subprocess.run(config.command,
cwd=config.repository_root, check=True)`. JSON uses `json.load`; YAML uses
`yaml.safe_load`; both require `required_key`, `info.title`, and a non-empty
mapping at `paths`.

`catalog_result` recursively finds `catalog-info.yaml` while excluding `.git`,
`.venv`, `target`, and `node_modules`, calls shared `parse_file`, writes the
same normalized result schema as the existing parser, and exits non-zero after
writing when warnings or missing files exist. `summary` never raises for a
missing/malformed result; it returns Markdown containing the diagnostic.

- [ ] **Step 4: Add repository configurations**

DoctorApp:

```yaml
version: 1
openapi:
  generate:
    command: [cargo, run, --locked, -p, server, --bin, export_openapi, --, server/openapi.json]
  artifacts:
    - path: server/openapi.json
      format: json
      required_key: openapi
```

APMv2:

```yaml
version: 1
openapi:
  generate:
    command: [cargo, run, --locked, -p, cli, --bin, openapi, --, generate, --module, consultation-rs]
  artifacts:
    - path: specs/provides/consultation-rs.json
      format: json
      required_key: openapi
    - path: specs/provides/consultation-rs.yaml
      format: yaml
      required_key: openapi
```

- [ ] **Step 5: Copy shared files and verify GREEN**

Copy `catalog_ci.py` and `test_catalog_ci.py` byte-for-byte to APMv2. Run:

```bash
python3 tdh-mordee-doctor-app/.github/scripts/test_catalog_ci.py -v
python3 tdh-biz-doctor-apmv2/.github/scripts/test_catalog_ci.py -v
cmp tdh-mordee-doctor-app/.github/scripts/catalog_ci.py tdh-biz-doctor-apmv2/.github/scripts/catalog_ci.py
cmp tdh-mordee-doctor-app/.github/scripts/test_catalog_ci.py tdh-biz-doctor-apmv2/.github/scripts/test_catalog_ci.py
```

Expected: both suites PASS and both comparisons exit 0.

### Task 3: Replace both workflows with one byte-identical workflow

**Files:**
- Modify: both `.github/workflows/ci.yml`

**Interfaces:**
- Consumes: runner CLI and repository config from Task 2.
- Produces: identical GitHub Actions behavior in both repositories.

- [ ] **Step 1: Add a failing identity assertion**

```bash
cmp tdh-mordee-doctor-app/.github/workflows/ci.yml tdh-biz-doctor-apmv2/.github/workflows/ci.yml
```

Expected: FAIL because current workflows differ.

- [ ] **Step 2: Install the same workflow in both repositories**

The workflow must have:

```yaml
name: Backstage Catalog Sync
on:
  push:
    branches: [main]
  workflow_dispatch: {}
permissions:
  contents: write
concurrency:
  group: backstage-catalog-sync-${{ github.repository }}-${{ github.ref }}
  cancel-in-progress: true
```

Use `actions/checkout@v6` with `submodules: recursive`, Python venv + PyYAML,
`dtolnay/rust-toolchain@stable`, and `Swatinem/rust-cache@v2`. Steps, in order:

1. `python .github/scripts/test_catalog_ci.py -v`
2. `python .github/scripts/catalog_ci.py generate-openapi`
3. `python .github/scripts/catalog_ci.py validate-openapi`
4. If `artifacts-changed` succeeds, add every newline-delimited
   `artifact-paths`, commit `[skip ci]`, rebase `origin/main`, and push without
   force.
5. `python .github/scripts/catalog_ci.py catalog-result --output /tmp/catalog_result.json`
6. Register the root catalog location with curl timeout/retry handling, accept
   HTTP 2xx or 409, then refresh every entity and require HTTP 2xx. Skip both
   operations when either secret is missing; any configured network or other
   HTTP failure exits non-zero.
7. With `if: always()`, append `python .github/scripts/catalog_ci.py summary
   --result /tmp/catalog_result.json` to `$GITHUB_STEP_SUMMARY`.

Environment variables for the Backstage step must be
`BACKSTAGE_URL`, `BACKSTAGE_TOKEN`, and
`CATALOG_LOCATION_URL=${{ github.server_url }}/${{ github.repository }}/blob/${{ github.ref_name }}/catalog-info.yaml`.

- [ ] **Step 3: Verify workflow identity and YAML parsing**

```bash
cmp tdh-mordee-doctor-app/.github/workflows/ci.yml tdh-biz-doctor-apmv2/.github/workflows/ci.yml
python3 -c 'import yaml; yaml.safe_load(open("tdh-mordee-doctor-app/.github/workflows/ci.yml"))'
```

Expected: both commands exit 0.

### Task 4: End-to-end local verification and commits

**Files:** All files from Tasks 1–3.

- [ ] **Step 1: Validate existing generated artifacts without generating**

```bash
python3 tdh-mordee-doctor-app/.github/scripts/catalog_ci.py --repository tdh-mordee-doctor-app validate-openapi
python3 tdh-biz-doctor-apmv2/.github/scripts/catalog_ci.py --repository tdh-biz-doctor-apmv2 validate-openapi
```

Expected: both report valid configured artifacts and exit 0.

- [ ] **Step 2: Parse both live catalogs**

```bash
python3 tdh-mordee-doctor-app/.github/scripts/catalog_ci.py --repository tdh-mordee-doctor-app catalog-result --output /tmp/doctor-catalog.json
python3 tdh-biz-doctor-apmv2/.github/scripts/catalog_ci.py --repository tdh-biz-doctor-apmv2 catalog-result --output /tmp/apm-catalog.json
```

Expected: no warnings or missing local definitions; both exit 0.

- [ ] **Step 3: Verify byte identity and repository differences**

```bash
for file in workflows/ci.yml scripts/catalog_ci.py scripts/parse_catalog.py scripts/test_catalog_ci.py; do cmp "tdh-mordee-doctor-app/.github/$file" "tdh-biz-doctor-apmv2/.github/$file"; done
! cmp tdh-mordee-doctor-app/.github/catalog-ci.yaml tdh-biz-doctor-apmv2/.github/catalog-ci.yaml
git -C tdh-mordee-doctor-app diff --check
git -C tdh-biz-doctor-apmv2 diff --check
```

Expected: shared files match, configs differ, and diff checks exit 0.

- [ ] **Step 4: Commit each repository**

```bash
git -C tdh-mordee-doctor-app add .github/workflows/ci.yml .github/scripts/catalog_ci.py .github/scripts/parse_catalog.py .github/scripts/test_catalog_ci.py .github/catalog-ci.yaml
git -C tdh-mordee-doctor-app commit -m "ci: unify catalog sync workflow"
git -C tdh-biz-doctor-apmv2 add .github/workflows/ci.yml .github/scripts/catalog_ci.py .github/scripts/parse_catalog.py .github/scripts/test_catalog_ci.py .github/catalog-ci.yaml
git -C tdh-biz-doctor-apmv2 commit -m "ci: unify catalog sync workflow"
```

Expected: one successful commit per repository and clean worktrees.
