# Database

One PostgreSQL 17 database, `pulsars`, on the same machine as the API.
**The API is the only program that connects to it**: `pulsars-web`, the
games and the languages section go through the API's routes. The tables
keep the names and types the old Laravel site gave them, so its content
copied over as is (`pulsars-api import-content`, `deploy/sql/README.md`).

## 1. Layout

```
database pulsars
├── schema site      accounts, sessions, mail links, roles, progress, exercise attempts,
│                    brains, forum, moderation, follows, friends, rooms, private messages,
│                    reports, suggestions, resource requests, settings, the
│                    dashboard's daily HTTP counts and content views
├── schema contenu   courses, chapters, lessons, exercises, exams, exam parts, resources,
│                    news, translations, stored files, downloads, reference tables
├── schema game      matches, players' seats, ratings, seasons, the migrations' history
└── schema langues   the languages section: languages, levels, words, learners' events
```

| Schema | Tables |
|---|---|
| `site` | `users`, `auth_sessions`, `email_tokens`, `comptes_sociaux`, `roles`, `role_user`, `enrollments`, `lesson_progress`, `exercise_attempts`, `cerveaux_mouvements`, `game_results`, `forum_themes`, `questions`, `answers`, `forum_votes`, `forum_translations`, `abonnements`, `friendships`, `rooms`, `room_members`, `room_invites`, `room_messages`, `direct_conversations`, `direct_messages`, `signalements`, `suggestions`, `demandes_ressources`, `reglages`, `http_daily`, `content_views` |
| `contenu` | `courses`, `chapters`, `lessons`, `exercises`, `exams`, `exam_parties`, `ressources`, `actualites`, `contenu_translations`, `fichiers`, `telechargements`, `licences`, `matieres`, `sources_officielles` |
| `game` | `matches`, `match_players`, `ratings`, `seasons`, `_sqlx_migrations` |
| `langues` | `languages`, `levels`, `words`, `events` |

The whole structure is in `migrations/` (`20261101000000_schema.sql` and the
ones after it); `20261101000001_reference.sql` adds the reference data every
server needs: roles, forum themes, licences, subject names, official exam
sources and the first language taught.

Files are rows too: `contenu.fichiers` holds every stored file as `bytea`,
on one of two "disks" (`disque`): `local` (statement and correction PDFs,
hosted resources, proposed resources: served only by the routes that check
who may read them) and `public` (avatars, suggestion images: served by
`GET /v1/storage/{path}`).

## 2. Roles: who writes what

| Role | Login | Rights | Used by |
|---|---|---|---|
| `pulsars_owner` | no | owns every schema, table, sequence, view and function | nobody: it only holds the structure |
| `pulsars_migrate` | yes | member of `pulsars_owner`; its session runs as `pulsars_owner` | `pulsars-api migrate` only (`DATABASE_MIGRATION_URL`, `/etc/pulsars-api/migrate.env`) |
| `pulsars_api` | yes | `USAGE` on the four schemas; `SELECT`, `INSERT`, `UPDATE`, `DELETE` on every table; `USAGE`, `SELECT`, `UPDATE` on every sequence; `EXECUTE` on the functions of `contenu`. **No structure right**: it creates, alters and drops nothing | the service (`DATABASE_URL`) |

- **Only the API writes**, through `pulsars_api`. No other service has a
  role, and none needs one.
- `deploy/sql/roles.sql` creates the three roles and the four empty
  schemas, once, on a new server. Running it again changes nothing.
- The migrations grant `pulsars_api` its row rights, and set the default
  privileges of `pulsars_owner` in each schema (`ALTER DEFAULT PRIVILEGES
  FOR ROLE pulsars_owner IN SCHEMA …`), so a table a later migration
  creates gets the same rights. A migration that adds a **schema** must
  grant it as well: default privileges are per schema.
- `search_path`: `game, site, contenu, langues` for `pulsars_migrate` (the
  migrations' history lands in `game`); `site, contenu, game, langues` for
  `pulsars_api`, whose queries name their schema anyway.
- `pulsars_api` also has `statement_timeout = 5s` and
  `idle_in_transaction_session_timeout = 30s`.
- The service never migrates: at start-up it waits, and logs which
  migrations are missing, until `pulsars-api migrate` has applied them.
  `deploy-api` runs it before each switch (`docs/deployment.md`).
- `scripts/dev-db-init.sh` builds the same layout locally and in CI, with
  development passwords, then loads `dev/db/30-demo-data.sql`.

## 3. Views

Four views of `site` put in one place what every route that shows a member
needs:

| View | Columns | Meaning |
|---|---|---|
| `site.members` | `id`, `pseudo`, `avatar_path`, `bio`, `member_since` | The public face of an account. `avatar_path` is relative to the `public` disk: the API builds the URL as `<PUBLIC_STORAGE_URL>/<path>` (`https://pulsars.science/storage/…`, which `pulsars-web` relays to `GET /v1/storage/{path}`). |
| `site.member_roles` | `user_id`, `slug`, `name`, `category`, `color`, `permission_level`, `display_order` | `category` is `permission`, `matiere` or `distinction`; `permission_level` is 0 except for the permission roles (1 `moderateur`, 2 `admin`, 3 `super-admin`). |
| `site.member_brains` | `user_id`, `balance`, `public` | The sum of the member's `cerveaux_mouvements.montant` (0 without movements); `public` is `users.cerveaux_publics` (`brains.md`). |
| `site.member_xp` | `user_id`, `total`, `level` | Experience points, **computed** from activity (lessons, exercises, courses, forum posts, accepted answers, games), never stored, with the old site's scale (`App\Services\Experience`). |

There is no member directory: no route lists every member, as on the old
site (`ProfilPublicController::index` showed nothing without a search).

## 4. Public content: what is read, what never is

The public content routes read `contenu` through `src/db/sources.rs`: one
SQL subquery per resource, each keeping **published rows only** and
**allowed columns only**. It is the security boundary of the public
content; a test dumps every source and checks that no answer, correction or
private column gets through.

| Source | Publication rule |
|---|---|
| courses, and their chapters, lessons and exercises | `statut = 'publie'` and `published_at <= now()` (the old `Course::scopePublies`) |
| exams, and their parts | `statut = 'publie'` and `published_at <= now()`; `attente_import` and `brouillon` (an AI import not yet reviewed) are left out |
| resources | `statut = 'publie'`, and a hosted file or a link |
| news | `published_at <= now()`; null is a draft |
| translations (`contenu_translations`) | their parent is visible |
| stored files | the statement PDF of a published exam, the file of a published hosted resource; never the correction, a draft's file or a path |

Never selected by these sources, whatever route uses them:

- `exercises.solution_md`, `exercises.donnees` (its `correctes` and
  `acceptees`), and the `solution_md` key of translations;
- `exam_parties.corrige_md` and its translation, `exams.fichier_corrige`;
- storage paths, original file names, AI prompts and errors, view and
  download counters, author account ids.

The answers and corrections have their own sources, read only by the
member routes that apply the old site's rules on when to show them (an
exercise's solution after an attempt, an exam's corrections on its page).

Response fields, with their types and meaning, are in `openapi.yaml`.

### Conventions of the stored data

- Timestamps are `timestamp(0) without time zone` in UTC, as the old site
  stored them; the API reads them `AT TIME ZONE 'UTC'` and answers ISO 8601
  with `Z`.
- Small integer columns are cast to `int` when read, so a type change
  cannot break decoding.
- Translations whose `source_hash` no longer matches the original are still
  served, as the old site did; `pulsars-api content translate` refreshes
  them.
- Reference values that the old site kept in PHP are tables now:
  `contenu.licences` (`config/licences.php`), `contenu.matieres` (subject
  names, `lang/*.json`), `contenu.sources_officielles` (the official exam
  URL allow-list, `SujetOfficiel::SOURCES`).

## 5. Known gaps

- No domain → subject → topic taxonomy: subject and topic are derived from
  a course's single `theme` string (`"<Subject> – <Topic>"`).
- No level column on courses: derived from tags with two spellings.
- Topic names have no translation.
- QCM option texts, exam titles and exam descriptions are not translatable.
- Lesson slug uniqueness is per chapter, not per course (unique per course
  in practice).
- Deleting an author deletes their courses (`auteur_id` cascades), as on
  the old site.
