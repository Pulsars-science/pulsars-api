-- =============================================================================
-- Pulsars: read-only views for api.pulsars.science
-- =============================================================================
--
-- Target: PostgreSQL 16 or later.
--
-- This file creates the `api_read` schema: one view per resource exposed by
-- openapi.yaml, restricted to PUBLISHED rows and to ALLOWED columns. The API's
-- database role (`pulsars_api`) can read these views and nothing else of the
-- `site` schema.
--
-- How the isolation works: a PostgreSQL view runs with the privileges of its
-- OWNER (unless `security_invoker` is set, which must NOT be done here). The
-- views are owned by `pulsars_site`, which can read `site`; `pulsars_api` only
-- gets SELECT on the views. It has no privilege on any `site` table, so a
-- column that is not in a view does not exist for the API.
--
-- Who runs this file: a Laravel migration (see laravel-tasks.md), executed as
-- `pulsars_site`. The role creation block at the end needs a superuser (or a
-- role with CREATEROLE) and is run once, by hand, by whoever administers the
-- database. It is commented out so the migration never needs those rights.
--
-- Excluded on purpose, everywhere:
--   * exercises.solution_md, exercises.donnees->correctes,
--     exercises.donnees->acceptees, and the `solution_md` key of English
--     translations;
--   * exam_parties.corrige_md and its translation, exams.fichier_corrige;
--   * every personal column of `users` except pseudo, avatar, bio, created_at
--     (and cerveaux_publics, with the brains balance in member_brains);
--   * storage paths, original file names, AI prompts and errors, counters,
--     author account ids.
--
-- Values copied from PHP (keep in sync, a Laravel test must compare them):
--   * Experience::BAREME            -> api_read.member_xp
--   * config/licences.php           -> api_read.licenses
--   * SujetOfficiel::SOURCES        -> api_read.exams.official_url
--   * lang/en.json subject names    -> api_read.subject_names
--
-- Laravel stores timestamps as `timestamp(0) without time zone` in UTC
-- (config/app.php timezone = UTC). The views convert them to `timestamptz`
-- with `AT TIME ZONE 'UTC'` so the API never has to guess.
-- =============================================================================

-- The `api_read` schema itself is created once by the administrator (see the
-- block at the end of this file). Even `CREATE SCHEMA IF NOT EXISTS` requires
-- the CREATE privilege on the database, which `pulsars_site` does not have.

-- -----------------------------------------------------------------------------
-- Helpers
-- -----------------------------------------------------------------------------

-- Same result as Laravel's Str::slug() for the French and English strings
-- found in course themes ("Mathématiques" -> "mathematiques",
-- "La matière" -> "la-matiere"). Written without the `unaccent` extension so
-- the migration needs no superuser.
CREATE OR REPLACE FUNCTION api_read.slugify(value text)
RETURNS text
LANGUAGE sql
IMMUTABLE
PARALLEL SAFE
AS $$
    SELECT trim(BOTH '-' FROM regexp_replace(
        lower(translate(
            replace(replace(replace(replace(value, 'œ', 'oe'), 'Œ', 'oe'), 'æ', 'ae'), 'Æ', 'ae'),
            'ÀÂÄÁÃÅàâäáãåÇçÉÈÊËéèêëÍÌÎÏíìîïÑñÓÒÔÖÕóòôöõÚÙÛÜúùûüÝŸýÿ',
            'AAAAAAaaaaaaCcEEEEeeeeIIIIiiiiNnOOOOOoooooUUUUuuuuYYyy'
        )),
        '[^a-z0-9]+', '-', 'g'
    ))
$$;

-- -----------------------------------------------------------------------------
-- Static reference data copied from PHP
-- -----------------------------------------------------------------------------

-- config/licences.php
CREATE OR REPLACE VIEW api_read.licenses (key, name, url) AS
VALUES
    ('cc-by-4.0',            'CC BY 4.0',                'https://creativecommons.org/licenses/by/4.0/'),
    ('cc-by-sa-4.0',         'CC BY-SA 4.0',             'https://creativecommons.org/licenses/by-sa/4.0/'),
    ('cc-by-nc-4.0',         'CC BY-NC 4.0',             'https://creativecommons.org/licenses/by-nc/4.0/'),
    ('cc-by-nc-sa-4.0',      'CC BY-NC-SA 4.0',          'https://creativecommons.org/licenses/by-nc-sa/4.0/'),
    ('cc-by-nd-4.0',         'CC BY-ND 4.0',             'https://creativecommons.org/licenses/by-nd/4.0/'),
    ('cc0-1.0',              'CC0 1.0 (domaine public)', 'https://creativecommons.org/publicdomain/zero/1.0/'),
    ('domaine-public',       'Domaine public',           NULL),
    ('tous-droits-reserves', 'Tous droits réservés',     NULL),
    ('autre',                'Autre (à préciser)',       NULL);

-- English names of course subjects, from lang/en.json. Topics have none.
-- "Biologie" has no entry in lang/en.json; "Biology" is added here and should
-- be added there too.
CREATE OR REPLACE VIEW api_read.subject_names (subject_slug, lang, name) AS
VALUES
    ('mathematiques', 'en', 'Mathematics'),
    ('physique',      'en', 'Physics'),
    ('chimie',        'en', 'Chemistry'),
    ('biologie',      'en', 'Biology'),
    ('informatique',  'en', 'Computer science'),
    ('cryptographie', 'en', 'Cryptography');

-- -----------------------------------------------------------------------------
-- Courses
-- -----------------------------------------------------------------------------

CREATE OR REPLACE VIEW api_read.courses AS
SELECT
    c.id,
    c.slug,
    c.titre                                              AS title,
    c.description,
    c.theme,
    -- "Mathématiques – Analyse" -> subject "Mathématiques", topic "Analyse".
    -- The separator is an en dash (U+2013) between two spaces.
    split_part(c.theme, ' – ', 1)                        AS subject_name,
    api_read.slugify(split_part(c.theme, ' – ', 1))      AS subject_slug,
    nullif(split_part(c.theme, ' – ', 2), '')            AS topic_name,
    nullif(api_read.slugify(split_part(c.theme, ' – ', 2)), '') AS topic_slug,
    c.difficulte                                         AS difficulty,
    -- Levels only exist as free tags, with and without accents.
    ARRAY(
        SELECT DISTINCT CASE
            WHEN t IN ('collège', 'college')  THEN 'college'
            WHEN t IN ('lycée', 'lycee')      THEN 'lycee'
            WHEN t IN ('supérieur', 'superieur') THEN 'superieur'
        END
        FROM jsonb_array_elements_text(coalesce(c.tags::jsonb, '[]'::jsonb)) AS t
        WHERE t IN ('collège', 'college', 'lycée', 'lycee', 'supérieur', 'superieur')
        ORDER BY 1
    )                                                    AS levels,
    c.duree_estimee                                      AS duration_minutes,
    coalesce(c.tags::jsonb, '[]'::jsonb)                 AS tags,
    u.pseudo                                             AS author_pseudo,
    (SELECT count(*) FROM site.chapters ch WHERE ch.course_id = c.id)::int AS chapter_count,
    (SELECT count(*) FROM site.lessons l JOIN site.chapters ch ON ch.id = l.chapter_id
      WHERE ch.course_id = c.id)::int                    AS lesson_count,
    (SELECT count(*) FROM site.exercises e JOIN site.chapters ch ON ch.id = e.chapter_id
      WHERE ch.course_id = c.id)::int                    AS exercise_count,
    c.published_at AT TIME ZONE 'UTC'                    AS published_at,
    c.updated_at AT TIME ZONE 'UTC'                      AS updated_at
FROM site.courses c
LEFT JOIN site.users u ON u.id = c.auteur_id
-- Same rule as Course::scopePublies().
WHERE c.statut = 'publie'
  AND c.published_at IS NOT NULL
  AND c.published_at <= (now() AT TIME ZONE 'UTC');

CREATE OR REPLACE VIEW api_read.course_translations AS
SELECT
    t.traduisible_id                AS course_id,
    t.langue                        AS lang,
    nullif(t.champs::jsonb ->> 'titre', '')       AS title,
    nullif(t.champs::jsonb ->> 'description', '') AS description
FROM site.contenu_translations t
JOIN api_read.courses c ON c.id = t.traduisible_id
WHERE t.traduisible_type = 'course';

CREATE OR REPLACE VIEW api_read.chapters AS
SELECT
    ch.id,
    ch.course_id,
    ch.ordre AS position,
    ch.titre AS title
FROM site.chapters ch
JOIN api_read.courses c ON c.id = ch.course_id;

CREATE OR REPLACE VIEW api_read.chapter_translations AS
SELECT
    t.traduisible_id                        AS chapter_id,
    t.langue                                AS lang,
    nullif(t.champs::jsonb ->> 'titre', '') AS title
FROM site.contenu_translations t
JOIN api_read.chapters ch ON ch.id = t.traduisible_id
WHERE t.traduisible_type = 'chapter';

CREATE OR REPLACE VIEW api_read.lessons AS
SELECT
    l.id,
    l.chapter_id,
    ch.course_id,
    l.slug,
    l.ordre      AS position,
    ch.position  AS chapter_position,
    l.titre      AS title,
    l.contenu_md AS content_md,
    l.updated_at AT TIME ZONE 'UTC' AS updated_at
FROM site.lessons l
JOIN api_read.chapters ch ON ch.id = l.chapter_id;

CREATE OR REPLACE VIEW api_read.lesson_translations AS
SELECT
    t.traduisible_id                             AS lesson_id,
    t.langue                                     AS lang,
    nullif(t.champs::jsonb ->> 'titre', '')      AS title,
    nullif(t.champs::jsonb ->> 'contenu_md', '') AS content_md
FROM site.contenu_translations t
JOIN api_read.lessons l ON l.id = t.traduisible_id
WHERE t.traduisible_type = 'lesson';

-- -----------------------------------------------------------------------------
-- Exercises: never the answer
-- -----------------------------------------------------------------------------

CREATE OR REPLACE VIEW api_read.exercises AS
SELECT
    e.id,
    e.chapter_id,
    ch.course_id,
    e.type,
    e.titre       AS title,
    e.question_md,
    e.ordre       AS position,
    ch.position   AS chapter_position,
    -- QCM options without `correctes`, same as Exercise::donneesPubliques().
    CASE WHEN e.type = 'qcm' THEN (
        SELECT coalesce(
            jsonb_agg(jsonb_build_object('id', o ->> 'id', 'text', o ->> 'texte') ORDER BY n),
            '[]'::jsonb)
        FROM jsonb_array_elements(
            CASE WHEN jsonb_typeof(e.donnees::jsonb -> 'options') = 'array'
                 THEN e.donnees::jsonb -> 'options' ELSE '[]'::jsonb END
        ) WITH ORDINALITY AS x(o, n)
    ) END         AS options,
    -- Checkbox vs radio. Already shown by the website; a deliberate hint.
    CASE WHEN e.type = 'qcm' THEN
        CASE WHEN jsonb_typeof(e.donnees::jsonb -> 'correctes') = 'array'
             THEN jsonb_array_length(e.donnees::jsonb -> 'correctes') > 1
             ELSE false END
    END           AS multiple,
    e.updated_at AT TIME ZONE 'UTC' AS updated_at
FROM site.exercises e
JOIN api_read.chapters ch ON ch.id = e.chapter_id;

CREATE OR REPLACE VIEW api_read.exercise_translations AS
SELECT
    t.traduisible_id                              AS exercise_id,
    t.langue                                      AS lang,
    nullif(t.champs::jsonb ->> 'titre', '')       AS title,
    nullif(t.champs::jsonb ->> 'question_md', '') AS question_md,
    -- `solution_md` is in the same JSON and is deliberately not selected.
    -- QCM options: the translation stores `{"a": "...", "b": "..."}` under
    -- `options`; served like api_read.exercises.options (same ids, same
    -- order), falling back option by option to the original text. The
    -- answers stay in `donnees`, never read here.
    CASE WHEN e.options IS NOT NULL
          AND jsonb_typeof(t.champs::jsonb -> 'options') = 'object' THEN (
        SELECT coalesce(jsonb_agg(jsonb_build_object(
                   'id', o ->> 'id',
                   'text', coalesce(nullif(t.champs::jsonb -> 'options' ->> (o ->> 'id'), ''), o ->> 'text')
               ) ORDER BY n), '[]'::jsonb)
        FROM jsonb_array_elements(e.options) WITH ORDINALITY AS x(o, n)
    ) END                                         AS options
FROM site.contenu_translations t
JOIN api_read.exercises e ON e.id = t.traduisible_id
WHERE t.traduisible_type = 'exercise';

-- -----------------------------------------------------------------------------
-- Exams: statements only
-- -----------------------------------------------------------------------------

CREATE OR REPLACE VIEW api_read.exams AS
SELECT
    x.id,
    x.slug,
    x.titre            AS title,
    x.type,
    x.concours         AS competition,
    x.matiere          AS subject,
    x.pays             AS country,
    x.niveau           AS level,
    coalesce(x.filieres::jsonb, '[]'::jsonb) AS tracks,
    x.session,
    x.duree_minutes    AS duration_minutes,
    x.points_total     AS total_points,
    x.description,
    (x.fichier_pdf IS NOT NULL) AS has_pdf,
    -- Same allow-list as App\Support\SujetOfficiel::estAutorisee().
    CASE WHEN trim(x.url_officielle) ~* '^https://www\.concours-centrale-supelec\.fr/sites/default/files/documents/[^?#]*\.pdf([?#].*)?$'
          AND position('..' IN x.url_officielle) = 0
         THEN trim(x.url_officielle) END AS official_url,
    x.url_source       AS source_url,
    (SELECT count(*) FROM site.exam_parties p WHERE p.exam_id = x.id)::int AS part_count,
    x.published_at AT TIME ZONE 'UTC' AS published_at,
    x.updated_at   AT TIME ZONE 'UTC' AS updated_at
FROM site.exams x
-- Same rule as Exam::scopePublies(). `attente_import` and `brouillon`
-- (AI import not yet reviewed by a human) are excluded.
WHERE x.statut = 'publie'
  AND x.published_at IS NOT NULL
  AND x.published_at <= (now() AT TIME ZONE 'UTC');

CREATE OR REPLACE VIEW api_read.exam_parts AS
SELECT
    p.id,
    p.exam_id,
    p.ordre     AS position,
    p.titre     AS title,
    p.enonce_md AS statement_md,
    p.points
    -- `corrige_md` is deliberately not selected.
FROM site.exam_parties p
JOIN api_read.exams x ON x.id = p.exam_id;

CREATE OR REPLACE VIEW api_read.exam_part_translations AS
SELECT
    t.traduisible_id                            AS exam_part_id,
    t.langue                                    AS lang,
    nullif(t.champs::jsonb ->> 'titre', '')     AS title,
    nullif(t.champs::jsonb ->> 'enonce_md', '') AS statement_md
    -- `corrige_md` is in the same JSON and is deliberately not selected.
FROM site.contenu_translations t
JOIN api_read.exam_parts p ON p.id = t.traduisible_id
WHERE t.traduisible_type = 'exam_partie';

-- -----------------------------------------------------------------------------
-- Resources
-- -----------------------------------------------------------------------------

CREATE OR REPLACE VIEW api_read.resources AS
SELECT
    r.id,
    r.slug,
    r.titre       AS title,
    r.description,
    r.categorie   AS category,
    r.langue      AS doc_lang,
    -- Ressource::estLien(): a link only when nothing is hosted here.
    CASE WHEN r.fichier IS NULL AND r.lien IS NOT NULL THEN 'link' ELSE 'host' END AS mode,
    r.auteur_nom  AS attribution,
    r.licence     AS license_key,
    -- Free text, only meaningful when license_key = 'autre'.
    r.licence_personnalisee AS license_custom,
    r.lien        AS source_url,
    CASE WHEN r.fichier IS NOT NULL THEN r.taille END AS size_bytes,
    r.created_at AT TIME ZONE 'UTC' AS created_at,
    r.updated_at AT TIME ZONE 'UTC' AS updated_at
FROM site.ressources r
-- Same rule as Ressource::scopePubliees(). A resource that is neither hosted
-- nor linked has nothing to offer and is left out.
WHERE r.statut = 'publie'
  AND (r.fichier IS NOT NULL OR r.lien IS NOT NULL);

CREATE OR REPLACE VIEW api_read.resource_translations AS
SELECT
    t.traduisible_id                              AS resource_id,
    t.langue                                      AS lang,
    nullif(t.champs::jsonb ->> 'titre', '')       AS title,
    nullif(t.champs::jsonb ->> 'description', '') AS description,
    nullif(t.champs::jsonb ->> 'categorie', '')   AS category
FROM site.contenu_translations t
JOIN api_read.resources r ON r.id = t.traduisible_id
WHERE t.traduisible_type = 'ressource';

-- -----------------------------------------------------------------------------
-- Members: public profile only
-- -----------------------------------------------------------------------------

-- What /membres/{pseudo} shows to anyone, minus school level and class.
-- Never: email, name, password, 2FA, remember_token, objectif,
-- niveau_scolaire, classe, locale, friend_token, onboarding_at, mute_*,
-- email_verified_at.
CREATE OR REPLACE VIEW api_read.members AS
SELECT
    u.id,
    u.pseudo,
    u.avatar     AS avatar_path,   -- relative to the `public` disk
    u.bio,
    u.created_at AT TIME ZONE 'UTC' AS member_since
FROM site.users u;

CREATE OR REPLACE VIEW api_read.member_roles AS
SELECT
    ru.user_id,
    r.slug,
    r.nom       AS name,
    r.categorie AS category,
    r.couleur   AS color,
    r.niveau    AS permission_level,
    r.ordre     AS display_order
FROM site.role_user ru
JOIN site.roles r ON r.id = ru.role_id;

-- XP, computed exactly like App\Services\Experience::pour(). The points per
-- action are Experience::BAREME and MUST be changed together with it.
-- Correlated subqueries rather than GROUP BY: the API always filters on one
-- `user_id`, and each subquery then uses the (user_id, ...) indexes.
CREATE OR REPLACE VIEW api_read.member_xp AS
WITH counts AS (
    SELECT
        u.id AS user_id,
        (SELECT count(*) FROM site.lesson_progress lp
          WHERE lp.user_id = u.id AND lp.completed_at IS NOT NULL)          AS lessons,
        (SELECT count(DISTINCT ea.exercise_id) FROM site.exercise_attempts ea
          WHERE ea.user_id = u.id AND ea.correct = true)                     AS exercises,
        (SELECT count(*) FROM site.enrollments en
          WHERE en.user_id = u.id AND en.completed_at IS NOT NULL)          AS courses,
        (SELECT count(*) FROM site.questions q WHERE q.user_id = u.id)       AS questions,
        (SELECT count(*) FROM site.answers a WHERE a.user_id = u.id)         AS answers,
        (SELECT count(*) FROM site.answers a
           JOIN site.questions q ON q.accepted_answer_id = a.id
          WHERE a.user_id = u.id)                                            AS accepted_answers,
        -- Game matches (webhook results): 3 per match, 5 more per win,
        -- at most 30 per UTC day (game-design.md Q9).
        (SELECT coalesce(sum(least(30, parties * 3 + victoires * 5)), 0) FROM (
            SELECT count(*) AS parties,
                   count(*) FILTER (WHERE gr.outcome = 'win') AS victoires
              FROM site.game_results gr
             WHERE gr.user_id = u.id
             GROUP BY gr.ended_at::date) j)                                  AS game_xp
    FROM site.users u
), totals AS (
    SELECT
        user_id,
        (lessons * 10 + exercises * 8 + courses * 40
         + questions * 4 + answers * 4 + accepted_answers * 12
         + game_xp)::int AS total
    FROM counts
), levels AS (
    -- Level n starts at 50 * (n - 1) * n; n = floor((1 + sqrt(1 + 4 * xp / 50)) / 2).
    -- The two corrections guard against floating-point error at exact thresholds.
    SELECT user_id, total,
           floor((1 + sqrt(1 + total * 0.08)) / 2)::int AS n
    FROM totals
)
SELECT
    user_id,
    total,
    CASE
        WHEN 50 * n * (n + 1) <= total THEN n + 1
        WHEN 50 * (n - 1) * n > total  THEN n - 1
        ELSE n
    END AS level
FROM levels;

-- Brains balance (docs/contract/brains.md §4): the sum of the member's
-- ledger movements. `public` is `users.cerveaux_publics`; the API shows the
-- balance to its owner only (/v1/me, /v1/game/me), never on public endpoints.
CREATE OR REPLACE VIEW api_read.member_brains AS
SELECT u.id AS user_id,
       coalesce((SELECT sum(m.montant) FROM site.cerveaux_mouvements m
                  WHERE m.user_id = u.id), 0)::int AS balance,
       u.cerveaux_publics AS public
FROM site.users u;

-- -----------------------------------------------------------------------------
-- Grants
-- -----------------------------------------------------------------------------

GRANT USAGE ON SCHEMA api_read TO pulsars_api;
GRANT SELECT ON ALL TABLES IN SCHEMA api_read TO pulsars_api;
GRANT EXECUTE ON FUNCTION api_read.slugify(text) TO pulsars_api;

-- =============================================================================
-- One-time setup, run by a database administrator (NOT by the migration).
-- Kept here so the whole permission model is in one file.
-- =============================================================================
--
-- CREATE ROLE pulsars_site LOGIN PASSWORD '...';   -- Laravel
-- CREATE ROLE pulsars_api  LOGIN PASSWORD '...';   -- Rust API
--
-- -- Nobody gets anything by default.
-- REVOKE ALL ON DATABASE pulsars FROM PUBLIC;
-- REVOKE CREATE ON SCHEMA public FROM PUBLIC;
-- GRANT CONNECT ON DATABASE pulsars TO pulsars_site, pulsars_api;
--
-- -- site: owned and written by Laravel only.
-- CREATE SCHEMA site AUTHORIZATION pulsars_site;
--
-- -- api_read: owned by Laravel (it owns the views), read by the API.
-- CREATE SCHEMA api_read AUTHORIZATION pulsars_site;
--
-- -- game: owned and written by the API only. Laravel gets nothing: it reads
-- -- game data through /v1/game (architecture.md).
-- CREATE SCHEMA game AUTHORIZATION pulsars_api;
--
-- -- Explicitly: the API cannot touch site.
-- REVOKE ALL ON SCHEMA site FROM pulsars_api;
-- REVOKE ALL ON ALL TABLES IN SCHEMA site FROM pulsars_api;
--
-- -- Views created later by Laravel migrations are readable by the API.
-- ALTER DEFAULT PRIVILEGES FOR ROLE pulsars_site IN SCHEMA api_read
--     GRANT SELECT ON TABLES TO pulsars_api;
--
-- -- Each role finds its own schema first.
-- ALTER ROLE pulsars_site SET search_path = site;
-- ALTER ROLE pulsars_api  SET search_path = game, api_read;
--
-- -- Safety nets for the API role.
-- ALTER ROLE pulsars_api SET statement_timeout = '5s';
-- ALTER ROLE pulsars_api SET idle_in_transaction_session_timeout = '30s';
-- ALTER ROLE pulsars_api CONNECTION LIMIT 40;
