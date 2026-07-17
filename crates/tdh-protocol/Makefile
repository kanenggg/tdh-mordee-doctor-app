.PHONY: gen lint build break clean test test-all test-rust test-scala test-python test-typescript publish-all publish-scala publish-rust publish-python publish-typescript list-packages

# Load unified publishing configuration (Makefile format)
# Local overrides can be placed in .publish-config.mk.local
-include .publish-config.mk
-include .publish-config.mk.local

# Default target
all: gen

# Generate code for all languages (uses protoc)
gen:
	./scripts/generate.sh

# Lint proto files (requires buf)
lint:
	buf lint protos

# Build proto files (requires buf)
build:
	buf build protos

# Check for breaking changes against previous version (requires buf)
break:
	buf breaking --against '.git#branch=main' protos

# Clean generated files
clean:
	rm -rf scala/target rust/target python/__pycache__ typescript/dist
	find . -name "*.generated.rs" -delete

# Test all languages
test-all: test-rust test-scala test-python test-typescript

# Test Rust
test-rust:
	cd rust && cargo test

# Test Scala
test-scala:
	cd scala && sbt test

# Test Python
test-python:
	cd python && python -m pytest tests/

# Test TypeScript
test-typescript:
	cd typescript && npm test

# Publish all languages to GCP Artifact Registry
publish-all: publish-scala publish-rust publish-python publish-typescript

# Publish Scala to GCP
publish-scala:
	@echo "Publishing Scala to GCP Artifact Registry..."
	cd scala && ARTIFACT_REGISTRY_TOKEN=$(GCP_TOKEN) sbt publish

# Publish Rust to GCP
publish-rust:
	@echo "Publishing Rust to GCP Artifact Registry..."
	cd rust && ./scripts/publish.sh

# Publish Python to GCP
publish-python:
	@echo "Publishing Python to GCP Artifact Registry..."
	cd python && ARTIFACT_REGISTRY_TOKEN=$(GCP_TOKEN) ./scripts/publish.sh

# Publish TypeScript to GCP
publish-typescript:
	@echo "Publishing TypeScript to GCP Artifact Registry..."
	cd typescript && npm run publish:gcp

# List published packages
list-packages:
	@echo "Maven packages:"
	gcloud artifacts packages list --repository=$(PUBLISH_MAVEN_REPO_NAME) --location=$(PUBLISH_REGION)
	@echo "\nPython packages:"
	gcloud artifacts packages list --repository=$(PUBLISH_PYTHON_REPO_NAME) --location=$(PUBLISH_REGION)
	@echo "\nnpm packages:"
	gcloud artifacts packages list --repository=$(PUBLISH_NPM_REPO_NAME) --location=$(PUBLISH_REGION)
