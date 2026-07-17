#!/bin/bash
set -e

# Source unified publishing configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)

if [ -f "${REPO_ROOT}/.publish-config" ]; then
    source "${REPO_ROOT}/.publish-config"
else
    # Fallback to legacy configuration
    export PUBLISH_PYTHON_REGISTRY="https://asia-southeast1-python.pkg.dev/tdh-project/tdh-python"
fi

# Build package
cd python
python -m build

# Get GCP token
cd "${REPO_ROOT}"
TOKEN=$(get_publish_token)

# Publish to GCP Artifact Registry
twine upload \
  --repository-url ${PUBLISH_PYTHON_REGISTRY}/ \
  --username oauth2accesstoken \
  --password ${TOKEN} \
  python/dist/*
