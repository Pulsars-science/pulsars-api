# Pulsars architecture

Target architecture, validated on 2026-10-13. Translated from the product
owner's French original; the "Notes" at the end record where the current
Laravel code differs.

## Repositories

| Repository | Content |
|---|---|
| `pulsars-web` | The Laravel site (today's GitHub repository is `0jikuji0/pulsars`). |
| `pulsars-api` | This repository: the public API and the game server, in Rust. |
| `pulsars-game` | The front end of `pulsars.science/jeux` (Vue 3 + Vite). |

## Services

- **pulsars.science**: Laravel 13 with Inertia/Vue SSR. Pages, SEO,
  translations, accounts (e-mail, Google, Discord), forum, XP, guilds, admin.
  **Issues game tokens (JWT).**
- **api.pulsars.science**: Rust (Axum + Tokio). `/v1` read-only content
  (courses, exercises, exams, resources), `/v1/game` (matchmaking, matches),
  the match WebSocket, `/metrics` on an internal port.
- **pulsars.science/jeux**: Vue 3 + Vite SPA, static. Sends actions, never
  scores: the server arbitrates.
- **Edge**: Caddy / FrankenPHP: HTTPS and routing by domain.

## Data

- PostgreSQL, one schema per service: `site` (written by Laravel), `game`
  (written by the API). The API reads `site` **only** through the read-only
  views of the `api_read` schema.
- Redis: matchmaking queues, pub/sub between instances, rate limits.
- Execution sandbox (if code matches): self-hosted Judge0 or Piston, on a
  separate VM, without network, with time and memory limits.

## Flow of a match

1. The front end asks `pulsars.science` for a token (Laravel session cookie,
   CORS with credentials).
2. Laravel returns a JWT signed with EdDSA, valid 5 minutes.
3. The front end opens the WebSocket to the API; the API verifies the JWT
   with Laravel's public key (JWKS).
4. Matchmaking in Redis, match arbitrated by the API, match stored in `game`.
5. End: the API writes the match, the ratings, the players' results
   (`site.game_results`, from which XP is computed) and their brains in one
   transaction.

## Data ownership

| Data | Written by | Read by |
|---|---|---|
| Accounts, profiles | Laravel | API (token) |
| Courses, exercises, exams | Laravel | API, read-only |
| XP, brains | API (results, brains ledger) | API |
| Matches, Elo, rankings | API | Laravel through `/v1/game` |
| Queues, live state | API (Redis) | — |

## Rules

- No service writes into the other's schema.
- The public API never exposes solutions or expected answers.
- Players' code never runs on Pulsars' machine.
- `/metrics` and PostgreSQL stay on the private network.
- JSON logs everywhere, and the `traceparent` header is propagated.

## Later: observability

Grafana Alloy (collection) → Prometheus (metrics), Loki (logs), Tempo
(traces) → Grafana. PostgreSQL, Redis and node exporters.
Rust: `tracing` (JSON) + `metrics-exporter-prometheus`. Laravel: JSON log
channel + `spatie/laravel-prometheus`.

## Prerequisite

- Move production to PostgreSQL before the game.

## Notes: where the Laravel code differs today (2026-10-13)

Details and sources are in `docs/contract/`.

- Production runs **MySQL**, not SQLite (`docs/MISE-EN-PROD.md`, systemd units
  `After=mysql.service`). The prerequisite is a MySQL → PostgreSQL migration:
  `contract/postgres-migration.md`.
- The game front end is served by nginx under **`pulsars.science/jeux/`**,
  not a `game.` subdomain (decided 2026-10-17): no extra DNS or certificate,
  same origin as the site. The flash questions (30 generators, wall, défi)
  moved there from Laravel, ported to TypeScript with identical output.
- The edge in production is **nginx** in front of Octane/FrankenPHP, not Caddy
  (`deploy/pulsars.science.conf`).
- **Guilds and badges do not exist** in Laravel (planned v2). Distinction
  roles act as badges.
- **XP is computed, not stored** (`App\Services\Experience`): "Laravel awards
  XP" means Laravel stores a result row that its XP computation counts.
- The domain → subject → topic catalogue does not exist; courses have a single
  `theme` string, from which the API derives subject and topic.
- Rule "Laravel reads game data through `/v1/game`": the contract also
  forbids Laravel any read access to the `game` schema, for symmetry.
