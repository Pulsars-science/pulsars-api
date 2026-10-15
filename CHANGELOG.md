# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Architecture and API contract documents (`docs/`).
- Axum service skeleton: typed configuration validated at startup, JSON logs,
  W3C `traceparent` handling, Prometheus metrics on an internal listener,
  RFC 9457 problem responses, graceful shutdown on SIGTERM.
- `GET /healthz` checking PostgreSQL and Redis.
- Local development stack (`compose.yaml`) with demo data and the real
  `api_read` views.
- CI: formatting, Clippy, tests against PostgreSQL and Redis, cargo-deny,
  DCO sign-off check.
- Open-source project files: licence, contributing guide, code of conduct,
  security policy, trademark policy, issue and pull request templates.
- Game token verification (`docs/contract/auth-jwt.md`): EdDSA (Ed25519)
  JWTs checked in the contract's order, with 401 problems carrying a
  `reason` and a `WWW-Authenticate` challenge. Laravel's JWKS is loaded at
  startup, refreshed every 10 minutes, kept for up to 24 hours when Laravel
  is unreachable, and refetched at most once a minute on an unknown `kid`.
- `GET /v1/me`: public profile, roles and XP of the token's member.
- Settings `JWKS_URL` (required), `JWT_ISSUER`, `JWT_AUDIENCE` and
  `PUBLIC_STORAGE_URL`.
- Metrics `auth_rejections_total`, `jwks_fetches_total` and `jwks_keys`.

### Changed

- `GET /healthz` answers 503 (`down`) until Laravel's JWKS has been loaded
  once.
