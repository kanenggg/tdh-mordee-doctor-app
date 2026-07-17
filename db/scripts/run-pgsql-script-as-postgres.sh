#!/bin/sh

if [ -z "${POSTGRES_DB}" ];
then
  export POSTGRES_DB="postgres"
fi

if [ -z "${POSTGRES_USER}" ];
then
  export POSTGRES_USER="postgres"
fi

if [ -z "${POSTGRES_USER_PASS}" ];
then
  echo "POSTGRES_USER_PASS env variable is not set."
  exit 1
fi

SQL_SCRIPT_FILE=$1
if [ -z "${SQL_SCRIPT_FILE}" ];
then
  echo "An SQL Script file is not set. $CMD";
  exit 1;
fi

run-pgsql-script.sh "$POSTGRES_DB" "$POSTGRES_USER" "$POSTGRES_USER_PASS" "$SQL_SCRIPT_FILE"
