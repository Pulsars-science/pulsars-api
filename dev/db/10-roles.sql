-- Roles and schemas, as in the administrator block at the end of
-- docs/contract/views.sql. DEVELOPMENT PASSWORDS ONLY.

CREATE ROLE pulsars_site LOGIN PASSWORD 'pulsars_site_dev';
CREATE ROLE pulsars_api  LOGIN PASSWORD 'pulsars_api_dev';

REVOKE ALL ON DATABASE pulsars FROM PUBLIC;
REVOKE CREATE ON SCHEMA public FROM PUBLIC;
GRANT CONNECT ON DATABASE pulsars TO pulsars_site, pulsars_api;

CREATE SCHEMA site     AUTHORIZATION pulsars_site;
CREATE SCHEMA api_read AUTHORIZATION pulsars_site;
CREATE SCHEMA game     AUTHORIZATION pulsars_api;

REVOKE ALL ON SCHEMA site FROM pulsars_api;

ALTER DEFAULT PRIVILEGES FOR ROLE pulsars_site IN SCHEMA api_read
    GRANT SELECT ON TABLES TO pulsars_api;

ALTER ROLE pulsars_site SET search_path = site;
ALTER ROLE pulsars_api  SET search_path = game, api_read;
ALTER ROLE pulsars_api  SET statement_timeout = '5s';
ALTER ROLE pulsars_api  SET idle_in_transaction_session_timeout = '30s';
