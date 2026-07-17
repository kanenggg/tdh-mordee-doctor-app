#!/bin/sh

# Create a database with a given owner.
# The owner role is created (or updated) via create-user-admin.sh.
#
# Usage: create-db-with-owner.sh {db-name} {owner} {owner-password}
#
# Required env vars:
#   POSTGRES_USER_PASS    - Superuser password
#   POSTGRES_USER         - Superuser name (default: postgres)
#   POSTGRES_DB           - Superuser database to connect to (default: postgres)

_fail() {
  echo "$1"
  exit 1
}

[ -z "${POSTGRES_USER_PASS}" ] && _fail "POSTGRES_USER_PASS env variable is not set"

CMD="create-db-with-owner.sh {db-name} {owner} {owner-password}"

NEW_DB_NAME=$1
NEW_DB_OWNER=$2
NEW_DB_OWNER_PASS=$3

[ -z "${NEW_DB_NAME}" ]      && _fail "A db-name parameter is not set. $CMD"
[ -z "${NEW_DB_OWNER}" ]     && _fail "An owner parameter is not set. $CMD"
[ -z "${NEW_DB_OWNER_PASS}" ] && _fail "An owner-password parameter is not set. $CMD"

# Create or update the owner role
create-user-admin.sh "${NEW_DB_OWNER}" "${NEW_DB_OWNER_PASS}"
ret_val=$?
if [ "$ret_val" -ne 0 ]; then
  _fail "Failed to create/update owner role '${NEW_DB_OWNER}'"
fi

# Create the database only if it does not already exist.
# \gexec is an interactive psql metacommand and cannot be used with --command=,
# so we use a shell-level existence check instead.
if [ -z "${POSTGRES_USER}" ]; then POSTGRES_USER="postgres"; fi
if [ -z "${POSTGRES_DB}" ];   then POSTGRES_DB="postgres";   fi
if [ -z "${DB_PORT}" ];       then DB_PORT=5432;             fi

DB_EXISTS=$(PGPASSWORD="${POSTGRES_USER_PASS}" psql \
  --tuples-only \
  --dbname="${POSTGRES_DB}" \
  --host="${DB_HOST}" \
  --port="${DB_PORT}" \
  --no-password \
  --username="${POSTGRES_USER}" \
  --command="SELECT 1 FROM pg_catalog.pg_database WHERE datname = '${NEW_DB_NAME}'" \
  2>/dev/null | tr -d ' \n')

if [ "${DB_EXISTS}" = "1" ]; then
  echo "Database '${NEW_DB_NAME}' already exists, skipping creation."
else
  run-pgsql-cmd-as-postgres.sh "CREATE DATABASE \"${NEW_DB_NAME}\" OWNER \"${NEW_DB_OWNER}\""
  ret_val=$?
  if [ "$ret_val" -ne 0 ]; then
    _fail "Failed to create database '${NEW_DB_NAME}'"
  fi
fi
