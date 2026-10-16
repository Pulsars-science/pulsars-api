# Production migration: MySQL → PostgreSQL

**Status: plan only. Nothing here has been executed.**

The brief asked for a SQLite → PostgreSQL plan. Production does not run SQLite:
according to `docs/MISE-EN-PROD.md` and the systemd units in `deploy/`, it
runs **MySQL** on the VPS. This plan therefore covers MySQL → PostgreSQL. If
the server turns out to use SQLite after all, the same steps apply, with
`pgloader`'s SQLite source instead of MySQL.

Run `deploy/diagnostic-serveur.sh` first to confirm which engine is active.

## 1. Why

The target architecture needs one PostgreSQL database with separate schemas
(`site`, `game`, `api_read`) and per-role privileges, so the Rust API can read
published content through views without any access to Laravel's tables.
MySQL has neither views with owner privileges in the same form nor schemas
inside a database.

## 2. Strategy

Do **not** copy MySQL's schema. Let Laravel create the PostgreSQL schema with
its own migrations (`php artisan migrate` on an empty database), then copy
**data only**. That way the PostgreSQL schema is exactly what Laravel expects
(types, defaults, sequences), and data is the only thing that crosses.

Data copy tool: `pgloader` with `data only`, or a small artisan command that
streams each table with chunked inserts. `pgloader` is preferred: it is
mature, it handles `utf8mb4`, `tinyint(1)` → boolean and zero dates.

## 3. Code risks found in the repository

Things that work on MySQL and SQLite but behave differently on PostgreSQL:

| Location | Issue on PostgreSQL | Fix |
|---|---|---|
| `app/Http/Controllers/ExerciseController.php:65` | `selectRaw('exercise_id, MAX(correct) as reussi')`: `max(boolean)` does not exist in PostgreSQL → **SQL error** on the exercise page. | `bool_or(correct)` on pgsql, or `MAX(CASE WHEN correct THEN 1 ELSE 0 END)` everywhere. |
| Every `where(..., 'like', ...)` (course, exam, member, forum search; `Api/V1/CatalogueController`, `ExamenController`, `ProfilPublicController`) | `LIKE` is **case-sensitive** on PostgreSQL; MySQL's `utf8mb4_unicode_ci` is case- and accent-insensitive. Search for "algebre" would stop finding "Algèbre". | Use `ilike` on pgsql (a `whereLike()` helper, Laravel 11+ has `whereLike(..., caseSensitive: false)`), plus `unaccent` if accent-insensitivity is wanted. |
| `whereJsonContains('tags', $tag)` | Works: Laravel's PostgreSQL grammar casts to `jsonb` (`(tags)::jsonb @> ?`). | None. |
| `orderByRaw('epingle_le is null')` (`Question.php:109`) | Works. Booleans sort `false` before `true` on both. | None. |
| `selectRaw('date(created_at) …')` (`StatistiquesAdmin.php:195`) | Works. | None. |
| Migrations using `->change()` (`2026_07_21_100000`, `2026_08_07_100000`, …) | Run on an empty database: fine. | None. |
| Migration `2026_08_17_100000` | Raw SQL with single quotes: valid on PostgreSQL. | None. |
| Foreign keys | PostgreSQL does not index them automatically. `questions.user_id`, `answers.user_id` (used by XP) have no index. | Add indexes in a migration. |
| Sequences | After a data-only copy, every `id` sequence must be reset to `max(id)`, or the next insert fails on a duplicate key. | `pgloader` does it with `reset sequences`; otherwise `setval()` per table. |
| Case of `unique` on text columns | `users.pseudo`, `email`: MySQL compares case-insensitively, PostgreSQL does not. Two accounts `Jikuji` / `jikuji` could become possible, and login by e-mail becomes case-sensitive. | Check for collisions before copying (`GROUP BY lower(email) HAVING count(*) > 1`); lower-case e-mails at write time, or use `citext`. |
| `.env.example` lists `APP_LOCALE` twice (`fr` then `en`) | Unrelated to PostgreSQL, found on the way; the second wins. | Remove one. |

The test suite runs on SQLite (`phpunit.xml`). Before the switch, run it on
PostgreSQL too (CI service) to catch the rest.

## 4. Steps

### Preparation (no downtime)

1. Install PostgreSQL 17 on the VPS (or use the `compose.yaml` service with a
   volume and a local-only port).
2. Create the roles and schemas: the administrator block at the end of
   `views.sql` (`pulsars_site`, `pulsars_api`, schemas `site`, `api_read`,
   `game`).
3. Merge the code fixes of §3 and the `search_path` setting
   (`DB_SEARCH_PATH=site`).
4. **Rehearsal** on a copy:
   - `mysqldump --single-transaction pulsars > pulsars.sql` from production;
   - load it into a local MySQL;
   - `php artisan migrate --force` against an empty local PostgreSQL;
   - `pgloader` data only, MySQL → PostgreSQL `site`, excluding `migrations`,
     `sessions`, `cache`, `cache_locks`, `jobs`, `failed_jobs` (transient);
   - reset sequences;
   - run the checks of step 9 below;
   - time the whole thing.

   Repeat until it runs clean twice in a row.

### Cutover (maintenance window)

5. `php artisan down --render="errors::503" --retry=60` (as in `deploy-pulsars`).
6. Stop `pulsars-queue` (no job may write during the copy).
7. Fresh `mysqldump --single-transaction`, kept as the **rollback point**.
8. `pgloader` data only into the already migrated PostgreSQL; reset sequences.
9. Checks:
   - row counts equal for every table (script comparing `SELECT count(*)`);
   - `max(id)` equal, and `nextval` > `max(id)` for every sequence;
   - spot checks: one course with its chapters, lessons and translations; one
     exam with parts; the admin account can log in; one resource downloads.
10. Switch `.env`: `DB_CONNECTION=pgsql`, `DB_HOST`, `DB_PORT=5432`,
    `DB_DATABASE=pulsars`, `DB_USERNAME=pulsars_site`, `DB_PASSWORD`,
    `DB_SEARCH_PATH=site`. `php artisan config:cache`.
11. Update `deploy/*.service` (`After=postgresql.service`), restart
    `pulsars-octane`, `pulsars-ssr`, `pulsars-queue`.
12. `php artisan up`. Watch `/api/v1/statut` (database check) and the error log.
13. Sessions were not copied: every member is logged out once. Announce it.

### Afterwards

14. Keep MySQL **read-only** (and its dump) for two weeks, then remove it.
15. Replace the daily `mysqldump` in `MISE-EN-PROD.md` §backups with
    `pg_dump -Fc` (and test a restore).
16. Only then: run the `api_read` views migration (`views.sql`).

## 5. Downtime estimate

Database size today (local seed): ~10 MB of content, dominated by
`contenu_translations`, `lessons` and `exercises`. Production adds sessions
(not copied), visits and downloads.

| Step | Estimate |
|---|---|
| Dump | < 1 min |
| `pgloader` data copy | 1–3 min |
| Sequences and checks | 5 min |
| Config switch, restarts, smoke test | 5 min |
| **Total** | **≈ 15 minutes**, plan a 30-minute window |

The rehearsal gives the real figure; schedule the window at the lowest-traffic
hour (Umami shows it).

## 6. Rollback

Until step 12, rollback is: restore `.env` (MySQL), `config:cache`, restart,
`artisan up`. Nothing was written to MySQL in the meantime.

After step 12, writes go to PostgreSQL. Rolling back then loses those writes
unless they are copied back (`pgloader` PostgreSQL → MySQL, data only, new rows
only). Decide a cut-off: if a blocking problem appears within the first hour,
roll back and accept the loss of that hour's writes (accounts, progress,
forum posts), announced to members; after that, fix forward.

## 7. Commands (reference)

```bash
# Rehearsal: dump production
mysqldump --single-transaction --routines=false --triggers=false \
  -u pulsars -p pulsars > /tmp/pulsars-$(date +%F).sql

# Empty PostgreSQL schema built by Laravel
DB_CONNECTION=pgsql DB_DATABASE=pulsars DB_USERNAME=pulsars_site \
  DB_SEARCH_PATH=site php artisan migrate --force

# Data copy (pulsars.load)
cat > pulsars.load <<'EOF'
LOAD DATABASE
  FROM mysql://pulsars:***@127.0.0.1/pulsars
  INTO postgresql://pulsars_site:***@127.0.0.1/pulsars
WITH data only, truncate, reset sequences, workers = 4, concurrency = 1
SET search_path TO 'site'
CAST type tinyint when (= precision 1) to boolean using tinyint-to-boolean,
     type datetime to timestamp drop default using zero-dates-to-null,
     type timestamp to timestamp drop default using zero-dates-to-null
EXCLUDING TABLE NAMES MATCHING 'migrations', 'sessions', 'cache', 'cache_locks',
                               'jobs', 'job_batches', 'failed_jobs'
ALTER SCHEMA 'pulsars' RENAME TO 'site';
EOF
pgloader pulsars.load

# Row count comparison (one line per table)
for t in $(mysql -N -u pulsars -p pulsars -e 'show tables'); do
  m=$(mysql -N -u pulsars -p pulsars -e "select count(*) from \`$t\`")
  p=$(psql -At -U pulsars_site pulsars -c "select count(*) from site.\"$t\"")
  [ "$m" = "$p" ] || echo "DIFF $t mysql=$m pg=$p"
done
```
