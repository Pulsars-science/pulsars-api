# Game token (JWT)

Laravel (`pulsars.science`) is the only identity provider. When the game front
end (`pulsars.science/jeux`) needs to act as a member, it asks Laravel for a
short-lived token, then presents that token to the API (`api.pulsars.science`),
which verifies it offline with Laravel's public key.

```
pulsars.science/jeux                 pulsars.science                  api.pulsars.science
        │  OPTIONS /jeu/jeton (preflight)  │                                   │
        │─────────────────────────────────▶│                                   │
        │  POST /jeu/jeton                 │                                   │
        │  Cookie: pulsars-session=…       │                                   │
        │  Origin: https://pulsars.science │                                   │
        │─────────────────────────────────▶│ checks session + origin           │
        │◀──── { "token": "eyJ…" } ────────│ signs with Ed25519 private key    │
        │                                                                      │
        │  WebSocket /v1/game/ws, first frame { "type": "auth", "token": … }   │
        │─────────────────────────────────────────────────────────────────────▶│
        │                                  │◀── GET /.well-known/jwks.json ────│ (cached)
        │◀──────────────────────────── { "type": "auth.ok" } ──────────────────│
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
| `kid` | Identifies the signing key in the JWKS. Format `YYYY-MM-ed25519-N` (creation month and a counter), readable in logs. Required: a token without `kid` is rejected. |

### Claims

```json
{
  "iss": "https://pulsars.science",
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
| `iss` | string | Exactly `https://pulsars.science`. | Standard. Lets the API reject tokens from any other issuer, including a local Laravel. |
| `aud` | string | Exactly `api.pulsars.science`. | Standard. A token minted for another audience (a future service) cannot be replayed here. |
| `sub` | string | The Laravel `users.id`, **as a string** (`"42"`). | Standard; RFC 7519 makes `sub` a string. The API uses the same string everywhere (`/v1/me` `id`, webhook `user_id`). |
| `iat` | number | Issue time, Unix seconds. | Standard. |
| `nbf` | number | Equal to `iat`. | Standard; makes clock-skew handling explicit. |
| `exp` | number | `iat + 300` (5 minutes). | Short lifetime limits a stolen token's value; nothing to revoke. |
| `jti` | string | UUIDv7, unique per token. | Lets the API make the token **single-use for opening a game session** (see §4) and correlate logs. |
| `name` | string | The member's `pseudo`. | The game lobby and opponents see it immediately, without a database read on every WebSocket connection. Already public (profile URL). |
| `locale` | string or null | `users.locale` (`fr`, `en` or null). | The game server picks the language of questions and system messages. Null → `fr` (source language). Not a personal data issue: it is a UI preference. |
| `roles` | string[] | **Permission roles only**: zero or more of `moderateur`, `admin`, `super-admin`. | Lets the game server allow moderation actions (end a match, kick a player) without querying `site`. Subject (`matiere`) and badge (`distinction`) roles are **not** included: the game does not need them, and they are readable through `/v1/me`. |

No other claim. In particular: no e-mail, no school level or class, no mute
status (if the game gets a chat, the mute decision is a `game-design.md`
question first), no XP (it changes; read `/v1/me`).

The whole token stays under 600 bytes.

## 2. Issuing endpoint (Laravel)

### Request

```http
POST /jeu/jeton HTTP/1.1
Host: pulsars.science
Origin: https://pulsars.science
Cookie: pulsars-session=…
Content-Type: application/json
Accept: application/json

{}
```

The front end calls it with `fetch(url, { method: "POST", credentials: "include", headers: { "Content-Type": "application/json", "Accept": "application/json" }, body: "{}" })`.

The URL follows the French naming of the site's other routes
(`/profil/jetons-api`). It lives in the **`web`** middleware group, because it
needs the session.

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
  "expires_at": "2026-10-13T10:17:31Z"
}
```

The front end requests a new token shortly before `expires_at` (or on demand,
right before opening a WebSocket). It keeps the token **in memory only**, never
in `localStorage`.

### Who gets a token

The route uses the same guards as the site's other member actions
(`routes/web.php`, group `['auth', 'onboarding', 'verified']`):

| Situation | Response |
|---|---|
| No session | `401 {"message": "Unauthenticated."}` (Laravel's default JSON for `auth`) |
| Onboarding not done | `403` (`EnsureOnboarded` redirects HTML requests; for JSON it must answer 403 — see `laravel-tasks.md`) |
| E-mail not verified | `403` (`verified` middleware, JSON) |
| Origin not allowed | `403 {"message": "Origin not allowed."}` |
| More than 30 requests / minute for this user | `429` with `Retry-After` |

### How it is protected (CORS and CSRF)

**What exists today** (checked in the code):

- Session: `SESSION_DRIVER=database`, cookie `SameSite=Lax`,
  `HttpOnly`, `SESSION_DOMAIN` unset (host-only cookie on `pulsars.science`).
- CSRF: the `web` group runs `ValidateCsrfToken`. The game front end does
  not go through Inertia and does not handle the `XSRF-TOKEN` cookie.
- CORS: no `config/cors.php` is published; Laravel's defaults apply
  (`paths: api/*, sanctum/csrf-cookie`, `allowed_origins: *`,
  `supports_credentials: false`). The current `/api/v1` relies on that `*`.
- Sanctum is installed for personal access tokens; `statefulApi()` is not
  enabled, so there is no SPA cookie authentication on `/api/*`.

**Why the session cookie reaches Laravel at all.** The game front end is
served by nginx under `pulsars.science/jeux/`: it is the **same origin** as
the site, so the session cookie is sent like on any page of the site. A
request from another site (`evil.example`) is cross-site: the browser does
not send the `SameSite=Lax` cookie on a POST, so it arrives unauthenticated.
(The design was first written for a `game.` subdomain — same-site, not
same-origin — which is why the route does not rely on the CSRF cookie; it
still works if the game moves to its own subdomain one day.)

**Proposed protection.** Rather than relying on the CSRF cookie, the route is excluded from `ValidateCsrfToken` and protected by a
dedicated middleware, `VerifierOrigineJeu`, which:

1. Answers the preflight `OPTIONS /jeu/jeton` itself (the preflight carries no
   cookie, so it must not pass through `auth`): `204` with
   `Access-Control-Allow-Origin: <origin>`, `Access-Control-Allow-Credentials: true`,
   `Access-Control-Allow-Methods: POST`,
   `Access-Control-Allow-Headers: Content-Type, Accept`,
   `Access-Control-Max-Age: 600`, `Vary: Origin` — only when the origin is
   allowed; otherwise `403` without CORS headers.
2. On `POST`, requires an `Origin` header **exactly equal** to one of
   `GAME_ALLOWED_ORIGINS` (`https://pulsars.science`, plus
   `http://localhost:5173` when `APP_ENV=local`). A missing `Origin` is
   refused.
3. Requires `Content-Type: application/json`. A cross-site HTML form cannot
   send that type, and a script must go through the preflight, which step 1
   refuses for any other origin.
4. When the browser sends `Sec-Fetch-Site`, requires `same-site` or
   `same-origin`.
5. Adds `Access-Control-Allow-Origin: <origin>`,
   `Access-Control-Allow-Credentials: true` and `Vary: Origin` to the response.

Why not the alternatives:

| Option | Rejected because |
|---|---|
| Publish `config/cors.php` with `supports_credentials: true` for `pulsars.science/jeux` | Laravel's `HandleCors` has one global origin list. The public `/api/v1` needs `*`, which is forbidden together with credentials. Mixing both in one config would either break the public API or open credentials to every origin. |
| `SESSION_DOMAIN=.pulsars.science` so the game can read `XSRF-TOKEN` | Sends the Laravel session cookie to **every** subdomain, including `api.pulsars.science` (the Rust API would receive it in every request and its logs) and any future subdomain. A larger blast radius for a problem the `Origin` check already solves. |
| `GET` endpoint | A token-issuing endpoint is not a safe method; GETs get prefetched, cached and logged. |

The response is never cached (`Cache-Control: no-store`) and the token is not
logged by Laravel.

## 3. Public keys: JWKS

### Endpoint

```http
GET /.well-known/jwks.json HTTP/1.1
Host: pulsars.science
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

- Only public keys (`x`); never `d`.
- Served by Laravel (a route, no session). nginx today proxies everything
  except `/build/` and dot-files to Octane, and `location ~ /\.` **denies**
  paths starting with a dot: `/.well-known/jwks.json` needs an explicit
  `location = /.well-known/jwks.json` before that rule (see `laravel-tasks.md`).
- The set contains one to three keys: the **active** signing key, plus the
  **next** key during a rotation, plus the **previous** key until every token
  it signed has expired.

### Cache on the API side

- Fetched at startup. The API refuses to become ready (`/healthz` stays `503`,
  or the process exits) if it cannot load at least one key.
- Kept in memory for 10 minutes, then refreshed in the background. A failed
  refresh keeps the previous set and logs a warning; the set is kept for up to
  24 hours of consecutive failures, after which tokens are rejected.
- **Unknown `kid`**: the API refetches the JWKS immediately, at most once every
  60 seconds (a flood of tokens with random `kid`s must not turn the API into
  a JWKS amplifier). If the `kid` is still unknown, `401 unknown_kid`.
- Keys whose `alg`, `kty` or `crv` are not `EdDSA` / `OKP` / `Ed25519` are
  ignored.
- The JWKS URL comes from configuration (`JWKS_URL`), fetched over HTTPS only.

### Key storage (Laravel)

- Private keys are files outside the repository, readable only by the
  application user: `storage/app/private/jeu/<kid>.key` (raw 32-byte seed,
  base64), mode `0600`. `storage/app/private` is already git-ignored.
- Which key signs is set by `GAME_JWT_ACTIVE_KID`. Which public keys are
  published is the set of `*.pub` files in the same directory.
- Signing uses `sodium_crypto_sign_detached` (the `sodium` extension is
  available) through `firebase/php-jwt` (supports `EdDSA`), or directly; either
  way the header and claims are exactly those above.

### Rotation procedure

Constraints: tokens live 5 minutes; the API caches the JWKS 10 minutes and
re-reads it on an unknown `kid`.

**Planned rotation** (recommended every 6 months, and whenever someone with
server access leaves):

1. `php artisan jeu:cle:generer` → writes `2027-04-ed25519-1.key` and `.pub`.
   The new public key is now **published** but not used.
2. Wait at least **15 minutes** (API cache 10 min + margin). Every API instance
   now knows the new key.
3. Set `GAME_JWT_ACTIVE_KID=2027-04-ed25519-1`, reload Octane
   (`php artisan octane:reload`). New tokens use the new key.
4. Wait at least **10 minutes** (token lifetime 5 min + clock skew + margin).
   No valid token signed with the old key remains.
5. Remove the old `.pub` (and archive then delete the `.key`). It disappears
   from the JWKS at the next cache expiry.

Because the API refetches on an unknown `kid`, skipping step 2 does not break
anything; it only adds one JWKS fetch per API instance.

**Emergency rotation** (private key leaked):

1. Generate a new key and make it active at once (steps 1 and 3 together).
2. Remove the compromised `.pub` immediately.
3. Restart the API instances (or wait 10 minutes) to drop the compromised key
   from their cache. Tokens signed with it are rejected from then on.
4. Every member gets a new token at their next request; nothing else to revoke,
   since nothing long-lived was signed.

## 4. Using the token with the API

### HTTP

```http
GET /v1/me HTTP/1.1
Host: api.pulsars.science
Authorization: Bearer eyJ…
```

The same token can be used for HTTP calls until it expires.

### WebSocket

Browsers cannot set an `Authorization` header on a WebSocket. Passing the
token in the URL would leak it into access logs. The token is therefore sent
in the **first frame**:

1. The client opens `wss://api.pulsars.science/v1/game/ws` (the `Origin` is
   checked against the CORS allow-list during the upgrade).
2. Within **5 seconds** it sends `{"type": "auth", "token": "eyJ…"}`.
3. The server verifies the token exactly as for HTTP, and also records the
   `jti` in Redis until `exp`: **a given token opens at most one game
   session**. A second WebSocket with the same `jti` is refused.
4. The session stays authenticated after `exp`: the token proves identity at
   connection time, it is not re-checked on every frame. A reconnection needs a
   new token.

The exact message format belongs to `docs/game-protocol.md` in `pulsars-api`.

## 5. Verification rules and errors

The API checks, in this order:

| # | Check | Failure `reason` |
|---|---|---|
| 1 | Header present and `Bearer <token>` | `missing` / `malformed` |
| 2 | Three base64url parts, JSON header and claims | `malformed` |
| 3 | `alg` is exactly `EdDSA` | `bad_algorithm` |
| 4 | `kid` present and in the JWKS (after one refetch) | `unknown_kid` |
| 5 | Signature valid | `bad_signature` |
| 6 | `iss` equals `https://pulsars.science` | `bad_issuer` |
| 7 | `aud` equals `api.pulsars.science` (string, or array containing it) | `bad_audience` |
| 8 | `exp` > now − 30 s | `expired` |
| 9 | `nbf` and `iat` ≤ now + 30 s | `not_yet_valid` |
| 10 | `sub`, `jti` present and strings | `malformed` |

Leeway is 30 seconds on time claims. Laravel's and the API's clocks are both
NTP-synchronised.

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
| Valid token, action not allowed (e.g. moderation without role) | 403 | `insufficient_scope` |
| JWKS never loaded (startup) | 503 | none |

The `reason` field helps the front end react (refresh on `expired`), but
`detail` never echoes token contents. Failed verifications are logged with
`reason`, `kid` and the client IP, never with the token.

### WebSocket close codes

| Code | Meaning |
|---|---|
| `4000` | No `auth` frame within 5 seconds |
| `4001` | Invalid token (same `reason` values, sent in the close reason) |
| `4003` | Token already used to open a session (`jti` replay) |
| `4029` | Rate limited |

## 6. Known gaps

- `EnsureOnboarded` redirects; it has never had to answer a JSON request. The
  token route needs it to return `403` JSON (task in `laravel-tasks.md`).
- There is no ban concept in Laravel (only `mute_jusqu_a` on the forum). A
  banned-from-game state, if wanted, is a `game-design.md` decision.
- `PROJET.md` still describes Laravel 11 / PHP 8.3; the code is Laravel 13
  (`composer.lock` v13.20.0) and production runs PHP 8.4.
