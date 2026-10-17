# Pulsars API contract

This folder is the contract between the three Pulsars services:

| Service | Repository | Role |
|---|---|---|
| `pulsars.science` | this Laravel repository | Pages, SEO, accounts, forum, XP, admin. **Issues game tokens.** Writes the `site` schema. |
| `api.pulsars.science` | `pulsars-api` (Rust, Axum) | Read-only content API (`/v1`) and the game server (`/v1/game`, WebSocket). Writes the `game` schema. |
| `game.pulsars.science` | `pulsars-game` (Vue 3 + Vite, static) | Game front end. Sends actions, never scores. |

It was written from the Laravel code as it stands on `main` at commit `6551770`
(2026-10-13). When this document and the code disagree, **the code is right**
and the gap is listed in the "Known gaps" section of the relevant document.

## Documents

| File | What it defines | Who implements it |
|---|---|---|
| [`openapi.yaml`](openapi.yaml) | Public read endpoints (`/v1/*`, `/healthz`), OpenAPI 3.1, with real examples | `pulsars-api` |
| [`auth-jwt.md`](auth-jwt.md) | The game token: format, claims, issuing endpoint, JWKS, key rotation, errors | Laravel issues, `pulsars-api` verifies |
| [`webhooks.md`](webhooks.md) | Match results sent from the API to Laravel: signature, idempotency, retries, current XP rules | `pulsars-api` sends, Laravel receives |
| [`database.md`](database.md) | Schemas, roles, exposed column dictionary | both |
| [`views.sql`](views.sql) | The `api_read` schema: one view per exposed resource | Laravel migration creates it |
| [`postgres-migration.md`](postgres-migration.md) | Plan to move production from MySQL to PostgreSQL (not executed) | ops |
| [`game-design.md`](game-design.md) | Game rules decided by the product owner | product owner |
| [`laravel-tasks.md`](laravel-tasks.md) | What the Laravel repository must implement for the API to work | Laravel |

Reading order for someone building `pulsars-api`: this README, `database.md`,
`views.sql`, `openapi.yaml`, `auth-jwt.md`, `webhooks.md`, `game-design.md`.

## Request flow

```
game.pulsars.science ──(1) POST /jeu/jeton, session cookie──▶ pulsars.science
                     ◀─(2) JWT (EdDSA, 5 min) ─────────────────┘
                     ──(3) WebSocket + JWT ──▶ api.pulsars.science
                                                 │ verifies with JWKS
                                                 │ (https://pulsars.science/.well-known/jwks.json)
                                                 │ reads site data via api_read views
                                                 │ writes matches to game schema
                                                 └─(4) signed webhook (private network) ──▶ pulsars.science
                                                                                            computes XP
```

## Glossary

The Laravel code is written in French. This contract uses English names; the
table maps both.

| Term | Laravel name | Meaning |
|---|---|---|
| **course** | `Course`, table `courses` | The base unit of the library. Has a title, description, difficulty, estimated duration, free tags, a theme, and a status. Identified publicly by its `slug`. |
| **chapter** | `Chapter`, table `chapters` | An ordered section of a course (`ordre`). No public identifier of its own; addressed by its position. |
| **lesson** | `Lesson`, table `lessons` | Markdown content inside a chapter. Identified by a `slug` that is unique **within its chapter** (see gaps). |
| **exercise** | `Exercise`, table `exercises` | A question attached to a chapter. Three types: `qcm` (multiple choice), `reponse_courte` (short answer, auto-graded), `reponse_libre` (free answer, self-assessed). Identified by its numeric `id` (exercises have no slug). |
| **exam** | `Exam`, table `exams` | A real exam paper (brevet, bac, competitive exam) split into **parts** (`exam_parties`). Identified by its `slug`. |
| **exam part** | `ExamPartie`, table `exam_parties` | One exercise of an exam: statement and model answer (`corrige_md`, never exposed). |
| **resource** | `Ressource`, table `ressources` | A free document: either **hosted** by Pulsars (a PDF on the private disk) or a **link** to its author's site. Carries a licence and an attribution. Identified by its `slug`. |
| **flash questions** | `App\QuestionsFlash\*` | 30 generators of short automatism questions, computed from a seed. **Not stored in the database**, therefore not exposed by this API. See `game-design.md`. |
| **domain / subject / topic** | *(target taxonomy)* | The target three-level catalogue: domain (pôle, e.g. "Sciences"), subject (matière, e.g. "Mathématiques"), topic (thème, e.g. "Analyse"). **Not implemented**: today a course has a single free-text `theme` string, usually `"<Subject> – <Topic>"`. The API derives subject and topic from it. There is no domain level. See `database.md`. |
| **member** | `User` | A Pulsars account. Its public face is the pseudo, avatar, bio and roles. |
| **role** | `Role`, tables `roles`, `role_user` | Three categories: `permission` (`moderateur` < `admin` < `super-admin`), `matiere` (declared interests), `distinction` (badges). |
| **XP / level** | `App\Services\Experience` | Experience points, **computed** from activity, never stored. See `webhooks.md`. |

## General conventions

### Versioning

- Every content endpoint is prefixed with `/v1`. The game lives under `/v1/game`.
- Within `v1`, changes are **additive only**: new endpoints, new optional query
  parameters, new response fields. Clients must ignore unknown fields.
- Removing or renaming a field, changing a type or tightening validation
  requires `/v2`. `/v1` then stays available for at least six months, and its
  responses carry `Deprecation` and `Sunset` headers (RFC 8594 / RFC 9745).
- `/healthz` is not versioned.

### Errors: RFC 9457

Every error response has the media type `application/problem+json`:

```json
{
  "type": "https://api.pulsars.science/problems/not-found",
  "title": "Resource not found",
  "status": 404,
  "detail": "No published course has the slug 'les-bases-du-calcul-2'.",
  "instance": "/v1/courses/les-bases-du-calcul-2"
}
```

| `type` suffix | Status | When |
|---|---|---|
| `invalid-parameter` | 400 | A query parameter is malformed or out of range. Adds `"errors": [{"parameter": "limit", "detail": "must be between 1 and 100"}]`. |
| `invalid-cursor` | 400 | The `cursor` cannot be decoded, or belongs to another endpoint or filter set. |
| `unauthorized` | 401 | Missing, invalid or expired token. Also sets `WWW-Authenticate: Bearer error="invalid_token"` (RFC 6750). See `auth-jwt.md`. |
| `forbidden` | 403 | Valid token, but not allowed. |
| `not-found` | 404 | Unknown, unpublished or hidden resource. **Unpublished content answers 404, never 403**, so its existence does not leak. |
| `method-not-allowed` | 405 | |
| `not-acceptable` | 406 | `Accept` excludes `application/json`. |
| `rate-limited` | 429 | See rate limits below. Sets `Retry-After`. |
| `internal` | 500 | Never includes a stack trace, SQL or a file path. Adds `"trace_id"` so a report can be matched to logs. |
| `unavailable` | 503 | A required dependency (PostgreSQL) is down. |

The `type` URIs do not have to resolve today; they are stable identifiers.

### Pagination: cursor-based

- Lists accept `limit` (default `20`, minimum `1`, maximum `100`) and `cursor`.
- Lists answer:

  ```json
  { "data": [ ... ], "next_cursor": "eyJrIjpbIkxlcyBiYXNlcyBkdSBjYWxjdWwiLDFdfQ" }
  ```

- `next_cursor` is `null` on the last page.
- A cursor is **opaque** to clients. Internally it is a base64url JSON with the
  sort key of the last returned row plus its `id` (keyset pagination), and a
  hash of the filters it was issued for; reusing it with other filters gives
  `400 invalid-cursor`.
- Each list endpoint has one fixed, documented sort order, ending with `id` as
  a tie-breaker. There is no total count: it would cost a full scan per page.

### Content language: `?lang=fr|en`

- All content was written in **French** (`fr`, the source language,
  `config/localisation.php` → `source`). English translations live in the
  `contenu_translations` table and cover most courses, chapters, lessons,
  exercises, exam parts and resources.
- `?lang=` accepts `fr` or `en`. When absent, the API serves the original
  language (`fr`). `Accept-Language` is **not** used: the query string keeps
  responses cacheable without `Vary`.
- Translation falls back **per object and per field**: a missing or empty
  translated field is replaced by the original, exactly like
  `App\Models\Concerns\Traduisible::t()`.
- Every translatable object in a response carries a `lang` field: the language
  actually served for that object. It is `en` when at least the main text field
  (title) came from the English translation, `fr` otherwise.
- Responses carry `Content-Language: <requested lang>`.
- Some fields are never translated, whatever `lang` says (known gaps): QCM option
  texts, exam titles and descriptions, topic names, resource category in
  lists derived from the original. See `openapi.yaml` field descriptions.

### Dates, identifiers, formats

- Dates are ISO 8601 in UTC with a `Z` suffix: `2026-08-17T09:27:34Z`.
- Public identifiers are **slugs** where they exist (courses, lessons within a
  course, exams, resources). Exercises and exam parts use their numeric `id`.
- User identifiers are **strings** (`"42"`), the same value as the JWT `sub`
  claim, so a client never has to convert between the two.
- Markdown fields end in `_md` and contain the raw Markdown stored by Laravel,
  including LaTeX (`$...$`) and fenced code. The API does not render HTML.
- Every resource has a `web_url` pointing to its canonical page on
  `https://pulsars.science`, so a client can always link back to the site.
- JSON keys are `snake_case`. Enumerated values keep their Laravel values
  (`debutant`, `reponse_courte`, `brevet`), which are stable database values.

### Rate limits

| Caller | Limit | Key |
|---|---|---|
| Anonymous | 60 requests / minute | client IP (from `X-Forwarded-For`, set by the edge only) |
| Authenticated (`Authorization: Bearer`) | 120 requests / minute | JWT `sub` |

These are the limits Laravel's current `/api/v1` applies. Every response
carries the draft IETF headers (`draft-ietf-httpapi-ratelimit-headers`):

```http
RateLimit-Limit: 60
RateLimit-Remaining: 57
RateLimit-Reset: 41
```

A `429` adds `Retry-After: <seconds>`.

### Caching

| Response | Headers |
|---|---|
| Public content, `200` | `Cache-Control: public, max-age=300, stale-while-revalidate=60` and a strong `ETag` (hash of the body). |
| Conditional request with matching `If-None-Match` | `304 Not Modified`, no body. |
| `/v1/me` and anything authenticated | `Cache-Control: private, no-store`, `Vary: Authorization`. |
| Errors | `Cache-Control: no-store`, except `404` on public content: `public, max-age=60`. |
| `/healthz` | `Cache-Control: no-store`. |

`max-age=300` matches what Laravel's `PublicApiHeaders` middleware sends today.

### Other headers

- `X-Content-Type-Options: nosniff` and `X-Robots-Tag: noindex, nofollow` on
  every response (same as Laravel's API today).
- `traceparent` (W3C Trace Context) is accepted and propagated; responses
  carry `X-Request-Id`.
- CORS: content endpoints allow any origin without credentials
  (`Access-Control-Allow-Origin: *`); `/v1/me` and `/v1/game/*` allow only
  `https://game.pulsars.science` (plus `http://localhost:5173` in development).

## What this API never exposes

These rules are enforced twice: by the `api_read` views (the columns do not
exist for the API's database role) and by the OpenAPI schemas.

1. **Unpublished content.** Drafts, archived courses, exams awaiting import or
   review, resources not in `publie` status, hidden or deleted forum content.
2. **Solutions and expected answers.** Exercise `solution_md`, QCM `correctes`,
   short-answer `acceptees`, exam `corrige_md` and the correction PDF. The game
   may reuse this content: an exposed answer is a cheat.
3. **Personal data** beyond the public profile: no e-mail, IP address, date of
   birth, school level or class, locale, social logins, 2FA data, friend token,
   mute status, onboarding answers.
4. **Internal fields**: storage paths, original file names, AI import prompts
   and errors, view and download counters, author account ids.

## Known gaps between the target architecture and the code

Each document has its own list. The most important ones:

- Production runs **MySQL**, not SQLite and not PostgreSQL
  (`docs/MISE-EN-PROD.md`, `deploy/*.service`). The architecture requires
  PostgreSQL: see `postgres-migration.md`.
- There are **no guilds** in the code (planned in `PROJET.md` v2, only a
  marketing card on the home page).
- **XP is not stored**; it is computed from activity tables. Awarding XP for
  a match means adding a new source to `App\Services\Experience`.
- The **domain → subject → topic** taxonomy does not exist; courses have one
  `theme` string.
- Laravel already serves a JSON API under `/api/v1` (`docs/API.md`). Its
  `/api/v1/me` returns the e-mail address and its `/api/v1/examens/{slug}`
  returns exam corrections publicly: both contradict the rules above.
