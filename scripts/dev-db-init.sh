#!/usr/bin/env bash
#
# Creates the development database: the production roles and schemas
# (deploy/sql/roles.sql, with development passwords), the tables of
# migrations/ (applied with psql and recorded as sqlx records them, so
# `pulsars-api migrate` and the tests find them applied), then the demo data
# (dev/db/30-demo-data.sql). Used by `docker compose` (on first start) and by
# CI.
#
# Connects with the standard libpq variables (PGHOST, PGPORT, PGUSER,
# PGPASSWORD) as a superuser, to the database named by PGDATABASE (default
# `pulsars`), which must exist and be empty.
#
#     PGHOST=127.0.0.1 PGUSER=postgres PGPASSWORD=postgres ./scripts/dev-db-init.sh

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export PGDATABASE="${PGDATABASE:-pulsars}"

psql_run() {
    psql --no-psqlrc --quiet --set ON_ERROR_STOP=1 "$@"
}

echo "==> $root/deploy/sql/roles.sql"
psql_run --set mdp_migrate=pulsars_migrate_dev --set mdp_api=pulsars_api_dev \
    --file "$root/deploy/sql/roles.sql"

# The migrations, as pulsars_owner, in order, each recorded with the SHA-384
# of its file, as sqlx does.
psql_run --command "CREATE TABLE IF NOT EXISTS game._sqlx_migrations (
    version bigint PRIMARY KEY, description text NOT NULL,
    installed_on timestamptz NOT NULL DEFAULT now(), success boolean NOT NULL,
    checksum bytea NOT NULL, execution_time bigint NOT NULL);
    ALTER TABLE game._sqlx_migrations OWNER TO pulsars_owner;"
for file in "$root"/migrations/*.sql; do
    name="$(basename "$file" .sql)"
    version="${name%%_*}"
    description="${name#*_}"
    description="${description//_/ }"
    checksum="$(sha384sum "$file" | cut -d' ' -f1)"
    echo "==> $file"
    {
        echo "BEGIN; SET ROLE pulsars_owner;"
        cat "$file"
        echo
        echo "INSERT INTO game._sqlx_migrations (version, description, success, checksum, execution_time)"
        echo "VALUES ($version, '$description', true, '\\x$checksum', 0);"
        echo "COMMIT;"
    } | psql_run
done

echo "==> $root/dev/db/30-demo-data.sql"
psql_run --file "$root/dev/db/30-demo-data.sql"

where="${PGHOST:-localhost}:${PGPORT:-5432}/${PGDATABASE}"
echo "Done. The API connects as: postgres://pulsars_api:pulsars_api_dev@$where"
echo "Migrations run as:         postgres://pulsars_migrate:pulsars_migrate_dev@$where"
