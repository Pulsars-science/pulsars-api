# Pulsars API

The back end of [Pulsars](https://pulsars.science), a free learning
platform: courses, exercises, exams, free resources, a forum, duels and a
languages section. It owns the database and every feature; the site, the
games and the languages section are its clients.

This service runs at `api.pulsars.science`, and on the loopback for the
site. It provides:

- **public content** (`/v1/courses`, `/v1/exercises`, `/v1/exams`,
  `/v1/resources`, `/v1/news`, `/v1/languages`…): published content only,
  never solutions, never personal data;
- **accounts** (`/v1/auth/*`): sign-up, login sessions, passwords, account
  mails, Google accounts, and the short game token (`POST /v1/auth/token`)
  with its public key (`/.well-known/jwks.json`);
- **what the site's pages need** (`/v1/me/*`, `/v1/forum/*`,
  `/v1/members/*`, `/v1/friends/*`, `/v1/rooms/*`, `/v1/conversations/*`,
  `/v1/moderation/*`, `/v1/admin/*`…): progress, exercises, the dashboard,
  the forum and its moderation, the social part, private messages
  (encrypted at rest), reports, suggestions and the administration;
- **the game server** of the flash duels (`/v1/game`, `pulsars-game`):
  matchmaking, private rooms, ghost runs and real-time matches over the
  WebSocket `/v1/game/ws`, Glicko-2 ratings per subject, leaderboards,
  brains and monthly seasons. Protocol:
  [`docs/game-protocol.md`](docs/game-protocol.md); rules:
  [`docs/contract/game-design.md`](docs/contract/game-design.md).

## Architecture in brief

```
browser ──▶ pulsars.science ──▶ pulsars-web (Nuxt) ──Pulsars-Session──▶ pulsars-api ──▶ PostgreSQL
                                                     Pulsars-Service-Key      │     └──▶ Redis
pulsars.science/jeux, /langues ──JWT──▶ api.pulsars.science ──────────────────┘
```

- `pulsars-web` calls the API from its server, with the visitor's session
  token and the server's service key
  ([`auth-sessions.md`](docs/contract/auth-sessions.md)).
- The games and the languages section get a short EdDSA token from
  `POST /v1/auth/token`; the API signs it and verifies it with its own key
  ([`auth-jwt.md`](docs/contract/auth-jwt.md)).
- The API is the only program that connects to the database
  ([`database.md`](docs/contract/database.md)). A finished match writes its
  results, ratings and brains in one transaction
  ([`brains.md`](docs/contract/brains.md)).

Details: [`docs/architecture.md`](docs/architecture.md) and the contract in
[`docs/contract/`](docs/contract/README.md). The contract is the source of
truth: code follows it, and changes to it are proposed before the code (see
[CONTRIBUTING.md](CONTRIBUTING.md)).

## Run it locally

Requirements: Rust (stable), Docker.

```bash
docker compose up -d          # PostgreSQL with demo data, and Redis
cp .env.example .env
set -a; . ./.env; set +a
cargo run -- migrate          # as pulsars_migrate (DATABASE_MIGRATION_URL)
cargo run
```

The service never migrates: its role may not create tables. It waits, and
logs which migrations are missing, until `pulsars-api migrate` has run. Then
`curl http://127.0.0.1:8080/healthz`. Metrics are on
`http://127.0.0.1:9464/metrics`. Content needs no token:

```bash
curl 'http://127.0.0.1:8080/v1/courses?lang=en'
curl 'http://127.0.0.1:8080/v1/courses/les-bases-du-calcul/lessons/les-fractions'
curl 'http://127.0.0.1:8080/v1/exercises?type=qcm&limit=2'
```

The demo account `ada` (password `secret-pass-demo`, `dev/db/30-demo-data.sql`)
logs in with `POST /v1/auth/login`, which gives a session token for
`Pulsars-Session`.
Game tokens need a signing key: `cargo run -- jwt-keygen 2026-10-ed25519-1 .`
writes one (`*.key` is git-ignored), and prints the `JWT_SIGNING_KEY_FILE`
and `JWT_SIGNING_KID` to put in `.env`. Without them (development only), `POST /v1/auth/token` and
the routes that need a game token answer 503.

The game WebSocket is `ws://127.0.0.1:8080/v1/game/ws`: the `Origin` must
be one of `CORS_ALLOWED_ORIGINS`, and the first frame authenticates
(`{"type": "auth", "guest": true}` needs no token). At startup, and every
10 minutes, the API also closes the previous UTC month's duel season if
needed (standings, rating decay, brain rewards; `docs/contract/brains.md`
§3): a fresh local database gets its last month closed, with empty
standings, on the first start.

The demo database contains a handful of courses, exercises, exams and
resources from the open Pulsars content, plus drafts and answers that the
public routes must hide, and a few demo accounts. It contains no
production data.

## Tests

```bash
cargo test                                   # unit tests; integration tests skip
TEST_DATABASE_URL=postgres://pulsars_api:pulsars_api_dev@localhost:5432/pulsars \
TEST_MIGRATION_DATABASE_URL=postgres://pulsars_migrate:pulsars_migrate_dev@localhost:5432/pulsars \
TEST_REDIS_URL=redis://localhost:6379 \
cargo test                                   # everything, against the compose services
```

CI runs everything against real PostgreSQL and Redis
(`REQUIRE_INTEGRATION=1`: a missing service fails instead of skipping),
plus `cargo fmt`, `cargo clippy -D warnings`, `cargo deny`, `shellcheck` and
a DCO check.

## Deployment

In production the API runs on the Pulsars server behind nginx, as a systemd
service installed from release archives that CI publishes on each `vX.Y.Z`
tag. A new server, the database, the environment, upgrades and rollback:
[`docs/deployment.md`](docs/deployment.md).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Commits must be signed off
(`git commit -s`). Security issues: see [SECURITY.md](SECURITY.md), never a
public issue.

## Licence

Copyright © the Pulsars contributors.

Pulsars API is free software: you can redistribute it and/or modify it under
the terms of the [GNU Affero General Public License](LICENSE) as published by
the Free Software Foundation, either version 3 of the License, or (at your
option) any later version (`AGPL-3.0-or-later`). If you run a modified
version for users over a network, you must offer them its source code.

The Pulsars name and logo are not covered by this licence: see
[TRADEMARK.md](TRADEMARK.md).
