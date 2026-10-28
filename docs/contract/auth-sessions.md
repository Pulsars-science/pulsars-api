# Sessions and passwords (phase 2, part 1)

Contract between `pulsars-api` and the Laravel site for logging in,
signing up and changing passwords. Routes and schemas are in
`openapi.yaml` (tag `auth`).

## 1. Principle

- The API is the authority for sessions: it checks passwords, creates
  accounts, opens and closes sessions. The site no longer reads
  `users.password` and keeps no login session of its own.
- A session is an **opaque token**: 32 random bytes in base64url without
  padding (43 characters). The API stores only its SHA-256, in
  `site.auth_sessions`.
- The site keeps the token in a `pulsars_auth` cookie (httpOnly, Secure in
  production, `SameSite=Lax`, `Domain` from `PULSARS_AUTH_COOKIE_DOMAIN`:
  empty locally, `.pulsars.science` in production so part 2 can read it on
  `api.pulsars.science`). The site does not encrypt this cookie.
- The site calls the API from its server (`PULSARS_API_URL`) with
  `Pulsars-Service-Key`, the visitor's address in `X-Forwarded-For` (the API
  takes the last entry) and the visitor's browser in `User-Agent`.
- A session's token is given in the `Pulsars-Session: <token>` header.
- Part 2 (later): `POST /v1/auth/token` (cookie `pulsars_auth` → a JWT
  signed by the API), the API's JWKS, mail and Google in the API.

## 2. Table `site.auth_sessions`

Created by the migration `20261029000000_auth_sessions`, run as
`pulsars_migrate` (owner `pulsars_owner`).

| Column | Type | Rule |
|---|---|---|
| `id` | uuid PK | UUIDv7 |
| `user_id` | bigint NOT NULL | FK `site.users(id)` ON DELETE CASCADE, indexed |
| `token_hash` | bytea NOT NULL UNIQUE | SHA-256 of the token |
| `remember` | boolean NOT NULL | « Se souvenir de moi » |
| `created_at` | timestamptz NOT NULL default now() | |
| `last_used_at` | timestamptz NOT NULL default now() | written at most once a minute |
| `expires_at` | timestamptz NOT NULL | sliding: `last_used_at` + 120 minutes, or + 365 days with `remember` |
| `revoked_at` | timestamptz NULL | logout, password change or reset |
| `ip` | inet NULL | |
| `user_agent` | text NULL | first 500 characters |

The same migration grants `pulsars_api`:

- `USAGE` on schema `site`;
- `SELECT (id, pseudo, name, email, email_verified_at, password, locale,
  onboarding_at, created_at, updated_at)` on `site.users`, **never** the
  `two_factor_*` columns nor `remember_token`;
- `INSERT (pseudo, name, email, password, locale, created_at, updated_at)`
  and `UPDATE (password, updated_at)` on `site.users`;
- `USAGE` on the sequence of `site.users.id`;
- `SELECT, INSERT, UPDATE, DELETE` on `site.auth_sessions`.

Nothing else of `site` (`the_api_role_reads_content_writes_game_and_changes_no_structure`
proves it). `deploy/sql/phase-1-retour.sql` drops the table and its
migration row.

## 3. Objects

`AccountUser`:

```json
{ "id": "42", "pseudo": "jikuji", "email": "a@b.c", "email_verified": true,
  "onboarded": true, "locale": "fr" }
```

`id` is a string, like a JWT `sub`. `locale` may be null.

`Session`, only in the response that creates it:

```json
{ "token": "…43 characters…", "expires_at": "2026-10-29T12:00:00Z", "remember": false }
```

## 4. Routes

Every response has `Cache-Control: no-store`. Bodies are JSON
(`application/json`, 415 otherwise); unknown fields are refused (400
`invalid-body`). Errors are the usual problems.

**Field errors** are `422 invalid-body` with every error at once, each with
a stable `code` (the `detail` stays in English):

```json
{ "type": "https://api.pulsars.science/problems/invalid-body",
  "title": "Invalid body", "status": 422,
  "detail": "One or more fields are invalid.",
  "errors": [ { "parameter": "pseudo", "detail": "is already used by another account", "code": "taken" } ] }
```

`parameter` is the field name (not a JSON Pointer): these are rule errors on
a well-formed body. Errors come in the form's order.

### `POST /v1/auth/login`

Body `{"login", "password", "remember"}` (`remember` optional, false).

- `login` is an e-mail or a pseudo, compared **ignoring case** (Fortify
  lowercased the login before comparing it to the pseudo exactly, so pseudos
  with capitals could not log in). When two accounts match, the exact e-mail
  wins, then the lowest id.
- Account without a password (made through Google), wrong password, unknown
  account: `422 invalid-credentials`, always the same body, and the same
  time (an unknown login is checked against a dummy hash).
- Limit: 5 attempts per minute per `lower(login)|ip`, in Redis, **even with
  the service key**. Beyond: `429 rate-limited` with `Retry-After`.
- Success: `200 {"data": {"session": Session, "user": AccountUser}}`.
- Hashes are bcrypt. PHP's `$2y$` hashes are checked as they are.

### `POST /v1/auth/register`

Body `{"pseudo", "email", "password", "password_confirmation", "locale"}`
(`locale` optional: `fr`, `en`, `ru` or null). Missing strings count as
empty.

| Field | Rules (Laravel's `CreateNewUser`) | Codes |
|---|---|---|
| `pseudo` | required, 3 to 30 characters, letters and digits (Unicode), `_`, `-`; unique ignoring case | `required`, `too_short`, `too_long`, `invalid_format`, `taken` |
| `email` | required, at most 255, one `@`, a local part, a domain with a dot, no whitespace; unique ignoring case | `required`, `too_long`, `invalid_email`, `taken` |
| `password` | required, 8 to 255 characters, equal to `password_confirmation` | `required`, `too_short`, `too_long`, `mismatch` |
| `locale` | `fr`, `en`, `ru` or null | `invalid` |

- A mismatch is reported on `password`, as Laravel's `confirmed` rule.
- Creates the account: `name` = pseudo, bcrypt **cost 12 in the `$2y$`
  format** (Laravel's `Hash::check` refuses other prefixes), `locale`,
  `created_at` = `updated_at` = now (UTC). `email_verified_at` and
  `onboarding_at` stay null. A uniqueness conflict caught by the database
  (two sign-ups racing) gives the same `taken`.
- Opens a session (`remember` false): `201 {"data": {"session", "user"}}`.
- The verification e-mail is still sent by the site (part 1).
- Limit: 10 sign-ups per hour per address, even with the service key,
  counted only for valid requests.

### `GET /v1/auth/session`

`Pulsars-Session` header; what the site calls once per request of a
logged-in visitor.

- Missing, malformed, unknown, revoked or expired: `401 session-invalid`,
  `WWW-Authenticate: Session realm="api.pulsars.science"`.
- Otherwise extends the session and answers
  `200 {"data": {"user": AccountUser, "session": {"expires_at", "remember"}}}`.

### `POST /v1/auth/logout`

`Pulsars-Session` header. Revokes the session. Always `204`: logging out
twice is not an error.

### `POST /v1/auth/sessions` (service key)

Body `{"user_id": "42", "remember": false}`. Opens a session for an account
the site authenticated another way: Google, a reset link, the seeder's
administrator.

- No valid service key: `403 forbidden`. `user_id` not an integer string:
  `422`, code `invalid`. Unknown account: `404`.
- `201 {"data": {"session", "user"}}`.

### `PUT /v1/auth/password`

`Pulsars-Session` header. Body
`{"current_password", "password", "password_confirmation"}`.

- `current_password` is required only when the account has a password: an
  account made through Google can set its first one (it could not with
  Fortify). Missing: code `required`; wrong: code `incorrect`.
- `password`: the sign-up rules.
- Saves the hash, **revokes every other session** of the account, keeps the
  request's. `204`.

### `POST /v1/auth/password/reset` (service key)

Body `{"user_id", "password", "password_confirmation"}`. The site checked
the reset link (`password_reset_tokens`, unchanged in part 1). Saves the
hash and **revokes every session** of the account. `204`.

## 5. What the site does (`PULSARS_AUTH_VIA_API`, false by default)

- A Laravel guard `pulsars-api` (a `StatefulGuard`) replaces the `session`
  driver of the `web` guard. `$request->user()`, `auth`, `verified`,
  `onboarding` work unchanged: the guard reads the cookie, calls
  `GET /v1/auth/session` once per request and loads the `User` model by id.
- `login($user, $remember)` (Google, reset, seeder): `POST
  /v1/auth/sessions`, then sets the cookie. A session just opened by the
  API's login or sign-up for the same account is reused.
- Fortify's `authenticateUsing`: `POST /v1/auth/login`; errors come back as
  Laravel's (`email` key, `auth.failed`, `auth.throttle`).
- `CreateNewUser`: `POST /v1/auth/register`, then the site sends the
  verification e-mail; codes become Laravel's validation messages.
- Logout: `POST /v1/auth/logout`, the cookie is cleared, the Laravel session
  (only CSRF, flash messages and the OAuth state now) is invalidated.
- `UpdateUserPassword`: `PUT /v1/auth/password`; `ResetUserPassword`:
  `POST /v1/auth/password/reset`.
- If the API does not answer, the visitor is seen as logged out and a login
  fails with a clear message. **No fallback to the database** for logging
  in: it would create sessions the API does not know.
- Switch-over: everyone logs in again once; passwords do not change.
- `/jeu/jeton`, the site's JWKS and Sanctum (`/api/v1`) stay as they are.
