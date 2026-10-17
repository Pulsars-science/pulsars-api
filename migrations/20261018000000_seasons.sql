-- Monthly duel seasons (docs/contract/brains.md §3). One row per closed
-- UTC month and subject key: the primary key is what makes a close happen
-- once, even across restarts and API instances.
CREATE TABLE IF NOT EXISTS game.seasons (
    -- 'YYYY-MM', a UTC calendar month.
    month       text NOT NULL CHECK (month ~ '^[0-9]{4}-(0[1-9]|1[0-2])$'),
    subject     text NOT NULL
                CHECK (subject IN ('maths', 'physics', 'chemistry', 'computer-science', 'all')),
    closed_at   timestamptz NOT NULL DEFAULT now(),
    -- [{"user_id": "2", "rank": 1, "rating": 1720, "ranked_matches": 7}],
    -- the `standings` of the `season.ended` event (empty: no event).
    standings   jsonb NOT NULL DEFAULT '[]',
    -- The `season.ended` outbox row, when one was written.
    event_id    uuid,
    PRIMARY KEY (month, subject)
);

-- The standings of a month: ranked matches of a subject by end time.
CREATE INDEX IF NOT EXISTS matches_ranked_ended
    ON game.matches (subject, ended_at)
    WHERE ranked AND end_reason IS NOT NULL;
