# Backstage OpenAPI Sync Design

## Goal

Keep the TDH Doctor API definition in Backstage synchronized with the OpenAPI document generated from the Rust service whenever relevant changes reach `main`.

## Design

The repository remains the source that Backstage reads. `catalog-info.yaml` defines the Doctor API with `spec.definition.$text` pointing to `./server/openapi.json`. The CI workflow generates that JSON from `server::openapi::ApiDoc`, validates its minimum required OpenAPI fields, and commits it back to `main` only when its content changes.

After the generated file is available on `main`, CI parses every `catalog-info.yaml`, verifies that each referenced local definition exists, and calls the Backstage catalog refresh endpoint for every discovered entity. Backstage then reloads the catalog from GitHub and resolves the updated API definition.

The workflow runs on manual dispatch and on pushes to `main` that touch catalog metadata or inputs capable of changing the generated specification. It uses a per-ref concurrency group so a newer run supersedes an obsolete run.

## Components

- `server/src/bin/export_openapi.rs` exports `ApiDoc` as formatted JSON without starting the service or requiring runtime infrastructure.
- `server/openapi.json` is the generated, versioned artifact consumed by Backstage.
- `catalog-info.yaml` references the generated file through a relative `$text` definition.
- `.github/scripts/parse_catalog.py` discovers catalog entities and validates local definition references.
- `.github/workflows/ci.yml` orchestrates generation, validation, publication, and Backstage refresh.

## Workflow and Failure Handling

1. Check out the repository and its protocol submodule.
2. Install Python, PyYAML, and the stable Rust toolchain; restore the Rust build cache.
3. Generate `server/openapi.json` and reject an invalid or empty OpenAPI document.
4. If the file changed, commit and push only that generated file using the GitHub Actions bot. A bot commit must not create an endless workflow loop.
5. Discover and parse all catalog files. Missing referenced definitions fail the workflow before Backstage is contacted.
6. Refresh each Backstage entity using `BACKSTAGE_URL` and `BACKSTAGE_TOKEN` secrets. Successful 2xx responses pass; an unregistered 404 is reported as a warning; other responses fail the job after all entities have been attempted.
7. Always provide a readable job summary, including partial-failure cases where parsing did not complete.

A failed generation, validation, push, catalog check, or Backstage request leaves the previous valid spec available in Backstage. No deployment is performed by this workflow.

## Security and Permissions

The job receives only repository content write permission needed for the generated-file commit. Backstage credentials remain GitHub Actions secrets and must never be printed. Pull-request workflows do not receive these credentials because synchronization runs only after changes reach `main` or through an authorized manual dispatch.

## Verification

- Validate the workflow YAML structure and shell-sensitive expressions.
- Run the exporter and assert the resulting JSON contains `openapi`, `info.title`, and non-empty `paths`.
- Run the catalog parser against the repository catalog and assert there are no missing referenced files.
- Run Rust formatting and targeted compilation/tests for the exporter where feasible.
- Review the final diff to ensure unrelated working-tree changes are preserved.

## Success Criteria

- A change to an OpenAPI-covered Rust handler merged to `main` produces an updated `server/openapi.json` when the schema changes.
- The generated file is readable through the catalog's relative `$text` reference.
- Backstage refresh is triggered automatically after the current spec is present on `main`.
- No-op runs create no commit, and generated commits do not cause an infinite CI cycle.
- Missing specs, invalid OpenAPI output, push failures, and Backstage refresh failures are visible as actionable CI failures.
