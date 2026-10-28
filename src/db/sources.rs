//! What the API may read of `contenu`: one macro per resource, each a
//! parenthesized subquery to put after `FROM` or `JOIN`.
//!
//! This is the security boundary of the public content: each macro keeps
//! PUBLISHED rows only and selects ALLOWED columns only. Never selected
//! by these, whatever route uses them:
//!
//! - `exercises.solution_md`, `exercises.donnees` (its `correctes` and
//!   `acceptees`), and the `solution_md` key of translations;
//! - `exam_parties.corrige_md` and its translation, `exams.fichier_corrige`;
//! - storage paths, original file names, AI prompts and errors, counters,
//!   author account ids.
//!
//! The answers and corrections have their own macros, at the end
//! ([`ANSWER_SOURCES`]): published content only too, read by the member
//! routes that apply Laravel's rules on when to show them (an exercise's
//! solution after an attempt, an exam's corrections on its page), never by
//! a content route.
//!
//! Laravel stores timestamps as `timestamp(0) without time zone` in UTC;
//! `AT TIME ZONE 'UTC'` makes them `timestamptz`.

/// Same rule as `Course::scopePublies()`, on alias `c`.
macro_rules! course_is_published {
    () => {
        "c.statut = 'publie'
          AND c.published_at IS NOT NULL
          AND c.published_at <= (now() AT TIME ZONE 'UTC')"
    };
}

/// Same rule as `Exam::scopePublies()`, on alias `x`. `attente_import` and
/// `brouillon` (AI import not yet reviewed by a human) are left out.
macro_rules! exam_is_published {
    () => {
        "x.statut = 'publie'
          AND x.published_at IS NOT NULL
          AND x.published_at <= (now() AT TIME ZONE 'UTC')"
    };
}

/// Published courses. "Mathématiques – Analyse" is subject "Mathématiques",
/// topic "Analyse" (an en dash between two spaces); levels only exist as free
/// tags, with and without accents.
macro_rules! published_courses {
    () => {
        concat!(
            "(SELECT c.id, c.slug, c.titre AS title, c.description, c.theme,
                split_part(c.theme, ' – ', 1) AS subject_name,
                contenu.slugify(split_part(c.theme, ' – ', 1)) AS subject_slug,
                nullif(split_part(c.theme, ' – ', 2), '') AS topic_name,
                nullif(contenu.slugify(split_part(c.theme, ' – ', 2)), '') AS topic_slug,
                c.difficulte AS difficulty,
                ARRAY(
                    SELECT DISTINCT CASE
                        WHEN tag IN ('collège', 'college') THEN 'college'
                        WHEN tag IN ('lycée', 'lycee') THEN 'lycee'
                        WHEN tag IN ('supérieur', 'superieur') THEN 'superieur'
                    END
                    FROM jsonb_array_elements_text(coalesce(c.tags::jsonb, '[]'::jsonb)) AS tag
                    WHERE tag IN ('collège', 'college', 'lycée', 'lycee', 'supérieur', 'superieur')
                    ORDER BY 1
                ) AS levels,
                c.duree_estimee AS duration_minutes,
                coalesce(c.tags::jsonb, '[]'::jsonb) AS tags,
                u.pseudo AS author_pseudo,
                (SELECT count(*) FROM contenu.chapters ch WHERE ch.course_id = c.id)::int
                    AS chapter_count,
                (SELECT count(*) FROM contenu.lessons l
                   JOIN contenu.chapters ch ON ch.id = l.chapter_id
                  WHERE ch.course_id = c.id)::int AS lesson_count,
                (SELECT count(*) FROM contenu.exercises e
                   JOIN contenu.chapters ch ON ch.id = e.chapter_id
                  WHERE ch.course_id = c.id)::int AS exercise_count,
                c.published_at AT TIME ZONE 'UTC' AS published_at,
                c.updated_at AT TIME ZONE 'UTC' AS updated_at
           FROM contenu.courses c
           LEFT JOIN site.members u ON u.id = c.auteur_id
          WHERE ",
            course_is_published!(),
            ")"
        )
    };
}

/// Chapters of published courses.
macro_rules! published_chapters {
    () => {
        concat!(
            "(SELECT ch.id, ch.course_id, ch.ordre AS position, ch.titre AS title
           FROM contenu.chapters ch
           JOIN contenu.courses c ON c.id = ch.course_id
          WHERE ",
            course_is_published!(),
            ")"
        )
    };
}

/// Lessons of published courses, with their chapter's position.
macro_rules! published_lessons {
    () => {
        concat!(
            "(SELECT l.id, l.chapter_id, ch.course_id, l.slug, l.ordre AS position,
                ch.ordre AS chapter_position, l.titre AS title, l.contenu_md AS content_md,
                l.updated_at AT TIME ZONE 'UTC' AS updated_at
           FROM contenu.lessons l
           JOIN contenu.chapters ch ON ch.id = l.chapter_id
           JOIN contenu.courses c ON c.id = ch.course_id
          WHERE ",
            course_is_published!(),
            ")"
        )
    };
}

/// Exercises of published courses: never the answer. QCM options without
/// `correctes`, as `Exercise::donneesPubliques()`; `multiple` (checkbox or
/// radio) is a deliberate hint, the website shows it too.
macro_rules! published_exercises {
    () => {
        concat!(
            "(SELECT e.id, e.chapter_id, ch.course_id, e.type, e.titre AS title,
                e.question_md, e.ordre AS position, ch.ordre AS chapter_position,
                CASE WHEN e.type = 'qcm' THEN (
                    SELECT coalesce(
                        jsonb_agg(jsonb_build_object('id', o ->> 'id', 'text', o ->> 'texte')
                                  ORDER BY n),
                        '[]'::jsonb)
                    FROM jsonb_array_elements(
                        CASE WHEN jsonb_typeof(e.donnees::jsonb -> 'options') = 'array'
                             THEN e.donnees::jsonb -> 'options' ELSE '[]'::jsonb END
                    ) WITH ORDINALITY AS opt(o, n)
                ) END AS options,
                CASE WHEN e.type = 'qcm' THEN
                    CASE WHEN jsonb_typeof(e.donnees::jsonb -> 'correctes') = 'array'
                         THEN jsonb_array_length(e.donnees::jsonb -> 'correctes') > 1
                         ELSE false END
                END AS multiple,
                e.updated_at AT TIME ZONE 'UTC' AS updated_at
           FROM contenu.exercises e
           JOIN contenu.chapters ch ON ch.id = e.chapter_id
           JOIN contenu.courses c ON c.id = ch.course_id
          WHERE ",
            course_is_published!(),
            ")"
        )
    };
}

/// Published exam papers: statements only. `official_url` is the official
/// URL when `contenu.sources_officielles` allows it (`https`, known host,
/// path prefix, `.pdf`, no `..`), as `SujetOfficiel::estAutorisee()`.
macro_rules! published_exams {
    () => {
        concat!(
            "(SELECT x.id, x.slug, x.titre AS title, x.type, x.concours AS competition,
                x.matiere AS subject, x.pays AS country, x.niveau AS level,
                coalesce(x.filieres::jsonb, '[]'::jsonb) AS tracks, x.session,
                x.duree_minutes AS duration_minutes, x.points_total AS total_points,
                x.description, (x.fichier_pdf IS NOT NULL) AS has_pdf,
                CASE WHEN lower(u.chemin) LIKE '%.pdf'
                          AND position('..' IN u.chemin) = 0
                          AND EXISTS (SELECT 1 FROM contenu.sources_officielles s
                                       WHERE s.hote = u.hote
                                         AND starts_with(u.chemin, s.prefixe))
                     THEN o.url END AS official_url,
                x.url_source AS source_url,
                (SELECT count(*) FROM contenu.exam_parties p WHERE p.exam_id = x.id)::int
                    AS part_count,
                x.published_at AT TIME ZONE 'UTC' AS published_at,
                x.updated_at AT TIME ZONE 'UTC' AS updated_at
           FROM contenu.exams x
           CROSS JOIN LATERAL (SELECT btrim(x.url_officielle, E' \\t\\n\\r\\x0B') AS url) o
           CROSS JOIN LATERAL (
               SELECT lower(substring(o.url FROM '^https://([^/?#]*)')) AS hote,
                      substring(o.url FROM '^https://[^/?#]*([^?#]*)') AS chemin) u
          WHERE ",
            exam_is_published!(),
            ")"
        )
    };
}

/// Parts of published exams: never `corrige_md`.
macro_rules! published_exam_parts {
    () => {
        concat!(
            "(SELECT p.id, p.exam_id, p.ordre AS position, p.titre AS title,
                p.enonce_md AS statement_md, p.points
           FROM contenu.exam_parties p
           JOIN contenu.exams x ON x.id = p.exam_id
          WHERE ",
            exam_is_published!(),
            ")"
        )
    };
}

/// The statement PDF of published exams, from `contenu.fichiers`: never the
/// correction (`fichier_corrige`), never a draft's file, never the path.
macro_rules! exam_statement_files {
    () => {
        concat!(
            "(SELECT x.slug, f.id AS file_id, octet_length(f.contenu)::bigint AS size,
                f.type_mime::text AS mime,
                coalesce(f.updated_at, f.created_at) AT TIME ZONE 'UTC' AS updated_at
           FROM contenu.exams x
           JOIN contenu.fichiers f ON f.disque = 'local' AND f.chemin = x.fichier_pdf
          WHERE ",
            exam_is_published!(),
            ")"
        )
    };
}

/// The hosted file of published resources, from `contenu.fichiers`.
macro_rules! resource_files {
    () => {
        "(SELECT r.slug, f.id AS file_id, octet_length(f.contenu)::bigint AS size,
                f.type_mime::text AS mime,
                coalesce(f.updated_at, f.created_at) AT TIME ZONE 'UTC' AS updated_at
           FROM contenu.ressources r
           JOIN contenu.fichiers f ON f.disque = 'local' AND f.chemin = r.fichier
          WHERE r.statut = 'publie')"
    };
}

/// Published resources, as `Ressource::scopePubliees()`. One that is neither
/// hosted nor linked has nothing to offer and is left out; a link only when
/// nothing is hosted here (`Ressource::estLien()`).
macro_rules! published_resources {
    () => {
        "(SELECT r.id, r.slug, r.titre AS title, r.description, r.categorie AS category,
                r.langue AS doc_lang,
                CASE WHEN r.fichier IS NULL AND r.lien IS NOT NULL THEN 'link' ELSE 'host' END
                    AS mode,
                r.auteur_nom AS attribution, r.licence AS license_key,
                r.licence_personnalisee AS license_custom, r.lien AS source_url,
                CASE WHEN r.fichier IS NOT NULL THEN r.taille END AS size_bytes,
                r.created_at AT TIME ZONE 'UTC' AS created_at,
                r.updated_at AT TIME ZONE 'UTC' AS updated_at
           FROM contenu.ressources r
          WHERE r.statut = 'publie'
            AND (r.fichier IS NOT NULL OR r.lien IS NOT NULL))"
    };
}

/// Published news (`Actualite::scopePubliees()`): visibility rests on the
/// publication date alone; null is a draft, future is scheduled.
macro_rules! published_news {
    () => {
        "(SELECT a.id, a.slug, a.titre AS title, a.chapeau AS summary,
                a.contenu_md AS content_md, u.pseudo AS author_pseudo,
                a.published_at AT TIME ZONE 'UTC' AS published_at,
                a.updated_at AT TIME ZONE 'UTC' AS updated_at
           FROM contenu.actualites a
           LEFT JOIN site.members u ON u.id = a.auteur_id
          WHERE a.published_at IS NOT NULL
            AND a.published_at <= (now() AT TIME ZONE 'UTC'))"
    };
}

/// Translations of one kind of content (`traduisible_type`), restricted to
/// the rows of `$parent` (a `published_*!()` subquery). `$fields` are the
/// translated columns, read from the `champs` JSON (`t.f`); never
/// `solution_md` nor `corrige_md`.
macro_rules! translations {
    ($kind:literal, $id:literal, $parent:expr, $fields:literal) => {
        concat!(
            "(SELECT t.traduisible_id AS ",
            $id,
            ", t.langue AS lang, ",
            $fields,
            "
           FROM contenu.contenu_translations t
           CROSS JOIN LATERAL (SELECT t.champs::jsonb AS champs) f
          WHERE t.traduisible_type = '",
            $kind,
            "'
            AND t.traduisible_id IN (SELECT id FROM ",
            $parent,
            " parent))"
        )
    };
}

macro_rules! course_translations {
    () => {
        translations!(
            "course",
            "course_id",
            published_courses!(),
            "nullif(f.champs ->> 'titre', '') AS title,
             nullif(f.champs ->> 'description', '') AS description"
        )
    };
}

macro_rules! chapter_translations {
    () => {
        translations!(
            "chapter",
            "chapter_id",
            published_chapters!(),
            "nullif(f.champs ->> 'titre', '') AS title"
        )
    };
}

macro_rules! lesson_translations {
    () => {
        translations!(
            "lesson",
            "lesson_id",
            published_lessons!(),
            "nullif(f.champs ->> 'titre', '') AS title,
             nullif(f.champs ->> 'contenu_md', '') AS content_md"
        )
    };
}

/// QCM options: the translation stores `{"a": "...", "b": "..."}` under
/// `options`; served like the exercise's options (same ids, same order),
/// falling back option by option to the original text.
macro_rules! exercise_translations {
    () => {
        concat!(
            "(SELECT t.exercise_id, t.lang, t.title, t.question_md,
                CASE WHEN e.options IS NOT NULL
                      AND jsonb_typeof(t.champs -> 'options') = 'object' THEN (
                    SELECT coalesce(jsonb_agg(jsonb_build_object(
                               'id', o ->> 'id',
                               'text', coalesce(nullif(t.champs -> 'options' ->> (o ->> 'id'), ''),
                                                o ->> 'text')
                           ) ORDER BY n), '[]'::jsonb)
                    FROM jsonb_array_elements(e.options) WITH ORDINALITY AS opt(o, n)
                ) END AS options
           FROM ",
            translations!(
                "exercise",
                "exercise_id",
                published_exercises!(),
                "nullif(f.champs ->> 'titre', '') AS title,
                 nullif(f.champs ->> 'question_md', '') AS question_md,
                 f.champs"
            ),
            " t
           JOIN ",
            published_exercises!(),
            " e ON e.id = t.exercise_id)"
        )
    };
}

macro_rules! exam_part_translations {
    () => {
        translations!(
            "exam_partie",
            "exam_part_id",
            published_exam_parts!(),
            "nullif(f.champs ->> 'titre', '') AS title,
             nullif(f.champs ->> 'enonce_md', '') AS statement_md"
        )
    };
}

macro_rules! news_translations {
    () => {
        translations!(
            "actualite",
            "news_id",
            published_news!(),
            "nullif(f.champs ->> 'titre', '') AS title,
             nullif(f.champs ->> 'chapeau', '') AS summary,
             nullif(f.champs ->> 'contenu_md', '') AS content_md"
        )
    };
}

macro_rules! resource_translations {
    () => {
        translations!(
            "ressource",
            "resource_id",
            published_resources!(),
            "nullif(f.champs ->> 'titre', '') AS title,
             nullif(f.champs ->> 'description', '') AS description,
             nullif(f.champs ->> 'categorie', '') AS category"
        )
    };
}

/// Subject names in a translated language (`contenu.matieres`), with the
/// columns of the old `api_read.subject_names`. French has no row there:
/// its name is the course theme itself.
macro_rules! subject_names {
    () => {
        "(SELECT slug AS subject_slug, langue AS lang, nom AS name
           FROM contenu.matieres WHERE langue <> 'fr')"
    };
}

/// `contenu.licences`, with the columns of the old `api_read.licenses`.
macro_rules! licenses {
    () => {
        "(SELECT cle AS key, nom AS name, url FROM contenu.licences)"
    };
}

/// Every source, by the name of the resource it reads: the tests dump them
/// all to prove that no answer, correction or private column gets through.
pub const SOURCES: &[(&str, &str)] = &[
    ("courses", published_courses!()),
    ("course_translations", course_translations!()),
    ("chapters", published_chapters!()),
    ("chapter_translations", chapter_translations!()),
    ("lessons", published_lessons!()),
    ("lesson_translations", lesson_translations!()),
    ("exercises", published_exercises!()),
    ("exercise_translations", exercise_translations!()),
    ("exams", published_exams!()),
    ("exam_parts", published_exam_parts!()),
    ("exam_part_translations", exam_part_translations!()),
    ("resources", published_resources!()),
    ("resource_translations", resource_translations!()),
    ("subject_names", subject_names!()),
    ("licenses", licenses!()),
];

// --- Answers and corrections ------------------------------------------------

/// The answer key of the exercises of published courses: the QCM's
/// `correctes`, the short answers' `acceptees` and the solution. Read only
/// by `crate::db::practice`, to grade an attempt and to show the solution
/// once the member tried.
macro_rules! exercise_answers {
    () => {
        concat!(
            "(SELECT e.id, ch.course_id, c.slug AS course_slug, e.type,
                e.donnees::jsonb -> 'correctes' AS correct,
                e.donnees::jsonb -> 'acceptees' AS accepted,
                e.solution_md
           FROM contenu.exercises e
           JOIN contenu.chapters ch ON ch.id = e.chapter_id
           JOIN contenu.courses c ON c.id = ch.course_id
          WHERE ",
            course_is_published!(),
            ")"
        )
    };
}

/// What the translations of those exercises add: their accepted answers
/// (every language's count, `Exercise::reponsesAcceptees`) and solution.
macro_rules! exercise_answer_translations {
    () => {
        translations!(
            "exercise",
            "exercise_id",
            exercise_answers!(),
            "f.champs -> 'acceptees' AS accepted,
             nullif(f.champs ->> 'solution_md', '') AS solution_md"
        )
    };
}

/// The model answers of the parts of published exams (`corrige_md`).
macro_rules! exam_corrections {
    () => {
        concat!(
            "(SELECT p.id, p.exam_id, p.ordre AS position, p.corrige_md AS correction_md
           FROM contenu.exam_parties p
           JOIN contenu.exams x ON x.id = p.exam_id
          WHERE ",
            exam_is_published!(),
            ")"
        )
    };
}

macro_rules! exam_correction_translations {
    () => {
        translations!(
            "exam_partie",
            "exam_part_id",
            exam_corrections!(),
            "nullif(f.champs ->> 'corrige_md', '') AS correction_md"
        )
    };
}

/// The correction PDF of published exams (`fichier_corrige`), from
/// `contenu.fichiers`.
macro_rules! exam_correction_files {
    () => {
        concat!(
            "(SELECT x.slug, f.id AS file_id, octet_length(f.contenu)::bigint AS size,
                f.type_mime::text AS mime,
                coalesce(f.updated_at, f.created_at) AT TIME ZONE 'UTC' AS updated_at
           FROM contenu.exams x
           JOIN contenu.fichiers f ON f.disque = 'local' AND f.chemin = x.fichier_corrige
          WHERE ",
            exam_is_published!(),
            ")"
        )
    };
}

/// The answer sources: the tests prove they hold published content only.
pub const ANSWER_SOURCES: &[(&str, &str)] = &[
    ("exercise_answers", exercise_answers!()),
    (
        "exercise_answer_translations",
        exercise_answer_translations!(),
    ),
    ("exam_corrections", exam_corrections!()),
    (
        "exam_correction_translations",
        exam_correction_translations!(),
    ),
    ("exam_correction_files", exam_correction_files!()),
];
