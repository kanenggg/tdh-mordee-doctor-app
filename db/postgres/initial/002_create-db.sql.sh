#!/bin/sh

result=$(
	psql \
		--echo-errors \
		--dbname="$DB_NAME" \
		--host="$DB_HOST" \
		--port="$DB_PORT" \
		--no-password \
		--username="$USER" \
		--command="SELECT 1 FROM pg_database WHERE datname='$DB_NAME'" | grep 1
)

if [ -z "$result" ]; then IS_EXIST=0; else IS_EXIST=1; fi

if [ "$IS_EXIST" -ne 1 ]; then
	CMD="CREATE DATABASE $DB_NAME WITH OWNER $DB_ADMIN_USER ENCODING=UTF8"
	psql \
		--echo-errors \
		--dbname="$DB_NAME" \
		--host="$DB_HOST" \
		--port="$DB_PORT" \
		--no-password \
		--username="$USER" \
		--command="$SQL_CMD"
else
	echo "Database \"$DB_NAME\" already exists."
fi
