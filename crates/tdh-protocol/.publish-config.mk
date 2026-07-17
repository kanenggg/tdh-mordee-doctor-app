# Unified Publishing Configuration for tdh-protocol (Makefile format)

# GCP Configuration
PUBLISH_GCP_PROJECT := tdg-dh-truehealth-core-nonprod
PUBLISH_REGION := asia-southeast1
PUBLISH_REGISTRY_NAME := tdh-protocol

# Repository names for gcloud commands
PUBLISH_MAVEN_REPO_NAME := $(PUBLISH_REGISTRY_NAME)-maven
PUBLISH_CARGO_REPO_NAME := $(PUBLISH_REGISTRY_NAME)-cargo
PUBLISH_PYTHON_REPO_NAME := $(PUBLISH_REGISTRY_NAME)-python
PUBLISH_NPM_REPO_NAME := $(PUBLISH_REGISTRY_NAME)-npm

# Package Configuration
PUBLISH_PACKAGE_NAME := tdh-protocol
PUBLISH_PACKAGE_VERSION := 0.1.0

# GCP token (retrieved at runtime)
GCP_TOKEN := $(shell gcloud auth print-access-token)
