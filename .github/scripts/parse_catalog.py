"""
Parse catalog-info.yaml files and extract entity refs + spec definition metadata.

Supports all Backstage spec.definition patterns:
  - inline  : definition is a YAML/JSON string written directly in the catalog file
  - $text   : definition: $text: path/to/spec.yaml  (local file reference)
  - $openapi: definition: $openapi: https://...     (remote URL reference)
  - $json   : definition: $json: path/to/spec.json  (local JSON file reference)
"""

import json
import os
import sys

import yaml


def resolve(path: str, catalog_dir: str) -> str:
    return os.path.normpath(os.path.join(catalog_dir, path))


def parse_definition(definition, catalog_dir: str, entity_ref: str):
    missing = []

    if definition is None:
        return {"mode": "none"}, missing

    if isinstance(definition, dict):
        if "$text" in definition:
            rel = definition["$text"]
            resolved = resolve(rel, catalog_dir)
            entry = {"mode": "$text", "path": rel, "resolved_path": resolved}
            if not os.path.isfile(resolved):
                missing.append({"entityRef": entity_ref, "path": rel, "resolved_path": resolved})
            return entry, missing

        if "$openapi" in definition:
            return {"mode": "$openapi", "url": definition["$openapi"]}, missing

        if "$json" in definition:
            rel = definition["$json"]
            resolved = resolve(rel, catalog_dir)
            entry = {"mode": "$json", "path": rel, "resolved_path": resolved}
            if not os.path.isfile(resolved):
                missing.append({"entityRef": entity_ref, "path": rel, "resolved_path": resolved})
            return entry, missing

    # inline string / embedded YAML block
    return {"mode": "inline"}, missing


def parse_file(catalog_path: str):
    catalog_dir = os.path.dirname(os.path.abspath(catalog_path))
    entities = []
    warnings = []
    missing_files = []

    with open(catalog_path, encoding="utf-8") as f:
        docs = list(yaml.safe_load_all(f))

    for doc in docs:
        if not doc or not isinstance(doc, dict):
            continue

        kind = doc.get("kind", "")
        name = doc.get("metadata", {}).get("name", "")
        namespace = doc.get("metadata", {}).get("namespace", "default")

        if not kind or not name:
            warnings.append(f"{catalog_path}: document missing kind or metadata.name — skipped")
            continue

        entity_ref = f"{kind.lower()}:{namespace}/{name}"
        entry = {"entityRef": entity_ref, "catalog_file": catalog_path}

        spec = doc.get("spec", {}) or {}
        if kind.lower() == "api" and "definition" in spec:
            defn, missing = parse_definition(spec["definition"], catalog_dir, entity_ref)
            entry["definition"] = defn
            missing_files.extend(missing)

        entities.append(entry)

    return entities, warnings, missing_files


def main():
    if len(sys.argv) < 2:
        print("Usage: parse_catalog.py <catalog-info.yaml> [...]", file=sys.stderr)
        sys.exit(1)

    all_entities = []
    all_warnings = []
    all_missing = []

    for path in sys.argv[1:]:
        entities, warnings, missing = parse_file(path)
        all_entities.extend(entities)
        all_warnings.extend(warnings)
        all_missing.extend(missing)

    result = {
        "entities": all_entities,
        "entity_refs": [e["entityRef"] for e in all_entities],
        "warnings": all_warnings,
        "missing_files": all_missing,
    }
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
