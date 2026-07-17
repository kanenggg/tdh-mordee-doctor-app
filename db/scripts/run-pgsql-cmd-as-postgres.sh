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

SQL_CMD=$1
if [ -z "${SQL_CMD}" ];
then
  echo "An SQL Script file is not set. $CMD";
  exit 1;
fi

run-pgsql-cmd.sh "$POSTGRES_DB" "$POSTGRES_USER" "$POSTGRES_USER_PASS" "$SQL_CMD"
