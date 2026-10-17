# Migrations

sqlx migrations for the **`game` schema only**. This repository never
creates, alters or drops anything in `site` or `api_read`: `site` belongs to
Laravel, and the `api_read` views are created by a Laravel migration from
`docs/contract/views.sql` (see `docs/contract/database.md`).

The binary applies them at startup (`sqlx::migrate!`, embedded at build
time), retrying while PostgreSQL is down; sqlx records them in
`game._sqlx_migrations` (the role's `search_path` starts with `game`).

- Never edit a migration that has been released: add a new one.
- Keep them additive, so that rolling the binary back leaves a schema it can
  still use.
- Name: `<UTC timestamp>_<what>.sql`, e.g. `20261017000000_game_schema.sql`.
