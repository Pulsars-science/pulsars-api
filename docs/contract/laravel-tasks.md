# Laravel tasks for the API

What this Laravel repository must implement so that `pulsars-api` can run.
**Nothing here is coded yet.** These tasks form their own phase,
**"Phase API: game integration"**, separate from the work in progress. They
touch no existing page and can be merged independently.

The brief mentions a `PROMPT-pulsars.md` describing the work in progress; it
is not in the repository (neither on `main` nor in the `pulsars-seo`
worktree). Once found, this phase should be appended to it.

Order and dependencies:

```
A1 (PostgreSQL) ──▶ A8 (views) ──▶ pulsars-api Phase 3 (content)
A2 (keys) ──▶ A3 (JWKS) ──▶ A4 (token) ──▶ pulsars-api Phase 2 (auth)
A5 (webhook) + A6 (XP source) ──▶ pulsars-api Phase 5 (results)   [A6 needs game-design Q9]
A7 (env), A9 (nginx), A10 (gaps), A11 (tests) alongside
```

Tasks A2–A5 work on MySQL too; only A8 needs PostgreSQL.

---

## A1. Move production to PostgreSQL

Follow `postgres-migration.md`. Code changes before the cutover:

- `ExerciseController::catalogue`: replace `MAX(correct)` (fails on
  PostgreSQL: `function max(boolean) does not exist`, verified).
- Case-insensitive search: `like` → `whereLike(…, caseSensitive: false)` /
  `ilike` in the catalogue, exams, members and forum searches (verified:
  `'Algèbre' LIKE '%alg%'` is false on PostgreSQL).
- Migration adding indexes on `questions.user_id` and `answers.user_id`.
- `config/database.php` pgsql: `'search_path' => env('DB_SEARCH_PATH', 'site')`.
- Run the test suite on PostgreSQL in CI (in addition to SQLite).

Done when: the rehearsal of `postgres-migration.md` §4 runs clean twice and
the suite is green on PostgreSQL.

## A2. Ed25519 signing keys

- Artisan command `jeu:cle:generer`: generates an Ed25519 key pair with
  `sodium_crypto_sign_keypair()`, writes
  `storage/app/private/jeu/<kid>.key` (secret seed, base64, mode `0600`) and
  `<kid>.pub` (public key, base64url), `kid` = `YYYY-MM-ed25519-N`. Refuses to
  overwrite an existing file.
- Artisan command `jeu:cle:lister`: lists keys, marks the active one.
- Config `config/jeu.php`: `cles_dossier`, `cle_active` (`GAME_JWT_ACTIVE_KID`),
  `issuer` (`https://pulsars.science`), `audience` (`api.pulsars.science`),
  `duree` (300), `origines` (`GAME_ALLOWED_ORIGINS`), webhook secrets.
- The app refuses to issue tokens (503 on the token route, log error) if the
  active key file is missing; it does not crash the site.
- Document the rotation procedure of `auth-jwt.md` §3 in `docs/`.

## A3. JWKS endpoint

- `GET /.well-known/jwks.json`, outside the `web` group (no session, no
  cookies, no `EnregistrerVisite`), `Content-Type: application/jwk-set+json`,
  `Cache-Control: public, max-age=300`, `Access-Control-Allow-Origin: *`.
- Body: one JWK per `*.pub` file (`kty OKP`, `crv Ed25519`, `x`, `kid`,
  `use sig`, `alg EdDSA`). Never the private part.
- `noindex` via `X-Robots-Tag`; not in the sitemap.

## A4. Token endpoint

- Dependency: `firebase/php-jwt` ^6.10 (supports `EdDSA` through sodium), or
  a 30-line signer using `sodium_crypto_sign_detached`. Either way, output
  exactly the header and claims of `auth-jwt.md` §1.
- Routes in `routes/web.php`:
  - `OPTIONS /jeu/jeton` → `VerifierOrigineJeu` only (preflight, no auth);
  - `POST /jeu/jeton` → middleware `VerifierOrigineJeu`, `auth`,
    `onboarding`, `verified`, `throttle:30,1`; name `jeu.jeton`.
- Exclude `jeu/jeton` from CSRF:
  `$middleware->validateCsrfTokens(except: ['jeu/jeton'])` in
  `bootstrap/app.php`.
- New middleware `App\Http\Middleware\VerifierOrigineJeu` implementing
  `auth-jwt.md` §2 "Proposed protection": exact `Origin` allow-list,
  `Content-Type: application/json`, `Sec-Fetch-Site` check, CORS response
  headers with credentials, `Vary: Origin`.
- **Do not** publish a global `config/cors.php` with credentials: it would
  conflict with the public `/api/v1` that relies on `*`.
- `EnsureOnboarded` currently always redirects. Make it answer
  `403 {"message": "Onboarding required."}` when `$request->expectsJson()`.
- Response `Cache-Control: no-store`; the token is never logged.
- Claims: `sub` = `(string) $user->id`, `name` = `pseudo`, `locale` =
  `$user->locale` (nullable), `roles` = slugs of the user's roles with
  `categorie = permission`, `jti` = `Str::uuid7()`.

## A5. Webhook receiver

- Route `POST /internal/jeu/evenements`, registered outside the `web` and
  `api` groups (for example with `withRouting(then: …)` and a dedicated
  middleware stack): no session, no CSRF, no `PublicApiHeaders`, body limit
  64 KiB.
- Middleware `VerifierSignatureJeu`: `webhooks.md` §2 (timestamp window 300 s,
  HMAC-SHA256 over `timestamp.raw_body`, `hash_equals`, current and previous
  secret, `v1=` scheme), problem+json `401` on failure.
- Migration `game_events` (`event_id` uuid unique, `type`, `payload` json,
  `received_at`, `processed_at`).
- Controller: insert the event and its results in one transaction; unique
  violation on `event_id` → `200 {"status":"duplicate"}`; unknown
  `type`/`version` → `422`; deleted `user_id` → skip that player, `200`.
- Log every event with `event_id` and the incoming `traceparent`.

## A6. Match results as an XP source (blocked by `game-design.md` Q9)

- Migration `game_results` (`webhooks.md` §5), unique `(match_id, user_id)`.
- `Experience::BAREME`: new keys (values from Q9), new sources in `comptes()`
  and `classement()`, caps if decided.
- Same change: update `api_read.member_xp` in `views.sql` and its migration.
- Profile page: show the new XP lines (translations in `lang/en.json`).

## A7. Environment variables

To add to `.env.example`, with comments in the style of the existing ones:

```env
# Jeton de jeu (game.pulsars.science -> api.pulsars.science). Voir
# docs/api-contract/auth-jwt.md. Les cles sont dans storage/app/private/jeu.
GAME_JWT_ACTIVE_KID=
GAME_JWT_ISSUER=https://pulsars.science
GAME_JWT_AUDIENCE=api.pulsars.science
GAME_JWT_TTL=300
# Origines autorisees a demander un jeton, separees par des virgules.
GAME_ALLOWED_ORIGINS=https://game.pulsars.science

# Webhook des resultats de partie (api -> site, reseau prive uniquement).
# 32 octets aleatoires minimum : openssl rand -hex 32
GAME_WEBHOOK_SECRET=
GAME_WEBHOOK_SECRET_PREVIOUS=
GAME_WEBHOOK_VERSION=1

# PostgreSQL (apres la migration, voir docs/api-contract/postgres-migration.md)
DB_SEARCH_PATH=site
```

Also remove the duplicated `APP_LOCALE` / `APP_FALLBACK_LOCALE` lines in
`.env.example` (the file sets them to `fr`, then to `en`).

## A8. `api_read` views migration (needs A1)

- A migration that runs `views.sql` (everything above the commented
  administrator block) when the driver is `pgsql`, and does nothing on SQLite
  and MySQL (tests keep running on SQLite).
- The static views (`licenses`, `subject_names`), the XP scale in
  `member_xp` and the allow-list in `exams.official_url` are generated from
  PHP at migration time (`config('licences')`, `Experience` scale exposed by a
  public method, `SujetOfficiel::SOURCES`, `lang/en.json`), so the SQL copy
  cannot drift silently.
- A PHPUnit test (pgsql only) compares each copy with its PHP source, and
  checks `member_xp` against `Experience::pour()` for a few seeded users.
- Any later migration that renames or drops a column read by a view must
  recreate the views (PostgreSQL refuses to drop a column a view uses: the
  migration will fail loudly, which is the desired behaviour).
- Add "Biologie" → "Biology" to `lang/en.json`.

## A9. nginx

In `deploy/pulsars.science.conf`, before `location ~ /\.` (which denies
dot-paths, including `/.well-known/`):

```nginx
location = /.well-known/jwks.json {
    try_files /jamais_ce_fichier @octane;
}

location ^~ /internal/ {
    return 404;
}
```

And the vhost for `api.pulsars.science` (proxy to the Rust API, WebSocket
upgrade). The architecture mentions Caddy at the edge; production runs nginx
today. Which one fronts the API is a decision (see the summary).

## A10. Existing gaps to fix (optional for the API, found while writing the contract)

| # | Where | Problem | Proposed fix |
|---|---|---|---|
| G1 | `Api/V1/CatalogueController::correction` | Reads `donnees['reponses_acceptees']`, but short answers are stored under `donnees['acceptees']` (`CorrecteurExercice`, seeders): the endpoint always returns `[]` for short answers. | Read `acceptees`. |
| G2 | `Api/V1/CompteController::show` | `/api/v1/me` returns the e-mail address, school level and class. | Decide whether the old API keeps them (token holders are the account owners, so it is not a leak, but it contradicts the new rule). |
| G3 | `Api/V1/ExamenController::show` | Public `/api/v1/examens/{slug}` returns `correction_markdown` for every part. | Consistent with the website (corrections are public there), but not with the new API's rule. Decide (see summary). |
| G4 | `Experience` | Hidden and soft-deleted forum posts still earn XP (the queries bypass the model scopes). | Filter `masque_le` / `deleted_at`, or accept. |
| G5 | Lessons | Slug unique per chapter only; URLs assume unique per course. | Unique index on (course, slug) through a check in the course seeder/editor. |
| G6 | `PROJET.md` | Says Laravel 11 / PHP 8.3 / PostgreSQL 16 / Reverb / Redis; the code is Laravel 13, prod PHP 8.4, MySQL, no Reverb, no Redis. | Update when convenient. |

## A11. Tests

- Token endpoint: no session → 401; not verified → 403; not onboarded →
  403 JSON; wrong origin → 403; missing `Origin` → 403; `text/plain` → 403;
  preflight from allowed / other origin; valid call returns a JWT that
  verifies against the JWKS, with exactly the expected claims and `exp - iat
  = 300`; only permission roles in `roles`.
- JWKS: public keys only, every `.pub` present, no `d`.
- Webhook: valid signature → 200 processed; same event twice → one row, second
  answer `duplicate`; bad signature → 401; timestamp 301 s old → 401;
  previous secret accepted; unknown type → 422; deleted user skipped.
- Views (pgsql CI only): `pulsars_api` cannot read `site`; no forbidden
  column in `api_read`; draft content invisible; `member_xp` equals
  `Experience`.
