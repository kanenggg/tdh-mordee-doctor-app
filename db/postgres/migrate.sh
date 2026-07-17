#!/usr/bin/env sh

LOCAL_DIR=$(dirname $0)

onboard-db.sh --db ${DB_NAME} \
	--admin-user "${DB_USER_ADMIN_NAME}" \
	--admin-pass "${DB_USER_ADMIN_PASS}" \
	--ro-user "${DB_USER_SERVICE_RO_NAME}" \
	--ro-pass "${DB_USER_SERVICE_RO_PASS}" \
	--rw-user "${DB_USER_SERVICE_RW_NAME}" \
	--rw-pass "${DB_USER_SERVICE_RW_PASS}"

echo "Working $LOCAL_DIR Onboarding complete."
ls $LOCAL_DIR

for file in $LOCAL_DIR/migrations/*.sql; do
	echo "Running $file"
	run-pgsql-script.sh ${DB_NAME} ${DB_USER_ADMIN_NAME} "${DB_USER_ADMIN_PASS}" $file
done
