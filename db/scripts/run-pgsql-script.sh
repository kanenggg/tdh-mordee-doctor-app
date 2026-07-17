#!/bin/sh

DB_NAME=$1
USER=$2
export PGPASSWORD=$3
SQL_SCRIPT_FILE=$4

if [ -z "${DB_HOST}" ];
then
  echo "DB_HOST env variable is not set"
  exit 1;
fi

CMD="run-pgsql-script.sh {db-name} {user} {password} {sql-script-file}";

if [ -z "${DB_NAME}" ];
then
  echo "A db-name parameter is not set. $CMD";
  exit 1;
fi

if [ -z "${USER}" ];
then
  echo "A user parameter is not set. $CMD";
  exit 1;
fi

if [ -z "${PGPASSWORD}" ];
then
  echo "A password parameter is not set. $CMD";
  exit 1;
fi

if [ -z "${SQL_SCRIPT_FILE}" ];
then
  echo "An SQL Script file is not set. $CMD";
  exit 1;
fi

if [ -z "${DB_PORT}" ];
then
  DB_PORT=5432;
fi

psql \
  --echo-errors \
  --dbname="$DB_NAME" \
  --file="$SQL_SCRIPT_FILE" \
  --host="$DB_HOST" \
  --port="$DB_PORT" \
  --no-password \
  --username="$USER"
