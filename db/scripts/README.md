# postgresql-client scripts

Scripts for provisioning PostgreSQL databases and users.  
All scripts are available on `PATH` inside the container.

---

## Environment variables (infrastructure)

These env vars are shared across all scripts and must be set at the container level.

| Variable | Default | Description |
|---|---|---|
| `DB_HOST` | _(required)_ | PostgreSQL host |
| `DB_PORT` | `5432` | PostgreSQL port |
| `POSTGRES_USER` | `postgres` | Superuser name |
| `POSTGRES_USER_PASS` | _(required)_ | Superuser password |
| `POSTGRES_DB` | `postgres` | Superuser database to connect through |

---

## Scripts

### `onboard-db.sh`

Onboard a new database end-to-end: creates the database, its admin/owner, and optionally a read-only and/or read-write user.

```
onboard-db.sh --db <db> --admin-user <user> --admin-pass <pass> \
              [--ro-user <user> --ro-pass <pass>]               \
              [--rw-user <user> --rw-pass <pass>]
```

| Flag | Required | Description |
|---|:---:|---|
| `--db` | ✅ | Database name to create |
| `--admin-user` | ✅ | Owner role name |
| `--admin-pass` | ✅ | Owner role password |
| `--ro-user` | optional | Read-only role name (skip if omitted) |
| `--ro-pass` | if `--ro-user` | Read-only role password |
| `--rw-user` | optional | Read-write role name (skip if omitted) |
| `--rw-pass` | if `--rw-user` | Read-write role password |

**All three users:**
```sh
export DB_HOST=postgres.example.com
export POSTGRES_USER_PASS=superpassword

onboard-db.sh \
  --db         myapp \
  --admin-user myapp_admin \
  --admin-pass adminpass \
  --ro-user    myapp_ro \
  --ro-pass    ropass \
  --rw-user    myapp_rw \
  --rw-pass    rwpass
```

**Admin + RW only (skip RO):**
```sh
export DB_HOST=postgres.example.com
export POSTGRES_USER_PASS=superpassword

onboard-db.sh \
  --db         myapp \
  --admin-user myapp_admin \
  --admin-pass adminpass \
  --rw-user    myapp_rw \
  --rw-pass    rwpass
```

**Admin only:**
```sh
export DB_HOST=postgres.example.com
export POSTGRES_USER_PASS=superpassword

onboard-db.sh \
  --db         myapp \
  --admin-user myapp_admin \
  --admin-pass adminpass
```

---

### `create-db-with-owner.sh`

Create a database and its owner role (idempotent: skips if the DB already exists, upserts the role).

```
create-db-with-owner.sh {db-name} {owner} {owner-password}
```

```sh
export DB_HOST=postgres.example.com
export POSTGRES_USER_PASS=superpassword

create-db-with-owner.sh myapp myapp_admin adminpass
```

---

### `create-user-admin.sh`

Create (or update) an admin/owner role.

```
create-user-admin.sh {user} {password}
```

```sh
export POSTGRES_USER_PASS=superpassword

create-user-admin.sh myapp_admin adminpass
```

---

### `create-user-ro.sh`

Create (or update) a read-only user and grant `SELECT` privileges on the target database.

Role creation and existing-object grants are performed as the superuser (`POSTGRES_USER`).
`ALTER DEFAULT PRIVILEGES` (for future tables) is performed as the admin/owner user, which is
required for compatibility with Cloud SQL where the superuser is `NOSUPERUSER`.

```
create-user-ro.sh {db-name} {admin-user} {admin-pass} {ro-user} {ro-password}
```

```sh
export DB_HOST=postgres.example.com
export POSTGRES_USER_PASS=superpassword

create-user-ro.sh myapp myapp_admin adminpass myapp_ro ropass
```

---

### `create-user-rw.sh`

Create (or update) a read-write user and grant `SELECT, INSERT, UPDATE, DELETE` privileges on the target database.

Role creation and existing-object grants are performed as the superuser (`POSTGRES_USER`).
`ALTER DEFAULT PRIVILEGES` (for future tables) is performed as the admin/owner user, which is
required for compatibility with Cloud SQL where the superuser is `NOSUPERUSER`.

```
create-user-rw.sh {db-name} {admin-user} {admin-pass} {rw-user} {rw-password}
```

```sh
export DB_HOST=postgres.example.com
export POSTGRES_USER_PASS=superpassword

create-user-rw.sh myapp myapp_admin adminpass myapp_rw rwpass
```

---

### `run-pgsql-cmd.sh`

Run a SQL command against a specific database as a given user.

```
run-pgsql-cmd.sh {db-name} {user} {password} {sql-cmd}
```

```sh
export DB_HOST=postgres.example.com

run-pgsql-cmd.sh myapp myapp_admin adminpass "SELECT version();"
```

---

### `run-pgsql-cmd-as-postgres.sh`

Run a SQL command as the superuser.

```
run-pgsql-cmd-as-postgres.sh {sql-cmd}
```

```sh
export DB_HOST=postgres.example.com
export POSTGRES_USER_PASS=superpassword

run-pgsql-cmd-as-postgres.sh "SELECT datname FROM pg_database;"
```

---

### `run-pgsql-script.sh`

Run a SQL script file against a specific database as a given user.

```
run-pgsql-script.sh {db-name} {user} {password} {sql-script-file}
```

```sh
export DB_HOST=postgres.example.com

run-pgsql-script.sh myapp myapp_admin adminpass ./migrate.sql
```

---

### `run-pgsql-script-as-postgres.sh`

Run a SQL script file as the superuser.

```
run-pgsql-script-as-postgres.sh {sql-script-file}
```

```sh
export DB_HOST=postgres.example.com
export POSTGRES_USER_PASS=superpassword

run-pgsql-script-as-postgres.sh ./init.sql
```
