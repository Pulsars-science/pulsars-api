# Migrations

sqlx migrations, embedded in the binary at build time. Since the Laravel
migrations were frozen (0jikuji0/pulsars `docs/MIGRATIONS.md`), these are
the only ones: `game` and `langues` today, `contenu` and `site` as the API
takes them over. `api_read` stays as Laravel created it until it is dropped.

## Who runs them

`pulsars-api migrate`, never the service:

- in production, `deploy-api` runs it with the new release, before the
  switch, as `pulsars_migrate` (`DATABASE_MIGRATION_URL` in
  `/etc/pulsars-api/migrate.env`, read by root only);
- locally, `cargo run -- migrate` with `DATABASE_MIGRATION_URL` from `.env`;
- in tests, `TEST_MIGRATION_DATABASE_URL`.

Objects created by `pulsars_migrate` belong to `pulsars_owner`
(`deploy/sql/phase-1.sql`); `pulsars_api` and `pulsars_site` get their rows'
rights from the default privileges of each schema. sqlx records the history
in `game._sqlx_migrations` (`pulsars_migrate`'s `search_path` starts with
`game`). At startup the service only checks that every embedded migration is
applied, and waits until it is.

## Rules

- Never edit a migration that has been released: add a new one.
- Keep them additive. `deploy-api` applies them while the previous release
  still runs, and rolling the binary back leaves the schema as it is.
- A migration that creates a **schema** also grants it: `USAGE` to the
  roles that need it, and `ALTER DEFAULT PRIVILEGES FOR ROLE pulsars_owner IN
  SCHEMA …` for their rows. Default privileges are per schema, so a new one
  starts with none.
- Name: `<UTC timestamp>_<what>.sql`, e.g. `20261017000000_game_schema.sql`.
