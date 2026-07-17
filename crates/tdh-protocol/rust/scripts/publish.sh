#!/bin/bash
set -e

# Source unified publishing configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

if [ -f "${REPO_ROOT}/.publish-config" ]; then
    source "${REPO_ROOT}/.publish-config"
else
    # Fallback to legacy configuration
    export PUBLISH_CARGO_REGISTRY="https://asia-southeast1-maven.pkg.dev/tdh-project/tdh-cargo"
fi

# Get package name and version
NAME=$(cargo read-manifest | jq -r '.name')
VERSION=$(cargo read-manifest | jq -r '.version')

# Package file
CRATE_FILE="${NAME}-${VERSION}.crate"

# Package the crate
cargo package

# Publish to GCP Artifact Registry (generic repository)
gcloud artifacts generic upload \
  --project="${PUBLISH_GCP_PROJECT}" \
  --location="${PUBLISH_REGION}" \
  --repository="${PUBLISH_CARGO_REPO_NAME}" \
  --package="${NAME}" \
  --version="${VERSION}" \
  --source="target/package/${CRATE_FILE}"

echo "Published ${NAME} ${VERSION} to GCP Artifact Registry"
