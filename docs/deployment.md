# Deployment

How `pulsars-api` is installed on a **new** Pulsars server, then upgraded.
This is the reference: every variable, file and command the API needs. The
site (`pulsars-web`) is installed by its own repository's guide; this one
only says what the API needs from it.

The server: Ubuntu 26.04, x86_64, nginx in front of every site, no Docker.

```
Internet → nginx (80/443) ─┬→ pulsars.science      ─┬→ /jeux/, /langues/ (static files)
                           │                        └→ pulsars-web 127.0.0.1:3000 ─┐
                           └→ api.pulsars.science  ──────────────────────────────────┴→ pulsars-api 127.0.0.1:8080
                                                                                         │   │   └─ 127.0.0.1:9464 /metrics (never proxied)
                                                                                PostgreSQL   Redis
```

## Contents

1. [Prerequisites](#1-prerequisites)
2. [Choices made](#2-choices-made)
3. [Open decisions](#3-open-decisions)
4. [First installation](#4-first-installation)
5. [Environment variables](#5-environment-variables)
6. [Upload size limits](#6-upload-size-limits)
7. [Upgrades](#7-upgrades)
8. [Rollback](#8-rollback)
9. [Logs](#9-logs)
10. [Metrics](#10-metrics)
11. [The sandbox](#11-the-sandbox)
12. [Troubleshooting](#12-troubleshooting)

Files: [`deploy/`](../deploy/).

| File | Installed as |
|---|---|
| `pulsars-api.service` | `/etc/systemd/system/pulsars-api.service` (by `deploy-api`) |
| `api.pulsars.science.conf` | `/etc/nginx/sites-available/pulsars-api` (by `deploy-api`; linked by hand once) |
| `pulsars-api.env.example` | `/etc/pulsars-api/pulsars-api.env` (filled in) |
| `sql/roles.sql`, `sql/README.md` | run once on a new server; in every release under `deploy/sql/` |
| `deploy-api` | run from each release; a copy stays in every release directory |
| `package-release` | used by CI to build the release archive |

---

## 1. Prerequisites

| # | What | How to check |
|---|---|---|
| P1 | A server running **x86_64** with **glibc 2.39 or newer** (CI builds on Ubuntu 24.04; Ubuntu 26.04 is newer) | `uname -m`, `ldd --version \| head -1`. `deploy-api` checks it too before touching anything. |
| P2 | **DNS**: `pulsars.science` and `api.pulsars.science` → the server | `dig +short api.pulsars.science` gives the server's address and nothing else |
| P3 | **PostgreSQL 17** or newer, **Redis** (or Valkey), **nginx**, **certbot**, `curl`, `jq` | `psql --version`, `redis-server --version`, `nginx -v` |
| P4 | A **release** of this repository built by CI (a `vX.Y.Z` tag on `main`) | the GitHub release page lists `pulsars-api-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz` and its `.sha256` |
| P5 | For the content: the copy of the old database made before the reset (`~/pulsars-secrets/pulsars-avant-reset.dump`) | `pg_restore -l` on it lists `contenu` or `site` tables |
| P6 | Secrets kept off the server too: the database passwords, the admin password, the signing key, `MESSAGES_KEY`, `SERVICE_KEY`, the SMTP and DeepL keys | a password manager |

`pulsars-web` can be installed before or after the API; it needs the API's
`SERVICE_KEY` (§4.8) and the address `http://127.0.0.1:8080`.

---

## 2. Choices made

**The binary is built by CI, not on the server.** A tag `vX.Y.Z` runs
`.github/workflows/release.yml`: `cargo build --release --locked` for
`x86_64-unknown-linux-gnu` on a pinned `ubuntu-24.04` runner, packaged with
`deploy/package-release` into one archive plus its SHA-256. The server
stays free of a Rust toolchain, the archive deployed is the archive that
was built once, and a deployment takes seconds.

**The archive is copied to the server by the operator** (`scp` from a
machine logged in to `gh`). If the repository is public, the server can
fetch it with `curl -LO` instead; nothing else changes.

**The release carries its own deployment files**: the binary, `deploy-api`,
the systemd unit, the nginx vhost, the example environment file and
`deploy/sql/`. The server needs no clone of the repository, and the vhost
and unit always match the binary they ship with.

**Layout on the server.**

| Path | Owner, mode | Content |
|---|---|---|
| `/opt/pulsars-api/releases/vX.Y.Z/` | `root:root`, `0755` | one directory per release, never modified after install |
| `/opt/pulsars-api/current` | symlink | the running release; `ExecStart` goes through it |
| `/opt/pulsars-api/previous` | symlink | the release `--rollback` returns to |
| `/etc/pulsars-api/` | `root:pulsars-api`, `0750` | the configuration |
| `/etc/pulsars-api/pulsars-api.env` | `root:root`, `0600` | configuration and secrets of the service (§5) |
| `/etc/pulsars-api/migrate.env` | `root:root`, `0600` | `DATABASE_MIGRATION_URL`, for `pulsars-api migrate` only |
| `/etc/pulsars-api/keys/` | `pulsars-api:pulsars-api`, `0700` | the game token signing key, `<kid>.key`, `0600` |
| account `pulsars-api` | system user, no home, no shell | runs the service; owns only its key |

The service account cannot read its configuration: systemd reads
`EnvironmentFile=` as root before dropping privileges. It reads only its
signing key, which it must (the API refuses a key file readable by anyone
else in production).

**The API writes nothing to disk.** Files are in the database; logs go to
the journal.

**Migrations run before the switch**, by `deploy-api`, as `pulsars_migrate`
(`pulsars-api migrate`); the service only waits for the schema. Migrations
are additive, so the release still running keeps working on the migrated
schema, and rolling the binary back leaves the schema as it is.

**Swap and rollback.** `deploy-api` renames a new symlink over `current`
(atomic: the link is never missing), restarts the unit, then waits up to 45 s
for `/healthz` to answer 200 **with the new version number**. Otherwise it
points `current` back to the previous release, restarts it and checks it.

**nginx** sets `X-Forwarded-For` to the address it sees, replacing whatever
the client sent. The API reads the **last** entry of that header as the client
IP (`client_ip` in `src/middleware.rs`) and keys the anonymous rate limit on
it: with one entry chosen by nginx, a client cannot pick its own bucket.
`pulsars-web` does the same on the loopback: it sends the visitor's address
as the last entry.

---

## 3. Open decisions

Each has a default already written into the files; change the files if the
answer differs.

| # | Question | Default in the files |
|---|---|---|
| D1 | How do release archives reach the server: `scp` from the operator's machine, a read-only GitHub token on the server, or a public repository? (A public repository also settles the AGPL obligation to offer the running source to network users.) | `scp` from the operator's machine |
| D2 | Certificate: a separate one for `api.pulsars.science`, or the site's certificate with a second name? | separate certificate, renewed by certbot |
| D3 | HSTS: one year, without `includeSubDomains` or `preload`. | `max-age=31536000` |
| D4 | PostgreSQL authentication: password over TCP on 127.0.0.1 (scram-sha-256), or a Unix socket with peer authentication? | password over TCP |
| D5 | Redis: memory cap, eviction policy, persistence. | 128 MB, `noeviction`, default snapshots |
| D6 | Schools behind one public IP: 60 anonymous requests a minute per IP, and nginx's 10 requests a second per IP, are shared by a whole classroom on `api.pulsars.science`. The site's own calls are not limited (`SERVICE_KEY`). | contract values |
| D7 | Resource limits: `MemoryMax=512M`, `LimitNOFILE=65536`, `TasksMax=256`. | as listed |
| D8 | Downtime on deploy: a plain restart drops new connections for about a second, in-flight requests get up to 20 s, and running matches over WebSocket are cut. | plain restart |
| D9 | IPv6: the vhost listens on `[::]` too. An `AAAA` record only if the server has a public IPv6 address. | no `AAAA` record |
| D10 | Monitoring: nothing scrapes `/metrics` yet. Who gets alerted when `/healthz` fails? | nothing |

---

## 4. First installation

Commands run **on the server**, as the usual administrator account (with
`sudo`), unless marked *operator's machine*.

### 4.1 Check the machine

```bash
uname -m                  # x86_64
ldd --version | head -1   # 2.39 or newer
systemctl --version | head -1
nginx -v
psql --version            # 17 or newer
dig +short api.pulsars.science
```

### 4.2 Packages

```bash
sudo apt install postgresql redis-server nginx certbot python3-certbot-nginx curl jq
```

PostgreSQL listens on localhost only (`listen_addresses`, Ubuntu's default):
keep it that way.

### 4.3 Account and directories

```bash
sudo useradd --system --user-group --no-create-home --home-dir /nonexistent \
    --shell /usr/sbin/nologin pulsars-api
sudo install -d -m 0755 -o root -g root /opt/pulsars-api /opt/pulsars-api/releases
sudo install -d -m 0750 -o root -g pulsars-api /etc/pulsars-api
sudo install -d -m 0700 -o pulsars-api -g pulsars-api /etc/pulsars-api/keys
```

### 4.4 The release archive

*Operator's machine*, logged in to `gh`:

```bash
V=v0.6.0
gh release download "$V" -R Pulsars-science/pulsars-api \
    -p "pulsars-api-$V-x86_64-unknown-linux-gnu.tar.gz*"
scp pulsars-api-$V-x86_64-unknown-linux-gnu.tar.gz* <admin>@<server>:/tmp/
```

On the server:

```bash
cd /tmp
V=v0.6.0
sha256sum -c "pulsars-api-$V-x86_64-unknown-linux-gnu.tar.gz.sha256"   # OK
tar xzf "pulsars-api-$V-x86_64-unknown-linux-gnu.tar.gz"
R="/tmp/pulsars-api-$V-x86_64-unknown-linux-gnu"
ls "$R" "$R/deploy/sql"
```

### 4.5 PostgreSQL: database, roles, schemas

[`deploy/sql/README.md`](../deploy/sql/README.md) §1: `createdb pulsars`,
then `roles.sql` from `$R/deploy/sql/` with the two passwords
(`openssl rand -hex 32` each), then the `pg_hba.conf` lines. The tables
come with the first deployment (§4.13).

### 4.6 Redis

Install it listening on localhost only, with a user that can only touch the
API's keys (`pulsars-api:*`):

```bash
openssl rand -hex 32        # the Redis password, for REDIS_URL
sudo install -m 0640 -o redis -g redis /dev/null /etc/redis/users.acl
sudoedit /etc/redis/users.acl
```

```
user default off
user pulsars_api on >THE_REDIS_PASSWORD ~pulsars-api:* &* +@all -@dangerous +info
```

`+info`: the client reads the server version on connect. `-@dangerous`
removes `KEYS`, `FLUSHALL`, `CONFIG`, `DEBUG` and the like.

In `/etc/redis/redis.conf`, make sure of:

```
bind 127.0.0.1 -::1
protected-mode yes
port 6379
aclfile /etc/redis/users.acl
maxmemory 128mb
maxmemory-policy noeviction
```

`noeviction`: when full, Redis refuses writes instead of silently dropping
keys. The rate limiter then lets requests through (it fails open) and
`rate_limit_errors_total` rises. Matchmaking queues live in the API's
memory; Redis records the single-use `jti` of game tokens, and a game
session is refused (`1013`) while Redis cannot record it.

```bash
sudo systemctl restart redis-server
sudo systemctl enable redis-server
redis-cli --user pulsars_api --askpass ping        # PONG
redis-cli ping                                     # NOAUTH or error: default user is off
```

### 4.7 The game token signing key

```bash
sudo -u pulsars-api "$R/pulsars-api" jwt-keygen 2026-10-ed25519-1 /etc/pulsars-api/keys
```

It writes `/etc/pulsars-api/keys/2026-10-ed25519-1.key` (the 32-byte
Ed25519 seed in base64, mode `0600`, never over an existing file) and
prints the public key and the two lines for the environment file:

```
JWT_SIGNING_KEY_FILE=/etc/pulsars-api/keys/2026-10-ed25519-1.key
JWT_SIGNING_KID=2026-10-ed25519-1
```

The `kid` reads `YYYY-MM-ed25519-N`: the month the key was made and a
counter. Keep a copy of the file with the other secrets (P6). Rotation:
`docs/contract/auth-jwt.md` §3.

### 4.8 The other secrets

```bash
openssl rand -base64 32     # MESSAGES_KEY: the AES-256-GCM key of private messages
openssl rand -hex 32        # SERVICE_KEY: shared with pulsars-web
```

- **`MESSAGES_KEY`** seals every private message. Losing it makes them all
  unreadable; changing it too. Generate it once, and keep a copy with the
  database backups' secrets.
- **`SERVICE_KEY`** goes in both environments: the API's and
  `pulsars-web`'s. The site's calls carry it in `Pulsars-Service-Key`.
- **SMTP**: the account mails (address verification, forgotten password)
  leave through `MAIL_SMTP_URL`, `smtps://user:password@host:465` (TLS) or
  `smtp://user:password@host:587?tls=required` (STARTTLS), as lettre reads
  it, with `MAIL_FROM` as sender. Production uses Resend:
  `smtps://resend:<API key>@smtp.resend.com:465`; the sender's domain must
  be verified there (SPF, DKIM).
- **DeepL**: `DEEPL_API_KEY`, from the DeepL account (a free key ends in
  `:fx` and goes with `https://api-free.deepl.com`; a paid one with
  `https://api.deepl.com`).
- **Umami**: the four `UMAMI_*` values, if the site's audience is measured
  by an Umami instance; the admin dashboard reads it.

### 4.9 The environment files

```bash
sudo install -m 0600 -o root -g root "$R/pulsars-api.env.example" /etc/pulsars-api/pulsars-api.env
sudoedit /etc/pulsars-api/pulsars-api.env
```

Replace every `CHANGE_ME` (§5 lists every variable). Then the migrations'
own file, which only `pulsars-api migrate` reads:

```bash
sudo install -m 0600 -o root -g root /dev/null /etc/pulsars-api/migrate.env
sudoedit /etc/pulsars-api/migrate.env
```

```
DATABASE_MIGRATION_URL=postgres://pulsars_migrate:THE_MIGRATE_PASSWORD@127.0.0.1:5432/pulsars
```

Check the configuration as the service will read it (exit `124`, stopped by
`timeout` after starting, means it is valid; `78` prints every problem):

```bash
sudo systemd-run --pipe --wait --quiet --collect -p User=pulsars-api \
    -p EnvironmentFile=/etc/pulsars-api/pulsars-api.env \
    timeout 3 "$R/pulsars-api"; echo "exit $?"
```

### 4.10 Certificates

Before the vhost exists, certbot's nginx plugin answers the challenge with a
temporary server block of its own:

```bash
sudo certbot certonly --nginx -d api.pulsars.science
sudo ls /etc/letsencrypt/live/api.pulsars.science/   # fullchain.pem privkey.pem
sudo certbot renew --dry-run
```

`certonly`: certbot must not edit the vhost, which already holds the 443
block. `pulsars.science` gets its own certificate the same way (its guide).

### 4.11 nginx for `api.pulsars.science`

```bash
sudo cp "$R/api.pulsars.science.conf" /etc/nginx/sites-available/pulsars-api
sudo ln -s /etc/nginx/sites-available/pulsars-api /etc/nginx/sites-enabled/
sudo nginx -t && sudo systemctl reload nginx
```

`reload`, not `restart`: connections to the other sites are not cut. Until
the API runs, `https://api.pulsars.science/` answers 502. `deploy-api`
keeps the file up to date afterwards.

What the vhost does: HTTP/2 and the redirect from port 80; HSTS,
`Content-Security-Policy: default-src 'none'` and `Referrer-Policy`;
`X-Forwarded-For` set to the client's address; a flood guard (10 requests
a second per IP, burst 40) under the API's own limits; 256 KiB bodies at
most (uploads go through `pulsars.science`, §6); long timeouts on
`/v1/game/ws`; `/metrics` answers 404.

### 4.12 nginx for `pulsars.science`: what the API needs

The `pulsars.science` vhost belongs to `pulsars-web`'s guide. For the API,
it must:

- proxy everything but `/jeux/` and `/langues/` to `pulsars-web`
  (`127.0.0.1:3000`), with `X-Forwarded-For $remote_addr`: the site passes
  it on, and the API counts downloads and views per visitor;
- let the uploads through, with the limits of §6.

### 4.13 First deployment

```bash
"$R/deploy-api"
```

It checks the binary, copies the release to `/opt/pulsars-api/releases/`,
installs and enables the unit, puts the vhost in place, **applies the
migrations** (the whole schema and the reference data, as
`pulsars_migrate`), points `current` to the release, starts the service,
waits for `/healthz`, then checks the public URL.

### 4.14 The administration account and the content

[`deploy/sql/README.md`](../deploy/sql/README.md) §3 and §4:
`pulsars-api create-admin <pseudo> <email>` (the password on standard
input), then `pulsars-api import-content <old database URL> <pseudo>` from
the restored copy of the old database, then `pulsars-api content check`.

### 4.15 Verify

```bash
systemctl status pulsars-api --no-pager            # active (running), enabled
curl -s http://127.0.0.1:8080/healthz | jq         # "status": "ok"
curl -sI https://api.pulsars.science/healthz       # HTTP/2 200, strict-transport-security
curl -s 'https://api.pulsars.science/v1/courses?limit=1' | jq '.data[0].slug'
curl -s https://api.pulsars.science/.well-known/jwks.json | jq '.keys[0].kid'   # the kid of §4.7
curl -s -o /dev/null -w '%{http_code}\n' https://api.pulsars.science/metrics   # 404
curl -s http://127.0.0.1:9464/metrics | head       # Prometheus text
sudo ss -tlnp | grep pulsars-api                   # 127.0.0.1:8080 and 127.0.0.1:9464 only
sudo systemd-analyze security pulsars-api | tail -1   # about 1.2 "OK"

# X-Forwarded-For cannot be forged: RateLimit-Remaining keeps decreasing.
for i in 1 2 3; do
  curl -s -o /dev/null -D - -H "X-Forwarded-For: 192.0.2.$i" \
    https://api.pulsars.science/v1/courses | grep -i ratelimit-remaining
done
```

Then, once `pulsars-web` runs: log in on the site, open a game (it asks
`POST /v1/auth/token`), send a private message.

Clean up: `rm -rf /tmp/pulsars-api-v*`.

---

## 5. Environment variables

The source of truth is `src/config.rs`; `deploy/pulsars-api.env.example`
lists them all with the production values. Every problem is reported at
once, and the service then exits with status 78 (no restart loop).
**Required** means the service refuses to start without it in production.

| Variable | Production | Default | Notes |
|---|---|---|---|
| `PULSARS_ENV` | **required**: `production` | — | `production` refuses `http://` URLs and CORS origins, and requires the signing key and `MESSAGES_KEY` |
| `DATABASE_URL` | **required** | — | `postgres://pulsars_api:…@127.0.0.1:5432/pulsars`; secret |
| `REDIS_URL` | **required** | — | `redis://pulsars_api:…@127.0.0.1:6379/0` (`redis://` or `rediss://`); secret |
| `JWT_SIGNING_KEY_FILE` | **required** | — | the game token signing key (§4.7); mode 0600 enforced |
| `JWT_SIGNING_KID` | **required** | — | its `kid`, `YYYY-MM-ed25519-N`; one goes with the other |
| `MESSAGES_KEY` | **required** | — | 32 bytes in base64 (`openssl rand -base64 32`); secret |
| `MESSAGES_KEY_VERSION` | optional | `1` | stored with each private message, 1 to 32767 |
| `SERVICE_KEY` | optional for the API, **needed by the site** | unset | at least 32 characters; unset: every call is rate limited and the service-key routes answer 403 |
| `MAIL_SMTP_URL` | optional, set in production | unset | `smtp://` or `smtps://user:password@host:port`; unset: mails are only logged, without their link; secret |
| `MAIL_FROM` | optional | `Pulsars <noreply@blackenium.com>` | a mailbox |
| `DEEPL_API_KEY` | optional | unset | unset: the translation routes answer 503 `not_configured` and `content translate` refuses to run; secret |
| `DEEPL_API_URL` | optional | `https://api-free.deepl.com` | `https://api.deepl.com` for a paid plan |
| `DEEPL_LANGUAGES` | optional | `fr=FR,en=EN-GB,ru=RU` | the site's languages and DeepL's `target_lang` for each |
| `UMAMI_URL`, `UMAMI_WEBSITE_ID`, `UMAMI_USERNAME`, `UMAMI_PASSWORD` | optional | unset | all four or none; unset: the admin dashboard says the audience is not configured; http accepted (usually the same machine) |
| `JWT_TOKEN_TTL_SECS` | optional | `300` | lifetime of game tokens, 60 to 3600 |
| `PUBLIC_STORAGE_URL` | optional | `https://pulsars.science/storage` | base of avatar and image URLs; the site relays it to `/v1/storage/` |
| `CORS_ALLOWED_ORIGINS` | optional | `https://pulsars.science` (plus `http://localhost:5173` in development) | origins that may ask for a game token, call `/v1/me` with credentials and open the game WebSocket; comma-separated, no `*` |
| `BIND_ADDR` | optional | `127.0.0.1:8080` | what nginx and `pulsars-web` call |
| `METRICS_ADDR` | optional | `127.0.0.1:9464` | loopback or private address only, another port than `BIND_ADDR` (enforced) |
| `DATABASE_MAX_CONNECTIONS` | optional | `10` | 1 to 200 |
| `RATE_LIMIT_ANONYMOUS_PER_MINUTE` | optional | `60` | per client IP (D6) |
| `RATE_LIMIT_AUTHENTICATED_PER_MINUTE` | optional | `120` | per game token `sub` |
| `REQUEST_TIMEOUT_SECS` | optional | `10` | 1 to 120; below nginx's `proxy_read_timeout` (30 s) |
| `BODY_LIMIT_BYTES` | optional | `65536` | JSON bodies, 1024 to 10485760; the uploads have their own limits (§6) |
| `ARGON2_MEMORY_KIB` | optional | `19456` | memory cost of password hashes (OWASP); lowered by the tests only |
| `LOG_FORMAT` | optional | `json` | or `pretty` |
| `RUST_LOG` | optional | `info,sqlx=warn,tower_http=info` | tracing filter |
| `SHUTDOWN_GRACE_SECS` | optional | `20` | 1 to 300; below the unit's `TimeoutStopSec` (30) |
| `DATABASE_MIGRATION_URL` | **required by `pulsars-api migrate`**, never read by the service | — | in `/etc/pulsars-api/migrate.env`: `postgres://pulsars_migrate:…@127.0.0.1:5432/pulsars`; secret |

Development (`PULSARS_ENV=development`) needs only `DATABASE_URL` and
`REDIS_URL`: without a signing key, the game token routes answer 503;
without `MESSAGES_KEY`, the private message routes answer 503.

Changing the configuration: `sudoedit /etc/pulsars-api/pulsars-api.env`
then `sudo systemctl restart pulsars-api`.

---

## 6. Upload size limits

JSON bodies are capped by `BODY_LIMIT_BYTES` (64 KiB). The uploads
(`multipart/form-data`, or a large JSON body for the exam import) have
their own limits, applied by `mw::body_limit` before anything is read:

| Route | File limit | Body limit |
|---|---|---|
| `PUT /v1/me/avatar` | 4 MiB image | 21 MiB |
| `POST /v1/suggestions` | 4 MiB image | 21 MiB |
| `POST /v1/resource-requests` | 20 MiB PDF | 21 MiB |
| `POST /v1/admin/exams` | 20 MiB PDF | 21 MiB |
| `POST /v1/admin/exams/{slug}/import` | — | 16 MiB (JSON) |

A file a little over its limit gets a `422` (`too_large`), a body over the
route's limit a `413`.

These uploads come from browsers through `pulsars.science` and the site's
relay (`/api/v1/**`). So, in the `pulsars.science` server block, before the
general `location /`:

```nginx
location ~ ^/api/v1/(me/avatar|suggestions|resource-requests|admin/exams(/[^/]+/import)?)$ {
    client_max_body_size 21m;
    client_body_timeout 60s;
    proxy_request_buffering on;   # the relay gets the whole body at once
    proxy_set_header X-Forwarded-For $remote_addr;
    proxy_pass http://127.0.0.1:3000;
}
```

and the relay itself must accept bodies of 21 MiB on those paths and pass
them on as they are (no JSON parsing), with `User-Agent` and
`X-Forwarded-For`. `api.pulsars.science` keeps its 256 KiB
(`client_max_body_size 256k`): no upload is offered there.

---

## 7. Upgrades

1. Tag a release on `main`: bump `version` in `Cargo.toml` (and the
   `CHANGELOG.md` section), merge, then `git tag -s vX.Y.Z && git push origin
   vX.Y.Z`. The Release workflow refuses a tag that does not match
   `Cargo.toml`.
2. *Operator's machine*: download and copy the archive (§4.4).
3. Add by hand any new variable the release needs (the CHANGELOG lists
   them): `deploy-api` never touches the environment files.
4. On the server:

   ```bash
   cd /tmp && sha256sum -c pulsars-api-vX.Y.Z-*.sha256 \
     && tar xzf pulsars-api-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz
   ./pulsars-api-vX.Y.Z-x86_64-unknown-linux-gnu/deploy-api
   ```

`deploy-api` options: `--no-system` (leave the unit and vhost alone),
`--no-check` (skip the final request through nginx). Environment overrides:
`HEALTH_TIMEOUT` (seconds, default 45), `KEEP_RELEASES` (default 5; `current`
and `previous` are always kept), `HEALTH_URL`, `PUBLIC_URL`.

What it does, in order, stopping at the first error:

1. checks that `/etc/pulsars-api/pulsars-api.env`, `migrate.env` and
   `/opt/pulsars-api/releases` exist;
2. runs the new binary with an empty environment: it must exit 78
   (configuration error), which proves it executes on this machine;
3. copies the release to `/opt/pulsars-api/releases/vX.Y.Z` (refuses if
   that version exists with a different binary);
4. installs the unit (and enables it, the first time) and the vhost if they
   changed (`nginx -t` first; the old vhost is put back if it fails);
5. applies the migrations with the new binary, as `pulsars_migrate`;
6. points `current` to the new release and `previous` to the old one,
   restarts;
7. waits for `/healthz` to answer 200 with the new version; otherwise puts
   the old release back, restarts and checks it;
8. removes old releases;
9. checks `https://api.pulsars.science/healthz` through nginx.

**Fallback without CI.** On any x86_64 Linux machine with Rust and glibc
≤ 2.39 (an Ubuntu 24.04 container will do), from a checkout of the tag:
`cargo build --release --locked --target x86_64-unknown-linux-gnu && deploy/package-release x86_64-unknown-linux-gnu dist`.
Not on the server itself.

---

## 8. Rollback

To the release that ran before the last deployment:

```bash
/opt/pulsars-api/current/deploy-api --rollback
```

It swaps `current` and `previous`, restarts and checks. Running it twice
returns to where you started. The schema stays as the newer release left
it: migrations are additive.

To any release still on disk (`ls /opt/pulsars-api/releases`), by hand:

```bash
sudo ln -sfn releases/v0.6.0 /opt/pulsars-api/.current.new
sudo mv -T /opt/pulsars-api/.current.new /opt/pulsars-api/current
sudo systemctl restart pulsars-api
curl -s http://127.0.0.1:8080/healthz
```

A rollback does not touch the unit, the vhost or the environment files. If
the release being left had changed one of them, restore the old one from
the release directory you go back to (`/opt/pulsars-api/current/`).

---

## 9. Logs

The API writes one JSON object per line to stdout; systemd sends it to the
journal under `pulsars-api`.

```bash
sudo journalctl -u pulsars-api -f -o cat | jq .                  # follow
sudo journalctl -u pulsars-api --since today -o cat \
  | jq -c 'select(.level == "ERROR" or .level == "WARN")'         # problems
sudo journalctl -u pulsars-api -o cat | grep <trace_id>           # one request
sudo journalctl -u pulsars-api -n 50 --no-pager                   # start-up errors
```

Every request line carries `request_id` and `trace_id` (also returned in
`X-Request-Id` and in 500 problem documents). A configuration error at
start-up is plain text listing every problem, and the unit then stays
`failed` with status 78 instead of restarting in a loop. Tokens, session
tokens, passwords, mail links and message contents are never logged.

nginx: `/var/log/nginx/pulsars-api.access.log` and
`/var/log/nginx/pulsars-api.error.log`, rotated by the system's logrotate.

The journal's size is the system default (10% of the file system, at most
4 GB). Cap it in `/etc/systemd/journald.conf` (`SystemMaxUse=`) if needed.

---

## 10. Metrics

Prometheus text format on `http://127.0.0.1:9464/metrics`, a separate
listener that nginx never proxies to. The API refuses to start if
`METRICS_ADDR` is a public address.

| Metric | Type | Labels |
|---|---|---|
| `http_requests_total` | counter | `method`, `route` (template), `status` |
| `http_request_duration_seconds` | histogram | `method`, `route` |
| `health_check_up` | gauge | `dependency` (`postgres`, `redis`) |
| `auth_rejections_total` | counter | `reason` (`expired`, `bad_signature`, …, `no_key`) |
| `rate_limited_total` | counter | `scope` |
| `rate_limit_errors_total` | counter | |
| `game_connections_total`, `game_auth_failures_total` | counter | |
| `game_sessions`, `game_queued_players`, `game_matches_live` | gauge | |
| `game_matches_started_total`, `game_matches_ended_total`, `game_persist_errors_total`, `game_seasons_closed_total` | counter | |

Nothing scrapes them yet (D10). A collector on the same machine (Grafana
Alloy or Prometheus) scrapes:

```yaml
scrape_configs:
  - job_name: pulsars-api
    scrape_interval: 15s
    static_configs:
      - targets: ["127.0.0.1:9464"]
```

A collector on another machine needs `METRICS_ADDR` on a private interface
(10.x, 172.16-31.x, 192.168.x or `fc00::/7`) and a firewall rule; never a
public address. Worth alerting on: `health_check_up == 0`, a rising
`rate_limit_errors_total` (Redis down), `game_persist_errors_total`, and
the 5xx share of `http_requests_total`.

---

## 11. The sandbox

The unit runs the binary with almost nothing: no capability, a read-only
file system, no device, only IP and Unix sockets, and a system call filter.
Each setting was checked against this binary (Tokio, rustls with *ring*,
sqlx, fred) under systemd 255:

| Setting | Why it is compatible |
|---|---|
| `User=pulsars-api`, `CapabilityBoundingSet=` (empty), `AmbientCapabilities=` | Port 8080 needs no capability. |
| `NoNewPrivileges=yes`, `RestrictSUIDSGID=yes` | The binary never executes anything else. |
| `ProtectSystem=strict`, `ProtectHome=yes`, `PrivateTmp=yes`, `UMask=0077` | The binary writes no file: logs go to stdout, files to the database. Reading its signing key, `/etc/resolv.conf`, `/etc/hosts` and the binary itself still works. TLS roots are compiled in (`webpki-roots`), no certificate store is read. |
| `PrivateDevices=yes`, `DevicePolicy=closed` | Randomness comes from the `getrandom` system call, not `/dev/urandom`. |
| `ProtectProc=invisible`, `ProcSubset=pid` | Tokio sizes its thread pool from `sched_getaffinity` and `/proc/self/cgroup`, which stay visible. |
| `RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX` | TCP for the listeners, PostgreSQL, Redis, SMTP, DeepL and Umami; `AF_UNIX` for name resolution through systemd-resolved. |
| `MemoryDenyWriteExecute=yes` | No JIT: *ring* is precompiled assembly, the regex engine of the log filter interprets. |
| `SystemCallFilter=@system-service` minus `@privileged @resources` | Tokio uses epoll, eventfd, timerfd, futex and clone3, all in `@system-service`; no io_uring. `getrlimit` (used by the standard library for stack guards) is `prlimit64`, which is not in `@resources`. |
| `ProtectKernel*`, `ProtectControlGroups`, `ProtectClock`, `ProtectHostname`, `RestrictNamespaces`, `RestrictRealtime`, `LockPersonality`, `RemoveIPC`, `SystemCallArchitectures=native` | Nothing in the binary uses these. |

`systemd-analyze security pulsars-api` rated the result 1.2 ("OK"; 0 is the
most locked down, 10 the least) under systemd 255. Run it again on the
server: Ubuntu 26.04 ships a newer systemd, which may know more settings.

Not set: `IPAddressAllow=`/`IPAddressDeny=`. SMTP and DeepL are reached at
public addresses that change, so an address allow-list would break
silently on the day they do.

### What was tested, and what was not

The unit, the vhost and `deploy-api` were tested on 2026-10-15 in an Ubuntu
24.04 container running systemd 255 as PID 1, with PostgreSQL 17, Redis 8
with the ACL of §4.6, and nginx 1.28: installation, upgrade, re-run,
`--rollback` twice, automatic rollback of a release failing its health
check, refusal of a different binary under an existing version, refusal of
a binary that does not run, pruning, invalid configuration (exit 78, no
restart loop), stop within the grace period, the security headers, the
redirect, `/metrics` → 404, a 300 KB body refused with 413, and the forged
`X-Forwarded-For` test of §2.

Not tested on a real server yet: this fresh-install path as a whole (the
database steps of `deploy/sql/README.md`, the key in `/etc/pulsars-api/keys`,
the first `deploy-api` run that enables the unit), certbot, x86_64 and
systemd 258+ (the server's).

---

## 12. Troubleshooting

| Symptom | Where to look |
|---|---|
| Unit `failed`, exit status 78 | `journalctl -u pulsars-api -n 20`: the list of invalid variables. A missing signing key or `MESSAGES_KEY` is one of them in production. |
| `JWT_SIGNING_KEY_FILE: … readable by its group or others` | `sudo chmod 600` the key file. |
| `JWT_SIGNING_KEY_FILE: …: Permission denied` | the service account cannot reach the key: `/etc/pulsars-api` must be `root:pulsars-api 0750`, `keys/` and the file owned by `pulsars-api` (§4.3). |
| `deploy-api` rolls back: "no healthy answer" | the last answer it prints. `postgres` not ok: the database, `DATABASE_URL`, `pg_hba.conf` (§4.5). |
| `deploy-api` stops at "Migrations" | `migrate.env` and the `pulsars_migrate` password; `roles.sql` must have run first. |
| The service logs "migrations not applied" | `pulsars-api migrate` has not run: `deploy-api` does it. |
| `start-limit-hit` | five starts within a minute. Fix the cause, then `sudo systemctl reset-failed pulsars-api`. |
| 502 from nginx | the service is down: `systemctl status pulsars-api`. |
| 504 from nginx | a request took more than 30 s: the API's own timeout (10 s) should answer first; look for a blocked PostgreSQL. |
| `redis` not ok in `/healthz` (`degraded`) | `systemctl status redis-server`; `redis-cli --user pulsars_api --askpass ping`. Content is still served, without rate limits; game sessions are refused. |
| Games get `503 token-unavailable` | no signing key: only possible in development. |
| Private messages answer 503 | no `MESSAGES_KEY`: only possible in development. |
| No account mail arrives | `account mail not sent: MAIL_SMTP_URL is not set` in the log, or `account mail failed` / `timed out`: the SMTP account, the sender's domain. |
| Every client shares one rate limit | nginx is not the one setting `X-Forwarded-For` (a CDN or another proxy was put in front): the vhost must then trust that proxy with `set_real_ip_from`. |
| `the binary does not run on this machine` | wrong architecture or a glibc older than 2.39 (`ldd --version`). |
| nginx refuses to reload | `sudo nginx -t` names the line. `deploy-api` restores the previous vhost by itself. |
| Certificate not issued | DNS: `api.pulsars.science` must point to this server only. |
