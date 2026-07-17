#!/bin/sh

# Create (or update) a read-only user and grant appropriate privileges
# on the target database.
#
# Usage: create-user-ro.sh {db-name} {admin-user} {admin-pass} {ro-user} {ro-password}
#
# Args:
#   $1  db-name      - Database to grant privileges on
#   $2  admin-user   - Owner role (used to set default privileges for future tables)
#   $3  admin-pass   - Password for the admin/owner role
#   $4  ro-user      - Role name to create
#   $5  ro-password  - Password for the role
#
# Required env vars:
#   DB_HOST               - PostgreSQL host
#   POSTGRES_USER_PASS    - Superuser (postgres/cloudadmin) password
#   POSTGRES_USER         - Superuser name (default: postgres)
#   POSTGRES_DB           - Superuser database to connect to (default: postgres)
#   DB_PORT               - PostgreSQL port (default: 5432)
#
# Why two connections?
#   Role creation and GRANT CONNECT are executed as the superuser (postgres/cloudadmin).
#   ALTER DEFAULT PRIVILEGES must be executed as the admin/owner user because in Cloud SQL
#   the superuser is NOSUPERUSER — only the role that owns the objects can set default
#   privileges for objects it will create in the future.

_fail() {
  echo "$1"
  exit 1
}

CMD="create-user-ro.sh {db-name} {admin-user} {admin-pass} {ro-user} {ro-password}"

DB_NAME=$1
ADMIN_USER=$2
ADMIN_PASS=$3
RO_USER=$4
RO_PASS=$5

[ -z "${DB_HOST}" ]            && _fail "DB_HOST env variable is not set"
[ -z "${POSTGRES_USER_PASS}" ] && _fail "POSTGRES_USER_PASS env variable is not set"
[ -z "${DB_NAME}" ]            && _fail "A db-name parameter is not set. $CMD"
[ -z "${ADMIN_USER}" ]         && _fail "An admin-user parameter is not set. $CMD"
[ -z "${ADMIN_PASS}" ]         && _fail "An admin-pass parameter is not set. $CMD"
[ -z "${RO_USER}" ]            && _fail "A ro-user parameter is not set. $CMD"
[ -z "${RO_PASS}" ]            && _fail "A ro-password parameter is not set. $CMD"

if [ -z "${POSTGRES_USER}" ]; then
  POSTGRES_USER="postgres"
fi

if [ -z "${DB_PORT}" ]; then
  DB_PORT=5432
fi

# ---------------------------------------------------------------------------
# Step 1: Create or update role  (as superuser — only superuser can CREATE ROLE)
# ---------------------------------------------------------------------------
run-pgsql-cmd-as-postgres.sh "DO \$\$
BEGIN
  IF NOT EXISTS (SELECT FROM pg_catalog.pg_roles WHERE rolname = '${RO_USER}') THEN
    CREATE ROLE \"${RO_USER}\" WITH LOGIN PASSWORD '${RO_PASS}';
  ELSE
    ALTER ROLE \"${RO_USER}\" WITH LOGIN PASSWORD '${RO_PASS}';
  END IF;
END
\$\$;"
ret_val=$?
if [ "$ret_val" -ne 0 ]; then
  _fail "Failed to create/update read-only role '${RO_USER}'"
fi

# ---------------------------------------------------------------------------
# Step 2: Grant CONNECT on the database  (as superuser)
# ---------------------------------------------------------------------------
run-pgsql-cmd-as-postgres.sh "GRANT CONNECT ON DATABASE \"${DB_NAME}\" TO \"${RO_USER}\";"
ret_val=$?
if [ "$ret_val" -ne 0 ]; then
  _fail "Failed to grant CONNECT on database '${DB_NAME}' to '${RO_USER}'"
fi

# ---------------------------------------------------------------------------
# Step 3: Grant schema-level privileges on existing objects  (as superuser)
#         Must connect to the target database for GRANT ON ALL TABLES to work.
# ---------------------------------------------------------------------------
export PGPASSWORD="${POSTGRES_USER_PASS}"

psql \
  --echo-errors \
  --dbname="${DB_NAME}" \
  --host="${DB_HOST}" \
  --port="${DB_PORT}" \
  --no-password \
  --username="${POSTGRES_USER}" \
  --command="
GRANT USAGE ON SCHEMA public TO \"${RO_USER}\";
GRANT SELECT ON ALL TABLES IN SCHEMA public TO \"${RO_USER}\";
GRANT SELECT ON ALL SEQUENCES IN SCHEMA public TO \"${RO_USER}\";"

ret_val=$?
if [ "$ret_val" -ne 0 ]; then
  _fail "Failed to grant schema/table privileges to '${RO_USER}'"
fi

# ---------------------------------------------------------------------------
# Step 4: Set default privileges for future tables  (as admin/owner user)
#         ALTER DEFAULT PRIVILEGES must run as the role that will CREATE the
#         objects — not as the superuser — to work correctly in Cloud SQL.
# ---------------------------------------------------------------------------
export PGPASSWORD="${ADMIN_PASS}"

psql \
  --echo-errors \
  --dbname="${DB_NAME}" \
  --host="${DB_HOST}" \
  --port="${DB_PORT}" \
  --no-password \
  --username="${ADMIN_USER}" \
  --command="
ALTER DEFAULT PRIVILEGES IN SCHEMA public
  GRANT SELECT ON TABLES TO \"${RO_USER}\";
ALTER DEFAULT PRIVILEGES IN SCHEMA public
  GRANT SELECT ON SEQUENCES TO \"${RO_USER}\";"

ret_val=$?
if [ "$ret_val" -ne 0 ]; then
  _fail "Failed to set default privileges for '${RO_USER}'"
fi
