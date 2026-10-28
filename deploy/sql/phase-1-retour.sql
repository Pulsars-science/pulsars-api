-- Retour arriere de phase-1.sql : remet tables, proprietaires et droits
-- comme avant. A lancer en superutilisateur :
--
--     sudo -u postgres psql pulsars -v ON_ERROR_STOP=1 -f phase-1-retour.sql
--
-- Les migrations creees entre-temps par pulsars_migrate (dans game ou
-- langues) reviennent a pulsars_api ; une table creee dans `contenu` revient
-- dans `site`, a pulsars_site. Relancer le script ne change rien.
--
-- Apres ce script, l'API doit retrouver sa version qui migre au demarrage
-- (`deploy-api --rollback`) : pulsars_api redevient proprietaire de game.

\set ON_ERROR_STOP 1
BEGIN;

-- 0. Ce que les migrations de l'API ont cree (contenu, site.auth_sessions) -----
--
-- Les tables de reference (migration 20261028000000_contenu_reference) n'ont
-- pas d'equivalent dans `site` : elles partent, avec leur ligne dans
-- game._sqlx_migrations, pour que `pulsars-api migrate` les recree apres un
-- nouveau phase-1.sql. La release d'avant ne les lit pas.

DROP TABLE IF EXISTS contenu.licences, contenu.matieres, contenu.sources_officielles;
DROP FUNCTION IF EXISTS contenu.slugify(text);
-- Les sessions et les liens envoyes par mail de l'API (migrations
-- 20261029000000_auth_sessions, 20261031000000_email_tokens) : le site
-- d'avant ne les connait pas. Les droits de pulsars_api sur site.users,
-- site.roles, site.role_user (20261030000000_auth_roles) et
-- site.comptes_sociaux partent a l'etape 3.
DROP TABLE IF EXISTS site.auth_sessions, site.email_tokens;
DO $$
BEGIN
    IF to_regclass('game._sqlx_migrations') IS NOT NULL THEN
        DELETE FROM game._sqlx_migrations
         WHERE version IN (20261028000000, 20261029000000, 20261030000000,
                           20261031000000);
    END IF;
END $$;

-- 1. Le contenu revient dans `site` ------------------------------------------

DO $$
DECLARE r record;
BEGIN
    IF EXISTS (SELECT FROM pg_namespace WHERE nspname = 'contenu') THEN
        FOR r IN SELECT tablename FROM pg_tables WHERE schemaname = 'contenu' LOOP
            EXECUTE format('ALTER TABLE contenu.%I SET SCHEMA site', r.tablename);
        END LOOP;
    END IF;
END $$;

-- 2. Proprietaires d'avant : site a pulsars_site, game et langues a pulsars_api

DO $$
DECLARE
    s text;
    proprietaire text;
    r record;
BEGIN
    FOREACH s IN ARRAY ARRAY['site', 'game', 'langues'] LOOP
        CONTINUE WHEN NOT EXISTS (SELECT FROM pg_namespace WHERE nspname = s);
        proprietaire := CASE s WHEN 'site' THEN 'pulsars_site' ELSE 'pulsars_api' END;
        EXECUTE format('ALTER SCHEMA %I OWNER TO %I', s, proprietaire);
        FOR r IN
            SELECT c.relname, c.relkind
              FROM pg_class c
              JOIN pg_namespace n ON n.oid = c.relnamespace
             WHERE n.nspname = s
               AND c.relkind IN ('r', 'p', 'S')
               AND NOT (c.relkind = 'S' AND EXISTS (
                       SELECT FROM pg_depend d
                        WHERE d.objid = c.oid AND d.deptype IN ('a', 'i')
                          AND d.classid = 'pg_class'::regclass))
        LOOP
            EXECUTE format('ALTER %s %I.%I OWNER TO %I',
                           CASE r.relkind WHEN 'S' THEN 'SEQUENCE' ELSE 'TABLE' END,
                           s, r.relname, proprietaire);
        END LOOP;
    END LOOP;
END $$;

-- 3. Droits : l'API perd tout acces a `site` (comme avant la phase 1) --------

REVOKE ALL ON ALL TABLES IN SCHEMA site FROM pulsars_api;
REVOKE ALL ON ALL SEQUENCES IN SCHEMA site FROM pulsars_api;
REVOKE ALL ON SCHEMA site FROM pulsars_api;

DROP SCHEMA IF EXISTS contenu;

ALTER ROLE pulsars_site SET search_path = site;
ALTER ROLE pulsars_api  SET search_path = game, api_read;

-- 4. Les roles de la phase 1 disparaissent -----------------------------------

DO $$
BEGIN
    IF EXISTS (SELECT FROM pg_roles WHERE rolname = 'pulsars_migrate') THEN
        EXECUTE 'DROP OWNED BY pulsars_migrate';
        EXECUTE 'DROP ROLE pulsars_migrate';
    END IF;
    IF EXISTS (SELECT FROM pg_roles WHERE rolname = 'pulsars_owner') THEN
        -- Ne lui reste que ses privileges par defaut : plus aucun objet.
        EXECUTE 'DROP OWNED BY pulsars_owner';
        EXECUTE 'DROP ROLE pulsars_owner';
    END IF;
END $$;

COMMIT;
