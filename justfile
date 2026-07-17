DEV_ARTIFACT_NAME := "asia-southeast1-docker.pkg.dev/tdg-dh-truehealth-core-nonprod/cossack-docker"
PROJECT := "tdg-dh-truehealth-core-nonprod"
REGION := "asia-southeast1"
TAG := `git rev-parse --short HEAD`

default:
    @just --list

init-project:
    git submodule update --init crates/tdh-protocol

[arg("platform", long="platform")]
buildx-local module_name tag=TAG platform="linux/amd64":
    docker buildx build --platform {{platform}} \
      --ssh default \
      --build-arg MODULE_NAME={{module_name}} \
      -f Dockerfile \
      -t {{DEV_ARTIFACT_NAME}}/doctorapp-{{module_name}}:{{tag}} \
      .

build-gcloud-docker module_name tag=TAG:
    gcloud builds submit \
      --config=rust.cloudbuild.yaml \
      --substitutions=_IMAGE={{DEV_ARTIFACT_NAME}}/doctorapp-{{module_name}},_MODULE_NAME={{module_name}},_TAG={{tag}} \
      --project={{PROJECT}} \
      --region={{REGION}}

build-buildpack module_name tag=TAG platform="linux/amd64":
    gcloud builds submit \
      --pack \
      --builder=gcr.io/buildpacks/builder:v1 \
      --env GOOGLE_ENTRYPOINT="./application" \
      --env GO_BUILD=true \
      --env GOPROXY=direct \
      --tag {{DEV_ARTIFACT_NAME}}/doctorapp-{{module_name}}:{{tag}} \
      --substitutions=_MODULE_NAME={{module_name}},_TAG={{tag}} \
      --project={{PROJECT}} \
      --region={{REGION}}

# Build CLI tool
# build-cli:
#   cargo build -p cli --release
#
# # Generate OpenAPI specification
# openapi module:
#   cargo build -p cli --bin openapi --release
#   ./target/release/openapi generate --module {{module}}
#
# # Generate all OpenAPI specifications
# openapi-all:
#   cargo build -p cli --bin openapi --release
#   ./target/release/openapi generate-all

# Set or update a doctor's schedule config in Redis
set-doctor-schedule doctor_id *args:
  cargo run --bin set_doctor_schedule -- {{doctor_id}} {{args}}

# Run PostgreSQL onboarding and migrations from this machine.
db command="local-migrate" *args:
  ./migrate.sh {{command}} {{args}}

# Create database migration
add-migration db_name migration_name:
  mkdir -p db/{{db_name}}/migrations
  touch db/{{db_name}}/migrations/$(date +%Y%m%d%H%M%S)__{{migration_name}}.sql

dev:
  cargo run --bin server -- --config-dir ./server/config

# Start the local Pub/Sub emulator (project test-project, port 8085) and create
# the approved and status-updated doctor-profile topics/subscriptions for local testing.
pubsub-emulator project="test-project" port="8085":
  #!/usr/bin/env bash
  set -euo pipefail
  host="localhost:{{port}}"
  echo "Starting Pub/Sub emulator on ${host} (project {{project}})..."
  gcloud beta emulators pubsub start --project={{project}} --host-port="${host}" &
  emu_pid=$!
  trap 'kill ${emu_pid} 2>/dev/null || true' EXIT
  # Wait until the emulator's REST API accepts requests (max ~30s).
  for _ in $(seq 1 100); do
    if curl -sf -o /dev/null "http://${host}/v1/projects/{{project}}/topics"; then break; fi
    sleep 0.3
  done
  # Create topics + subscriptions (idempotent: ignore "already exists").
  curl -sf -X PUT "http://${host}/v1/projects/{{project}}/topics/doctor-profile" >/dev/null || true
  curl -sf -X PUT "http://${host}/v1/projects/{{project}}/subscriptions/doctor-profile-sub" \
    -H 'Content-Type: application/json' \
    -d '{"topic":"projects/{{project}}/topics/doctor-profile"}' >/dev/null || true
  curl -sf -X PUT "http://${host}/v1/projects/{{project}}/topics/doctor-profile-status-updated" >/dev/null || true
  curl -sf -X PUT "http://${host}/v1/projects/{{project}}/subscriptions/doctor-profile-status-updated-sub" \
    -H 'Content-Type: application/json' \
    -d '{"topic":"projects/{{project}}/topics/doctor-profile-status-updated"}' >/dev/null || true
  echo ""
  echo "Pub/Sub emulator ready at ${host}"
  echo "  approved topic:         doctor-profile"
  echo "  approved subscription:  doctor-profile-sub"
  echo "  status topic:           doctor-profile-status-updated"
  echo "  status subscription:    doctor-profile-status-updated-sub"
  echo ""
  echo "Run the publish test in another terminal:"
  echo "  PUBSUB_EMULATOR_HOST=${host} cargo test -p server-bg --test doctor_profile_outbox_pubsub_emulator_test -- --ignored --nocapture"
  echo ""
  echo "Press Ctrl-C to stop the emulator."
  wait ${emu_pid}
