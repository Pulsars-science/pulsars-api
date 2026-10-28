# Languages section

`pulsars.science/langues` teaches foreign languages, starting with Mandarin.
The front end is the `pulsars-langues` repository (Vue 3 + Vite, static,
served by nginx under `/langues/`, like the games under `/jeux/`). This API
serves its content.

## Data

Schema `langues`, owned by `pulsars_api` and written by this API only
(`migrations/20261020000000_langues_schema.sql`). The administrator creates
the schema once, **before** deploying the release that brings the
migration:

```sql
CREATE SCHEMA langues AUTHORIZATION pulsars_api;
```

| Table | Content |
|---|---|
| `languages` | One row per language taught: BCP 47 `code` (`zh`), `native_name` (`中文`), ISO 15924 `script` (`Hans`), `reading_system` (`pinyin`, or null), `countries` (ISO 3166-1 codes where it is spoken, `{CN,TW,SG}`), `position`, `published`. |
| `levels` | The levels of a language, in `position` order: `slug` (`hsk-1`), `framework` (`HSK`, `CECRL`…). |
| `words` | The words of a level, in `position` order: `text` (`你`), `reading` (`nǐ`), `meaning`. |

Interface texts (`name`, `title`, `description`, `meaning`) are JSON objects
keyed by interface language, French required (`{"fr": "…", "en": "…"}`).
Adding an interface language needs no schema change: only the `?lang=`
values of the API (today `fr` and `en`; Russian readers get French).

A new language taught is a new `languages` row with its levels and words,
added by a new migration until the site has an editor for it. Unpublished
languages are invisible to the API.

## Endpoints

Public content, same rules as the courses (`contract/README.md`): any
origin, ETags, `Cache-Control: public, max-age=300`, `?lang=fr|en` with
per-field fallback to French. See `contract/openapi.yaml`, tag `languages`.

- `GET /v1/languages`: published languages, with level and word counts.
- `GET /v1/languages/{code}`: one language and its levels.
- `GET /v1/languages/{code}/levels/{level}`: one level, all its words, and
  the previous and next levels.

## Statistics

The front end posts what learners do to `POST /v1/languages/events`, and
the site's admin dashboard reads the sums from
`GET /v1/admin/languages/stats` (session), like the game's;
`GET /v1/languages/admin/stats` gives the same to an admin game token,
server to server. See `contract/openapi.yaml`.

- `lesson.completed`: level, score, right answers out of questions,
  duration, ids of the missed words.
- `writing.completed`: level, character, `trace` (over the model) or
  `memory`, mistakes.

Rows go to `langues.events`. They are anonymous: `learner` is a random UUID
the browser draws once (one person on two devices counts twice); no
account, no IP address, no user agent. The posting origin must be in
`CORS_ALLOWED_ORIGINS` (`https://pulsars.science` in production).

## Later

Progress of each member (words learnt, reviews) will live in `langues` too,
behind a game token, like `/v1/game/me`.
