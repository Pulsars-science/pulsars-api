# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.5.0] - 2026-10-29

### Upgrading

- From 0.2.0 (0.3.0 and 0.4.0 were never installed): `deploy/sql/README.md`,
  phase 1 (steps 1 to 7), then phase 2 (steps 8 to 10).

- Two migrations (`20261029000000_auth_sessions`,
  `20261030000000_auth_roles`): `deploy-api` applies them as
  `pulsars_migrate`. They give `pulsars_api` the login columns of
  `site.users` and read access to the permission roles. A third
  (`20261031000000_email_tokens`) adds the links sent by mail and lets the
  API mark addresses verified and link Google accounts.
- Optional: `MAIL_SMTP_URL` (and `MAIL_FROM`) for the account mails; unset,
  they are only logged, without their link.
- Optional: the API's game-token key (`pulsars-api jwt-keygen`, then
  `JWT_SIGNING_KEY_FILE` and `JWT_SIGNING_KID`).

### Added

- Account mails sent by the API (`lettre`, SMTP), in fr/en/ru with the
  site's wording: `POST /v1/auth/email/verification` and
  `/v1/auth/email/verify` (verification link, 24 h, sent on sign-up too),
  `POST /v1/auth/password/forgot` (never says whether the account exists,
  one mail a minute per account) and `/v1/auth/password/reset-with-token`
  (60 min, revokes every session). Links are single-use token hashes in
  `site.email_tokens`.
- `POST /v1/auth/oauth/resolve` (service key): finds, links or creates the
  account of a Google identity with the site's rules, then opens a
  session; `DELETE /v1/auth/oauth/{provider}` unlinks, except the last way
  to log in.
- Problems `invalid-token` (422) and `conflict` (409, with a `reason`).

- `/v1/auth/*` (`docs/contract/auth-sessions.md`): log in by e-mail or
  pseudo (ignoring case), sign up with Laravel's rules, sessions as opaque
  tokens in `site.auth_sessions` (`Pulsars-Session`), log out, change a
  password (also for accounts made through Google), and, with the service
  key, open a session for an account or reset its password. Bcrypt in
  Laravel's `$2y$` format; per-action limits on logins and sign-ups.
- `422 invalid-body` with a `code` per field, `invalid-credentials`,
  `session-invalid`.
- `BCRYPT_COST` (default 12).
- `POST /v1/auth/token`: the API issues the game token itself, from the
  member's session (`pulsars_auth` cookie or `Pulsars-Session`), with the
  same origin checks, gates and body as Laravel's `/jeu/jeton`. Signed with
  the API's own key, `iss` `https://api.pulsars.science`; 503
  `token-unavailable` without a key.
- `GET /.well-known/jwks.json` on the API: its public signing key.
- `pulsars-api jwt-keygen <kid> <dir>`: makes that key.

### Changed

- Game tokens are accepted from two issuers: the API's own (verified with
  its key, no network) and the site's (its JWKS), as before.

## [0.4.0] - 2026-10-28 (never tagged: its changes ship in 0.5.0)

### Upgrading

- From 0.2.0, as 0.3.0 was never installed: follow `deploy/sql/README.md`
  (the phase 1 script before `deploy-api`, as for 0.3.0), with the new
  step for `SERVICE_KEY`.
- From 0.3.0: a plain `deploy-api`. No migration.
- `SERVICE_KEY` is optional; the site needs it before it reads content
  through the API.

### Added

- `lang=ru`: Russian translations, falling back to French per object and
  per field as English does. `available_in=ru`, and `ru` in
  `available_langs`.
- `GET /v1/courses`: `sort=recent` (latest published first), `theme`, and
  `page` for numbered pages, answered with `total`.
- `GET /v1/exams`: `page` and `total`.
- `GET /v1/exams/facets`: the levels and countries of published exams.
- Lessons carry their `id`, in a course's outline and in a lesson.
- Exams in `GET /v1/exams` carry their `description`, as the detail did.
- `SERVICE_KEY`: requests carrying it in `Pulsars-Service-Key` (the site,
  reading content for its visitors) are not rate limited.

## [0.3.0] - 2026-10-28

### Upgrading

Not a plain `deploy-api`: 0.3.0 reads the `contenu` schema that
`deploy/sql/phase-1.sql` creates, and migrates as `pulsars_migrate`. Follow
`deploy/sql/README.md`: the site's `DB_SEARCH_PATH`, the script (from this
archive's `sql/`), `pg_hba.conf` and `/etc/pulsars-api/migrate.env`, then
`deploy-api`, which stops before the switch if anything is missing or if
`check-content` finds a difference. 0.2.0 keeps serving in between.

### Added

- `GET /v1/exams/{slug}/pdf` and `GET /v1/resources/{slug}/file`: the
  statement PDF of a published exam and the hosted file of a published
  resource, read from `contenu.fichiers`. One `Range`, `If-Range`,
  `If-None-Match`, `disposition=inline|attachment`; 416
  `range-not-satisfiable` past the end. Never a correction, a draft's file
  or a storage path: files are found by the published content's slug only.
  PDF responses are not compressed (that would drop `Accept-Ranges`).
- `GET /v1/news` and `GET /v1/news/{slug}`: the team's published
  announcements (`contenu.actualites`), newest first, with `q`, `lang` and
  cursor pagination; the detail adds the Markdown content. Drafts (no date)
  and scheduled ones (future date) answer 404, as on the site.
- `extract::Json<T>`, the extractor for JSON request bodies of the write
  endpoints to come. Its rejections are problems in this API's vocabulary:
  415 `unsupported-media-type` for a `Content-Type` other than
  `application/json`, 413 `payload-too-large`, and 400 `invalid-body`, whose
  `errors` names the field at fault by JSON Pointer (`docs/contract/README.md`,
  "Errors" and "Request bodies").
- `auth::Admin` and `auth::Moderator` extractors: a verified token whose
  `roles` include `admin`/`super-admin` (or also `moderateur`), else 403
  `insufficient-scope`. `GET /v1/game/admin/stats` uses `Admin`; its 403
  detail is now "This endpoint needs the admin or super-admin role.".
- `scripts/site-schema-snapshot.sh`: regenerates `dev/db/20-site-schema.sql`
  from the Laravel migrations, run on a throwaway PostgreSQL 17.
- `pulsars-api migrate`: applies the embedded migrations and exits, as
  `DATABASE_MIGRATION_URL` (the new `pulsars_migrate` role) or, without it,
  `DATABASE_URL`. `deploy-api` runs it with the new release before the
  switch, reading the migration account from `/etc/pulsars-api/migrate.env`
  (root only, never the service).
- `deploy/sql/phase-1.sql` and `phase-1-retour.sql` (in the release archive
  under `sql/`): `pulsars_owner` owns every table, `pulsars_migrate` runs the
  migrations, the content tables move to a `contenu` schema, and neither
  `pulsars_site` nor `pulsars_api` may create, alter or drop anything
  (`docs/contract/database.md` §2.1). Production order and rollback:
  `deploy/sql/README.md`. `scripts/dev-db-init.sh` applies it, so local and
  CI databases have the production layout.

- `pulsars-api check-content`: compares what the API reads of `contenu`
  with the `api_read` views it replaces, row for row, on the real data, and
  fails on a single difference (`deploy/sql/README.md`).
- Migration `20261028000000_contenu_reference`: `contenu.licences`,
  `contenu.matieres` (fr, en, ru) and `contenu.sources_officielles`, the
  values the views copied from PHP, and `contenu.slugify()`. It refuses to
  run before `deploy/sql/phase-1.sql`, so `deploy-api` stops before the
  switch on a database that is not ready.
- `dev/db/25-api-read.sql`: the `api_read` views as Laravel builds them,
  generated by `scripts/site-schema-snapshot.sh`; the tests run on them
  instead of `docs/contract/views.sql`, which had drifted.

### Changed

- Content is read from `contenu` directly, through the subqueries of
  `src/db/sources.rs` (same published rows, same columns as the views; no
  solution, no correction): `docs/contract/database.md` §2.2. Responses are
  byte for byte the same: compared on a copy of the production database.
  Needs `deploy/sql/phase-1.sql`.
- The service no longer migrates at startup: it waits, logging the missing
  versions, until every embedded migration is applied, then starts the
  season close. Integration tests migrate with `TEST_MIGRATION_DATABASE_URL`.
- `dev/db/20-site-schema.sql` is the real Laravel `site` schema (48 tables,
  generated), no longer a hand-written stand-in with only the columns the
  views read. Local and CI databases now have every table and constraint the
  site has; the demo data gained the forum theme a question requires.

### Security

- Dependencies with RustSec advisories published after 0.2.0 updated:
  tokio 1.54.1 (RUSTSEC-2026-0386), rustls-webpki 0.103.16
  (RUSTSEC-2026-0382), webpki-roots and webpki-root-certs 1.0.11
  (RUSTSEC-2026-0381, -0383), ctutils 0.4.6 (RUSTSEC-2026-0385).

### Fixed

- A 405 lists the methods of the path it answers for (`Allow: GET,HEAD`),
  instead of the same `GET, HEAD, OPTIONS` everywhere.
- CI and release workflows no longer set `SQLX_OFFLINE`: no query uses the
  checked macros, so it did nothing.

## [0.2.0] - 2026-10-17

### Added

- Russian (`ru`) for the flash questions and the game server
  (`docs/game-protocol.md` §2, §3.1): `Lang::Ru` (any locale starting with
  `ru`), `src/flash/ru.json` (copy of the game's `src/i18n/ru.json`),
  Russian numbers as `Intl.NumberFormat('ru')` writes them (U+00A0 thousands
  separator, decimal comma, real minus), `DuelQuestion::ru` drawn from the
  same cloned stream as English and held to the same agreement check with
  French. Players whose token or guest `auth` frame has a `ru*` locale get
  their questions, series titles and choices in Russian; guests are named
  `Гость 1234`. Unit symbols (`kg`, `m/s`, `base 2`…) now go through the
  dictionary so Russian can write them in Cyrillic; French and English
  output is unchanged (`ucfirst` also capitalises a non-ASCII first letter,
  which no French or English text starts with).
  `scripts/flash-fixtures.mjs` now also writes Russian duels and
  `tests/fixtures/flash/generators-ru.json`; `tests/flash.rs` checks the
  Rust port against them.

- `GET /v1/game/admin/stats?days=30` (`docs/game-protocol.md` §9.1), for
  the Laravel admin dashboard: live counters from the hub (sessions,
  guests, queued players, open rooms, running matches by mode, seated
  players), today's matches per UTC hour with the accounts and guest
  seats (running matches included), `days` (1–90) daily rows zero-filled,
  matches per subject and per rules, match quality (mean duration,
  answers per player, accuracy), rated players, webhook outbox backlog and
  the last 12 closed seasons. Needs a game token whose `roles` contain
  `admin` or `super-admin`, else `403` problem with
  `reason: insufficient_scope` and an RFC 6750 `insufficient_scope`
  challenge. Restricted CORS; `Cache-Control: no-store`. The queries run
  in one read-only repeatable-read transaction; new migration with the
  `game.matches (started_at)` index and a partial index on dead outbox
  rows.

- Brains (`docs/contract/brains.md`): monthly duel season close. At startup
  and every 10 minutes, the previous UTC month is closed for each subject
  key in one transaction: standings (at least 5 ranked duels ended in the
  month; by rating, then ranked duels, then `user_id`), a `season.ended`
  webhook when they are not empty, the decay of every rating of the subject
  (`1500 + 0.8 × (rating − 1500)`, RD + 50 capped at 350), and a
  `game.seasons` row (new migration) so that a month closes once, across
  restarts and instances (advisory lock). Metric
  `game_seasons_closed_total`.
- `brains` (the balance, from the new `api_read.member_brains` view) in
  `GET /v1/me` and `GET /v1/game/me`.

- Flash-duel game server (`docs/game-protocol.md`): WebSocket
  `GET /v1/game/ws` with the `Origin` checked before the upgrade, `auth`
  first frame for accounts (game token, single-use `jti` in Redis) and
  guests (resume token), one live session per account (`4009`), 20 frames
  a second per client (`4029`), pings every 30 s.
- Matches: 3-second countdown, the same seeded questions for every player at
  their own pace, one at a time, never with the answer; duration (60 s,
  300 s) and target (10 points, 300 s cap) modes, optional wrong-answer
  penalty; end reasons `completed`, `timeout`, `forfeit` and `void`; 20 s
  reconnection grace; ranking by score then total answer time; anti-cheat
  (more than 3 correct answers under 300 ms make the match unranked).
- In-memory matchmaking per subject and rules, with a rating window of ±100
  widened by 50 every 5 s; guests in a casual queue; private rooms of 2 to 8
  players with invitation codes; ghost matches replaying another account's
  recorded run on the same seed.
- Glicko-2 ratings per subject (and `all`) for ranked duels between
  accounts.
- `GET /v1/game/leaderboard` (public) and `GET /v1/game/me` (token): ratings,
  ranks and the last 20 matches.
- The `game` schema (`migrations/`), applied at startup: `matches`,
  `match_players`, `ratings`, `webhook_outbox`.
- Results webhook (`docs/contract/webhooks.md`): `match.completed` written to
  the outbox in the match-end transaction, delivered with an HMAC-SHA256
  signature over `timestamp.body`, exponential backoff with full jitter,
  dead letters after 20 attempts or 24 hours, and
  `pulsars-api webhooks replay <event_id>`. Settings
  `LARAVEL_WEBHOOK_URL`, `LARAVEL_WEBHOOK_SECRET`, `LARAVEL_WEBHOOK_HOST`.
- Metrics `game_connections_total`, `game_auth_failures_total`,
  `game_sessions`, `game_queued_players`, `game_matches_live`,
  `game_matches_started_total`, `game_matches_ended_total`,
  `game_persist_errors_total`, `webhook_attempts_total`, `webhook_pending`,
  `webhook_dead_total`, `webhook_latency_seconds`.
- `flash` module: the 30 flash-question generators of the site, ported from
  `pulsars-game` with PHP's Mt19937 randomness (`Hasard`), French and
  English texts and number formatting, and `duel_questions`, the questions
  of a duel in both languages (the "défi" algorithm, for any subject; a
  question with fewer than four distinct choices is padded without
  disturbing the following ones). Parity tests against fixtures produced
  by PHP and by the TypeScript port (`scripts/flash-fixtures.mjs`).

### Fixed

- The local and CI database stand-in (`dev/db/`) has the `site` tables the
  views now read: `game_events`, `game_results` (needed by
  `api_read.member_xp` since the game XP), `cerveaux_mouvements` and
  `users.cerveaux_publics`; demo brains for `ada`.
- QCM options are translated: `GET /v1/exercises` and `/v1/exercises/{id}`
  with `lang=en` serve the English option texts (same ids and order), falling
  back to the original text option by option. Needs the Laravel migration
  `add_options_to_api_read_exercise_translations`.

## [0.1.0] - 2026-10-16

### Added

- Architecture and API contract documents (`docs/`).
- Axum service skeleton: typed configuration validated at startup, JSON logs,
  W3C `traceparent` handling, Prometheus metrics on an internal listener,
  RFC 9457 problem responses, graceful shutdown on SIGTERM.
- `GET /healthz` checking PostgreSQL and Redis.
- Local development stack (`compose.yaml`) with demo data and the real
  `api_read` views.
- CI: formatting, Clippy, tests against PostgreSQL and Redis, cargo-deny,
  DCO sign-off check.
- Open-source project files: licence, contributing guide, code of conduct,
  security policy, trademark policy, issue and pull request templates.
- Game token verification (`docs/contract/auth-jwt.md`): EdDSA (Ed25519)
  JWTs checked in the contract's order, with 401 problems carrying a
  `reason` and a `WWW-Authenticate` challenge. Laravel's JWKS is loaded at
  startup, refreshed every 10 minutes, kept for up to 24 hours when Laravel
  is unreachable, and refetched at most once a minute on an unknown `kid`.
- `GET /v1/me`: public profile, roles and XP of the token's member.
- Settings `JWKS_URL` (required), `JWT_ISSUER`, `JWT_AUDIENCE` and
  `PUBLIC_STORAGE_URL`.
- Metrics `auth_rejections_total`, `jwks_fetches_total` and `jwks_keys`.
- Public content endpoints, read from the `api_read` views only:
  `GET /v1/catalog`, `/v1/courses`, `/v1/courses/{slug}`,
  `/v1/courses/{slug}/lessons/{lesson}`, `/v1/exercises`,
  `/v1/exercises/{id}`, `/v1/exams`, `/v1/exams/{slug}`, `/v1/resources`
  and `/v1/resources/{slug}`. Filters and search as in `openapi.yaml`,
  keyset pagination with opaque cursors bound to their filters,
  `?lang=fr|en` with per-field fallback to French, strong ETags and
  `304 Not Modified`, public caching headers, CORS for any origin, and
  RFC 9457 problems (`invalid-parameter`, `invalid-cursor`, `not-found`,
  `not-acceptable`). Never solutions, corrections, drafts or personal
  data.
- Rate limits in Redis: 60 requests a minute per client IP, 120 per token
  `sub`, with `RateLimit-*` headers and `429` + `Retry-After`. They fail
  open while Redis is down. Settings `RATE_LIMIT_ANONYMOUS_PER_MINUTE` and
  `RATE_LIMIT_AUTHENTICATED_PER_MINUTE`; metrics `rate_limited_total` and
  `rate_limit_errors_total`.
- Deployment (`deploy/`, `docs/deployment.md`), prepared but not deployed:
  a hardened systemd unit (`pulsars-api.service`), the nginx vhost for
  `api.pulsars.science` (TLS, WebSocket location, `X-Forwarded-For` set by
  nginx only, security headers, flood guard), a production environment
  file, and `deploy-api`, which installs a release, swaps it in atomically,
  checks `/healthz` and rolls back on failure (`--rollback` by hand).
- Release workflow: a `vX.Y.Z` tag builds `x86_64-unknown-linux-gnu` and
  publishes the archive that `deploy-api` installs, with its SHA-256.
- CI: `shellcheck` on every shell script.

### Changed

- `GET /healthz` answers 503 (`down`) until Laravel's JWKS has been loaded
  once.

[Unreleased]: https://github.com/Pulsars-science/pulsars-api/compare/v0.5.0...HEAD
[0.5.0]: https://github.com/Pulsars-science/pulsars-api/compare/v0.3.0...v0.5.0
[0.4.0]: https://github.com/Pulsars-science/pulsars-api/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/Pulsars-science/pulsars-api/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/Pulsars-science/pulsars-api/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/Pulsars-science/pulsars-api/releases/tag/v0.1.0
