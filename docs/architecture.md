# Pulsars architecture

Decided on 2026-10-28, when the Laravel site was retired: the server is
rebuilt from scratch, the site is rewritten in Nuxt, and this API owns the
database and every feature. Only the content (courses, exams, resources,
news, files, translations, the languages section) was carried over from
the old database.

## Repositories

| Repository | Content |
|---|---|
| `pulsars-web` | The site, `pulsars.science`: Nuxt with server rendering, a Node service. Pages, translations, the login cookie, the relay to the API. |
| `pulsars-api` | This repository: the back end and the game server, in Rust. |
| `pulsars-game` | The games, `pulsars.science/jeux` (Vue 3 + Vite, static). |
| `pulsars-langues` | The languages section, `pulsars.science/langues` (static). |

## Services

```
browser ──▶ pulsars.science (nginx)
              ├─ /jeux/, /langues/       static files
              └─ everything else ──▶ pulsars-web (Nuxt, Node, 127.0.0.1:3000)
                                       ├─ pages (server rendering)
                                       ├─ /api/**      relay to the API
                                       ├─ /storage/**  relay to /v1/storage/**
                                       └─ /auth/**     login, sign-up, Google, the cookie
                                              │  Pulsars-Session, Pulsars-Service-Key,
                                              ▼  X-Forwarded-For, User-Agent
                                 pulsars-api (Axum, 127.0.0.1:8080) ──▶ PostgreSQL, Redis
api.pulsars.science ──▶ pulsars-api (the games, the languages section, public clients)
```

- **pulsars-web** renders every page on its server and calls the API from
  there, server to server, with the visitor's session token
  (`Pulsars-Session`, from the httpOnly `pulsars_auth` cookie), the
  server's key (`Pulsars-Service-Key`) and the visitor's address. The
  browser only ever talks to `pulsars.science`: the site relays
  `/api/v1/**` to `http://127.0.0.1:8080/v1/**`, and refuses writes whose
  `Origin` is not its own (CSRF).
- **pulsars-api** (Rust, Axum + Tokio): public content (`/v1/courses`,
  `/v1/exams`, …), accounts and sessions (`/v1/auth/*`), the member's pages
  (`/v1/me/*`), the forum and its moderation, follows, friends, rooms and
  private messages, reports and suggestions, the administration
  (`/v1/admin/*`), the languages section (`/v1/languages/*`), the game
  (`/v1/game/*` and the match WebSocket), the game token
  (`POST /v1/auth/token`) and its public key (`/.well-known/jwks.json`).
  `/metrics` on a loopback port.
- **The games and the languages section** are static files served by nginx.
  They call `api.pulsars.science` from the browser with a short JWT the API
  signs (`POST /v1/auth/token`, with the `pulsars_auth` cookie set on
  `.pulsars.science`). The games send actions, never scores: the server
  arbitrates.
- **Edge**: nginx, in front of `pulsars.science` and `api.pulsars.science`,
  HTTPS with certbot.

## Data

- One PostgreSQL database, four schemas: `site` (accounts, progress, forum,
  social, moderation), `contenu` (the content and its files), `game`,
  `langues`. **Only the API connects to it**, as `pulsars_api`, which reads
  and writes rows and changes no structure; `pulsars-api migrate`, as
  `pulsars_migrate`, owns the structure (`contract/database.md`).
- Files (PDFs, avatars, images) are stored in the database
  (`contenu.fichiers`); the API serves them.
- Redis: rate limits, the single-use `jti` of game tokens, short-lived
  counters and caches.

## Flow of a match

1. The game asks `api.pulsars.science` for a token (`POST /v1/auth/token`,
   the `pulsars_auth` cookie, CORS with credentials).
2. The API returns a JWT signed with EdDSA, valid 5 minutes.
3. The game opens the WebSocket to the API; the API verifies the JWT with
   its own key.
4. Matchmaking and the match run in the API's memory; the match is stored
   in `game`.
5. End: the API writes the match, the ratings, the players' results
   (`site.game_results`, from which XP is computed) and their brains in one
   transaction.

## Rules

- The API owns the data: no other program reads or writes the database.
- The public routes never expose solutions or expected answers.
- Players' code never runs on Pulsars' machine (there is no code mode).
- `/metrics` and PostgreSQL stay on the loopback or the private network.
- JSON logs everywhere, and the `traceparent` header is propagated.
- Secrets live in the environment files of the server, never in a
  repository (`docs/deployment.md`).

## Later: observability

Grafana Alloy (collection) → Prometheus (metrics), Loki (logs), Tempo
(traces) → Grafana. PostgreSQL, Redis and node exporters.
Rust: `tracing` (JSON) + `metrics-exporter-prometheus`.

## Notes

- **Guilds and badges do not exist** (planned, never built). Distinction
  roles act as badges.
- **XP is computed, not stored** (`site.member_xp`): awarding XP means
  adding a source to that view.
- The domain → subject → topic catalogue does not exist; courses have a
  single `theme` string, from which the API derives subject and topic.
