-- Admin statistics (GET /v1/game/admin/stats, docs/game-protocol.md §9).

-- Matches by start time: the daily and hourly counts, subjects, rules and
-- quality of a period all filter on started_at.
CREATE INDEX IF NOT EXISTS matches_started
    ON game.matches (started_at);

-- Dead letters are counted on every call; the outbox keeps every sent row.
CREATE INDEX IF NOT EXISTS webhook_outbox_dead
    ON game.webhook_outbox (created_at)
    WHERE status = 'dead';
