# Game token (JWT)

The games (`pulsars.science/jeux`) and the languages section
(`pulsars.science/langues`) are static front ends: they cannot hold the
member's session token. When they need to act as a member, they ask the API
for a short-lived token, `POST /v1/auth/token`, with the session cookie
`pulsars-web` set on `.pulsars.science`. The API signs it with its own
Ed25519 key and verifies it the same way on every later request, by `kid`,
without any network. The API verifies **no other token**.

The site itself (`pulsars-web`) never uses this token: it calls the API
from its server with `Pulsars-Session` and `Pulsars-Service-Key`
(`auth-sessions.md`).

```
pulsars.science/jeux                                         api.pulsars.science
        │  OPTIONS /v1/auth/token (preflight)                        │
        │───────────────────────────────────────────────────────────▶│
        │  POST /v1/auth/token                                       │
        │  Cookie: pulsars_auth=…   Origin: https://pulsars.science  │
        │───────────────────────────────────────────────────────────▶│ checks session + origin
        │◀──────────────── { "token": "eyJ…" } ──────────────────────│ signs with its Ed25519 key
        │                                                            │
        │  WebSocket /v1/game/ws, first frame { "type": "auth", "token": … }
        │───────────────────────────────────────────────────────────▶│ verifies with the same key
        │◀──────────────── { "type": "auth.ok" } ────────────────────│
```

## 1. Token format

A JWS in compact serialisation (RFC 7515 / RFC 7519).

### Header

```json
{
  "alg": "EdDSA",
  "typ": "JWT",
  "kid": "2026-10-ed25519-1"
}
```

| Field | Rule |
|---|---|
| `alg` | Always `EdDSA` with an Ed25519 key (RFC 8037). The API **rejects every other value**, including `none` and any `HS*`; it never reads the algorithm from the token to choose how to verify. |
| `typ` | `JWT`. |
| `kid` | The API's signing key, `JWT_SIGNING_KID`. Format `YYYY-MM-ed25519-N` (creation month and a counter), readable in logs. Required: a token without `kid`, or with another one, is rejected. |

### Claims

```json
{
  "iss": "https://api.pulsars.science",
  "aud": "api.pulsars.science",
  "sub": "42",
  "iat": 1792059151,
  "nbf": 1792059151,
  "exp": 1792059451,
  "jti": "01929a7e-6c1f-7b3a-9d2e-5f0a8b4c3d21",
  "name": "jikuji",
  "locale": "fr",
  "roles": ["super-admin"]
}
```

| Claim | Type | Rule | Why it is there |
|---|---|---|---|
| `iss` | string | Exactly `https://api.pulsars.science`. | Standard. Tokens of any other issuer are refused, the old site's (`https://pulsars.science`) included. |
| `aud` | string | Exactly `api.pulsars.science`. | Standard. A token minted for another audience (a future service) cannot be replayed here. |
| `sub` | string | `site.users.id`, **as a string** (`"42"`). | Standard; RFC 7519 makes `sub` a string. The API uses the same string everywhere (`/v1/me` `id`, the game's `user_id`). |
| `iat` | number | Issue time, Unix seconds. | Standard. |
| `nbf` | number | Equal to `iat`. | Standard; makes clock-skew handling explicit. |
| `exp` | number | `iat + JWT_TOKEN_TTL_SECS` (300 by default, 5 minutes). | Short lifetime limits a stolen token's value; nothing to revoke. |
| `jti` | string | UUIDv7, unique per token. | Lets the API make the token **single-use for opening a game session** (§4) and correlate logs. |
| `name` | string | The member's `pseudo`. | The game lobby and opponents see it immediately, without a database read on every WebSocket connection. Already public (profile URL). |
| `locale` | string or null | `users.locale` (`fr`, `en`, `ru` or null). | The game server picks the language of questions and system messages. Null → `fr` (source language). A UI preference, not personal data. |
| `roles` | string[] | **Permission roles only**, highest first: zero or more of `moderateur`, `admin`, `super-admin` (`site.roles`, `site.role_user`). | `GET /v1/game/admin/stats` and `GET /v1/languages/admin/stats` need `admin` or `super-admin`. Subject (`matiere`) and badge (`distinction`) roles are **not** included: they are readable through `/v1/me`. |

No other claim. In particular: no e-mail, no school level or class, no mute
status, no XP (it changes; read `/v1/me`).

The whole token stays under 600 bytes.

## 2. Issuing endpoint: `POST /v1/auth/token`

### Request

```http
POST /v1/auth/token HTTP/1.1
Host: api.pulsars.science
Origin: https://pulsars.science
Cookie: pulsars_auth=…
Content-Type: application/json
Accept: application/json

{}
```

The front end calls it with `fetch(url, { method: "POST", credentials:
"include", headers: { "Content-Type": "application/json", "Accept":
"application/json" }, body: "{}" })`.

- **Session**: the `pulsars_auth` cookie, which `pulsars-web` sets on
  `.pulsars.science` when the member logs in (`auth-sessions.md`), or
  `Pulsars-Session` from a server. Missing, unknown, expired or revoked:
  `401 session-invalid`.
- **Who**: e-mail verified and onboarding done (as the old site's `['auth',
  'onboarding', 'verified']` group), else `403` (`Your email address is not
  verified.`, `Onboarding required.`). 30 tokens per minute per member,
  else `429 rate-limited` with `Retry-After`.
- **Without a signing key** (development only: production refuses to start
  without `JWT_SIGNING_KEY_FILE` and `JWT_SIGNING_KID`): `503
  token-unavailable`.

### Success

```http
HTTP/1.1 200 OK
Content-Type: application/json
Cache-Control: no-store
Access-Control-Allow-Origin: https://pulsars.science
Access-Control-Allow-Credentials: true
Vary: Origin

{
  "token": "eyJhbGciOiJFZERTQSIsInR5cCI6IkpXVCIsImtpZCI6IjIwMjYtMTAtZWQyNTUxOS0xIn0.eyJ…",
  "token_type": "Bearer",
  "expires_in": 300,
  "expires_at": "2026-10-29T10:17:31Z"
}
```

The front end requests a new token shortly before `expires_at` (or on demand,
right before opening a WebSocket). It keeps the token **in memory only**, never
in `localStorage`. The token is never logged.

### How it is protected (CORS and CSRF)

The cookie is `SameSite=Lax` and sent to every `*.pulsars.science` host, so
a cross-site page cannot make a browser send it on a `POST`. On top of that,
the route only answers the origins of `CORS_ALLOWED_ORIGINS`
(`https://pulsars.science`, plus `http://localhost:5173` in development), as
the old site's `VerifierOrigineJeu` middleware did:

1. The preflight `OPTIONS /v1/auth/token` (no cookie) answers `204` with
   `Access-Control-Allow-Origin: <origin>`,
   `Access-Control-Allow-Credentials: true`,
   `Access-Control-Allow-Methods: POST`,
   `Access-Control-Allow-Headers: Content-Type, Accept`,
   `Access-Control-Max-Age: 600`, `Vary: Origin`, for a listed origin;
   `403` without CORS headers otherwise.
2. On `POST`: `Origin` required and listed (else `403`); `Content-Type:
   application/json` (else `415`): a cross-site HTML form cannot send that
   type, and a script must go through the preflight; `Sec-Fetch-Site`, when
   the browser sends it, `same-site` or `same-origin` (else `403`).
3. Every answer to a listed origin carries `Access-Control-Allow-Origin`,
   `Access-Control-Allow-Credentials: true` and `Vary: Origin`.

Exception: a request with `Pulsars-Session`, no cookie and no `Origin` is a
server's (a browser always sends `Origin` on a cross-origin `POST`, and
cannot send that header without a preflight, which other origins are
refused). It skips the browser checks.

A `GET` was rejected: a token-issuing endpoint is not a safe method; GETs
get prefetched, cached and logged.

## 3. The signing key and its JWKS

### Key

- `pulsars-api jwt-keygen <kid> <dir>` writes `<dir>/<kid>.key`: the 32-byte
  Ed25519 seed in base64, mode `0600`, never over an existing file. It
  prints the public `x` and the two variables to set.
- `JWT_SIGNING_KEY_FILE` names the file, `JWT_SIGNING_KID` its `kid`.
  **Required in production**: without them the API refuses to start (exit
  78). In production it also refuses a file readable by its group or
  others. In development both may be left unset: `POST /v1/auth/token`
  answers `503 token-unavailable` and every route that needs a token `503
  unavailable`.
- The file must be readable by the service account (`pulsars-api`), which
  reads it at start-up (`docs/deployment.md`).

### Public key: `GET /.well-known/jwks.json`

For anyone who wants to verify the API's tokens; the API itself does not
read it.

```http
GET /.well-known/jwks.json HTTP/1.1
Host: api.pulsars.science
```

```http
HTTP/1.1 200 OK
Content-Type: application/jwk-set+json
Cache-Control: public, max-age=300
Access-Control-Allow-Origin: *

{
  "keys": [
    {
      "kty": "OKP",
      "crv": "Ed25519",
      "x": "11qYAYKxCrfVS_7TyWQHOg7hcvPapiMlrwIaaPcHURo",
      "kid": "2026-10-ed25519-1",
      "use": "sig",
      "alg": "EdDSA"
    }
  ]
}
```

(`x` above is the public key of RFC 8037 Appendix A, for illustration only.)
Only the public key (`x`); never `d`. `{"keys": []}` without a key.

### Rotation

One key at a time. The API verifies by `kid`, so switching keys refuses the
previous key's tokens at once. They live 5 minutes, and the games ask for a
new token when one is refused (`401`, then `POST /v1/auth/token` again), so
a rotation costs each player at most one extra request:

1. `sudo -u pulsars-api /opt/pulsars-api/current/pulsars-api jwt-keygen
   2027-04-ed25519-1 /etc/pulsars-api/keys`.
2. Set `JWT_SIGNING_KEY_FILE` and `JWT_SIGNING_KID` to the new values,
   `sudo systemctl restart pulsars-api`.
3. Delete the old key file.

Recommended every 6 months, and at once if the key may have leaked, or when
someone with server access leaves. Nothing else needs revoking: nothing
long-lived was signed.

## 4. Using the token with the API

### HTTP

```http
GET /v1/me HTTP/1.1
Host: api.pulsars.science
Authorization: Bearer eyJ…
```

The same token can be used for HTTP calls until it expires. Calls with a
valid token are rate limited per `sub` (`README.md`, rate limits).

### WebSocket

Browsers cannot set an `Authorization` header on a WebSocket. Passing the
token in the URL would leak it into access logs. The token is therefore sent
in the **first frame**:

1. The client opens `wss://api.pulsars.science/v1/game/ws` (the `Origin` is
   checked against the CORS allow-list during the upgrade).
2. Within **5 seconds** it sends `{"type": "auth", "token": "eyJ…"}`.
3. The server verifies the token exactly as for HTTP, and also records the
   `jti` in Redis until `exp`: **a given token opens at most one game
   session**. A second WebSocket with the same `jti` is refused. Without
   Redis the token cannot be proven unused, and the session is refused.
4. The session stays authenticated after `exp`: the token proves identity at
   connection time, it is not re-checked on every frame. A reconnection needs a
   new token.

The exact message format is in `docs/game-protocol.md`.

## 5. Verification rules and errors

The API checks, in this order:

| # | Check | Failure `reason` |
|---|---|---|
| 1 | Header present and `Bearer <token>` | `missing` / `malformed` |
| 2 | Three base64url parts, JSON header and claims | `malformed` |
| 3 | `alg` is exactly `EdDSA` | `bad_algorithm` |
| 4 | `kid` present and equal to `JWT_SIGNING_KID` | `unknown_kid` |
| 5 | Signature valid | `bad_signature` |
| 6 | `iss` equals `https://api.pulsars.science` | `bad_issuer` |
| 7 | `aud` equals `api.pulsars.science` (string, or array containing it) | `bad_audience` |
| 8 | `exp` > now − 30 s | `expired` |
| 9 | `nbf` and `iat` ≤ now + 30 s | `not_yet_valid` |
| 10 | `sub`, `jti` present and strings | `malformed` |

Leeway is 30 seconds on time claims.

### HTTP responses

All failures are `401` with a problem document and the RFC 6750 header:

```http
HTTP/1.1 401 Unauthorized
Content-Type: application/problem+json
WWW-Authenticate: Bearer realm="api.pulsars.science", error="invalid_token", error_description="token expired"
Cache-Control: no-store

{
  "type": "https://api.pulsars.science/problems/unauthorized",
  "title": "Invalid token",
  "status": 401,
  "detail": "The token has expired.",
  "instance": "/v1/me",
  "reason": "expired"
}
```

| Case | Status | `WWW-Authenticate` `error` |
|---|---|---|
| No `Authorization` header | 401 | none (just `Bearer realm="api.pulsars.science"`) |
| Expired | 401 | `invalid_token` |
| Wrong `aud` / `iss` | 401 | `invalid_token` |
| Unknown `kid` | 401 | `invalid_token` |
| Bad signature, wrong `alg` | 401 | `invalid_token` |
| Valid token, role missing (admin statistics) | 403 | `insufficient_scope` |
| No signing key configured (development only) | 503 | none |

The `reason` field helps the front end react (refresh on `expired`), but
`detail` never echoes token contents. Failed verifications are logged with
`reason`, `kid` and the client IP, never with the token, and counted in
`auth_rejections_total` (reason `no_key` for the 503).

### WebSocket close codes

| Code | Meaning |
|---|---|
| `4000` | No `auth` frame within 5 seconds |
| `4001` | Invalid token (same `reason` values, sent in the close reason) |
| `4003` | Token already used to open a session (`jti` replay) |
| `4029` | Rate limited |
| `1013` | Try again later: no signing key, or Redis cannot record the `jti` |
