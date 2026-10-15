# Migrations

sqlx migrations for the **`game` schema only**. This repository never
creates, alters or drops anything in `site` or `api_read`: `site` belongs to
Laravel, and the `api_read` views are created by a Laravel migration from
`docs/contract/views.sql` (see `docs/contract/database.md`).

The first migration arrives with the game (Phase 4).
