#!/bin/sh

# Onboard a new database by creating the database, its admin (owner),
# and optionally a read-only user and/or a read-write user.
#
# Usage:
#   onboard-db.sh --db <db-name> --admin-user <user> --admin-pass <pass> \
#                 [--ro-user <user> --ro-pass <pass>]                     \
#                 [--rw-user <user> --rw-pass <pass>]
#
# Flags:
#   --db          Name of the database to create                 (required)
#   --admin-user  Role name for the database owner (admin)       (required)
#   --admin-pass  Password for the admin role                    (required)
#   --ro-user     Role name for the read-only user               (optional)
#   --ro-pass     Password for the read-only user                (required if --ro-user is set)
#   --rw-user     Role name for the read-write user              (optional)
#   --rw-pass     Password for the read-write user               (required if --rw-user is set)
#
# Required env vars:
#   DB_HOST               - PostgreSQL host
#   POSTGRES_USER_PASS    - Superuser (postgres/cloudadmin) password
#   POSTGRES_USER         - Superuser name (default: postgres)
#   POSTGRES_DB           - Superuser database to connect to (default: postgres)
#   DB_PORT               - PostgreSQL port (default: 5432)

_fail() {
  echo "$1"
  exit 1
}

_usage() {
  echo "Usage: onboard-db.sh --db <db> --admin-user <user> --admin-pass <pass> \\"
  echo "                     [--ro-user <user> --ro-pass <pass>]               \\"
  echo "                     [--rw-user <user> --rw-pass <pass>]"
  exit 1
}

# ---------------------------------------------------------------------------
# Parse flags
# ---------------------------------------------------------------------------

DB_NAME=""
ADMIN_USER=""
ADMIN_PASS=""
RO_USER=""
RO_PASS=""
RW_USER=""
RW_PASS=""

# Sentinels: track whether optional flags were explicitly provided.
# This lets us distinguish "--ro-user ''" (misconfiguration → fail)
# from the flag being omitted entirely (intentional → skip).
HAS_RO_USER=0
HAS_RO_PASS=0
HAS_RW_USER=0
HAS_RW_PASS=0

while [ $# -gt 0 ]; do
  case "$1" in
    --db)         DB_NAME="$2";              shift 2 ;;
    --admin-user) ADMIN_USER="$2";           shift 2 ;;
    --admin-pass) ADMIN_PASS="$2";           shift 2 ;;
    --ro-user)    RO_USER="$2"; HAS_RO_USER=1; shift 2 ;;
    --ro-pass)    RO_PASS="$2"; HAS_RO_PASS=1; shift 2 ;;
    --rw-user)    RW_USER="$2"; HAS_RW_USER=1; shift 2 ;;
    --rw-pass)    RW_PASS="$2"; HAS_RW_PASS=1; shift 2 ;;
    --help|-h)    _usage ;;
    *) _fail "Unknown option: $1" ;;
  esac
done

# ---------------------------------------------------------------------------
# Validate
# ---------------------------------------------------------------------------

[ -z "${DB_HOST}" ]            && _fail "DB_HOST env variable is not set"
[ -z "${POSTGRES_USER_PASS}" ] && _fail "POSTGRES_USER_PASS env variable is not set"
[ -z "${DB_NAME}" ]            && _fail "--db is required$(_usage)"
[ -z "${ADMIN_USER}" ]         && _fail "--admin-user is required$(_usage)"
[ -z "${ADMIN_PASS}" ]         && _fail "--admin-pass is required$(_usage)"

# Fail clearly when a flag was provided but its value resolved to empty,
# which happens when the caller passes an unset env var (e.g. --ro-user "").
if [ "${HAS_RO_USER}" -eq 1 ] && [ -z "${RO_USER}" ]; then
  _fail "--ro-user was provided but its value is empty (check that the env var is set)"
fi
if [ "${HAS_RW_USER}" -eq 1 ] && [ -z "${RW_USER}" ]; then
  _fail "--rw-user was provided but its value is empty (check that the env var is set)"
fi
if [ "${HAS_RO_PASS}" -eq 1 ] && [ -z "${RO_PASS}" ]; then
  _fail "--ro-pass was provided but its value is empty (check that the env var is set)"
fi
if [ "${HAS_RW_PASS}" -eq 1 ] && [ -z "${RW_PASS}" ]; then
  _fail "--rw-pass was provided but its value is empty (check that the env var is set)"
fi

if [ "${HAS_RO_USER}" -eq 1 ] && [ "${HAS_RO_PASS}" -eq 0 ]; then
  _fail "--ro-pass is required when --ro-user is set"
fi

if [ "${HAS_RW_USER}" -eq 1 ] && [ "${HAS_RW_PASS}" -eq 0 ]; then
  _fail "--rw-pass is required when --rw-user is set"
fi

# ---------------------------------------------------------------------------
# Step 1: Create database with admin owner
# ---------------------------------------------------------------------------

echo ">>> [1] Creating database '${DB_NAME}' with owner '${ADMIN_USER}'..."
create-db-with-owner.sh "${DB_NAME}" "${ADMIN_USER}" "${ADMIN_PASS}"
ret_val=$?
if [ "$ret_val" -ne 0 ]; then
  _fail "Failed at step 1: create database with owner"
fi

# ---------------------------------------------------------------------------
# Step 2: Create read-only user (optional)
# ---------------------------------------------------------------------------

if [ "${HAS_RO_USER}" -eq 1 ]; then
  echo ">>> [2] Creating read-only user '${RO_USER}'..."
  create-user-ro.sh "${DB_NAME}" "${ADMIN_USER}" "${ADMIN_PASS}" "${RO_USER}" "${RO_PASS}"
  ret_val=$?
  if [ "$ret_val" -ne 0 ]; then
    _fail "Failed at step 2: create read-only user"
  fi
else
  echo ">>> [2] Skipping read-only user (--ro-user not provided)"
fi

# ---------------------------------------------------------------------------
# Step 3: Create read-write user (optional)
# ---------------------------------------------------------------------------

if [ "${HAS_RW_USER}" -eq 1 ]; then
  echo ">>> [3] Creating read-write user '${RW_USER}'..."
  create-user-rw.sh "${DB_NAME}" "${ADMIN_USER}" "${ADMIN_PASS}" "${RW_USER}" "${RW_PASS}"
  ret_val=$?
  if [ "$ret_val" -ne 0 ]; then
    _fail "Failed at step 3: create read-write user"
  fi
else
  echo ">>> [3] Skipping read-write user (--rw-user not provided)"
fi

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------

echo ">>> Onboarding complete."
echo "    Database : ${DB_NAME}"
echo "    Admin    : ${ADMIN_USER}  (owner)"
if [ "${HAS_RO_USER}" -eq 1 ]; then
  echo "    RO user  : ${RO_USER}  (read-only)"
else
  echo "    RO user  : (skipped)"
fi
if [ "${HAS_RW_USER}" -eq 1 ]; then
  echo "    RW user  : ${RW_USER}  (read-write)"
else
  echo "    RW user  : (skipped)"
fi
