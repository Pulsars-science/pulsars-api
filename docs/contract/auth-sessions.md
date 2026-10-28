# Sessions, passwords, mails and Google accounts (phase 2)

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

## 2b. Table `site.email_tokens`

Migration `20261031000000_email_tokens`.

| Column | Type | Rule |
|---|---|---|
| `id` | uuid PK | UUIDv7 |
| `user_id` | bigint NOT NULL | FK `site.users(id)` ON DELETE CASCADE |
| `kind` | text NOT NULL | `verify` or `reset` |
| `token_hash` | bytea NOT NULL UNIQUE | SHA-256 of the link's token |
| `email` | text NOT NULL | the address the link was sent to |
| `created_at` | timestamptz NOT NULL default now() | |
| `expires_at` | timestamptz NOT NULL | 24 h (`verify`), 60 min (`reset`) |
| `used_at` | timestamptz NULL | set when the link is used |

The same migration grants `pulsars_api` all rights on it, `UPDATE
(email_verified_at, updated_at)` and `INSERT (email_verified_at)` on
`site.users`, all rights on `site.comptes_sociaux` and `USAGE` on its
sequence.

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

## 4b. Mails (phase 2, part 3)

The API sends the two account mails itself, in the words of Laravel's
stock `VerifyEmail` and `ResetPassword` notifications as the site translates
them (`src/mail/templates.rs`, fr/en/ru, plain text and HTML). The mail's
language is the account's `locale`, else the request's `lang`, else `fr`.

- **SMTP**: `MAIL_SMTP_URL` (`smtp://127.0.0.1:1025` for Mailpit,
  `smtps://resend:<key>@smtp.resend.com:465` in production) and `MAIL_FROM`
  (default `Pulsars <noreply@blackenium.com>`). Without `MAIL_SMTP_URL`,
  mails are only logged (kind and account id, **never the link**), and the
  endpoints still succeed.
- Mails leave off the request path: the request answers at once; an SMTP
  failure is logged (`account mail failed`), never returned.
- Links are stored in `site.email_tokens` (§2b) as the SHA-256 of a
  32-byte random token (43 base64url characters). A link works once,
  before it expires, for the address it was sent to. Sending a new link of
  a kind makes the account's previous unused one stop working.

### `POST /v1/auth/email/verification`

`Pulsars-Session` header, body `{}` or `{"lang": "en"}`. Mails
`https://pulsars.science/email/verifier/{token}`, valid **24 hours**.

- Address already verified: `204`, nothing sent.
- 6 per minute per account (`429 rate-limited`), even with the service key.
- `204`.

`POST /v1/auth/register` now sends this mail itself. **The site must stop
sending its own** (`Registered` → `SendEmailVerificationNotification`) when
it signs up through the API, or the member gets two.

### `POST /v1/auth/email/verify`

Body `{"token": "…"}`, no session needed (the link may be opened in another
browser). Marks `email_verified_at` if the token is known, unused,
unexpired, and was sent to the address the account still has.

- `200 {"data": {"user": AccountUser}}`.
- Otherwise `422 invalid-token`, `errors: [{"parameter": "token", "code":
  "invalid"}]`.

### `POST /v1/auth/password/forgot`

Body `{"email": "…", "lang": "en"}` (`lang` optional). **Always `204`**: the
answer never tells whether an account has the address.

- When an account has it (ignoring case): mails
  `https://pulsars.science/reset-password/{token}?email={email}` (the URL
  Laravel's `ResetPassword` builds), valid **60 minutes**.
- At most one mail a minute per account (Laravel's `throttle: 60`): a
  second request in the minute answers `204` and sends nothing.
- 5 requests per minute per address (`429`), even with the service key.
- Empty `email`: `422`, code `required`.

### `POST /v1/auth/password/reset-with-token`

Body `{"email", "token", "password", "password_confirmation"}`: the form of
the reset link.

- `password`: the sign-up rules (`422 invalid-body` with codes; the token is
  then left untouched).
- Token unknown, used, expired, or sent to another address (compared
  ignoring case): `422 invalid-token`.
- Saves the hash, marks the link used, **revokes every session** of the
  account. `204`. The address is **not** marked verified, as with Laravel.

The service-key `POST /v1/auth/password/reset` stays, for a site that still
checks `password_reset_tokens` itself.

## 4c. Google accounts (phase 2, part 3)

The site still talks to Google (Socialite, its redirect URL, the OAuth
state in its session). It hands the identity it got to the API, which
applies the rules of the site's `ConnexionSociale::resoudre` and
`ConnexionSocialeController::rattacher`.

### `POST /v1/auth/oauth/resolve` (service key)

Body:

```json
{ "provider": "google", "subject": "1234567890", "email": "a@b.c",
  "email_verified": true, "name": "Ada Lovelace", "nickname": null,
  "remember": true, "current_user_id": null }
```

`provider`: `google` (or `discord`, whose rules are the same; the site
offers only Google). `subject`: the provider's id, 1 to 255 characters.
Otherwise `422`, code `invalid`.

In order:

1. **`current_user_id` given** (the member is logged in on the site): the
   identity is linked to that account (replacing its previous identity at
   that provider), unless another account has it: `409 conflict`, `reason:
   "taken"`. `200`, `session: null`, `linked: true`.
2. **The identity is already linked**: that account. `200`,
   `created: false`, `linked: false`.
3. **No `email`**: `422`, code `email_missing` on `email`.
4. **An account has the address** (ignoring case): linked only if
   `email_verified` is `true`, else `409 conflict`, `reason:
   "email_unverified"` (linking would give the account to whoever claims its
   address at the provider). `200`, `linked: true`.
5. **Otherwise a new account**: pseudo from `nickname`, else `name` (spaces
   become dashes, other characters than letters, digits, `_`, `-` dropped,
   30 at most, `membre` when shorter than 3); taken (ignoring case) → `-2`,
   `-3`… within 30 characters; then `membre-` and 12 random characters. No
   password; `email_verified_at` set only if `email_verified` is `true`.
   `201`, `created: true`, `linked: false`.

The account and its `comptes_sociaux` row are written in one transaction.
Two first logins at once with the same identity give one account (the
loser of the race finds the winner's link). Except in case 1, a session is
opened (`remember` as given):

```json
{ "data": { "user": AccountUser, "session": Session, "created": true, "linked": false } }
```

### `DELETE /v1/auth/oauth/{provider}`

`Pulsars-Session` header. Unlinks the provider from the session's account.
Refused when it is the account's last way to log in (no password, no other
provider): `409 conflict`, `reason: "last_login_method"`. Otherwise `204`
(also when nothing was linked).

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
