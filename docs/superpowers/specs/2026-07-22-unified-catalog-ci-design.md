# Unified Catalog CI Design

**Date:** 2026-07-22

## Goal

Use the same catalog/OpenAPI CI workflow and executable validation logic in
`tdh-mordee-doctor-app` and `tdh-biz-doctor-apmv2`, while keeping only
declarative repository-specific generation settings separate.

## Current State

Both repositories already contain byte-identical
`.github/scripts/parse_catalog.py` files. Their workflows implement the same
general stages but differ in trigger paths, OpenAPI generation commands,
generated files, Backstage location registration, secret handling, refresh
behavior, and summary error handling.

The OpenAPI generation commands cannot be made identical without either
changing each Rust workspace interface or moving those differences into
configuration:

- DoctorApp runs `cargo run --locked -p server --bin export_openapi -- server/openapi.json`.
- APMv2 runs `cargo run --locked -p cli --bin openapi -- generate --module consultation-rs`.

## Architecture

Each repository will contain byte-identical copies of:

```text
.github/workflows/ci.yml
.github/scripts/catalog_ci.py
.github/scripts/parse_catalog.py
```

Repository-specific values will live only in:

```text
.github/catalog-ci.yaml
```

The workflow will invoke `catalog_ci.py` for generation, validation, result
assembly, and summary production. The existing `parse_catalog.py` remains the
single parser for Backstage entities and all supported `spec.definition`
patterns.

The workflow will trigger on every push to `main` and through
`workflow_dispatch`. It will not use repository-specific `paths` filters,
because those filters cannot be loaded dynamically from repository
configuration and would prevent the workflow files from remaining identical.

## Repository Configuration

The configuration schema will contain:

```yaml
version: 1
openapi:
  generate:
    command: [cargo, run, --locked, -p, PACKAGE, --bin, BINARY, --, ARGUMENTS]
  artifacts:
    - path: path/to/generated-spec.json
      format: json
      required_key: openapi
    - path: path/to/generated-spec.yaml
      format: yaml
      required_key: openapi
```

DoctorApp will configure one JSON artifact at `server/openapi.json`. APMv2
will configure JSON and YAML artifacts under `specs/provides/`. Commands are
represented as YAML arrays and executed without a shell, preventing quoting
differences and shell expansion.

Unknown keys, a version other than `1`, an empty generation command, duplicate
artifact paths, unsupported formats, or paths outside the repository will be
configuration errors.

## Generic Runner Responsibilities

`catalog_ci.py` will provide explicit subcommands:

- `generate-openapi`: validate configuration and execute the configured
  command.
- `validate-openapi`: parse every configured artifact and require its
  configured top-level version key.
- `catalog-result`: find every `catalog-info.yaml`, invoke the shared parser,
  and write normalized JSON containing entities, entity references, warnings,
  and missing local definitions.
- `summary`: render GitHub step-summary Markdown from a result file; malformed
  or missing result data produces a warning summary rather than hiding the
  earlier failure.

The runner will use Python's standard library plus PyYAML, which the workflow
installs once in a local virtual environment. Failures will use non-zero exit
codes and plain diagnostic messages suitable for both local execution and
GitHub Actions.

## Supported Definition Patterns

The shared parser will continue to support:

- `$text` local paths and HTTP(S) URLs;
- `$json` local paths and HTTP(S) URLs;
- `$openapi` URLs;
- inline mapping, string, and list definitions;
- entities without a definition;
- multi-document catalog YAML;
- any entity kind that declares `spec.definition`.

Missing local definition files and malformed catalog YAML will fail the
workflow. Inline definitions and remote URLs will not require local-file
existence checks.

## Workflow Behavior

The identical workflow will:

1. Check out `main` with write credentials.
2. Set up Python, a virtual environment, PyYAML, Rust, and Rust caching.
3. Generate and validate configured OpenAPI artifacts.
4. Commit generated artifact changes back to `main` using `[skip ci]`; no
   commit is created when artifacts are unchanged.
5. Discover and parse all catalog files.
6. Fail on parser warnings, malformed entities, or missing local definitions.
7. Register the repository's root `catalog-info.yaml` location in Backstage.
   HTTP 2xx and `409 Already Exists` are successful outcomes.
8. Refresh every parsed entity reference.
9. Write the same summary format in both repositories.

If `BACKSTAGE_URL` or `BACKSTAGE_TOKEN` is absent, registration and refresh
will be skipped with a GitHub warning while generation and validation still
run. Network, authentication, registration, or refresh failures will fail the
workflow when both secrets are configured.

## Concurrency and Generated Commits

Both workflows will use the same concurrency group based on repository and Git
reference, with in-progress cancellation enabled. Generated commits use
`[skip ci]` to avoid loops. Before pushing, the workflow rebases on
`origin/main`; a real conflict fails without force-pushing.

## Testing

The runner will have standard-library `unittest` coverage for:

- valid DoctorApp and APMv2 configuration shapes;
- every supported definition pattern;
- missing and escaping local paths;
- malformed multi-document YAML;
- invalid configuration versions, commands, formats, and duplicate artifacts;
- OpenAPI JSON and YAML version-key validation;
- summary behavior for valid, missing, and malformed result files.

Implementation verification will also:

1. Compare SHA-256 hashes of both workflow files and both executable scripts.
2. Run the same test suite independently in both repositories.
3. Run `catalog-result` against each current catalog.
4. Validate current generated OpenAPI artifacts without rewriting them.
5. Run YAML parsing and `git diff --check` in both repositories.

## Non-goals

- Creating a third shared-actions repository or publishing a reusable GitHub
  Action.
- Making the two Rust workspaces or OpenAPI generator binaries identical.
- Validating remote URL availability during CI.
- Changing Backstage entities, API ownership, runtime code, or contracts.
