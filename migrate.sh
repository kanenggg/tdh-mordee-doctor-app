#!/usr/bin/env sh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT="$SCRIPT_DIR"
POSTGRES_DIR="${POSTGRES_DIR:-$REPO_ROOT/db/postgres}"
DB_SCRIPTS_DIR="${DB_SCRIPTS_DIR:-$REPO_ROOT/db/scripts}"

load_env_file() {
	env_file="$1"
	if [ ! -f "$env_file" ]; then
		return
	fi

	echo "Loading env from $env_file"

	case "$-" in
	*a*) restore_allexport=false ;;
	*) restore_allexport=true ;;
	esac
	case "$-" in
	*u*) restore_nounset=true ;;
	*) restore_nounset=false ;;
	esac

	set -a
	set +u
	. "$env_file"
	if [ "$restore_nounset" = true ]; then
		set -u
	fi
	if [ "$restore_allexport" = true ]; then
		set +a
	fi
}

load_env() {
	# load_env_file "$REPO_ROOT/.env"
	# load_env_file "$REPO_ROOT/.env.local"
	# load_env_file "$REPO_ROOT/server/.env"
	# load_env_file "$REPO_ROOT/server/.env.local"
	load_env_file "$POSTGRES_DIR/.env"
	load_env_file "$POSTGRES_DIR/.env.local"
}

load_env

DB_HOST="${DB_HOST:-postgresql.local}"
DB_PORT="${DB_PORT:-5432}"
DB_NAME="${DB_NAME:-biz_doctor}"

DB_USER_ADMIN_NAME="${DB_USER_ADMIN_NAME:-biz_doctor_admin}"
DB_USER_SERVICE_RW_NAME="${DB_USER_SERVICE_RW_NAME:-biz_doctor_rw}"
DB_USER_SERVICE_RO_NAME="${DB_USER_SERVICE_RO_NAME:-biz_doctor_ro}"

DB_ADMIN_USER="${DB_ADMIN_USER:-$DB_USER_ADMIN_NAME}"
DB_SERVICE_RW_USER="${DB_SERVICE_RW_USER:-$DB_USER_SERVICE_RW_NAME}"
DB_SERVICE_RO_USER="${DB_SERVICE_RO_USER:-$DB_USER_SERVICE_RO_NAME}"

ATLAS_ENV="${ATLAS_ENV:-local}"
ATLAS_DEV_URL="${ATLAS_DEV_URL:-docker://postgres/15/dev?search_path=public}"

usage() {
	cat <<'EOF'
Usage:
  ./migrate.sh <command>

Commands:
  check-bootstrap  Check DB connection with DB_BOOTSTRAP_USER.
  check-admin      Check DB connection with DB_USER_ADMIN_NAME.
  initial          Run db/postgres/initial bootstrap scripts.
  atlas-diff       Generate a migration from db/postgres/schema.
  migrate          Run db/postgres migrations through Docker Compose.
  local-migrate    Run db/postgres migrations from this machine.
  verify           List tables and functions.
  all              Run check-admin, migrate, then verify.

Required env:
  DB_USER_ADMIN_PASS      Required by check-admin, atlas-diff, migrate, verify.
  DB_BOOTSTRAP_USER       Required by check-bootstrap and initial.
  DB_BOOTSTRAP_PASS       Required by check-bootstrap and initial.
  DB_USER_SERVICE_RW_PASS Required by initial and migrate.
  DB_USER_SERVICE_RO_PASS Required by initial and migrate.

Optional env:
  DB_HOST                 Default: postgresql.local
  DB_PORT                 Default: 5432
  DB_NAME                 Default: biz_doctor
  MIGRATION_NAME          Required by atlas-diff unless passed as arg 2.
  DATABASE_URL            Override if DB password needs URL encoding.
  ATLAS_DEV_URL           Default: docker://postgres/15/dev?search_path=public
  ATLAS_ENV               Default: local
  DB_ANON_USER_PASS       Enables initial/005_create_biz_apm_anon.sh.

Env files:
  Existing db/postgres/.env and db/postgres/.env.local files are loaded before
  defaults. Start from db/postgres/env-example.
EOF
}

require_env() {
	var_name="$1"
	eval "value=\${$var_name:-}"
	if [ -z "$value" ]; then
		echo "Missing required env: $var_name" >&2
		exit 1
	fi
}

require_command() {
	command_name="$1"
	if ! command -v "$command_name" >/dev/null 2>&1; then
		echo "Missing required command: $command_name" >&2
		exit 1
	fi
}

database_url() {
	if [ -n "${DATABASE_URL:-}" ]; then
		printf '%s\n' "$DATABASE_URL"
		return
	fi

	require_env DB_USER_ADMIN_PASS
	printf 'postgres://%s:%s@%s:%s/%s?sslmode=disable\n' \
		"$DB_USER_ADMIN_NAME" \
		"$DB_USER_ADMIN_PASS" \
		"$DB_HOST" \
		"$DB_PORT" \
		"$DB_NAME"
}

docker_psql() {
	password="$1"
	user="$2"
	command="$3"

	docker run --rm \
		-e PGPASSWORD="$password" \
		postgres:15-alpine \
		psql \
		--host="$DB_HOST" \
		--port="$DB_PORT" \
		--username="$user" \
		--dbname="$DB_NAME" \
		--command="$command"
}

ensure_run_pgsql_script_as_postgres() {
	PGPASSWORD="${PGPASSWORD:-$DB_USER_ADMIN_PASS}"
	export PGPASSWORD
	run_pgsql_script_as_postgres_path="${RUN_PGSQL_SCRIPT_AS_POSTGRES_PATH:-/usr/local/bin/run-pgsql-script-as-postgres.sh}"

	cat >"$run_pgsql_script_as_postgres_path" <<'EOF'
#!/bin/sh
PGPASSWORD="$PGPASSWORD" psql \
  --host="$DB_HOST" \
  --port="$DB_PORT" \
  --username="$DB_ADMIN_USER" \
  --dbname="postgres" \
  --file="$1"
EOF
	chmod +x "$run_pgsql_script_as_postgres_path"
}

prepare_local_db_scripts() {
	PATH="$DB_SCRIPTS_DIR:$PATH"
	export PATH

	POSTGRES_USER="${POSTGRES_USER:-${DB_BOOTSTRAP_USER:-postgres}}"
	POSTGRES_USER_PASS="${POSTGRES_USER_PASS:-${DB_BOOTSTRAP_PASS:-}}"
	POSTGRES_DB="${POSTGRES_DB:-postgres}"
	export POSTGRES_USER POSTGRES_USER_PASS POSTGRES_DB

	require_env POSTGRES_USER_PASS
	require_command psql
	require_command onboard-db.sh
	require_command run-pgsql-script.sh
}

run_db_scripts_migrate() {
	require_env DB_USER_ADMIN_PASS
	require_env DB_USER_SERVICE_RW_PASS
	require_env DB_USER_SERVICE_RO_PASS

	onboard-db.sh --db "$DB_NAME" \
		--admin-user "$DB_USER_ADMIN_NAME" \
		--admin-pass "$DB_USER_ADMIN_PASS" \
		--ro-user "$DB_USER_SERVICE_RO_NAME" \
		--ro-pass "$DB_USER_SERVICE_RO_PASS" \
		--rw-user "$DB_USER_SERVICE_RW_NAME" \
		--rw-pass "$DB_USER_SERVICE_RW_PASS"

	echo "Working $POSTGRES_DIR. Onboarding complete."

	found_migration=false
	for file in "$POSTGRES_DIR"/migrations/*.sql; do
		if [ ! -f "$file" ]; then
			continue
		fi

		found_migration=true
		echo "Running $file"
		run-pgsql-script.sh "$DB_NAME" "$DB_USER_ADMIN_NAME" "$DB_USER_ADMIN_PASS" "$file"
	done

	if [ "$found_migration" = false ]; then
		echo "No migration files found in $POSTGRES_DIR/migrations."
	fi
}

container_migrate() {
	ensure_run_pgsql_script_as_postgres
	run_db_scripts_migrate
}

local_migrate() {
	prepare_local_db_scripts
	run_db_scripts_migrate
}

check_bootstrap() {
	require_env DB_BOOTSTRAP_USER
	require_env DB_BOOTSTRAP_PASS
	docker_psql "$DB_BOOTSTRAP_PASS" "$DB_BOOTSTRAP_USER" 'select current_database(), current_user;'
}

check_admin() {
	require_env DB_USER_ADMIN_PASS
	docker_psql "$DB_USER_ADMIN_PASS" "$DB_USER_ADMIN_NAME" 'select current_database(), current_user;'
}

run_initial() {
	require_env DB_BOOTSTRAP_USER
	require_env DB_BOOTSTRAP_PASS
	require_env DB_USER_ADMIN_PASS
	require_env DB_USER_SERVICE_RW_PASS
	require_env DB_USER_SERVICE_RO_PASS

	docker run --rm \
		-v "$POSTGRES_DIR:/db/postgres" \
		-e DB_HOST="$DB_HOST" \
		-e DB_PORT="$DB_PORT" \
		-e DB_NAME="$DB_NAME" \
		-e USER="$DB_BOOTSTRAP_USER" \
		-e PGPASSWORD="$DB_BOOTSTRAP_PASS" \
		-e DB_BOOTSTRAP_USER="$DB_BOOTSTRAP_USER" \
		-e DB_ADMIN_USER="$DB_ADMIN_USER" \
		-e DB_ADMIN_USER_PASS="$DB_USER_ADMIN_PASS" \
		-e DB_SERVICE_RW_USER="$DB_SERVICE_RW_USER" \
		-e DB_SERVICE_RW_USER_PASS="$DB_USER_SERVICE_RW_PASS" \
		-e DB_SERVICE_RO_USER="$DB_SERVICE_RO_USER" \
		-e DB_SERVICE_RO_USER_PASS="$DB_USER_SERVICE_RO_PASS" \
		-e DB_ANON_USER="${DB_ANON_USER:-biz_mordee_doctor_anon}" \
		-e DB_ANON_USER_PASS="${DB_ANON_USER_PASS:-}" \
		postgres:15-alpine \
		sh -c '
      set -eu

      cat > /usr/local/bin/run-pgsql-script-as-postgres.sh << "EOF"
#!/bin/sh
PGPASSWORD="$PGPASSWORD" psql \
  --host="$DB_HOST" \
  --port="$DB_PORT" \
  --username="$DB_BOOTSTRAP_USER" \
  --dbname="$DB_NAME" \
  --file="$1"
EOF
      chmod +x /usr/local/bin/run-pgsql-script-as-postgres.sh

      sh /db/postgres/initial/001_create-users.sql.sh
      sh /db/postgres/initial/004_grant.sql.sh
      sh /db/postgres/initial/003_grant-additions.sql.sh
      sh /db/postgres/initial/002_create-db.sql.sh

      if [ -n "${DB_ANON_USER_PASS:-}" ]; then
        sh /db/postgres/initial/005_create_biz_apm_anon.sh
      else
        echo "Skipping 005_create_biz_apm_anon.sh because DB_ANON_USER_PASS is not set."
      fi
    '
}

atlas_diff() {
	migration_name="${2:-${MIGRATION_NAME:-}}"
	if [ -z "$migration_name" ]; then
		echo "Missing migration name. Set MIGRATION_NAME or pass it as arg 2." >&2
		exit 1
	fi
	if ! command -v atlas >/dev/null 2>&1; then
		echo "atlas is not installed or not in PATH." >&2
		exit 1
	fi

	db_url=$(database_url)
	(
		cd "$POSTGRES_DIR"
		DATABASE_URL="$db_url" ATLAS_DEV_URL="$ATLAS_DEV_URL" atlas migrate diff "$migration_name" --env "$ATLAS_ENV"
		DATABASE_URL="$db_url" ATLAS_DEV_URL="$ATLAS_DEV_URL" atlas migrate validate --env "$ATLAS_ENV"
	)
}

run_migrate() {
	require_env DB_USER_ADMIN_PASS
	require_env DB_USER_SERVICE_RW_PASS
	require_env DB_USER_SERVICE_RO_PASS

	docker compose -f "$REPO_ROOT/compose.yaml" run --rm --no-deps \
		-e POSTGRES_DIR="/db/postgres" \
		-e DB_HOST="$DB_HOST" \
		-e DB_PORT="$DB_PORT" \
		-e DB_NAME="$DB_NAME" \
		-e USER="$DB_USER_ADMIN_NAME" \
		-e PGPASSWORD="$DB_USER_ADMIN_PASS" \
		-e DB_ADMIN_USER="$DB_USER_ADMIN_NAME" \
		-e DB_USER_ADMIN_NAME="$DB_USER_ADMIN_NAME" \
		-e DB_USER_ADMIN_PASS="$DB_USER_ADMIN_PASS" \
		-e DB_USER_SERVICE_RW_NAME="$DB_USER_SERVICE_RW_NAME" \
		-e DB_USER_SERVICE_RW_PASS="$DB_USER_SERVICE_RW_PASS" \
		-e DB_USER_SERVICE_RO_NAME="$DB_USER_SERVICE_RO_NAME" \
		-e DB_USER_SERVICE_RO_PASS="$DB_USER_SERVICE_RO_PASS" \
		migrate
}

verify() {
	require_env DB_USER_ADMIN_PASS
	docker_psql "$DB_USER_ADMIN_PASS" "$DB_USER_ADMIN_NAME" '\dt'
	docker_psql "$DB_USER_ADMIN_PASS" "$DB_USER_ADMIN_NAME" '\df'
}

command="${1:-}"
case "$command" in
check-bootstrap)
	check_bootstrap
	;;
check-admin)
	check_admin
	;;
initial)
	run_initial
	;;
atlas-diff)
	atlas_diff "$@"
	;;
container-migrate)
	container_migrate
	;;
local-migrate)
	local_migrate
	;;
migrate)
	run_migrate
	;;
verify)
	verify
	;;
all)
	check_admin
	run_migrate
	verify
	;;
-h | --help | help | "")
	usage
	if [ -z "$command" ]; then
		exit 2
	fi
	;;
*)
	echo "Unknown command: $command" >&2
	usage >&2
	exit 2
	;;
esac
