# Sessions, passwords, mails and Google accounts

How members log in to the site. The API is the authority: it checks
passwords, creates accounts, opens and closes sessions, sends the account
mails and links Google identities. The site, `pulsars-web`, holds the
session token in a cookie and calls the API from its server. Routes and
schemas are in `openapi.yaml` (tag `auth`).

## 1. Principle

- A session is an **opaque token**: 32 random bytes in base64url without
  padding (43 characters). The API stores only its SHA-256, in
  `site.auth_sessions`.
- `pulsars-web` keeps the token in the `pulsars_auth` cookie: httpOnly,
  Secure, `SameSite=Lax`, `Domain=.pulsars.science` in production, so that
  `api.pulsars.science/v1/auth/token` reads it too and gives the games and
  the languages section their short JWT (`auth-jwt.md`). The cookie is set
  and cleared by the site's own server routes (`/auth/login`,
  `/auth/register`, `/auth/logout`, `/auth/google/*`), never by the API.
- The site calls the API from its server, directly or through its `/api`
  relay, with:
  - `Pulsars-Session: <token>`, the token from the cookie;
  - `Pulsars-Service-Key`, the server's `SERVICE_KEY`: its calls are not
    rate limited by IP (`README.md`, rate limits), and some routes accept
    only them (marked *service key* below);
  - the visitor's address in `X-Forwarded-For` (the API takes the last
    entry) and the visitor's browser in `User-Agent`.
- Against CSRF, the site's relay refuses writes (anything but `GET` and
  `HEAD`) whose `Origin` is not the site's. Browsers never call these
  routes on `api.pulsars.science` directly.
- Passwords are hashed with **argon2id** (`$argon2id$v=19$m=19456,t=2,p=1$…`,
  the OWASP parameters; `ARGON2_MEMORY_KIB` lowers the memory cost in
  tests only).

## 2. Table `site.auth_sessions`

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

## 2b. Table `site.email_tokens`

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

Both tables are in the starting schema (`migrations/`); `pulsars_api` has
row rights on them like on every table (`database.md` §2).

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

### `POST /v1/auth/register`

Body `{"pseudo", "email", "password", "password_confirmation", "locale"}`
(`locale` optional: `fr`, `en`, `ru` or null). Missing strings count as
empty.

| Field | Rules (as the old site's `CreateNewUser`) | Codes |
|---|---|---|
| `pseudo` | required, 3 to 30 characters, letters and digits (Unicode), `_`, `-`; unique ignoring case | `required`, `too_short`, `too_long`, `invalid_format`, `taken` |
| `email` | required, at most 255, one `@`, a local part, a domain with a dot, no whitespace; unique ignoring case | `required`, `too_long`, `invalid_email`, `taken` |
| `password` | required, 8 to 255 characters, equal to `password_confirmation` | `required`, `too_short`, `too_long`, `mismatch` |
| `locale` | `fr`, `en`, `ru` or null | `invalid` |

- A mismatch is reported on `password`, as Laravel's `confirmed` rule did.
- Creates the account: `name` = pseudo, an argon2id hash, `locale`,
  `created_at` = `updated_at` = now (UTC). `email_verified_at` and
  `onboarding_at` stay null. A uniqueness conflict caught by the database
  (two sign-ups racing) gives the same `taken`.
- Opens a session (`remember` false): `201 {"data": {"session", "user"}}`.
- Sends the address verification mail (§4b).
- Limit: 10 sign-ups per hour per address, even with the service key,
  counted only for valid requests.

### `GET /v1/auth/session`

`Pulsars-Session` header; what the site calls to know who is logged in.

- Missing, malformed, unknown, revoked or expired: `401 session-invalid`,
  `WWW-Authenticate: Session realm="api.pulsars.science"`.
- Otherwise extends the session and answers
  `200 {"data": {"user": AccountUser, "session": {"expires_at", "remember"}}}`.

### `POST /v1/auth/logout`

`Pulsars-Session` header. Revokes the session. Always `204`: logging out
twice is not an error.

### `POST /v1/auth/sessions` (service key)

Body `{"user_id": "42", "remember": false}`. Opens a session for an account
the site authenticated another way. Only for a trusted server: the site
itself logs in through `login`, `register` and `oauth/resolve`.

- No valid service key: `403 forbidden`. `user_id` not an integer string:
  `422`, code `invalid`. Unknown account: `404`.
- `201 {"data": {"session", "user"}}`.

### `PUT /v1/auth/password`

`Pulsars-Session` header. Body
`{"current_password", "password", "password_confirmation"}`.

- `current_password` is required only when the account has a password: an
  account made through Google can set its first one (it could not on the
  old site). Missing: code `required`; wrong: code `incorrect`.
- `password`: the sign-up rules.
- Saves the hash, **revokes every other session** of the account, keeps the
  request's. `204`.

### `POST /v1/auth/password/reset` (service key)

Body `{"user_id", "password", "password_confirmation"}`, for a trusted
server that checked who asks itself; members reset theirs with a link
(`reset-with-token`, §4b). Saves the hash and **revokes every session** of
the account. `204`.

## 4b. Mails

The API sends the two account mails, in the words of the old site's
`VerifyEmail` and `ResetPassword` notifications and its translations (`src/mail/templates.rs`, fr/en/ru, plain text and HTML). The mail's
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

`POST /v1/auth/register` sends it too, once, after creating the account.

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
  `https://pulsars.science/reset-password/{token}?email={email}` (the old
  site's URL, which `pulsars-web` keeps), valid **60 minutes**.
- At most one mail a minute per account (the old site's `throttle: 60`): a
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
  account. `204`. The address is **not** marked verified, as on the old
  site.

## 4c. Google accounts

The site talks to Google (its redirect URL, the OAuth state, the code
exchange: `/auth/google/*`). It hands the identity it got to the API, which
applies the rules of the old site's `ConnexionSociale::resoudre` and
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

## 5. What the site does

- Every page the site renders on its server, and every call of its `/api`
  relay, reads the `pulsars_auth` cookie and passes it on as
  `Pulsars-Session`, with `Pulsars-Service-Key` and the visitor's address.
  The member routes take one of the API's member extractors (`Member`,
  `Verified`, …, `src/auth/member.rs`), which look the session up with its
  account and roles: the old site's `auth`, `verified` and `onboarding`
  middleware.
- Login, sign-up and Google: the site's server route calls `POST
  /v1/auth/login`, `POST /v1/auth/register` or `POST /v1/auth/oauth/resolve`,
  then sets the cookie from the `session` it gets. Logout: `POST
  /v1/auth/logout`, then the cookie is cleared, whatever the API answered.
- A `401 session-invalid` means the visitor is logged out: the site clears
  the cookie. If the API does not answer, the visitor is seen as logged out
  and a login fails with a clear message.
