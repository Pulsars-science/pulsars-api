-- What the admin dashboard keeps day by day (`GET /v1/admin/stats`). Both
-- tables are in `site`, whose default privileges already give `pulsars_api`
-- its rows' rights.

-- The HTTP traffic of every API instance, by UTC day: requests, 5xx
-- responses, and a latency histogram whose bucket bounds are fixed in the
-- code (`src/http_daily.rs`, `BOUNDS_MS`, plus one overflow bucket). Each
-- instance adds its counts every minute (`ON CONFLICT ... DO UPDATE` with
-- sums), so instances and restarts add up.
CREATE TABLE site.http_daily (
    day date PRIMARY KEY,
    requests bigint NOT NULL CHECK (requests >= 0),
    errors_5xx bigint NOT NULL CHECK (errors_5xx >= 0),
    buckets bigint[] NOT NULL
);

-- Views of a course page (`POST /v1/courses/{slug}/view`) or of an exam page
-- (`GET /v1/exams/{slug}/corrections`) by UTC day, as their `vues` counters
-- count them. No foreign key: `content_id` is a course or an exam id, after
-- `kind`; a deleted content's rows are left out by the dashboard's joins.
CREATE TABLE site.content_views (
    kind text NOT NULL CHECK (kind IN ('course', 'exam')),
    content_id bigint NOT NULL,
    day date NOT NULL,
    views integer NOT NULL CHECK (views >= 0),
    PRIMARY KEY (kind, content_id, day)
);
-- The dashboard sums a period across every content.
CREATE INDEX content_views_day_index ON site.content_views (day);

-- Downloads by day, for the dashboard's series and catalog.
CREATE INDEX telechargements_created_at_index ON contenu.telechargements (created_at);
