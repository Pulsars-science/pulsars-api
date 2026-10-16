# Contributing to Pulsars API

Thank you for helping. This guide covers setup, conventions, and how to
change the API contract.

## Setup

1. Install Rust (stable) with `rustup`, and Docker.
2. Start the services and run the API:

   ```bash
   docker compose up -d
   cp .env.example .env
   set -a; . ./.env; set +a; cargo run
   ```

3. Before pushing, run what CI runs:

   ```bash
   cargo fmt --all
   cargo clippy --all-targets -- -D warnings
   cargo test
   cargo deny check        # cargo install cargo-deny
   ```

To recreate the database from scratch: `docker compose down -v && docker compose up -d`.

## Conventions

- **Language**: code, comments, commit messages and documentation in English.
- **Errors**: never `unwrap()`, `expect()` or `panic!` outside tests (Clippy
  denies them). Request errors become RFC 9457 problem documents through
  `ApiError`.
- **Data**: the API reads Laravel's data only through the `api_read` views
  and writes only the `game` schema. Migrations in `migrations/` touch `game`
  only.
- **No secrets** in the repository. New settings are environment variables,
  documented in `.env.example` and validated in `src/config.rs`.
- **The server arbitrates**: game clients send actions, never scores or
  results.
- **Tests**: new behaviour comes with tests. Database and Redis behaviour is
  tested against the real services (see the README).
- **Commits**: small and focused, imperative subject line ("Add cursor
  pagination"), body explaining why when it is not obvious.
- **Changelog**: user-visible changes get a line under `[Unreleased]` in
  `CHANGELOG.md`.
- One phase or feature per branch and pull request.

## Developer Certificate of Origin

Every commit must be signed off, certifying that you wrote the change or have
the right to submit it under the project's licence, as described by the
[Developer Certificate of Origin 1.1](https://developercertificate.org/):

```bash
git commit -s -m "Add cursor pagination"
```

This appends `Signed-off-by: Your Name <you@example.org>`, which must match
the commit author. CI rejects pull requests with unsigned commits. To sign off
commits you already made: `git rebase --signoff origin/main`.

## Changing the contract

`docs/contract/` is the source of truth for the public API, the JWT, the
webhooks and the database views. It is shared with the Laravel site and the
game front end, so it changes **before** the code:

1. Open an issue with the "Contract change" template: what changes, why, and
   who is affected (API clients, Laravel, the game front end).
2. Open a pull request that changes only the documents in `docs/contract/`.
   For `openapi.yaml`, keep the spec valid and every example matching its
   schema.
3. Within `/v1`, only additive changes are allowed (new endpoints, optional
   parameters, new fields). Anything else needs `/v2`.
4. Once the contract change is merged, implement it in a separate pull request.

Changes to `views.sql` also require a matching migration in the Laravel
repository; mention it in the pull request.

## Reporting bugs and security issues

Bugs: open an issue with the bug template. Security vulnerabilities: follow
[SECURITY.md](SECURITY.md) and do **not** open a public issue.

## Code of conduct

Participation is governed by the [Code of Conduct](CODE_OF_CONDUCT.md).
