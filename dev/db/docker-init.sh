#!/usr/bin/env bash
# Runs inside the postgres container on first start (docker-entrypoint-initdb.d).
set -euo pipefail
PGUSER="$POSTGRES_USER" PGDATABASE="$POSTGRES_DB" /workspace/scripts/dev-db-init.sh
