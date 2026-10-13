# Pulsars API

Public content API and game server of [Pulsars](https://pulsars.science), a
free learning platform: courses, exercises, exams and free resources.

This service runs at `api.pulsars.science` and provides:

- **`/v1`**: read-only access to published content (courses, lessons,
  exercises, exams, resources). Never solutions, never personal data.
- **`/v1/game`**: the game server of `game.pulsars.science` (matchmaking and
  real-time matches over WebSocket). *Not built yet: the game rules are still
  being decided in [`docs/contract/game-design.md`](docs/contract/game-design.md).*

## Architecture in brief

```
game.pulsars.science ──JWT──▶ api.pulsars.science ──signed webhook──▶ pulsars.science
   (Vue SPA)                   (this repository)                       (Laravel)
                                 │        │
                       PostgreSQL         Redis
          reads `api_read` views,      matchmaking,
          writes `game` schema         pub/sub, limits
```

- Laravel (`pulsars.science`) owns accounts, content and XP. It issues
  short-lived EdDSA tokens that this API verifies with Laravel's public keys.
- This API reads Laravel's data **only** through read-only views
  (`api_read`), and writes only its own `game` schema.
- Match results go back to Laravel through a signed webhook.

Details: [`docs/architecture.md`](docs/architecture.md) and the contract in
[`docs/contract/`](docs/contract/README.md) (OpenAPI spec, JWT, webhooks,
database views). The contract is the source of truth: code follows it, and
changes to it are proposed before the code (see
[CONTRIBUTING.md](CONTRIBUTING.md)).

## Run it locally

Requirements: Rust (stable), Docker.

```bash
docker compose up -d          # PostgreSQL with demo data, and Redis
cp .env.example .env
set -a; . ./.env; set +a; cargo run
```

Then `curl http://127.0.0.1:8080/healthz`. Metrics are on
`http://127.0.0.1:9464/metrics`.

The demo database contains a handful of courses, exercises, exams and
resources from the open Pulsars content, plus drafts and answers that the
views must hide. It contains no production data.

## Tests

```bash
cargo test                                   # unit tests and HTTP tests
TEST_DATABASE_URL=postgres://pulsars_api:pulsars_api_dev@localhost:5432/pulsars \
TEST_REDIS_URL=redis://localhost:6379 \
cargo test                                   # plus tests against the compose services
```

CI runs everything against real PostgreSQL and Redis, plus `cargo fmt`,
`cargo clippy -D warnings`, `cargo deny` and a DCO check.

## Status

| Phase | Content | State |
|---|---|---|
| 0 | Contract and plan | done |
| 1 | Open-source skeleton, `/healthz`, CI | in review |
| 2 | JWT authentication, `/v1/me` | next |
| 3 | Content endpoints | planned |
| 4 | Game | blocked on game design |
| 5 | Results webhook to Laravel | planned |
| 6 | Code sandbox (only if the game runs code) | undecided |
| 7 | Deployment | planned |

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
