# Database

## 1. What runs today

| Environment | Engine | Source |
|---|---|---|
| Development | **SQLite** (`database/database.sqlite`) | `.env.example`: `DB_CONNECTION=sqlite`; `config/database.php` default |
| Production (`pulsars.science`) | **MySQL** on the VPS, native install, no Docker | `docs/MISE-EN-PROD.md` §1 and §5 (`CREATE DATABASE pulsars CHARACTER SET utf8mb4`), `deploy/pulsars-octane.service` and `deploy/pulsars-queue.service` (`After=… mysql.service`) |
| Docker stack (not the documented production path) | PostgreSQL 17 | `compose.yaml`, `docs/DOCKER.md`, `Dockerfile` (`pdo_pgsql`) |

Sessions, cache and queue are stored in the database (`SESSION_DRIVER`,
`CACHE_STORE`, `QUEUE_CONNECTION` = `database`). Redis is configured in
`.env.example` but not used in production.

These facts come from the repository. Nobody has checked the server for this
contract: `deploy/diagnostic-serveur.sh` lists the active database services
and should be run once to confirm.

**Gap.** The target architecture says "move production from SQLite to
PostgreSQL". Production is not on SQLite but on MySQL. The migration to plan
is therefore **MySQL → PostgreSQL**: see `postgres-migration.md`.

## 2. Target layout

One PostgreSQL database, `pulsars`, three schemas and two login roles.

```
database pulsars
├── schema site       owner pulsars_site   all Laravel tables (today's tables, unchanged)
├── schema api_read   owner pulsars_site   views only, created by a Laravel migration from views.sql
└── schema game       owner pulsars_api    tables created by pulsars-api migrations
```

| Role | `site` | `api_read` | `game` |
|---|---|---|---|
| `pulsars_site` (Laravel) | owner, all rights | owner (creates the views) | **none** |
| `pulsars_api` (Rust API) | **none** | `USAGE` + `SELECT` | owner, all rights |

- A PostgreSQL view runs with its **owner's** privileges: `pulsars_api` reads
  `site` data only through the views, never the tables. Tested: a `SELECT`
  on `site.courses` as `pulsars_api` fails with
  `permission denied for schema site`.
- The views must **not** set `security_invoker = true`, or the API would need
  rights on `site`.
- Laravel never reads `game` directly. Game data reaches the site through
  `/v1/game` (architecture rule "no service writes into the other's schema",
  extended here to reads, so the API's tables can evolve freely).
- The `api_read` schema is created by the administrator, not the migration:
  even `CREATE SCHEMA IF NOT EXISTS` needs the `CREATE` privilege on the
  database (found while testing `views.sql`).
- Laravel connects with `search_path = site`. The `pgsql` connection in
  `config/database.php` reads `DB_SCHEMA`/`search_path`; set
  `'search_path' => env('DB_SEARCH_PATH', 'site')`. The `migrations` table
  lands in `site` too.
- The full role and grant script is at the end of `views.sql`.

### 2.1 Since phase 1: `pulsars_owner` owns the structure

`deploy/sql/phase-1.sql` (run once by the administrator; rollback:
`phase-1-retour.sql`) moves the content tables to a `contenu` schema and
takes every structure right away from the service roles:

```
database pulsars
├── schema site       owner pulsars_owner   accounts, forum, progress… (Laravel's other tables)
├── schema contenu    owner pulsars_owner   courses, chapters, lessons, exercises, exams, exam_parties,
│                                           ressources, actualites, contenu_translations, fichiers, telechargements
├── schema api_read   owner pulsars_site    the views, unchanged: they follow the moved tables
├── schema game       owner pulsars_owner
└── schema langues    owner pulsars_owner   (once the languages section is deployed)
```

| Role | `site` | `contenu` | `api_read` | `game`, `langues` | Structure |
|---|---|---|---|---|---|
| `pulsars_owner` (no login) | owner | owner | — | owner | owns it all |
| `pulsars_migrate` (login) | member of `pulsars_owner`: runs `pulsars-api migrate` | | | | yes |
| `pulsars_site` (Laravel) | rows: read, write | rows: read, write | owner | **none** | **none** |
| `pulsars_api` (Rust API) | **none** | `SELECT` | `SELECT` | rows: read, write | **none** |

- `search_path`: `site, contenu` for `pulsars_site`, so Laravel's queries do
  not change; `game, langues, api_read, contenu` for `pulsars_api`.
- Tables a migration creates later get the same row rights through
  `ALTER DEFAULT PRIVILEGES FOR ROLE pulsars_owner`, per schema.
- `scripts/dev-db-init.sh` applies the script, so local and CI databases
  have this layout; `tests/http.rs` checks the service role's side of it.

### 2.2 Phase 1 (2/2): the API reads `contenu` itself

The API no longer reads the content views: `src/db/sources.rs` holds, as
one SQL subquery per resource, the same filters and the same columns
(published rows only; no solution, no correction, no private column). The
column dictionary of §4 still describes what they return, under the views'
names. Three reference tables, created by the migration
`20261028000000_contenu_reference`, replace the values the views copied from
PHP:

| Table | Replaces | Source in Laravel |
|---|---|---|
| `contenu.licences (cle, nom, url)` | `api_read.licenses` | `config/licences.php` |
| `contenu.matieres (slug, langue, nom)` | `api_read.subject_names` | `App\Support\Matieres`, `lang/*.json` |
| `contenu.sources_officielles (hote, prefixe)` | the allow-list of `api_read.exams.official_url` | `SujetOfficiel::SOURCES` |

Until the site writes content through the API (phase 4), the PHP values
stay the source: changing one needs a migration here too.

- Still read through views until phase 2: `api_read.members` (the author's
  pseudo), `member_roles`, `member_xp`, `member_brains`. The content views
  stay in place for Laravel's own tests and for the comparison below, and
  go away in phase 5.
- `pulsars-api check-content` compares each source with the view it
  replaces, row for row (`EXCEPT ALL` both ways), on the real data; so does
  `the_content_sources_read_exactly_what_the_views_show` on the demo data.
- `dev/db/25-api-read.sql` is the views as Laravel's migrations build them,
  generated by `scripts/site-schema-snapshot.sh`; `views.sql` is the
  original contract and had drifted (no Russian subject names, a simpler
  official-URL check).

### Isolation tested

`views.sql` was run on PostgreSQL 16 (PGlite) with Laravel-equivalent tables
loaded from the local seed (272 courses, 1,120 exercises, 32 exams,
5 resources, 2,735 translations), under the two roles above. Checks that
passed:

- `pulsars_api` cannot read `site` or create anything in `api_read`; it can
  create tables in `game`;
- no view has a column named `solution_md`, `correctes`, `acceptees`,
  `corrige_md`, `fichier_corrige`, `email`, `password`, `friend_token`,
  `locale`, `niveau_scolaire`, `classe`, `prompt_ia`, `fichier`, `donnees`,
  `champs`, `vues`, `telechargements`;
- QCM options carry only `id` and `text`; English solutions and translated
  exam corrections never appear in any view;
- draft, future-dated and archived courses, and draft exams with their parts
  and translations, are invisible;
- `member_xp` gives the same total and level as `App\Services\Experience`, and
  the level formula matches the PHP loop for every XP value from 0 to 200,000;
- the official exam URL allow-list rejects a path containing `..`.

The test script is not in the repository (it lives with the Rust API's
integration tests, task in `laravel-tasks.md`).

## 3. Views

One view per exposed resource, plus translation views and static reference
data. Every view is limited to published rows (same rules as the Eloquent
scopes) and to allowed columns.

| View | Source | Publication rule |
|---|---|---|
| `courses` | `courses` + author `pseudo` + counts | `statut = 'publie'` and `published_at <= now()` (`Course::scopePublies`) |
| `course_translations` | `contenu_translations` type `course` | parent visible |
| `chapters` | `chapters` | course visible |
| `chapter_translations` | type `chapter` | parent visible |
| `lessons` | `lessons` | chapter visible |
| `lesson_translations` | type `lesson` | parent visible |
| `exercises` | `exercises`, answers removed | chapter visible |
| `exercise_translations` | type `exercise`, without `solution_md` | parent visible |
| `exams` | `exams` | `statut = 'publie'` and `published_at <= now()` (`Exam::scopePublies`) |
| `exam_parts` | `exam_parties`, without `corrige_md` | exam visible |
| `exam_part_translations` | type `exam_partie`, without `corrige_md` | parent visible |
| `resources` | `ressources` | `statut = 'publie'` (`Ressource::scopePubliees`), and a file or a link exists |
| `resource_translations` | type `ressource` | parent visible |
| `members` | `users`, public columns only | all accounts (see note) |
| `member_roles` | `role_user` + `roles` | all |
| `member_xp` | activity tables, computed | all |
| `member_brains` | `cerveaux_mouvements` summed, `users.cerveaux_publics` | all (`brains.md` §4) |
| `licenses` | copy of `config/licences.php` | static |
| `subject_names` | copy of subject names in `lang/en.json` | static |

Note on `members`: every account is visible to the API role, but no endpoint
lists members. Laravel deliberately has no member directory
(`ProfilPublicController::index` shows nothing without a search); the API
must not create one either. `/v1/me` only reads the token's own row.

## 4. Column dictionary

Types are PostgreSQL types as Laravel creates them (`string` → `varchar(255)`,
`json` → `json`, `timestamp` → `timestamp(0)`), after the conversions done in
the views (`timestamptz`, `jsonb`).

### `api_read.courses`

| Column | Type | Null | Meaning |
|---|---|---|---|
| `id` | bigint | no | Internal id, stable. |
| `slug` | varchar | no | Public identifier, unique. |
| `title` | varchar | no | Original (French) title. |
| `description` | text | no | Original description. |
| `theme` | varchar | no | Raw theme, e.g. `Chimie – La matière`. Default `Divers`. |
| `subject_name` | text | no | Part before the first ` – ` (en dash). |
| `subject_slug` | text | no | `slugify(subject_name)`. |
| `topic_name` | text | yes | Part after ` – `; null for the 4 courses whose theme is just `Mathématiques` or `Physique`. |
| `topic_slug` | text | yes | |
| `difficulty` | varchar | no | `debutant`, `intermediaire`, `avance`. |
| `levels` | text[] | no | From tags: subset of `college`, `lycee`, `superieur`; often empty. |
| `duration_minutes` | smallint | yes | Estimated duration. |
| `tags` | jsonb | no | Array of free tags, `[]` when none. |
| `author_pseudo` | varchar | yes | Null if the author account is gone. Today every course has an author (`auteur_id` cascades on delete, so deleting an author deletes their courses: worth knowing). |
| `chapter_count`, `lesson_count`, `exercise_count` | int | no | |
| `published_at` | timestamptz | no | |
| `updated_at` | timestamptz | yes | |

### `api_read.course_translations`, `chapter_translations`, `lesson_translations`, `exercise_translations`, `exam_part_translations`, `resource_translations`

| Column | Type | Null | Meaning |
|---|---|---|---|
| `<parent>_id` | bigint | no | Id of the translated row. |
| `lang` | varchar(5) | no | `en` today. |
| translated fields | text | yes | `title`, `description`, `content_md`, `question_md`, `statement_md`, `category` depending on the view. Null when the translation lacks the field: the API then falls back to the original. |

`exercise_translations` also has `options` (jsonb, null unless the exercise
is a QCM whose translation has an `options` object): the same
`[{"id", "text"}]` array as `exercises.options`, same ids and order, each text
translated or, when missing, the original one.

Translations whose `source_hash` no longer matches the source are still
served, as Laravel does (`Traduisible::t()` ignores the hash).

### `api_read.chapters`

| Column | Type | Null | Meaning |
|---|---|---|---|
| `id` | bigint | no | |
| `course_id` | bigint | no | |
| `position` | smallint | no | `ordre`, from 1. |
| `title` | varchar | no | |

### `api_read.lessons`

| Column | Type | Null | Meaning |
|---|---|---|---|
| `id` | bigint | no | |
| `chapter_id`, `course_id` | bigint | no | |
| `slug` | varchar | no | Unique per chapter only (`unique(chapter_id, slug)`); unique per course in practice. |
| `position` | smallint | no | Order in the chapter. |
| `chapter_position` | smallint | no | For reading order across chapters. |
| `title` | varchar | no | |
| `content_md` | text | no | Markdown with LaTeX. |
| `updated_at` | timestamptz | yes | |

### `api_read.exercises`

| Column | Type | Null | Meaning |
|---|---|---|---|
| `id` | bigint | no | Public identifier. |
| `chapter_id`, `course_id` | bigint | no | |
| `type` | varchar | no | `qcm`, `reponse_courte`, `reponse_libre`. |
| `title` | varchar | no | |
| `question_md` | text | no | |
| `position`, `chapter_position` | smallint | no | |
| `options` | jsonb | yes | QCM only: `[{"id": "a", "text": "31"}, …]`. Never `correctes`. |
| `multiple` | boolean | yes | QCM only: more than one correct option. |
| `updated_at` | timestamptz | yes | |

Excluded: `solution_md`, the whole `donnees` column (it holds `correctes` for
QCM and `acceptees` for short answers).

### `api_read.exams`

| Column | Type | Null | Meaning |
|---|---|---|---|
| `id` | bigint | no | |
| `slug` | varchar | no | |
| `title` | varchar | no | Not translatable. |
| `type` | varchar | no | `brevet`, `bac`, `blanc`, `concours`. |
| `competition` | varchar | yes | `centrale-supelec` for competitive exams. |
| `subject` | varchar | no | `App\Enums\MatiereExamen` value. |
| `country` | varchar(2) | no | ISO code, default `FR`. |
| `level` | varchar | no | Free text: `3e`, `Terminale`, `CPGE`. |
| `tracks` | jsonb | no | CPGE tracks, `[]` otherwise. |
| `session` | varchar | yes | Free text label. |
| `duration_minutes` | smallint | yes | |
| `total_points` | smallint | yes | |
| `description` | text | yes | Not translatable. |
| `has_pdf` | boolean | no | Pulsars hosts the statement PDF. |
| `official_url` | text | yes | Organiser's PDF, **only if** it passes the `SujetOfficiel` allow-list (reimplemented in SQL). |
| `source_url` | varchar | yes | Organiser's listing page. |
| `part_count` | int | no | |
| `published_at`, `updated_at` | timestamptz | no / yes | |

Excluded: `statut`, `fichier_pdf`, `nom_fichier_original`, `fichier_corrige`,
`prompt_ia`, `erreur_ia`, `analyse_at`, `auteur_id`, `vues`,
`telechargements`.

### `api_read.exam_parts`

| Column | Type | Null | Meaning |
|---|---|---|---|
| `id` | bigint | no | |
| `exam_id` | bigint | no | |
| `position` | smallint | no | |
| `title` | varchar | no | |
| `statement_md` | text | no | `enonce_md`. |
| `points` | numeric(5,2) | yes | |

Excluded: `corrige_md`.

### `api_read.resources`

| Column | Type | Null | Meaning |
|---|---|---|---|
| `id` | bigint | no | |
| `slug` | varchar | no | |
| `title` | varchar | no | |
| `description` | text | yes | |
| `category` | varchar | yes | Free text, translatable. |
| `doc_lang` | varchar(5) | yes | Language of the document. |
| `mode` | text | no | `host` or `link`. |
| `attribution` | varchar(200) | yes | `auteur_nom`: credited author of the document. |
| `license_key` | varchar(60) | yes | Key of `licenses`; null for older resources. |
| `license_custom` | varchar(200) | yes | Free text when the key is `autre`. |
| `source_url` | varchar(500) | yes | The link (mode `link`) or the credited origin (mode `host`). |
| `size_bytes` | bigint | yes | Mode `host` only. |
| `created_at`, `updated_at` | timestamptz | yes | |

Excluded: `fichier`, `nom_fichier_original`, `statut`, `ajoute_par`,
`telechargements`, `vues`.

### `api_read.members`

| Column | Type | Null | Meaning |
|---|---|---|---|
| `id` | bigint | no | Equals the JWT `sub` (as a string on the API side). |
| `pseudo` | varchar | no | Unique. |
| `avatar_path` | varchar | yes | Path on Laravel's `public` disk. The API builds the URL as `https://pulsars.science/storage/<path>` (`PUBLIC_STORAGE_URL`). |
| `bio` | varchar(280) | yes | |
| `member_since` | timestamptz | yes | `created_at`. |

### `api_read.member_roles`

| Column | Type | Null | Meaning |
|---|---|---|---|
| `user_id` | bigint | no | |
| `slug`, `name` | varchar | no | |
| `category` | varchar | no | `permission`, `matiere`, `distinction`. |
| `color` | varchar(7) | no | Hex. |
| `permission_level` | smallint | no | 0 except permission roles (1–3). |
| `display_order` | smallint | no | |

### `api_read.member_xp`

| Column | Type | Null | Meaning |
|---|---|---|---|
| `user_id` | bigint | no | |
| `total` | int | no | Same as `Experience::pour()['total']`. |
| `level` | int | no | Same as `Experience::niveau()`. |

### `api_read.member_brains`

| Column | Type | Null | Meaning |
|---|---|---|---|
| `user_id` | bigint | no | |
| `balance` | int | no | Sum of the member's `cerveaux_mouvements.montant` (0 without movements). |
| `public` | boolean | no | `users.cerveaux_publics`. The API shows the balance to its owner only (`/v1/me`, `/v1/game/me`). |

See `brains.md`.

### `api_read.licenses`, `api_read.subject_names`

Static copies of `config/licences.php` and of the subject names in
`lang/en.json`. They are views over `VALUES`, regenerated by the same
migration as the other views.

## 5. Indexes the views rely on

The views add no index. On PostgreSQL, unlike MySQL, a foreign key does **not**
create an index. The `site` tables already have composite indexes on most
foreign keys, but `questions.user_id` and `answers.user_id` have none (used
by `member_xp` and by `Experience` itself). Add them during the PostgreSQL
migration (see `postgres-migration.md`).

## 6. Known gaps

- No domain → subject → topic taxonomy: derived from `theme` in the view.
- No level column on courses: derived from tags with two spellings.
- Topic names have no English translation; "Biologie" is missing from
  `lang/en.json`.
- QCM options, exam titles and exam descriptions are not translatable.
- Licence names, subject names, the `SujetOfficiel` allow-list and the XP scale
  live in PHP; the views copy them. A Laravel test must compare the copies
  (task in `laravel-tasks.md`).
- Lesson slug uniqueness is per chapter, not per course.
