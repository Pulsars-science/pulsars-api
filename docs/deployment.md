# Deployment

How `pulsars-api` runs on the Pulsars VPS, next to the Laravel site.
**Status: prepared, not deployed.** Nothing in this document has been run on
the server yet; the files it installs were tested in an Ubuntu 24.04 virtual
machine with systemd and nginx 1.28 (see [Testing](#how-these-files-were-tested)).

The server is the one described in the Laravel repository's
`docs/MISE-EN-PROD.md`: Ubuntu 26.04, nginx 1.28 in front of every site, no
Docker, `94.23.107.101`. Production runs **nginx, not Caddy**: the Caddy edge
of `architecture.md` is not used, and nothing here needs it.

```
Internet → nginx (80/443) ─┬→ stockage.tmh-transports.com → PHP-FPM
                           ├→ pulsars.science      → Octane  127.0.0.1:8000
                           └→ api.pulsars.science  → pulsars-api 127.0.0.1:8080
                                                      │   │   └─ 127.0.0.1:9464 /metrics (never proxied)
                                            PostgreSQL   Redis
                                       (api_read views,  (rate limits;
                                        game schema)      matchmaking later)
```

## Contents

1. [Prerequisites](#1-prerequisites)
2. [Choices made](#2-choices-made)
3. [Open decisions](#3-open-decisions)
4. [First installation](#4-first-installation)
5. [Upgrades](#5-upgrades)
6. [Rollback](#6-rollback)
7. [Logs](#7-logs)
8. [Metrics](#8-metrics)
9. [The sandbox](#9-the-sandbox)
10. [Troubleshooting](#10-troubleshooting)

Files: [`deploy/`](../deploy/).

| File | Installed as |
|---|---|
| `pulsars-api.service` | `/etc/systemd/system/pulsars-api.service` |
| `api.pulsars.science.conf` | `/etc/nginx/sites-available/pulsars-api` |
| `pulsars-api.env.example` | `/etc/pulsars-api/pulsars-api.env` (filled in) |
| `deploy-api` | run from each release; a copy stays in every release directory |
| `package-release` | used by CI to build the release archive |

---

## 1. Prerequisites

The API cannot be deployed before these exist. `deploy-api` would install it,
then roll it back: `/healthz` answers 503 until PostgreSQL answers **and**
Laravel's JWKS has been loaded once.

| # | What | Laravel task | How to check |
|---|---|---|---|
| P1 | Production on **PostgreSQL**, with the roles `pulsars_site` and `pulsars_api` and the schemas `site`, `api_read`, `game` (administrator block at the end of `docs/contract/views.sql`) | A1, `postgres-migration.md` | `sudo -u postgres psql -d pulsars -c '\dn'` lists `site`, `api_read`, `game` |
| P2 | The **`api_read` views**, created by the Laravel migration from `views.sql` | A8 | as `pulsars_api`: `SELECT count(*) FROM api_read.courses;` returns the number of published courses |
| P3 | The **JWKS endpoint** `https://pulsars.science/.well-known/jwks.json` with at least one key | A2 (keys), A3 (endpoint), A9 (the nginx `location` that lets `/.well-known/jwks.json` through the site's `location ~ /\.` deny rule) | `curl -s https://pulsars.science/.well-known/jwks.json` returns `{"keys":[…]}`. **On 2026-10-15 it answers 403**: the deny rule is still in place (A9 not done). |
| P4 | A **DNS record** `api.pulsars.science` → the server | none (OVH zone) | `dig +short api.pulsars.science` gives `94.23.107.101` and nothing else |
| P5 | A **release** of this repository built by CI (a `vX.Y.Z` tag on `main`) | none | the GitHub release page lists `pulsars-api-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz` and its `.sha256` |

Also useful, not blocking: A4 (token endpoint) is what makes `/v1/me` usable
by the game front end. A5 and A6 (webhooks, XP) only matter from Phase 5.

The server itself must be **x86_64** with **glibc 2.39 or newer** (CI builds
on Ubuntu 24.04; Ubuntu 26.04 is newer, so this holds). Check once:
`uname -m` and `ldd --version | head -1`. `deploy-api` checks it too before
touching anything.

---

## 2. Choices made

**The binary is built by CI, not on the server.** A tag `vX.Y.Z` runs
`.github/workflows/release.yml`: `cargo build --release --locked` for
`x86_64-unknown-linux-gnu` on a pinned `ubuntu-24.04` runner, packaged with
`deploy/package-release` into one archive plus its SHA-256. Reasons:

- the VPS stays free of a Rust toolchain (over a gigabyte) and never spends
  minutes of CPU and memory on an LTO build next to the live site;
- the archive deployed is the archive that was built once, from a tag, with
  the locked dependencies: deploying it twice or rolling back gives the same
  bytes;
- a deployment takes seconds.

**The archive is copied to the server by the operator.** The repository is
private today: a server-side download would need a GitHub token stored on the
VPS. Instead, the operator's machine, already logged in to `gh`, downloads the
archive and copies it with `scp`. If the repository becomes public, the
server can fetch it directly with `curl -LO` and nothing else changes.

**The release carries its own deployment files.** The archive holds the
binary, `deploy-api`, the systemd unit, the nginx vhost and the example
environment file. The server needs no clone of the repository, and the vhost
and unit always match the binary they ship with.

**Layout on the server.**

| Path | Owner, mode | Content |
|---|---|---|
| `/opt/pulsars-api/releases/vX.Y.Z/` | `root:root`, `0755` | one directory per release, never modified after install |
| `/opt/pulsars-api/current` | symlink | the running release; `ExecStart` goes through it |
| `/opt/pulsars-api/previous` | symlink | the release `--rollback` returns to |
| `/etc/pulsars-api/pulsars-api.env` | `root:root`, `0600` | configuration and secrets |
| account `pulsars-api` | system user, no home, no shell | runs the service; owns nothing |

The Laravel site lives in `/var/www/pulsars` as `www-data` because it is a
web root with writable `storage/`. The API is a single binary that writes
nothing, so it follows the FHS (`/opt` for the program, `/etc` for its
configuration) and gets its own account: compromising it gives no access to
the site's files or its `.env`. The service account cannot even read its own
configuration: systemd reads `EnvironmentFile=` as root before dropping
privileges.

**Swap and rollback.** `deploy-api` renames a new symlink over `current`
(atomic: the link is never missing), restarts the unit, then waits up to 45 s
for `/healthz` to answer 200 **with the new version number**. Otherwise it
points `current` back to the previous release, restarts it and checks it.

**nginx** sets `X-Forwarded-For` to the address it sees, replacing whatever
the client sent. The API reads the **last** entry of that header as the client
IP (`client_ip` in `src/middleware.rs`) and keys the anonymous rate limit on
it: with one entry chosen by nginx, a client cannot pick its own bucket.
Checked through nginx: seven requests with seven different forged headers
all counted against the same key, and the sixth got `429`.

---

## 3. Open decisions

Each has a default already written into the files; change the files if the
answer differs.

| # | Question | Default in the files |
|---|---|---|
| D1 | How do release archives reach the server: `scp` from the operator's machine, a read-only GitHub token on the VPS, or a public repository? (A public repository also settles the AGPL obligation to offer the running source to network users, which a private repository does not.) | `scp` from the operator's machine |
| D2 | Paths and account name: `/opt/pulsars-api`, `/etc/pulsars-api`, user `pulsars-api`. | as listed |
| D3 | Certificate: a separate one for `api.pulsars.science`, or the site's certificate extended with a second name? | separate certificate, renewed by certbot like the site's |
| D4 | HSTS: one year, without `includeSubDomains` or `preload`. Raise it once HTTPS on every subdomain is certain? | `max-age=31536000` |
| D5 | PostgreSQL authentication: password over TCP on 127.0.0.1 (scram-sha-256), or a Unix socket with peer authentication (no password, but an `pg_ident.conf` map from `pulsars-api` to `pulsars_api`)? | password over TCP |
| D6 | Redis: `redis-server` from Ubuntu (or `valkey-server`, protocol-compatible, if that is what 26.04 ships); memory cap and eviction policy; persistence. Nothing else on the server uses Redis today. | `redis-server`, 128 MB, `noeviction`, default snapshots |
| D7 | Schools behind one public IP: 60 anonymous requests a minute per IP (the contract's figure) and nginx's 10 requests a second per IP are shared by a whole classroom. Raise them, or rely on game tokens (120/min per member)? | contract values |
| D8 | Resource limits: `MemoryMax=512M`, `LimitNOFILE=65536`, `TasksMax=256`. | as listed |
| D9 | Downtime on deploy: a plain restart drops new connections for about a second, and in-flight requests get up to 20 s to finish. Matches over WebSocket (Phase 4) would be cut. Zero-downtime (socket activation, or two instances behind the nginx upstream) is left for Phase 4. | plain restart |
| D10 | WebSocket idle timeout in nginx (300 s): the game server pings every 30 s and closes a connection silent for 90 s (`game-protocol.md` §3.1), well inside it. | 300 s (settled) |
| D11 | Migrations: run by the binary at start-up, or by `deploy-api` before the swap. Rolling back the binary leaves the schema as it is; migrations must stay additive. | **`deploy-api` before the swap**, as `pulsars_migrate` (`pulsars-api migrate`, `/etc/pulsars-api/migrate.env`). The service only waits for the schema. Was: at start-up, until the roles of `deploy/sql/phase-1.sql` took DDL away from `pulsars_api`. |
| D12 | Versioning: the first release is `v0.1.0` (phases 1 to 3 and this one), tagged on `main` after `develop` is merged. Who tags, and do tags require a green CI on `main`? | tag on `main` by the maintainer |
| D13 | IPv6: the vhost listens on `[::]` too. Does the server have a public IPv6 address worth an `AAAA` record? The site has none today. | no `AAAA` record |
| D14 | Monitoring: nothing scrapes `/metrics` yet (Grafana Alloy and Prometheus are "later" in `architecture.md`). Who gets alerted when `/healthz` fails? | nothing |

---

## 4. First installation

Commands run **on the server**, as the usual administrator account (with
`sudo`), unless marked *operator's machine*. Steps 1 to 6 change nothing the
running sites use.

### 4.1 Check the machine

```bash
uname -m                  # x86_64
ldd --version | head -1   # 2.39 or newer
systemctl --version | head -1
nginx -v
sudo -u postgres psql -d pulsars -c '\dn'      # P1: site, api_read, game
curl -s https://pulsars.science/.well-known/jwks.json   # P3: {"keys":[...]}
dig +short api.pulsars.science                # P4: 94.23.107.101
```

Stop here if one of P1 to P4 is missing (section 1).

### 4.2 DNS

In the OVH zone of `pulsars.science`, one `A` record:

```
api.pulsars.science   A   94.23.107.101
```

Only this one: a second address would send part of the traffic, and
certificate validation, elsewhere. Check from outside with
`dig +short api.pulsars.science`.

### 4.3 Account and directories

```bash
sudo useradd --system --user-group --no-create-home --home-dir /nonexistent \
    --shell /usr/sbin/nologin pulsars-api
sudo install -d -m 0755 -o root -g root /opt/pulsars-api /opt/pulsars-api/releases
sudo install -d -m 0750 -o root -g root /etc/pulsars-api
```

### 4.4 PostgreSQL

The role `pulsars_api` and the schemas were created during the PostgreSQL
migration (P1, the administrator block of `views.sql`). It still needs a
password and a rule in `pg_hba.conf`.

Generate the password (hex: no character needs escaping in a URL) and keep it
for step 4.7:

```bash
openssl rand -hex 32
```

Set it without leaving it in the shell history (`\password` sends it already
hashed):

```bash
sudo -u postgres psql -d pulsars
```

```sql
\password pulsars_api
-- Settings from views.sql; check they are there:
SELECT rolname, rolconnlimit, rolconfig FROM pg_roles WHERE rolname = 'pulsars_api';
-- expected: 40, {search_path=game, api_read, statement_timeout=5s,
--           idle_in_transaction_session_timeout=30s}
\q
```

In `/etc/postgresql/<version>/main/pg_hba.conf`, **above** the generic
`host all all …` lines:

```
# pulsars-api: local TCP only, password (scram).
host    pulsars    pulsars_api    127.0.0.1/32    scram-sha-256
host    pulsars    pulsars_api    ::1/128         scram-sha-256
```

```bash
sudo systemctl reload postgresql
```

Check, as the API will connect:

```bash
psql -h 127.0.0.1 -U pulsars_api -d pulsars -c 'SELECT count(*) FROM api_read.courses;'
# a number: the views are readable
psql -h 127.0.0.1 -U pulsars_api -d pulsars -c 'SELECT 1 FROM site.users LIMIT 1;'
# ERROR: permission denied for schema site   ← expected
```

PostgreSQL listens on localhost only (`listen_addresses`, Ubuntu's default):
keep it that way.

### 4.5 Redis

Nothing on the server uses Redis yet. Install it, listening on localhost
only, with a user that can only touch the API's keys (`pulsars-api:*`):

```bash
sudo apt install redis-server
openssl rand -hex 32        # the Redis password, for step 4.7
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
`rate_limit_errors_total` rises. Matchmaking queues live in the API's memory,
not in Redis; Redis records the single-use `jti` of game tokens, and a game
session is refused (`1013`) while Redis cannot record it.

```bash
sudo systemctl restart redis-server
sudo systemctl enable redis-server
redis-cli --user pulsars_api --askpass ping        # PONG
redis-cli ping                                     # NOAUTH or error: default user is off
```

### 4.6 The release archive

*Operator's machine*, logged in to `gh`:

```bash
V=v0.1.0
gh release download "$V" -R Pulsars-science/pulsars-api \
    -p "pulsars-api-$V-x86_64-unknown-linux-gnu.tar.gz*"
scp pulsars-api-$V-x86_64-unknown-linux-gnu.tar.gz* <admin>@94.23.107.101:/tmp/
```

On the server:

```bash
cd /tmp
V=v0.1.0
sha256sum -c "pulsars-api-$V-x86_64-unknown-linux-gnu.tar.gz.sha256"   # OK
tar xzf "pulsars-api-$V-x86_64-unknown-linux-gnu.tar.gz"
R="/tmp/pulsars-api-$V-x86_64-unknown-linux-gnu"
ls "$R"
```

### 4.7 The environment file

```bash
sudo install -m 0600 -o root -g root "$R/pulsars-api.env.example" /etc/pulsars-api/pulsars-api.env
sudoedit /etc/pulsars-api/pulsars-api.env
```

Replace the two `CHANGE_ME` (PostgreSQL step 4.4, Redis step 4.5). Every
variable the API reads is in the file:

| Variable | Production value | Notes |
|---|---|---|
| `PULSARS_ENV` | `production` | refuses `http://` JWKS, storage URL and CORS origins |
| `DATABASE_URL` | `postgres://pulsars_api:…@127.0.0.1:5432/pulsars` | secret |
| `DATABASE_MAX_CONNECTIONS` | `10` | role limit is 40 |
| `REDIS_URL` | `redis://pulsars_api:…@127.0.0.1:6379/0` | secret |
| `JWKS_URL` | `https://pulsars.science/.well-known/jwks.json` | P3 |
| `JWT_ISSUER` | `https://pulsars.science` | default |
| `JWT_AUDIENCE` | `api.pulsars.science` | default |
| `PUBLIC_STORAGE_URL` | `https://pulsars.science/storage` | default; avatar URLs |
| `CORS_ALLOWED_ORIGINS` | `https://pulsars.science` | default; origins for `/v1/me` and the game |
| `BIND_ADDR` | `127.0.0.1:8080` | what nginx proxies to |
| `METRICS_ADDR` | `127.0.0.1:9464` | loopback or private only, enforced |
| `RATE_LIMIT_ANONYMOUS_PER_MINUTE` | `60` | D7 |
| `RATE_LIMIT_AUTHENTICATED_PER_MINUTE` | `120` | D7 |
| `REQUEST_TIMEOUT_SECS` | `10` | below nginx's `proxy_read_timeout` (30 s) |
| `BODY_LIMIT_BYTES` | `65536` | below nginx's `client_max_body_size` (256k) |
| `LOG_FORMAT` | `json` | |
| `RUST_LOG` | `info,sqlx=warn,tower_http=info` | |
| `SHUTDOWN_GRACE_SECS` | `20` | below the unit's `TimeoutStopSec` (30) |

Check the file before the first start: run the binary for three seconds as
the service user, with the file loaded by systemd exactly as the unit will
(possible only while port 8080 is free, so before step 4.11):

```bash
sudo systemd-run --pipe --wait --quiet -p User=pulsars-api \
    -p EnvironmentFile=/etc/pulsars-api/pulsars-api.env \
    timeout 3 "$R/pulsars-api"; echo "exit $?"
```

Exit `124` (stopped by `timeout` after starting) means the configuration is
valid; `78` prints the list of problems.

### 4.8 Certificate

Before the vhost exists, certbot's nginx plugin answers the challenge with a
temporary server block of its own:

```bash
sudo certbot certonly --nginx -d api.pulsars.science
sudo ls /etc/letsencrypt/live/api.pulsars.science/   # fullchain.pem privkey.pem
sudo certbot renew --dry-run
```

`certonly`: certbot must not edit the vhost, which already holds the 443
block. Renewals reuse the nginx plugin and leave the file alone, as for the
site.

### 4.9 nginx

```bash
sudo cp "$R/api.pulsars.science.conf" /etc/nginx/sites-available/pulsars-api
sudo ln -s /etc/nginx/sites-available/pulsars-api /etc/nginx/sites-enabled/
sudo nginx -t
```

`syntax is ok` **and** `test is successful`, then:

```bash
sudo systemctl reload nginx
```

`reload`, not `restart`: connections to the other sites are not cut. Until
the API runs, `https://api.pulsars.science/` answers 502.

### 4.10 systemd

```bash
sudo cp "$R/pulsars-api.service" /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable pulsars-api
```

`enable` without `--now`: the first start is the deployment's.

### 4.11 First deployment

```bash
"$R/deploy-api"
```

It checks the binary, copies the release to `/opt/pulsars-api/releases/`,
finds the unit and vhost already in place, points `current` to it, starts the
service, waits for `/healthz`, then checks the public URL.

### 4.12 Verify

```bash
systemctl status pulsars-api --no-pager            # active (running)
curl -s http://127.0.0.1:8080/healthz | jq         # "status": "ok"
curl -sI https://api.pulsars.science/healthz       # HTTP/2 200, strict-transport-security
curl -s 'https://api.pulsars.science/v1/courses?limit=1' | jq '.data[0].slug'
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

From a browser: `https://api.pulsars.science/v1/catalog?lang=en`.

Clean up: `rm -rf /tmp/pulsars-api-v*`.

---

## 5. Upgrades

1. Tag a release on `main`: bump `version` in `Cargo.toml` (and the
   `CHANGELOG.md` section), merge, then `git tag -s vX.Y.Z && git push origin
   vX.Y.Z`. The Release workflow refuses a tag that does not match
   `Cargo.toml`.
2. *Operator's machine*: download and copy the archive (step 4.6).
3. On the server:

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

1. runs the new binary with an empty environment: it must exit 78
   (configuration error), which proves it executes on this machine;
2. copies the release to `/opt/pulsars-api/releases/vX.Y.Z` (refuses if
   that version exists with a different binary);
3. installs the unit and the vhost if they changed (`nginx -t` first; the
   old vhost is put back if it fails);
4. points `current` to the new release and `previous` to the old one,
   restarts;
5. waits for `/healthz` to answer 200 with the new version; otherwise puts
   the old release back, restarts and checks it;
6. removes old releases;
7. checks `https://api.pulsars.science/healthz` through nginx.

The environment file is never touched: a new variable is added by hand
before deploying the release that needs it (the CHANGELOG lists them).

Changing only the configuration: `sudoedit /etc/pulsars-api/pulsars-api.env`
then `sudo systemctl restart pulsars-api`.

**Fallback without CI.** On any x86_64 Linux machine with Rust and glibc
≤ 2.39 (an Ubuntu 24.04 container will do), from a checkout of the tag:
`cargo build --release --locked --target x86_64-unknown-linux-gnu && deploy/package-release x86_64-unknown-linux-gnu dist`.
Not on the VPS itself.

---

## 6. Rollback

To the release that ran before the last deployment:

```bash
/opt/pulsars-api/current/deploy-api --rollback
```

It swaps `current` and `previous`, restarts and checks. Running it twice
returns to where you started.

To any release still on disk (`ls /opt/pulsars-api/releases`), by hand:

```bash
sudo ln -sfn releases/v0.1.0 /opt/pulsars-api/.current.new
sudo mv -T /opt/pulsars-api/.current.new /opt/pulsars-api/current
sudo systemctl restart pulsars-api
curl -s http://127.0.0.1:8080/healthz
```

A rollback does not touch the unit, the vhost or the environment file. If the
release being left had changed one of them, restore the old one from the
release directory you go back to (`/opt/pulsars-api/current/`).

---

## 7. Logs

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
`failed` with status 78 instead of restarting in a loop.

nginx: `/var/log/nginx/pulsars-api.access.log` and
`/var/log/nginx/pulsars-api.error.log`, rotated by the system's logrotate
like the site's.

The journal's size is the system default (10% of the file system, at most
4 GB). Cap it in `/etc/systemd/journald.conf` (`SystemMaxUse=`) if needed.

---

## 8. Metrics

Prometheus text format on `http://127.0.0.1:9464/metrics`, a separate
listener that nginx never proxies to. The API refuses to start if
`METRICS_ADDR` is a public address.

| Metric | Type | Labels |
|---|---|---|
| `http_requests_total` | counter | `method`, `route` (template), `status` |
| `http_request_duration_seconds` | histogram | `method`, `route` |
| `health_check_up` | gauge | `dependency` (`postgres`, `redis`) |
| `auth_rejections_total` | counter | `reason` |
| `jwks_fetches_total` | counter | `result` |
| `jwks_keys` | gauge | |
| `rate_limited_total` | counter | `scope` |
| `rate_limit_errors_total` | counter | |

Nothing scrapes them yet (D14). A collector on the same machine (Grafana
Alloy or Prometheus, as planned in `architecture.md`) scrapes:

```yaml
scrape_configs:
  - job_name: pulsars-api
    scrape_interval: 15s
    static_configs:
      - targets: ["127.0.0.1:9464"]
```

A collector on another machine needs `METRICS_ADDR` on a private interface
(10.x, 172.16-31.x, 192.168.x or `fc00::/7`) and a firewall rule; never a
public address. Worth alerting on: `health_check_up == 0`,
`jwks_keys == 0`, a rising `rate_limit_errors_total` (Redis down), and the
5xx share of `http_requests_total`.

---

## 9. The sandbox

The unit runs the binary with almost nothing: no capability, a read-only
file system, no device, only IP and Unix sockets, and a system call filter.
Each setting was checked against this binary (Tokio, rustls with *ring*,
sqlx, fred) under systemd 255:

| Setting | Why it is compatible |
|---|---|
| `User=pulsars-api`, `CapabilityBoundingSet=` (empty), `AmbientCapabilities=` | Port 8080 needs no capability. |
| `NoNewPrivileges=yes`, `RestrictSUIDSGID=yes` | The binary never executes anything else. |
| `ProtectSystem=strict`, `ProtectHome=yes`, `PrivateTmp=yes`, `UMask=0077` | The binary writes no file: logs go to stdout. Reading `/etc/resolv.conf`, `/etc/hosts` and the binary itself still works. TLS roots are compiled in (`webpki-roots`), no certificate store is read. |
| `PrivateDevices=yes`, `DevicePolicy=closed` | Randomness comes from the `getrandom` system call, not `/dev/urandom`. |
| `ProtectProc=invisible`, `ProcSubset=pid` | Tokio sizes its thread pool from `sched_getaffinity` and `/proc/self/cgroup`, which stay visible. |
| `RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX` | TCP for the listeners, PostgreSQL, Redis and the JWKS; `AF_UNIX` for name resolution through systemd-resolved. |
| `MemoryDenyWriteExecute=yes` | No JIT: *ring* is precompiled assembly, the regex engine of the log filter interprets. |
| `SystemCallFilter=@system-service` minus `@privileged @resources` | Tokio uses epoll, eventfd, timerfd, futex and clone3, all in `@system-service`; no io_uring. `getrlimit` (used by the standard library for stack guards) is `prlimit64`, which is not in `@resources`. |
| `ProtectKernel*`, `ProtectControlGroups`, `ProtectClock`, `ProtectHostname`, `RestrictNamespaces`, `RestrictRealtime`, `LockPersonality`, `RemoveIPC`, `SystemCallArchitectures=native` | Nothing in the binary uses these. |

`systemd-analyze security pulsars-api` rates the result 1.2 ("OK"; 0 is the
most locked down, 10 the least). On the server, run it again after the first
deployment: Ubuntu 26.04 ships a newer systemd, which may know more
settings.

Not set: `IPAddressAllow=`/`IPAddressDeny=`. The JWKS is fetched from the
public address of `pulsars.science`, which can change, so an address
allow-list would break silently on the day it does.

### How these files were tested

On 2026-10-15, on the maintainer's machine (no server was touched):

- `nginx -t` with both this vhost and the site's `pulsars.science.conf`, on
  nginx 1.28.3 (the server's version), with placeholder certificates.
- An Ubuntu 24.04 container running systemd 255 as PID 1, with PostgreSQL 17
  (the demo database and real `api_read` views), Redis 8 with the ACL of
  step 4.5, and nginx 1.28 in the same network namespace. The release was an
  aarch64 build (the test machine's architecture) packaged by
  `package-release`; CI builds x86_64 the same way.
- Steps 4.3, 4.6, 4.7, 4.10 and 4.11 as written, then: upgrade, re-run,
  `--rollback` twice, automatic rollback of a release failing its health
  check, refusal of a different binary under an existing version, refusal of
  a binary that does not run, pruning, invalid configuration (exit 78, no
  restart loop), stop within the grace period.
- Under the full sandbox: PostgreSQL, Redis (ACL user), DNS, and an HTTPS
  request to `https://pulsars.science/.well-known/jwks.json` (TLS worked; the
  answer was 403, see P3).
- Through nginx: HTTP/2, the redirect from port 80, the security headers
  (each once), `/metrics` → 404, gzip passed through untouched, a 300 KB body
  refused with 413, and the forged `X-Forwarded-For` test of section 2.
- `shellcheck` on both scripts; `systemd-analyze verify` on the unit.

Not tested: certbot (needs the real domain), the WebSocket location (no
WebSocket route exists before Phase 4), x86_64 and systemd 258+ (the
server's), the Release workflow (runs on the first tag).

---

## 10. Troubleshooting

| Symptom | Where to look |
|---|---|
| `deploy-api` rolls back: "no healthy answer" | the last answer it prints. `"status":"down"` with `postgres` ok: the JWKS is not loaded (`journalctl -u pulsars-api \| grep JWKS`, then P3). `postgres` not ok: step 4.4. |
| Unit `failed`, exit status 78 | `journalctl -u pulsars-api -n 20`: the list of invalid variables. |
| `start-limit-hit` | five starts within a minute. Fix the cause, then `sudo systemctl reset-failed pulsars-api`. |
| 502 from nginx | the service is down: `systemctl status pulsars-api`. |
| 504 from nginx | a request took more than 30 s: the API's own timeout (10 s) should answer first; look for a blocked PostgreSQL. |
| `redis` not ok in `/healthz` (`degraded`) | `systemctl status redis-server`; `redis-cli --user pulsars_api --askpass ping`. Content is still served, without rate limits. |
| Every client shares one rate limit | nginx is not the one setting `X-Forwarded-For` (a CDN or another proxy was put in front): the vhost must then trust that proxy with `set_real_ip_from`. |
| `the binary does not run on this machine` | wrong architecture or a glibc older than 2.39 (`ldd --version`). |
| nginx refuses to reload | `sudo nginx -t` names the line. `deploy-api` restores the previous vhost by itself. |
| Certificate not issued | DNS (step 4.2): `api.pulsars.science` must point to this server only. |
