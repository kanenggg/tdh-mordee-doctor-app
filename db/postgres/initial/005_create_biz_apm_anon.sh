#!/bin/sh
#
# Creates the biz_mordee_doctor_anon role for PostgREST anonymous access.
# This role has read-only access to public reference data tables (e.g., icd10).
#
# Environment variables:
#   DB_ANON_USER      - Anonymous role name (default: biz_mordee_doctor_anon)
#   DB_ANON_USER_PASS - Password (required for PostgREST connection)
#   DB_NAME           - Database name
#   DB_ADMIN_USER     - Admin user (for granting default privileges)
#
# Usage:
#   DB_NAME=mydb DB_ANON_USER_PASS=secret ./005_create_biz_apm_anon.sh
#
# PostgREST connection string example:
#   postgres://biz_mordee_doctor_anon:secret@/dbname?host=/cloudsql/project:region:instance
#

DB_ANON_USER="${DB_ANON_USER:-biz_mordee_doctor_anon}"

if [ -z "$DB_ANON_USER_PASS" ]; then
	echo "Warning: DB_ANON_USER_PASS not set. Creating role without password (local dev only)."
	PASSWORD_CLAUSE=""
else
	PASSWORD_CLAUSE="PASSWORD '${DB_ANON_USER_PASS}'"
fi

SQL_OUT_FILE=$(mktemp)

cat <<EOF >"$SQL_OUT_FILE"
  -- Create anonymous role for PostgREST
  DO \$\$
  BEGIN
    CREATE ROLE ${DB_ANON_USER} WITH LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE ${PASSWORD_CLAUSE};
  EXCEPTION
      WHEN duplicate_object THEN
        RAISE NOTICE 'Role ${DB_ANON_USER} already exists, skipping creation';
  END;
  \$\$;

  -- Grant connect and schema usage
  GRANT CONNECT ON DATABASE ${DB_NAME} TO ${DB_ANON_USER};
  GRANT USAGE ON SCHEMA public TO ${DB_ANON_USER};

  -- Grant read-only access to specific public reference tables
  -- Add tables here as needed for anonymous access
  GRANT SELECT ON TABLE public.icd10 TO ${DB_ANON_USER};

  -- Default privileges for future tables created by admin
  -- (Only grant SELECT on explicitly listed tables above, not all tables)
  -- If you want auto-grant on new tables, uncomment:
  -- ALTER DEFAULT PRIVILEGES FOR ROLE ${DB_ADMIN_USER} IN SCHEMA public
  --   GRANT SELECT ON TABLES TO ${DB_ANON_USER};
EOF

run-pgsql-script-as-postgres.sh "$SQL_OUT_FILE"
rm "$SQL_OUT_FILE"
