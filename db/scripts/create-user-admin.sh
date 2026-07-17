#!/bin/sh

# Create (or update) a database admin/owner role with a password.
#
# Usage: create-user-admin.sh {user} {password}
#
# Args:
#   $1  user      - Role name to create
#   $2  password  - Password for the role
#
# Required env vars:
#   POSTGRES_USER_PASS    - Superuser password
#   POSTGRES_USER         - Superuser name (default: postgres)
#   POSTGRES_DB           - Superuser database to connect to (default: postgres)

_fail() {
  echo "$1"
  exit 1
}

CMD="create-user-admin.sh {user} {password}"

USER=$1
PASSWORD=$2

[ -z "${POSTGRES_USER_PASS}" ] && _fail "POSTGRES_USER_PASS env variable is not set"
[ -z "${USER}" ]               && _fail "A user parameter is not set. $CMD"
[ -z "${PASSWORD}" ]           && _fail "A password parameter is not set. $CMD"

run-pgsql-cmd-as-postgres.sh "DO \$\$
BEGIN
  IF NOT EXISTS (SELECT FROM pg_catalog.pg_roles WHERE rolname = '${USER}') THEN
    CREATE ROLE \"${USER}\" WITH LOGIN PASSWORD '${PASSWORD}';
  ELSE
    ALTER ROLE \"${USER}\" WITH LOGIN PASSWORD '${PASSWORD}';
  END IF;
END
\$\$;"

ret_val=$?
if [ "$ret_val" -ne 0 ]; then
  _fail "Failed to create/update admin role '${USER}'"
fi
