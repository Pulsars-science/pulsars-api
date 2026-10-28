-- The database roles and the empty schemas of a new server
-- (docs/contract/database.md §2). Run ONCE, as a superuser, on an empty
-- `pulsars` database (deploy/sql/README.md):
--
--     sudo -u postgres createdb pulsars
--     sudo -u postgres psql pulsars -v ON_ERROR_STOP=1 \
--         -v mdp_migrate="$(cat ~/pulsars-secrets/MDP_MIGRATE)" \
--         -v mdp_api="$(cat ~/pulsars-secrets/MDP_API)" -f deploy/sql/roles.sql
--
-- Running it again changes nothing. Then `pulsars-api migrate` creates the
-- tables.
--
--   pulsars_owner    owns every schema and table; cannot log in;
--   pulsars_migrate  runs the migrations, as pulsars_owner;
--   pulsars_api      the service: reads and writes rows, creates nothing.

\set ON_ERROR_STOP 1
BEGIN;

SELECT 'CREATE ROLE pulsars_owner NOLOGIN'
 WHERE NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'pulsars_owner') \gexec

SELECT format('CREATE ROLE pulsars_migrate LOGIN PASSWORD %L IN ROLE pulsars_owner', :'mdp_migrate')
 WHERE NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'pulsars_migrate') \gexec

SELECT format('CREATE ROLE pulsars_api LOGIN PASSWORD %L', :'mdp_api')
 WHERE NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'pulsars_api') \gexec

REVOKE ALL ON DATABASE pulsars FROM PUBLIC;
REVOKE CREATE ON SCHEMA public FROM PUBLIC;
GRANT CONNECT ON DATABASE pulsars TO pulsars_migrate, pulsars_api;
-- A migration may add a schema.
GRANT CREATE ON DATABASE pulsars TO pulsars_owner;

CREATE SCHEMA IF NOT EXISTS site    AUTHORIZATION pulsars_owner;
CREATE SCHEMA IF NOT EXISTS contenu AUTHORIZATION pulsars_owner;
CREATE SCHEMA IF NOT EXISTS game    AUTHORIZATION pulsars_owner;
CREATE SCHEMA IF NOT EXISTS langues AUTHORIZATION pulsars_owner;

-- What pulsars_migrate creates belongs to pulsars_owner. The migrations
-- bookkeeping (`_sqlx_migrations`) lands in `game`, first in the path.
ALTER ROLE pulsars_migrate SET role = pulsars_owner;
ALTER ROLE pulsars_migrate SET search_path = game, site, contenu, langues;

ALTER ROLE pulsars_api SET search_path = site, contenu, game, langues;
ALTER ROLE pulsars_api SET statement_timeout = '5s';
ALTER ROLE pulsars_api SET idle_in_transaction_session_timeout = '30s';

COMMIT;
