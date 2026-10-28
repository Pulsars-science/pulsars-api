-- Members, step B of PLAN-REFONTE.md: the API credits brains itself and
-- records match results directly, so the results webhook to Laravel goes.

-- The match end writes `site.game_results` and the brain movements in its
-- own transaction: no outbox, no event row to point at.
DROP TABLE game.webhook_outbox;
ALTER TABLE game.seasons DROP COLUMN event_id;
ALTER TABLE site.game_results DROP COLUMN game_event_id;

-- The salt of the visitors' pseudonymous fingerprints (SHA-256 of address,
-- browser and salt): download counts and guest report limits. Laravel used
-- its APP_KEY; this one is drawn once, here, and never leaves the database.
INSERT INTO site.reglages (cle, valeur, created_at, updated_at)
VALUES ('sel_visiteurs',
        to_json(md5(gen_random_uuid()::text) || md5(gen_random_uuid()::text)),
        now() AT TIME ZONE 'UTC', now() AT TIME ZONE 'UTC')
ON CONFLICT (cle) DO NOTHING;

-- Deleting a lesson cascades to its progress rows, found by lesson.
CREATE INDEX lesson_progress_lesson_id_index ON site.lesson_progress (lesson_id);
