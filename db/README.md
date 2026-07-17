# Database Setup And Migrations

This README documents PostgreSQL setup and migration workflows for the doctor
app. PostgreSQL files live under `db/postgres`.

Run commands from the repository root.

## Schema Source

Atlas reads desired schema state from `db/postgres/schema`, not from the legacy
single-file `db/postgres/schema.sql`.

When changing database structure:

1. Update or add SQL files under `db/postgres/schema`.
2. Use Atlas to generate SQL into `db/postgres/migrations`.
3. Run migrations against the target database.

## Target Database

Default local target:

```text
postgresql.local:5432/biz_doctor
```

Prerequisites:

- The target PostgreSQL host is reachable from your machine.
- Docker is running.
- You have the real PostgreSQL passwords. Do not commit them.

## Environment

Use a bootstrap user only when running `db/postgres/initial` scripts. That user
must be a PostgreSQL superuser or have `CREATEROLE`, because the initial scripts
create roles.

```sh
export DB_HOST=postgresql.local
export DB_PORT=5432
export DB_NAME=biz_doctor

export DB_BOOTSTRAP_USER='<postgres-or-bootstrap-user>'
export DB_BOOTSTRAP_PASS='<real-bootstrap-password>'

export DB_USER_ADMIN_NAME=biz_doctor_admin
export DB_USER_ADMIN_PASS='<admin-role-password>'

export DB_USER_SERVICE_RW_NAME=biz_doctor_rw
export DB_USER_SERVICE_RW_PASS='<rw-role-password>'

export DB_USER_SERVICE_RO_NAME=biz_doctor_ro
export DB_USER_SERVICE_RO_PASS='<ro-role-password>'

export DB_ADMIN_USER="$DB_USER_ADMIN_NAME"
export DB_SERVICE_RW_USER="$DB_USER_SERVICE_RW_NAME"
export DB_SERVICE_RO_USER="$DB_USER_SERVICE_RO_NAME"

export DATABASE_URL="postgres://${DB_USER_ADMIN_NAME}:${DB_USER_ADMIN_PASS}@${DB_HOST}:${DB_PORT}/${DB_NAME}?sslmode=disable"
export ATLAS_DEV_URL="docker://postgres/15/dev?search_path=public"
```

Optional, only if PostgREST anonymous access is needed:

```sh
export DB_ANON_USER=biz_mordee_doctor_anon
export DB_ANON_USER_PASS='<anon-role-password>'
```

## Check Connection

For bootstrap work:

```sh
docker run --rm \
  -e PGPASSWORD="$DB_BOOTSTRAP_PASS" \
  postgres:15-alpine \
  psql \
    --host="$DB_HOST" \
    --port="$DB_PORT" \
    --username="$DB_BOOTSTRAP_USER" \
    --dbname="$DB_NAME" \
    --command='select current_database(), current_user;'
```

For app/admin migration work:

```sh
docker run --rm \
  -e PGPASSWORD="$DB_USER_ADMIN_PASS" \
  postgres:15-alpine \
  psql \
    --host="$DB_HOST" \
    --port="$DB_PORT" \
    --username="$DB_USER_ADMIN_NAME" \
    --dbname="$DB_NAME" \
    --command='select current_database(), current_user;'
```

Fix password or network connectivity errors before running scripts.

## Example Script

`./migrate.sh` wraps the common commands in this README. The `justfile` exposes
`just db` to run onboarding and migrations from this machine using local
`db/scripts`. It does not contain secrets; provide passwords through environment
variables.

Start from the example env file:

```sh
cp db/postgres/env-example db/postgres/.env
```

Then replace the placeholder passwords in `db/postgres/.env`.

```sh
./migrate.sh help
./migrate.sh check-admin
./migrate.sh initial
./migrate.sh atlas-diff add_consultation_settings
./migrate.sh migrate
./migrate.sh local-migrate
./migrate.sh verify
just db
just db help
```

If the database password contains URL-special characters, set `DATABASE_URL`
directly with an encoded password before running `atlas-diff`.

## Example End-To-End Steps

This example assumes the `biz_doctor` database already exists on the target
PostgreSQL host.

1. Export connection settings:

   ```sh
   export DB_HOST=postgresql.local
   export DB_PORT=5432
   export DB_NAME=biz_doctor

   export DB_BOOTSTRAP_USER=postgres
   export DB_BOOTSTRAP_PASS='<bootstrap-password>'

   export DB_USER_ADMIN_NAME=biz_doctor_admin
   export DB_USER_ADMIN_PASS='<admin-password>'

   export DB_USER_SERVICE_RW_NAME=biz_doctor_rw
   export DB_USER_SERVICE_RW_PASS='<rw-password>'

   export DB_USER_SERVICE_RO_NAME=biz_doctor_ro
   export DB_USER_SERVICE_RO_PASS='<ro-password>'

   export DB_ADMIN_USER="$DB_USER_ADMIN_NAME"
   export DB_SERVICE_RW_USER="$DB_USER_SERVICE_RW_NAME"
   export DB_SERVICE_RO_USER="$DB_USER_SERVICE_RO_NAME"

   export DATABASE_URL="postgres://${DB_USER_ADMIN_NAME}:${DB_USER_ADMIN_PASS}@${DB_HOST}:${DB_PORT}/${DB_NAME}?sslmode=disable"
   export ATLAS_DEV_URL="docker://postgres/15/dev?search_path=public"
   ```

2. Check the bootstrap connection:

   ```sh
   docker run --rm \
     -e PGPASSWORD="$DB_BOOTSTRAP_PASS" \
     postgres:15-alpine \
     psql \
       --host="$DB_HOST" \
       --port="$DB_PORT" \
       --username="$DB_BOOTSTRAP_USER" \
       --dbname="$DB_NAME" \
       --command='select current_database(), current_user;'
   ```

3. Run initial role/grant scripts if this database has not been bootstrapped:

   ```sh
   docker run --rm \
     -v "$PWD/db/postgres:/db/postgres" \
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
     '
   ```

4. Change the desired schema in `db/postgres/schema`, then generate a migration:

   ```sh
   cd db/postgres
   atlas migrate diff add_consultation_settings --env local
   atlas migrate validate --env local
   cd ../..
   ```

5. Review the generated SQL in `db/postgres/migrations`, then run migrations:

   ```sh
   docker compose -f compose.yaml run --rm --no-deps \
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
   ```

6. Verify the resulting tables and functions:

   ```sh
   docker run --rm \
     -e PGPASSWORD="$DB_USER_ADMIN_PASS" \
     postgres:15-alpine \
     psql \
       --host="$DB_HOST" \
       --port="$DB_PORT" \
       --username="$DB_USER_ADMIN_NAME" \
       --dbname="$DB_NAME" \
       --command='\dt'

   docker run --rm \
     -e PGPASSWORD="$DB_USER_ADMIN_PASS" \
     postgres:15-alpine \
     psql \
       --host="$DB_HOST" \
       --port="$DB_PORT" \
       --username="$DB_USER_ADMIN_NAME" \
       --dbname="$DB_NAME" \
       --command='\df'
   ```

## Generate Migrations With Atlas

`db/postgres/atlas.hcl` loads `DATABASE_URL` and `ATLAS_DEV_URL` from the
environment. It uses `file://schema`, so run Atlas commands from `db/postgres`.

```sh
cd db/postgres
atlas migrate diff '<migration-name>' --env local
atlas migrate validate --env local
cd ../..
```

Generated SQL is written to `db/postgres/migrations`.

## Run Initial Scripts

Use this when the target database needs roles, grants, or the optional PostgREST
anonymous role from `db/postgres/initial`.

```sh
docker run --rm \
  -v "$PWD/db/postgres:/db/postgres" \
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

    if [ -n "${DB_ANON_USER_PASS:-}" ]; then
      sh /db/postgres/initial/005_create_biz_apm_anon.sh
    else
      echo "Skipping 005_create_biz_apm_anon.sh because DB_ANON_USER_PASS is not set."
    fi
  '
```

Notes:

- `002_create-db.sql.sh` is intentionally skipped. This flow assumes the
  `biz_doctor` database already exists.
- Existing roles are not recreated, so their passwords are not changed by
  `001_create-users.sql.sh`.
- Run `005_create_biz_apm_anon.sh` only after required reference tables exist.

Verify roles:

```sh
docker run --rm \
  -e PGPASSWORD="$DB_BOOTSTRAP_PASS" \
  postgres:15-alpine \
  psql \
    --host="$DB_HOST" \
    --port="$DB_PORT" \
    --username="$DB_BOOTSTRAP_USER" \
    --dbname="$DB_NAME" \
    --command="\du"
```

## Run Migrations

Use this for SQL files under `db/postgres/migrations`. The compose migration job
uses `./migrate.sh`.

```sh
docker compose -f compose.yaml run --rm --no-deps \
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
```

`--no-deps` prevents Docker Compose from starting the local `postgres` service.

## Verify Migrations

List tables:

```sh
docker run --rm \
  -e PGPASSWORD="$DB_USER_ADMIN_PASS" \
  postgres:15-alpine \
  psql \
    --host="$DB_HOST" \
    --port="$DB_PORT" \
    --username="$DB_USER_ADMIN_NAME" \
    --dbname="$DB_NAME" \
    --command='\dt'
```

List functions:

```sh
docker run --rm \
  -e PGPASSWORD="$DB_USER_ADMIN_PASS" \
  postgres:15-alpine \
  psql \
    --host="$DB_HOST" \
    --port="$DB_PORT" \
    --username="$DB_USER_ADMIN_NAME" \
    --dbname="$DB_NAME" \
    --command="\df"
```

## Run The API

Update `server/config/local.toml` or environment variables with the same DB
credentials, then start the default Bacon job:

```sh
bacon
```
