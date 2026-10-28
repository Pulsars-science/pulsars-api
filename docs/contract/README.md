# Pulsars API contract

This folder is the contract of `pulsars-api`, the back end of Pulsars. The
API owns the database and every feature; three kinds of clients call it:

| Client | Repository | How it calls the API |
|---|---|---|
| `pulsars.science` | `pulsars-web` (Nuxt, server rendering, Node) | From its server, and through its `/api/v1/**` relay: `Pulsars-Session` (from the `pulsars_auth` cookie), `Pulsars-Service-Key`, the visitor's `X-Forwarded-For` and `User-Agent` (`auth-sessions.md`). |
| `pulsars.science/jeux` | `pulsars-game` (Vue 3 + Vite, static) | From the browser, to `api.pulsars.science`: a short JWT from `POST /v1/auth/token` (`auth-jwt.md`), then HTTP and the match WebSocket. Sends actions, never scores. |
| `pulsars.science/langues` | `pulsars-langues` (static) | From the browser: the languages content and anonymous learning events (`docs/languages.md`); the same short JWT when it acts as a member. |

Anyone may also read the public content (`/v1/courses`, `/v1/exams`, …)
without a token.

When this contract and the code disagree, the code is right and the
contract is fixed in the same change.

## Documents

| File | What it defines |
|---|---|
| [`openapi.yaml`](openapi.yaml) | Every route (`/v1/*`, `/healthz`, `/.well-known/jwks.json`), OpenAPI 3.1, with examples |
| [`auth-sessions.md`](auth-sessions.md) | Sessions, passwords, account mails and Google accounts: how the site logs members in |
| [`auth-jwt.md`](auth-jwt.md) | The game token: format, claims, issuing endpoint, the API's key and JWKS, errors |
| [`database.md`](database.md) | Schemas, roles, who writes what (only the API), the views, what public content never shows |
| [`brains.md`](brains.md) | The brains currency: ledger, earning, monthly duel seasons |
| [`game-design.md`](game-design.md) | Game rules decided by the product owner |

Reading order for someone new to the API: this README, `database.md`,
`openapi.yaml`, `auth-sessions.md`, `auth-jwt.md`, `brains.md`,
`game-design.md`, then `docs/game-protocol.md`.

## Request flow

```
browser ──▶ pulsars.science (nginx)
              ├─ /jeux/, /langues/   static files
              └─ everything else ──▶ pulsars-web (Nuxt, 127.0.0.1:3000)
                                       │  Pulsars-Session, Pulsars-Service-Key,
                                       ▼  X-Forwarded-For
                                     pulsars-api (127.0.0.1:8080) ──▶ PostgreSQL, Redis

pulsars.science/jeux ──(1) POST /v1/auth/token, pulsars_auth cookie──▶ api.pulsars.science
                     ◀─(2) JWT (EdDSA, 5 min, signed by the API) ──────┘
                     ──(3) WebSocket + JWT ──▶ api.pulsars.science
                                                 │ verifies with its own key
                                                 └─(4) match, ratings, results and brains, in one transaction
```

## Glossary

The tables keep the French names of the old Laravel site. This contract
uses English names; the table maps both.

| Term | Old site's name, table | Meaning |
|---|---|---|
| **course** | `Course`, table `courses` | The base unit of the library. Has a title, description, difficulty, estimated duration, free tags, a theme, and a status. Identified publicly by its `slug`. |
| **chapter** | `Chapter`, table `chapters` | An ordered section of a course (`ordre`). No public identifier of its own; addressed by its position. |
| **lesson** | `Lesson`, table `lessons` | Markdown content inside a chapter. Identified by a `slug` that is unique **within its chapter** (see gaps). |
| **exercise** | `Exercise`, table `exercises` | A question attached to a chapter. Three types: `qcm` (multiple choice), `reponse_courte` (short answer, auto-graded), `reponse_libre` (free answer, self-assessed). Identified by its numeric `id` (exercises have no slug). |
| **exam** | `Exam`, table `exams` | A real exam paper (brevet, bac, competitive exam) split into **parts** (`exam_parties`). Identified by its `slug`. |
| **exam part** | `ExamPartie`, table `exam_parties` | One exercise of an exam: statement and model answer (`corrige_md`, never exposed). |
| **resource** | `Ressource`, table `ressources` | A free document: either **hosted** by Pulsars (a PDF on the private disk) or a **link** to its author's site. Carries a licence and an attribution. Identified by its `slug`. |
| **flash questions** | `App\QuestionsFlash\*` | 30 generators of short automatism questions, computed from a seed. **Not stored in the database**: the API generates them for the game (`src/flash/`). See `game-design.md`. |
| **domain / subject / topic** | *(target taxonomy)* | The target three-level catalogue: domain (pôle, e.g. "Sciences"), subject (matière, e.g. "Mathématiques"), topic (thème, e.g. "Analyse"). **Not implemented**: a course has a single free-text `theme` string, usually `"<Subject> – <Topic>"`. The API derives subject and topic from it. There is no domain level. See `database.md`. |
| **member** | `User` | A Pulsars account. Its public face is the pseudo, avatar, bio and roles. |
| **role** | `Role`, tables `roles`, `role_user` | Three categories: `permission` (`moderateur` < `admin` < `super-admin`), `matiere` (declared interests), `distinction` (badges). |
| **XP / level** | `App\Services\Experience` | Experience points, **computed** from activity, never stored (the view `site.member_xp`). |

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
| `invalid-body` | 400 | The JSON body cannot be parsed (no `errors`), or does not fit the endpoint. In the second case `errors` names each field by JSON Pointer (RFC 6901), `""` for the body itself: ``"errors": [{"parameter": "/email", "detail": "invalid type: integer `3`, expected a string"}]``. A missing field is reported at `""`, an unknown field at its own pointer. |
| `invalid-body` | 422 | A well-formed body whose values break the endpoint's rules (`/v1/auth/*`): every field error at once, each with a stable `code`, e.g. `{"parameter": "pseudo", "detail": "is already used by another account", "code": "taken"}`. See `auth-sessions.md`. |
| `invalid-credentials` | 422 | `POST /v1/auth/login`: unknown login, wrong password, or an account without a password. Always the same body. |
| `invalid-cursor` | 400 | The `cursor` cannot be decoded, or belongs to another endpoint or filter set. |
| `unauthorized` | 401 | Missing, invalid or expired token. Also sets `WWW-Authenticate: Bearer error="invalid_token"` (RFC 6750). See `auth-jwt.md`. |
| `token-unavailable` | 503 | `POST /v1/auth/token` without a signing key configured (`JWT_SIGNING_KEY_FILE`). |
| `session-invalid` | 401 | `Pulsars-Session` missing, unknown, expired or revoked. Sets `WWW-Authenticate: Session realm="api.pulsars.science"`. |
| `invalid-token` | 422 | A link sent by mail (`/v1/auth/email/verify`, `/v1/auth/password/reset-with-token`) that is unknown, used, expired, or was sent to another address. `errors` names `token`, code `invalid`. |
| `conflict` | 409 | The request conflicts with an account's state; `reason` says how: `taken` (a provider identity linked to another account), `email_unverified` (an existing address the provider does not vouch for), `last_login_method` (unlinking the only way to log in). |
| `forbidden` | 403 | Valid token, but not allowed; or an endpoint reserved to the service key. |
| `not-found` | 404 | Unknown, unpublished or hidden resource. **Unpublished content answers 404, never 403**, so its existence does not leak. |
| `method-not-allowed` | 405 | Sets `Allow` to the methods of that path, e.g. `GET,HEAD,OPTIONS`. |
| `not-acceptable` | 406 | `Accept` excludes `application/json`. |
| `payload-too-large` | 413 | The body is over the size limit. |
| `unsupported-media-type` | 415 | A body whose `Content-Type` is not `application/json`. |
| `range-not-satisfiable` | 416 | A file route's `Range` starts past the end of the file; `Content-Range: bytes */{size}`. |
| `rate-limited` | 429 | See rate limits below, and the login and sign-up limits of `auth-sessions.md`. Sets `Retry-After`. |
| `internal` | 500 | Never includes a stack trace, SQL or a file path. Adds `"trace_id"` so a report can be matched to logs. |
| `unavailable` | 503 | A required dependency (PostgreSQL) is down. |

The `type` URIs do not have to resolve today; they are stable identifiers.

### Request bodies

Endpoints that take a body take JSON: `Content-Type: application/json`,
UTF-8, one JSON value and nothing after it. Fields the endpoint does not
know are refused (`invalid-body`), never ignored, so a misspelt field
cannot be silently dropped. The size limit is the server's
`BODY_LIMIT_BYTES` (64 KiB by default) unless an endpoint documents its own.

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
- Each list endpoint has a fixed, documented sort order, ending with `id` as
  a tie-breaker. `GET /v1/courses` also takes `sort=recent`.
- There is no total count on cursor pages. `GET /v1/courses` and
  `GET /v1/exams` also take `page` (from 1, not with `cursor`) for clients
  that show numbered pages, as the site does: the answer then has `total`
  and `next_cursor: null`. Both tables are small, so the count is cheap
  there.

### Content language: `?lang=fr|en|ru`

- All content was written in **French** (`fr`, the source language,
  `config/localisation.php` → `source`). English and Russian translations live
  in the `contenu_translations` table. English covers most courses,
  chapters, lessons, exercises, exam parts and resources.
- `?lang=` accepts `fr`, `en` or `ru`. When absent, the API serves the original
  language (`fr`). `Accept-Language` is **not** used: the query string keeps
  responses cacheable without `Vary`.
- Translation falls back **per object and per field**: a missing or empty
  translated field is replaced by the original, exactly like
  `App\Models\Concerns\Traduisible::t()`.
- Every translatable object in a response carries a `lang` field: the language
  actually served for that object. It is the requested language when at least the
  main text field (title) came from its translation, `fr` otherwise.
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
- Markdown fields end in `_md` and contain the raw Markdown as stored,
  including LaTeX (`$...$`) and fenced code. The API does not render HTML.
- Every resource has a `web_url` pointing to its canonical page on
  `https://pulsars.science`, so a client can always link back to the site.
- JSON keys are `snake_case`. Enumerated values keep their database values
  (`debutant`, `reponse_courte`, `brevet`), which are stable.

### Rate limits

| Caller | Limit | Key |
|---|---|---|
| Anonymous | 60 requests / minute | client IP (from `X-Forwarded-For`, set by the edge only) |
| Authenticated (`Authorization: Bearer`) | 120 requests / minute | JWT `sub` |
| The site (`Pulsars-Service-Key`) | none | — |

The site calls the API from the same machine for all its visitors, so its
calls would all share the loopback's limit. They carry the server's
`SERVICE_KEY` in `Pulsars-Service-Key` instead, and are not counted. The
key never leaves the server: browsers and games never see it. Some routes
have their own limits on top (logins, sign-ups, forms: `auth-sessions.md`,
`openapi.yaml`), counted even with the service key.

These are the limits the old site's `/api/v1` applied. Every response
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
| `/v1/me` and anything authenticated | `Cache-Control: private, no-store`, `Vary: Authorization`; member routes `no-store`. |
| Errors | `Cache-Control: no-store`, except `404` on public content: `public, max-age=60`. |
| `/healthz` | `Cache-Control: no-store`. |

`max-age=300` matches what the old site's `PublicApiHeaders` middleware sent.

### Other headers

- `X-Content-Type-Options: nosniff` and `X-Robots-Tag: noindex, nofollow` on
  every response.
- `traceparent` (W3C Trace Context) is accepted and propagated; responses
  carry `X-Request-Id`.
- CORS: content endpoints and `/v1/game/leaderboard` allow any origin
  without credentials (`Access-Control-Allow-Origin: *`); `/v1/me`,
  `/v1/game/me` and `/v1/game/admin/stats` allow only `CORS_ALLOWED_ORIGINS`
  (requests without `Origin`, server to server, are served as usual) (default
  `https://pulsars.science`, plus `http://localhost:5173` in
  development). The match WebSocket `/v1/game/ws` refuses (`403`, before the
  upgrade) any `Origin` outside that list. The site's routes (`/v1/auth/*`,
  `/v1/me/*`, the forum, social and admin routes) have no CORS layer: they
  are called by `pulsars-web`'s server, never straight from a browser.

## What this API never exposes

The public content routes enforce these rules in their SQL sources
(`src/db/sources.rs`, `database.md` §4) and in the OpenAPI schemas; the
member routes apply the old site's rules on what a member, an owner or a
moderator may see.

1. **Unpublished content.** Drafts, archived courses, exams awaiting import or
   review, resources not in `publie` status, hidden or deleted forum
   content, to anyone but the admins and moderators whose routes show them.
2. **Solutions and expected answers** on public routes. Exercise
   `solution_md`, QCM `correctes`, short-answer `acceptees`, exam
   `corrige_md` and the correction PDF. The game may reuse this content: an
   exposed answer is a cheat. A member sees an exercise's solution after an
   attempt, and an exam's corrections on its page, as on the old site.
3. **Personal data** beyond the public profile: no e-mail, IP address, date of
   birth, school level or class, locale, social logins, 2FA data, friend
   token, mute status, onboarding answers, except to the account's owner
   (`/v1/me` with a session, `/v1/me/profile`).
4. **Internal fields**: storage paths, original file names, AI import prompts
   and errors, view and download counters, author account ids.
5. **Secrets**: password hashes, session and mail-link tokens (only their
   SHA-256 is stored), private messages in clear (sealed with
   `MESSAGES_KEY`, AES-256-GCM).

## Known gaps

- There are **no guilds** (planned, never built).
- **XP is not stored**; it is computed from activity (`site.member_xp`).
- The **domain → subject → topic** taxonomy does not exist; courses have one
  `theme` string.
