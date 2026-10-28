# The database of a new server

The steps of `docs/deployment.md` §4.4 and §4.12, in detail. The server
starts empty: only the **content** is carried over from the old database
(courses, chapters, lessons, exercises, exams and their parts, resources,
news, files and translations). Accounts, the forum, the social part,
progress and games start from zero.

One command per line, on the server, as the administrator account (with
`sudo`). `$R` is the extracted release
(`/tmp/pulsars-api-vX.Y.Z-x86_64-unknown-linux-gnu`); this folder is in it,
at `$R/deploy/sql/`. The passwords are in `~/pulsars-secrets`
(`MDP_MIGRATE`, `MDP_API`, `MDP_ADMIN`), made with `openssl rand -hex 32`
(hex needs no escaping in a URL).

## 1. Database, roles and schemas (before the first deployment)

```
sudo -u postgres createdb pulsars
sudo -u postgres psql pulsars -v ON_ERROR_STOP=1 -v mdp_migrate="$(cat ~/pulsars-secrets/MDP_MIGRATE)" -v mdp_api="$(cat ~/pulsars-secrets/MDP_API)" < "$R/deploy/sql/roles.sql"
```

The script is read from standard input, so the `postgres` account needs no
access to the release directory. It creates `pulsars_owner` (no login,
owner of everything), `pulsars_migrate` (the migrations) and `pulsars_api`
(the service, which reads and writes rows and never changes the structure),
then the schemas `site`, `contenu`, `game` and `langues`. Running it again
changes nothing. Later, from an installed release:
`/opt/pulsars-api/current/deploy/sql/roles.sql`.

Then, in `/etc/postgresql/17/main/pg_hba.conf`, **above** the generic
`host all all …` lines, and `sudo systemctl reload postgresql`:

```
host    pulsars    pulsars_api,pulsars_migrate    127.0.0.1/32    scram-sha-256
host    pulsars    pulsars_api,pulsars_migrate    ::1/128         scram-sha-256
```

Check: `psql -h 127.0.0.1 -U pulsars_api -d pulsars -c 'SELECT 1'`.

## 2. The tables

`deploy-api` runs `pulsars-api migrate` before each switch, with
`DATABASE_MIGRATION_URL` (`/etc/pulsars-api/migrate.env`). The first
migration creates the whole schema, the second the reference data (roles,
forum themes, licences, subjects, official exam sources, Mandarin), the
others add to them.

## 3. The administration account (after the first deployment)

```
sudo systemd-run --quiet --wait --pipe --collect -p User=pulsars-api -p EnvironmentFile=/etc/pulsars-api/pulsars-api.env /opt/pulsars-api/current/pulsars-api create-admin jikuji ADDRESS < ~/pulsars-secrets/MDP_ADMIN
```

The account gets the `super-admin` role, its address verified and the
welcome steps done. The password is read on standard input (12 characters
at least), never from the command line. `systemd-run` loads the
environment file exactly as the service does (a shell `.` would misread
values such as `MAIL_FROM=Pulsars <…>`).

## 4. The content of the old database

The copy made before the reset (`pg_dump -Fc`) is restored into a separate
database, dropped afterwards:

```
sudo -u postgres createdb pulsars_ancien
sudo -u postgres pg_restore -d pulsars_ancien --no-owner --no-acl < ~/pulsars-secrets/pulsars-avant-reset.dump
sudo -u postgres psql pulsars_ancien -c "CREATE ROLE pulsars_lecture LOGIN PASSWORD 'lecture-temporaire'; GRANT USAGE ON SCHEMA site TO pulsars_lecture; GRANT SELECT ON ALL TABLES IN SCHEMA site TO pulsars_lecture;"
sudo systemd-run --quiet --wait --pipe --collect -p User=pulsars-api -p EnvironmentFile=/etc/pulsars-api/pulsars-api.env /opt/pulsars-api/current/pulsars-api import-content postgres://pulsars_lecture:lecture-temporaire@127.0.0.1/pulsars_ancien jikuji
sudo -u postgres dropdb pulsars_ancien
sudo -u postgres psql -c "DROP ROLE pulsars_lecture"
```

If the old copy keeps its content in a `contenu` schema, grant that schema
to `pulsars_lecture` the same way.

`import-content` copies the tables in order (courses, chapters, lessons,
exercises, exams, parts, resources, news, files, translations, downloads)
in one transaction, and gives every content to the named account. It
refuses a database that already has courses. Tried on the copy of
2026-10-18: 272 courses, 1,084 lessons, 1,120 exercises, 32 exams, 27 PDFs
and 6,260 translations, in 4 seconds.

Then `pulsars-api content check` (same `systemd-run` line) checks what was
copied, and `content translate` fills in missing translations with DeepL.
