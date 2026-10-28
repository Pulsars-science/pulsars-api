//! `pulsars-api content …`: the content commands of the old site (its
//! `contenu:*` Artisan commands) kept for one admin running a small site.
//!
//! - `content check [--lang en]` (`contenu:verifier`): checks the content
//!   in the database (courses, exercises, translations, files); changes
//!   nothing; exit 1 when there is a problem.
//! - `content fix <file.json> [--dry-run]` (`contenu:corriger`): targeted
//!   replacements in the original and its translations.
//! - `content translate [--lang en] [--force] [--limit N]`
//!   (`contenu:traduire`): translates courses, chapters, lessons, exercises
//!   and exam parts with DeepL, skipping what is up to date.
//!
//! A translation is up to date when its `source_hash` is the SHA-256 of the
//! original's translatable fields joined by NUL (Laravel's
//! `empreinteSource()`; a QCM adds `id NUL text` for each option).

use std::collections::{HashMap, HashSet};

use serde_json::{Map, Value};
use sha2::{Digest as _, Sha256};
use sqlx::PgPool;

use crate::deepl::Deepl;

/// `format!` for a query built from this module's constants only.
macro_rules! sql {
    ($($arg:tt)*) => {
        sqlx::AssertSqlSafe(format!($($arg)*))
    };
}

/// The language the content is written in.
pub const SOURCE_LANG: &str = "fr";
/// The languages content may be translated into.
pub const TARGET_LANGS: &[&str] = &["en", "ru"];

/// The subjects a course theme may start with (`App\Support\Matieres::COURS`).
pub const COURSE_SUBJECTS: &[&str] = &[
    "Mathématiques",
    "Physique",
    "Chimie",
    "Biologie",
    "Informatique",
    "Cryptographie",
];

/// A translatable model: its morph name, table and fields.
#[derive(Debug, Clone, Copy)]
pub struct Kind {
    pub morph: &'static str,
    pub table: &'static str,
    pub fields: &'static [&'static str],
    /// The table has `updated_at`.
    pub timestamps: bool,
}

pub const KINDS: &[Kind] = &[
    Kind {
        morph: "course",
        table: "contenu.courses",
        fields: &["titre", "description"],
        timestamps: true,
    },
    Kind {
        morph: "chapter",
        table: "contenu.chapters",
        fields: &["titre"],
        timestamps: true,
    },
    Kind {
        morph: "lesson",
        table: "contenu.lessons",
        fields: &["titre", "contenu_md"],
        timestamps: true,
    },
    Kind {
        morph: "exercise",
        table: "contenu.exercises",
        fields: &["titre", "question_md", "solution_md"],
        timestamps: true,
    },
    Kind {
        morph: "exam_partie",
        table: "contenu.exam_parties",
        fields: &["titre", "enonce_md", "corrige_md"],
        timestamps: false,
    },
    Kind {
        morph: "ressource",
        table: "contenu.ressources",
        fields: &["titre", "description", "categorie"],
        timestamps: true,
    },
    Kind {
        morph: "actualite",
        table: "contenu.actualites",
        fields: &["titre", "chapeau", "contenu_md"],
        timestamps: true,
    },
];

pub fn kind(morph: &str) -> Option<Kind> {
    KINDS.iter().copied().find(|k| k.morph == morph)
}

/// The options of a QCM, `(id, text)`, from `donnees`.
pub fn options(data: Option<&Value>) -> Vec<(String, String)> {
    data.and_then(|d| d.get("options"))
        .and_then(Value::as_array)
        .map(|options| {
            options
                .iter()
                .map(|o| (scalar(o.get("id")), scalar(o.get("texte"))))
                .collect()
        })
        .unwrap_or_default()
}

/// PHP's `(string)` of a JSON scalar.
fn scalar(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::Bool(true)) => "1".to_owned(),
        _ => String::new(),
    }
}

/// `empreinteSource()`.
pub fn source_hash(fields: &[Option<String>], qcm_options: &[(String, String)]) -> String {
    let mut values: Vec<String> = fields
        .iter()
        .map(|f| f.clone().unwrap_or_default())
        .collect();
    values.extend(qcm_options.iter().map(|(id, text)| format!("{id}\0{text}")));
    Sha256::digest(values.join("\0").as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// One element to translate or fix: its fields, and its QCM options.
#[derive(Debug, Clone)]
struct Element {
    fields: Vec<Option<String>>,
    data: Option<Value>,
    qcm: bool,
}

impl Element {
    fn hash(&self) -> String {
        let options = if self.qcm {
            options(self.data.as_ref())
        } else {
            Vec::new()
        };
        source_hash(&self.fields, &options)
    }
}

async fn load(
    db: impl sqlx::PgExecutor<'_>,
    kind: Kind,
    id: i64,
) -> Result<Option<Element>, sqlx::Error> {
    let fields = kind
        .fields
        .iter()
        .map(|f| format!("{f}::text"))
        .collect::<Vec<_>>()
        .join(", ");
    let (data, qcm) = if kind.morph == "exercise" {
        ("donnees::jsonb", "type = 'qcm'")
    } else {
        ("NULL::jsonb", "false")
    };
    let row: Option<(Vec<Option<String>>, Option<Value>, bool)> = sqlx::query_as(sql!(
        "SELECT ARRAY[{fields}], {data}, {qcm} FROM {} WHERE id = $1",
        kind.table
    ))
    .bind(id)
    .fetch_optional(db)
    .await?;
    Ok(row.map(|(fields, data, qcm)| Element { fields, data, qcm }))
}

// --- check ------------------------------------------------------------------

/// Problems found by `content check`, and what was looked at.
#[derive(Debug, Default)]
pub struct Check {
    pub problems: Vec<String>,
    pub courses: usize,
    pub exercises: usize,
    pub exams: usize,
    pub resources: usize,
}

/// `CorrecteurExercice::normaliser()`: case, accents, spaces and final
/// punctuation do not count; Cyrillic is compared in Latin letters.
pub fn normalise_answer(value: &str) -> String {
    const CYRILLIC: &[(char, &str)] = &[
        ('а', "a"),
        ('б', "b"),
        ('в', "v"),
        ('г', "g"),
        ('д', "d"),
        ('е', "e"),
        ('ё', "e"),
        ('ж', "zh"),
        ('з', "z"),
        ('и', "i"),
        ('й', "j"),
        ('к', "k"),
        ('л', "l"),
        ('м', "m"),
        ('н', "n"),
        ('о', "o"),
        ('п', "p"),
        ('р', "r"),
        ('с', "s"),
        ('т', "t"),
        ('у', "u"),
        ('ф', "f"),
        ('х', "h"),
        ('ц', "c"),
        ('ч', "ch"),
        ('ш', "sh"),
        ('щ', "shch"),
        ('ъ', ""),
        ('ы', "y"),
        ('ь', ""),
        ('э', "e"),
        ('ю', "yu"),
        ('я', "ya"),
    ];
    const LATIN: &[(&str, &str)] = &[
        ("àáâãäå", "a"),
        ("ç", "c"),
        ("èéêë", "e"),
        ("ìíîï", "i"),
        ("ñ", "n"),
        ("òóôõöø", "o"),
        ("ùúûü", "u"),
        ("ýÿ", "y"),
        ("œ", "oe"),
        ("æ", "ae"),
        ("ß", "ss"),
    ];
    let lower = value.trim().to_lowercase();
    let mut out = String::with_capacity(lower.len());
    for c in lower.chars() {
        if let Some((_, latin)) = CYRILLIC.iter().find(|(k, _)| *k == c) {
            out.push_str(latin);
        } else if let Some((_, plain)) = LATIN.iter().find(|(set, _)| set.contains(c)) {
            out.push_str(plain);
        } else if c == ',' {
            out.push('.');
        } else if c.is_ascii_lowercase()
            || c.is_ascii_digit()
            || matches!(
                c,
                '-' | '\'' | '.' | '/' | '+' | '*' | '=' | '^' | '(' | ')' | '%'
            )
        {
            out.push(c);
        }
        // Everything else, spaces included, goes.
    }
    out.trim_matches(['.', '!', '?', ';', ':', ',']).to_owned()
}

fn strings(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items.iter().map(|v| scalar(Some(v))).collect(),
        Some(v @ (Value::String(_) | Value::Number(_))) => vec![scalar(Some(v))],
        _ => Vec::new(),
    }
}

/// `content check --lang <lang>`.
pub async fn check(pool: &PgPool, lang: &str) -> Result<Check, sqlx::Error> {
    let mut out = Check::default();
    let mut problem = |p: String| out.problems.push(p);

    let translations: HashMap<(String, i64), Value> = sqlx::query_as::<_, (String, i64, Value)>(
        "SELECT traduisible_type::text, traduisible_id, champs::jsonb
           FROM contenu.contenu_translations WHERE langue = $1",
    )
    .bind(lang)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|(t, id, champs)| ((t, id), champs))
    .collect();
    let translated = |kind: &str, id: i64| translations.get(&(kind.to_owned(), id));
    // Accepted answers of every language count (`reponsesAcceptees()`).
    let mut accepted_tr: HashMap<i64, Vec<String>> = HashMap::new();
    for (id, champs) in sqlx::query_as::<_, (i64, Value)>(
        "SELECT traduisible_id, champs::jsonb FROM contenu.contenu_translations
          WHERE traduisible_type = 'exercise' ORDER BY id",
    )
    .fetch_all(pool)
    .await?
    {
        accepted_tr
            .entry(id)
            .or_default()
            .extend(strings(champs.get("acceptees")));
    }

    let courses: Vec<(i64, String, String)> =
        sqlx::query_as("SELECT id, slug, theme FROM contenu.courses ORDER BY id")
            .fetch_all(pool)
            .await?;
    let chapters: Vec<(i64, i64, i16)> =
        sqlx::query_as("SELECT id, course_id, ordre FROM contenu.chapters ORDER BY ordre, id")
            .fetch_all(pool)
            .await?;
    let lessons: Vec<(i64, i64, String, String)> = sqlx::query_as(
        "SELECT l.id, ch.course_id, l.slug, l.contenu_md FROM contenu.lessons l
           JOIN contenu.chapters ch ON ch.id = l.chapter_id ORDER BY ch.ordre, l.ordre, l.id",
    )
    .fetch_all(pool)
    .await?;
    #[allow(clippy::type_complexity)]
    let exercises: Vec<(i64, i64, String, Option<String>, Option<Value>)> = sqlx::query_as(
        "SELECT e.id, ch.course_id, e.type, e.solution_md, e.donnees::jsonb FROM contenu.exercises e
           JOIN contenu.chapters ch ON ch.id = e.chapter_id ORDER BY ch.ordre, e.ordre, e.id",
    )
    .fetch_all(pool)
    .await?;

    for (course_id, slug, theme) in &courses {
        let name = format!("Course '{slug}'");
        if let Some((subject, _)) = theme.split_once(" – ")
            && !COURSE_SUBJECTS.contains(&subject)
        {
            problem(format!(
                "{name}: subject unknown to the catalogue ('{theme}')."
            ));
        }
        if translated("course", *course_id).is_none() {
            problem(format!("{name}: no translation in {lang}."));
        }
        let own_chapters: Vec<_> = chapters.iter().filter(|c| c.1 == *course_id).collect();
        let own_lessons: Vec<_> = lessons.iter().filter(|l| l.1 == *course_id).collect();
        let own_exercises: Vec<_> = exercises.iter().filter(|e| e.1 == *course_id).collect();
        out.exercises += own_exercises.len();
        if own_chapters.is_empty() || own_lessons.is_empty() || own_exercises.is_empty() {
            problem(format!(
                "{name}: chapters, lessons or exercises are missing."
            ));
        }
        // A lesson is found by (course, slug): a duplicate hides one of them.
        let mut seen = HashSet::new();
        let mut reported = HashSet::new();
        for (_, _, lesson_slug, _) in &own_lessons {
            if !seen.insert(lesson_slug.as_str()) && reported.insert(lesson_slug.as_str()) {
                problem(format!("{name}: duplicate lesson slug '{lesson_slug}'."));
            }
        }
        for (chapter_id, _, position) in &own_chapters {
            if translated("chapter", *chapter_id).is_none() {
                problem(format!(
                    "{name}, chapter {position}: no translation in {lang}."
                ));
            }
        }
        for (lesson_id, _, lesson_slug, content) in &own_lessons {
            if content.trim().is_empty() {
                problem(format!("{name}: empty lesson '{lesson_slug}'."));
            }
            if translated("lesson", *lesson_id).is_none() {
                problem(format!(
                    "{name}, lesson '{lesson_slug}': no translation in {lang}."
                ));
            }
        }
        for (exercise_id, _, kind, solution, data) in &own_exercises {
            let label = format!("{name}, exercise {exercise_id}");
            if solution.as_deref().unwrap_or_default().trim().is_empty() {
                problem(format!("{label}: no solution."));
            }
            let translation = translated("exercise", *exercise_id);
            if translation.is_none() {
                problem(format!("{label}: no translation in {lang}."));
            }
            match kind.as_str() {
                "qcm" => {
                    let ids: Vec<String> = options(data.as_ref())
                        .into_iter()
                        .map(|(id, _)| id)
                        .collect();
                    let correct = strings(data.as_ref().and_then(|d| d.get("correctes")));
                    if correct.is_empty() || correct.iter().any(|c| !ids.contains(c)) {
                        problem(format!(
                            "{label}: right answers missing or not among the options."
                        ));
                    } else {
                        // The corrector compares the sorted answers with the
                        // distinct chosen ones: a duplicate never passes.
                        let distinct: HashSet<&String> = correct.iter().collect();
                        if distinct.len() != correct.len() {
                            problem(format!("{label}: the corrector rejects the right answers."));
                        }
                    }
                    if let Some(translation) = translation {
                        let keys: HashSet<String> = translation
                            .get("options")
                            .and_then(Value::as_object)
                            .map(|o| o.keys().cloned().collect())
                            .unwrap_or_default();
                        if ids.iter().any(|id| !keys.contains(id)) {
                            problem(format!("{label}: options not translated in {lang}."));
                        }
                    }
                }
                "reponse_courte" => {
                    let mut accepted = strings(data.as_ref().and_then(|d| d.get("acceptees")));
                    accepted.extend(accepted_tr.get(exercise_id).cloned().unwrap_or_default());
                    let mut unique = Vec::new();
                    for a in accepted {
                        if !unique.contains(&a) {
                            unique.push(a);
                        }
                    }
                    if unique.is_empty() {
                        problem(format!("{label}: no accepted answer."));
                    }
                    for answer in &unique {
                        if normalise_answer(answer).is_empty() {
                            problem(format!("{label}: '{answer}' is refused by the corrector."));
                        }
                    }
                }
                _ => {}
            }
        }
    }
    out.courses = courses.len();

    #[allow(clippy::type_complexity)]
    let exams: Vec<(
        i64,
        String,
        Option<String>,
        bool,
        Option<String>,
        bool,
        Option<String>,
        bool,
    )> = sqlx::query_as(
        "SELECT x.id, x.slug, x.url_officielle,
                x.url_officielle IS NULL OR (
                    lower(u.chemin) LIKE '%.pdf' AND position('..' IN u.chemin) = 0
                    AND EXISTS (SELECT 1 FROM contenu.sources_officielles s
                                 WHERE s.hote = u.hote AND starts_with(u.chemin, s.prefixe))),
                x.fichier_pdf,
                EXISTS (SELECT 1 FROM contenu.fichiers f
                         WHERE f.disque = 'local' AND f.chemin = x.fichier_pdf),
                x.fichier_corrige,
                EXISTS (SELECT 1 FROM contenu.fichiers f
                         WHERE f.disque = 'local' AND f.chemin = x.fichier_corrige)
           FROM contenu.exams x
           CROSS JOIN LATERAL (SELECT btrim(x.url_officielle, E' \\t\\n\\r\\x0B') AS url) o
           CROSS JOIN LATERAL (
               SELECT lower(substring(o.url FROM '^https://([^/?#]*)')) AS hote,
                      substring(o.url FROM '^https://[^/?#]*([^?#]*)') AS chemin) u
          ORDER BY x.id",
    )
    .fetch_all(pool)
    .await?;
    let parts: Vec<(i64, i64, i16)> =
        sqlx::query_as("SELECT id, exam_id, ordre FROM contenu.exam_parties ORDER BY ordre, id")
            .fetch_all(pool)
            .await?;
    for (exam_id, slug, url, official_ok, pdf, pdf_ok, correction, correction_ok) in &exams {
        let files = [(pdf, pdf_ok), (correction, correction_ok)];
        let name = format!("Exam '{slug}'");
        if !official_ok {
            problem(format!(
                "{name}: official address outside the allow list ({}).",
                url.as_deref().unwrap_or_default()
            ));
        }
        for (path, exists) in files {
            if let Some(path) = path
                && !*exists
            {
                problem(format!("{name}: file missing from the database ({path})."));
            }
        }
        for (part_id, _, position) in parts.iter().filter(|p| p.1 == *exam_id) {
            if translated("exam_partie", *part_id).is_none() {
                problem(format!(
                    "{name}, part {position}: no translation in {lang}."
                ));
            }
        }
    }
    out.exams = exams.len();

    let resources: Vec<(String, Option<String>, bool)> = sqlx::query_as(
        "SELECT r.slug, r.fichier,
                r.fichier IS NULL OR EXISTS (SELECT 1 FROM contenu.fichiers f
                                              WHERE f.disque = 'local' AND f.chemin = r.fichier)
           FROM contenu.ressources r ORDER BY r.id",
    )
    .fetch_all(pool)
    .await?;
    for (slug, path, exists) in &resources {
        if !exists {
            problem(format!(
                "Resource '{slug}': file missing from the database ({}).",
                path.as_deref().unwrap_or_default()
            ));
        }
    }
    out.resources = resources.len();
    Ok(out)
}

// --- fix --------------------------------------------------------------------

/// What `content fix` did: corrections applied (or valid, in a dry run),
/// and the elements refused with why.
#[derive(Debug, Default)]
pub struct Fixed {
    pub applied: usize,
    pub refused: Vec<String>,
}

/// `ancien` must appear exactly once in `text`.
fn replace_once(text: &str, old: &str, new: &str) -> Result<String, String> {
    match text.matches(old).count() {
        1 => Ok(text.replacen(old, new, 1)),
        0 => Err("text 'ancien' not found".to_owned()),
        n => Err(format!("text 'ancien' found {n} times (it must be unique)")),
    }
}

/// `content fix`: `{"corrections": [{"cle": "lesson:774", "langue": "fr",
/// "champ": "contenu_md", "ancien": "…", "nouveau": "…"}, …]}`. Each element
/// (`cle`) is fixed whole or not at all. Fixing the original (`fr`) changes
/// its hash: the translations that were up to date stay so.
pub async fn fix(pool: &PgPool, file: &Value, dry_run: bool) -> Result<Fixed, String> {
    let corrections = file
        .get("corrections")
        .and_then(Value::as_array)
        .ok_or_else(|| "unreadable file: {\"corrections\": [...]} expected".to_owned())?;
    let mut by_element: Vec<(String, Vec<&Value>)> = Vec::new();
    for (i, correction) in corrections.iter().enumerate() {
        let key = correction
            .get("cle")
            .map_or_else(|| format!("#{i}"), |k| scalar(Some(k)));
        match by_element.iter_mut().find(|(k, _)| *k == key) {
            Some((_, list)) => list.push(correction),
            None => by_element.push((key, vec![correction])),
        }
    }
    let mut out = Fixed::default();
    for (key, list) in by_element {
        match fix_element(pool, &key, &list, dry_run).await {
            Ok(()) => out.applied += list.len(),
            Err(error) => out.refused.push(format!("{key}: {error}")),
        }
    }
    Ok(out)
}

async fn fix_element(
    pool: &PgPool,
    key: &str,
    corrections: &[&Value],
    dry_run: bool,
) -> Result<(), String> {
    let (morph, id) = key
        .split_once(':')
        .filter(|(m, id)| {
            !m.is_empty()
                && m.bytes().all(|b| b.is_ascii_lowercase() || b == b'_')
                && !id.is_empty()
                && id.bytes().all(|b| b.is_ascii_digit())
        })
        .ok_or_else(|| "malformed key".to_owned())?;
    let id: i64 = id.parse().map_err(|_| "malformed key".to_owned())?;
    let kind = kind(morph).ok_or_else(|| "element not found".to_owned())?;
    let db = |e: sqlx::Error| format!("database error: {e}");
    let mut element = load(pool, kind, id)
        .await
        .map_err(db)?
        .ok_or_else(|| "element not found".to_owned())?;
    let hash_before = element.hash();
    let mut translations: Vec<(i64, String, Value, String)> = sqlx::query_as(
        "SELECT id, langue::text, champs::jsonb, source_hash::text FROM contenu.contenu_translations
          WHERE traduisible_type = $1 AND traduisible_id = $2 ORDER BY id",
    )
    .bind(kind.morph)
    .bind(id)
    .fetch_all(pool)
    .await
    .map_err(db)?;
    let mut fields_changed = false;
    let mut data_changed = false;

    for correction in corrections {
        let lang = scalar(correction.get("langue"));
        let field = scalar(correction.get("champ"));
        let (Some(Value::String(old)), Some(Value::String(new))) =
            (correction.get("ancien"), correction.get("nouveau"))
        else {
            return Err(format!("'ancien' or 'nouveau' missing ({lang}, {field})"));
        };
        if old.is_empty() {
            return Err(format!("'ancien' or 'nouveau' missing ({lang}, {field})"));
        }
        let result = if lang == SOURCE_LANG {
            fix_original(kind, &mut element, &field, old, new).map(|data| {
                if data {
                    data_changed = true;
                } else {
                    fields_changed = true;
                }
            })
        } else {
            match translations.iter_mut().find(|t| t.1 == lang) {
                None => return Err(format!("no translation in {lang}")),
                Some(translation) => fix_translation(&mut translation.2, &field, old, new),
            }
        };
        result.map_err(|e| format!("{lang}, {field}: {e}"))?;
    }
    if dry_run {
        return Ok(());
    }

    let hash_after = element.hash();
    let mut tx = pool.begin().await.map_err(db)?;
    if fields_changed {
        let sets = kind
            .fields
            .iter()
            .enumerate()
            .map(|(i, f)| format!("{f} = ${}", i + 2))
            .collect::<Vec<_>>()
            .join(", ");
        let touch = if kind.timestamps {
            ", updated_at = date_trunc('second', now() AT TIME ZONE 'UTC')"
        } else {
            ""
        };
        let mut query = sqlx::query(sql!(
            "UPDATE {} SET {sets}{touch} WHERE id = $1",
            kind.table
        ))
        .bind(id);
        for value in &element.fields {
            query = query.bind(value.clone());
        }
        query.execute(&mut *tx).await.map_err(db)?;
    }
    if data_changed {
        sqlx::query(
            "UPDATE contenu.exercises SET donnees = $2::jsonb::json,
                    updated_at = date_trunc('second', now() AT TIME ZONE 'UTC')
              WHERE id = $1",
        )
        .bind(id)
        .bind(sqlx::types::Json(element.data.clone()))
        .execute(&mut *tx)
        .await
        .map_err(db)?;
    }
    for (translation_id, _, champs, hash) in &translations {
        let hash = if hash_after != hash_before && *hash == hash_before {
            hash_after.clone()
        } else {
            hash.clone()
        };
        sqlx::query(
            "UPDATE contenu.contenu_translations
                SET champs = $2::jsonb::json, source_hash = $3,
                    updated_at = date_trunc('second', now() AT TIME ZONE 'UTC')
              WHERE id = $1",
        )
        .bind(translation_id)
        .bind(sqlx::types::Json(champs))
        .bind(hash)
        .execute(&mut *tx)
        .await
        .map_err(db)?;
    }
    tx.commit().await.map_err(db)?;
    Ok(())
}

/// Fixes a field of the original; `Ok(true)` when it was a QCM option.
fn fix_original(
    kind: Kind,
    element: &mut Element,
    field: &str,
    old: &str,
    new: &str,
) -> Result<bool, String> {
    if let Some(option) = field.strip_prefix("options.") {
        let options = element
            .data
            .as_mut()
            .and_then(|d| d.get_mut("options"))
            .and_then(Value::as_array_mut)
            .ok_or_else(|| "unknown option".to_owned())?;
        let entry = options
            .iter_mut()
            .find(|o| scalar(o.get("id")) == option)
            .ok_or_else(|| "unknown option".to_owned())?;
        let text = replace_once(&scalar(entry.get("texte")), old, new)?;
        if let Some(object) = entry.as_object_mut() {
            object.insert("texte".to_owned(), Value::String(text));
        }
        return Ok(true);
    }
    let index = kind
        .fields
        .iter()
        .position(|f| *f == field)
        .ok_or_else(|| "unknown field".to_owned())?;
    let text = replace_once(
        element.fields[index].as_deref().unwrap_or_default(),
        old,
        new,
    )?;
    element.fields[index] = Some(text);
    Ok(false)
}

/// Fixes `champ` or `champ.sous` of a translation.
fn fix_translation(champs: &mut Value, field: &str, old: &str, new: &str) -> Result<(), String> {
    let (root, sub) = match field.split_once('.') {
        Some((root, sub)) => (root, Some(sub)),
        None => (field, None),
    };
    let missing = || "field missing from the translation".to_owned();
    let slot = match sub {
        None => champs.get_mut(root),
        Some(sub) => champs.get_mut(root).and_then(|v| v.get_mut(sub)),
    }
    .ok_or_else(missing)?;
    let Value::String(text) = slot else {
        return Err(missing());
    };
    *text = replace_once(text, old, new)?;
    Ok(())
}

// --- translate --------------------------------------------------------------

/// What `content translate` did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Translated {
    pub translated: usize,
    pub up_to_date: usize,
    pub failed: usize,
    /// Stopped by `--limit`.
    pub limited: bool,
}

/// The kinds `content translate` goes through, in order, with their label.
const TRANSLATED_KINDS: &[(&str, &str)] = &[
    ("course", "Courses"),
    ("chapter", "Chapters"),
    ("lesson", "Lessons"),
    ("exercise", "Exercises"),
    ("exam_partie", "Exam parts"),
];

/// Translates element `morph:id` into `lang`; `false` when it was up to
/// date.
pub async fn translate_element(
    pool: &PgPool,
    deepl: &Deepl,
    morph: &str,
    id: i64,
    lang: &str,
    force: bool,
) -> Result<bool, String> {
    let kind = kind(morph).ok_or_else(|| format!("'{morph}' is not translatable"))?;
    translate_one(pool, deepl, kind, id, lang, force).await
}

async fn translate_one(
    pool: &PgPool,
    deepl: &Deepl,
    kind: Kind,
    id: i64,
    lang: &str,
    force: bool,
) -> Result<bool, String> {
    let db = |e: sqlx::Error| format!("database error: {e}");
    let element = load(pool, kind, id)
        .await
        .map_err(db)?
        .ok_or_else(|| "gone".to_owned())?;
    let hash = element.hash();
    let existing: Option<String> = sqlx::query_scalar(
        "SELECT source_hash::text FROM contenu.contenu_translations
          WHERE traduisible_type = $1 AND traduisible_id = $2 AND langue = $3",
    )
    .bind(kind.morph)
    .bind(id)
    .bind(lang)
    .fetch_optional(pool)
    .await
    .map_err(db)?;
    if !force && existing.as_deref() == Some(hash.as_str()) {
        return Ok(false);
    }
    let mut champs = Map::new();
    for (field, value) in kind.fields.iter().zip(&element.fields) {
        let value = value.clone().unwrap_or_default();
        let translated = if value.trim().is_empty() {
            value
        } else {
            deepl.translate_markdown(&value, lang).await?
        };
        champs.insert((*field).to_owned(), Value::String(translated));
    }
    // A QCM's options are translated too, never its right answers.
    if element.qcm {
        let mut translated = Map::new();
        for (option, text) in options(element.data.as_ref()) {
            let text = if text.trim().is_empty() {
                text
            } else {
                deepl.translate_markdown(&text, lang).await?
            };
            translated.insert(option, Value::String(text));
        }
        champs.insert("options".to_owned(), Value::Object(translated));
    }
    sqlx::query(
        "INSERT INTO contenu.contenu_translations (traduisible_type, traduisible_id, langue, champs,
                                                   source_hash, created_at, updated_at)
         VALUES ($1, $2, $3, $4::jsonb::json, $5, date_trunc('second', now() AT TIME ZONE 'UTC'),
                 date_trunc('second', now() AT TIME ZONE 'UTC'))
         ON CONFLICT (traduisible_type, traduisible_id, langue) DO UPDATE
            SET champs = excluded.champs, source_hash = excluded.source_hash,
                updated_at = excluded.updated_at",
    )
    .bind(kind.morph)
    .bind(id)
    .bind(lang)
    .bind(sqlx::types::Json(Value::Object(champs)))
    .bind(&hash)
    .execute(pool)
    .await
    .map_err(db)?;
    Ok(true)
}

/// `content translate --lang <lang>`: `log` gets one line per kind and one
/// per failure. One element failing does not stop the hundreds of others.
pub async fn translate(
    pool: &PgPool,
    deepl: &Deepl,
    lang: &str,
    force: bool,
    limit: Option<usize>,
    log: &mut dyn FnMut(String),
) -> Result<Translated, String> {
    if lang == SOURCE_LANG {
        return Err("this is already the content's original language".to_owned());
    }
    if !TARGET_LANGS.contains(&lang) {
        return Err(format!("unknown language '{lang}'"));
    }
    let mut out = Translated::default();
    let mut seen = 0usize;
    for (morph, label) in TRANSLATED_KINDS {
        let Some(kind) = kind(morph) else { continue };
        let ids: Vec<i64> = sqlx::query_scalar(sql!("SELECT id FROM {} ORDER BY id", kind.table))
            .fetch_all(pool)
            .await
            .map_err(|e| format!("database error: {e}"))?;
        if ids.is_empty() {
            continue;
        }
        log(format!("{label} ({})", ids.len()));
        for id in ids {
            if limit.is_some_and(|limit| seen >= limit) {
                out.limited = true;
                return Ok(out);
            }
            seen += 1;
            match translate_one(pool, deepl, kind, id, lang, force).await {
                Ok(true) => out.translated += 1,
                Ok(false) => out.up_to_date += 1,
                Err(error) => {
                    out.failed += 1;
                    log(format!("  x {morph} #{id}: {error}"));
                }
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_match_laravel() {
        // hash('sha256', implode("\0", ['Titre', ''])) in PHP.
        let hash = source_hash(&[Some("Titre".to_owned()), None], &[]);
        assert_eq!(
            hash,
            Sha256::digest(b"Titre\0")
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        );
        let with_options = source_hash(
            &[Some("Q".to_owned())],
            &[("a".to_owned(), "Oui".to_owned())],
        );
        assert_eq!(
            with_options,
            Sha256::digest(b"Q\0a\0Oui")
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        );
    }

    #[test]
    fn answers_are_normalised_like_the_corrector() {
        assert_eq!(normalise_answer(" Élève. "), "eleve");
        assert_eq!(normalise_answer("3,5"), "3.5");
        assert_eq!(normalise_answer("3a + 3"), "3a+3");
        assert_eq!(normalise_answer("Ёлка"), "elka");
        assert_eq!(normalise_answer("π"), "");
    }

    #[test]
    fn replacements_need_a_unique_match() {
        assert_eq!(replace_once("a b a", "b", "c").unwrap(), "a c a");
        assert!(
            replace_once("a b a", "a", "c")
                .unwrap_err()
                .contains("2 times")
        );
        assert!(
            replace_once("a", "z", "c")
                .unwrap_err()
                .contains("not found")
        );
    }

    #[test]
    fn translations_fix_fields_and_options() {
        let mut champs = serde_json::json!({"titre": "Un tire", "options": {"b": "Faux"}});
        fix_translation(&mut champs, "titre", "tire", "titre").unwrap();
        fix_translation(&mut champs, "options.b", "Faux", "Vrai").unwrap();
        assert_eq!(
            champs,
            serde_json::json!({"titre": "Un titre", "options": {"b": "Vrai"}})
        );
        assert!(fix_translation(&mut champs, "solution_md", "a", "b").is_err());
    }
}
