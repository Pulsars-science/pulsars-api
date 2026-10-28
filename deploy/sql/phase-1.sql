-- Phase 1 : roles et schema `contenu` (plan « tout passe par l'API », D13).
--
-- A lancer UNE fois sur la base de production, en superutilisateur, juste
-- apres une sauvegarde complete :
--
--     sudo -u postgres pg_dump -Fc pulsars > ~/pulsars-avant-phase-1.dump
--     sudo -u postgres psql pulsars -v ON_ERROR_STOP=1 \
--         -v mdp_migrate="$(cat ~/pulsars-secrets/MDP_MIGRATE)" -f phase-1.sql
--
-- Relancer le script ne change rien. Retour arriere : phase-1-retour.sql.
--
-- Ce qu'il fait :
--   1. cree pulsars_owner (sans connexion), proprietaire de toute la
--      structure, et pulsars_migrate, le seul compte qui lance les
--      migrations (`pulsars-api migrate`) ;
--   2. deplace les tables de contenu de `site` vers `contenu` ; les vues
--      api_read les suivent (elles designent les tables par leur oid) ;
--   3. donne toutes les tables a pulsars_owner, puis a pulsars_site et
--      pulsars_api la lecture et l'ecriture qui leur reviennent, sans aucun
--      droit de creer, modifier ou supprimer une table.

\set ON_ERROR_STOP 1
BEGIN;

-- 1. Roles -----------------------------------------------------------------

SELECT 'CREATE ROLE pulsars_owner NOLOGIN'
 WHERE NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'pulsars_owner') \gexec

SELECT format('CREATE ROLE pulsars_migrate LOGIN PASSWORD %L IN ROLE pulsars_owner', :'mdp_migrate')
 WHERE NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'pulsars_migrate') \gexec

-- Ce que pulsars_migrate cree appartient a pulsars_owner.
ALTER ROLE pulsars_migrate SET role = pulsars_owner;
ALTER ROLE pulsars_migrate SET search_path = game, langues, contenu, site;
GRANT CONNECT ON DATABASE pulsars TO pulsars_migrate;
-- Une migration peut creer un schema (celle de la partie langues le fait).
GRANT CREATE ON DATABASE pulsars TO pulsars_owner;

-- 2. Le contenu quitte `site` ------------------------------------------------

CREATE SCHEMA IF NOT EXISTS contenu AUTHORIZATION pulsars_owner;

-- Les sequences liees a une colonne suivent leur table.
ALTER TABLE IF EXISTS site.courses              SET SCHEMA contenu;
ALTER TABLE IF EXISTS site.chapters             SET SCHEMA contenu;
ALTER TABLE IF EXISTS site.lessons              SET SCHEMA contenu;
ALTER TABLE IF EXISTS site.exercises            SET SCHEMA contenu;
ALTER TABLE IF EXISTS site.exams                SET SCHEMA contenu;
ALTER TABLE IF EXISTS site.exam_parties         SET SCHEMA contenu;
ALTER TABLE IF EXISTS site.ressources           SET SCHEMA contenu;
ALTER TABLE IF EXISTS site.actualites           SET SCHEMA contenu;
ALTER TABLE IF EXISTS site.contenu_translations SET SCHEMA contenu;
ALTER TABLE IF EXISTS site.fichiers             SET SCHEMA contenu;
ALTER TABLE IF EXISTS site.telechargements      SET SCHEMA contenu;

-- 3. Toute la structure a pulsars_owner ---------------------------------------

-- `langues` n'existe qu'une fois la partie langues de l'API deployee.
DO $$
DECLARE
    s text;
    r record;
BEGIN
    FOREACH s IN ARRAY ARRAY['site', 'contenu', 'game', 'langues'] LOOP
        IF EXISTS (SELECT FROM pg_namespace WHERE nspname = s) THEN
            EXECUTE format('ALTER SCHEMA %I OWNER TO pulsars_owner', s);
        END IF;
    END LOOP;

    FOR r IN
        SELECT n.nspname, c.relname, c.relkind
          FROM pg_class c
          JOIN pg_namespace n ON n.oid = c.relnamespace
         WHERE n.nspname IN ('site', 'contenu', 'game', 'langues')
           AND c.relkind IN ('r', 'p', 'S')
           -- Une sequence liee a une colonne change de proprietaire avec sa
           -- table ; PostgreSQL refuse qu'on la change seule.
           AND NOT (c.relkind = 'S' AND EXISTS (
                   SELECT FROM pg_depend d
                    WHERE d.objid = c.oid AND d.deptype IN ('a', 'i')
                      AND d.classid = 'pg_class'::regclass))
    LOOP
        EXECUTE format('ALTER %s %I.%I OWNER TO pulsars_owner',
                       CASE r.relkind WHEN 'S' THEN 'SEQUENCE' ELSE 'TABLE' END,
                       r.nspname, r.relname);
    END LOOP;
END $$;

-- Aucun role applicatif ne cree d'objet.
DO $$
DECLARE s text;
BEGIN
    FOREACH s IN ARRAY ARRAY['site', 'contenu', 'game', 'langues'] LOOP
        IF EXISTS (SELECT FROM pg_namespace WHERE nspname = s) THEN
            EXECUTE format('REVOKE CREATE ON SCHEMA %I FROM pulsars_site, pulsars_api', s);
        END IF;
    END LOOP;
END $$;

-- 4. pulsars_site : lecture et ecriture dans site et contenu (transition) -----

GRANT USAGE ON SCHEMA site, contenu TO pulsars_site;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA site, contenu TO pulsars_site;
GRANT USAGE, SELECT, UPDATE ON ALL SEQUENCES IN SCHEMA site, contenu TO pulsars_site;
ALTER DEFAULT PRIVILEGES FOR ROLE pulsars_owner IN SCHEMA site, contenu
    GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO pulsars_site;
ALTER DEFAULT PRIVILEGES FOR ROLE pulsars_owner IN SCHEMA site, contenu
    GRANT USAGE, SELECT, UPDATE ON SEQUENCES TO pulsars_site;

-- 5. pulsars_api : lecture du contenu, ecriture dans game et langues ---------
--    Toujours aucun acces a `site` (il arrive a la phase 2).

GRANT USAGE ON SCHEMA contenu TO pulsars_api;
GRANT SELECT ON ALL TABLES IN SCHEMA contenu TO pulsars_api;
ALTER DEFAULT PRIVILEGES FOR ROLE pulsars_owner IN SCHEMA contenu
    GRANT SELECT ON TABLES TO pulsars_api;

DO $$
DECLARE s text;
BEGIN
    FOREACH s IN ARRAY ARRAY['game', 'langues'] LOOP
        IF EXISTS (SELECT FROM pg_namespace WHERE nspname = s) THEN
            EXECUTE format('GRANT USAGE ON SCHEMA %I TO pulsars_api', s);
            EXECUTE format('GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA %I TO pulsars_api', s);
            EXECUTE format('GRANT USAGE, SELECT, UPDATE ON ALL SEQUENCES IN SCHEMA %I TO pulsars_api', s);
            EXECUTE format('ALTER DEFAULT PRIVILEGES FOR ROLE pulsars_owner IN SCHEMA %I
                                GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO pulsars_api', s);
            EXECUTE format('ALTER DEFAULT PRIVILEGES FOR ROLE pulsars_owner IN SCHEMA %I
                                GRANT USAGE, SELECT, UPDATE ON SEQUENCES TO pulsars_api', s);
        END IF;
    END LOOP;
END $$;

-- 6. Chemins de recherche -----------------------------------------------------

ALTER ROLE pulsars_site SET search_path = site, contenu;
-- api_read avant contenu : un nom sans schema tombe sur la vue filtree,
-- jamais sur la table brute (les requetes de l'API nomment leur schema).
ALTER ROLE pulsars_api  SET search_path = game, langues, api_read, contenu;

COMMIT;
