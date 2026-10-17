-- The game schema (docs/game-protocol.md §8). The schema itself is created
-- by the administrator and owned by pulsars_api (docs/contract/database.md);
-- this migration only creates tables in it.

-- One row per match, written when it starts and completed when it ends.
CREATE TABLE IF NOT EXISTS game.matches (
    id          uuid PRIMARY KEY,                -- UUIDv7
    mode        text NOT NULL CHECK (mode IN ('duel', 'room', 'ghost')),
    ranked      boolean NOT NULL,
    subject     text NOT NULL
                CHECK (subject IN ('maths', 'physics', 'chemistry', 'computer-science', 'all')),
    -- {"duration": 60 | 300 | null, "target": 10 | null, "penalty": bool}
    rules       jsonb NOT NULL,
    seed        integer NOT NULL CHECK (seed BETWEEN 1 AND 999999),
    created_at  timestamptz NOT NULL,
    -- When the questions start (after the countdown).
    started_at  timestamptz NOT NULL,
    ended_at    timestamptz,
    end_reason  text CHECK (end_reason IN ('completed', 'timeout', 'forfeit', 'void')),
    CHECK ((ended_at IS NULL) = (end_reason IS NULL))
);

-- Ghost lookup: finished, non-void matches of a subject and rules.
CREATE INDEX IF NOT EXISTS matches_ghost_key
    ON game.matches (subject, rules)
    WHERE end_reason IS NOT NULL AND end_reason <> 'void';

-- One row per seat, written when the match ends.
CREATE TABLE IF NOT EXISTS game.match_players (
    match_id        uuid NOT NULL REFERENCES game.matches (id) ON DELETE CASCADE,
    seat            smallint NOT NULL CHECK (seat BETWEEN 0 AND 7),
    -- Laravel users.id; null for guests and ghosts.
    user_id         bigint,
    guest_name      text,
    -- The match whose run this ghost seat replayed.
    ghost_of        uuid,
    -- Name at match time.
    name            text NOT NULL,
    score           integer NOT NULL,
    correct         integer NOT NULL CHECK (correct >= 0),
    answered        integer NOT NULL CHECK (answered >= correct),
    -- Total time over correct answers, the tie-break.
    answer_time_ms  bigint NOT NULL CHECK (answer_time_ms >= 0),
    rank            smallint NOT NULL CHECK (rank >= 1),
    outcome         text NOT NULL CHECK (outcome IN ('win', 'loss', 'draw', 'forfeit')),
    rating_before   integer,
    rating_after    integer,
    -- [{"i": index, "c": choice, "ok": bool, "t": ms_since_started_at}]
    answers         jsonb NOT NULL DEFAULT '[]',
    PRIMARY KEY (match_id, seat),
    CHECK (num_nonnulls(user_id, guest_name, ghost_of) = 1)
);

-- A player's recent matches (/v1/game/me) and ghost candidates.
CREATE INDEX IF NOT EXISTS match_players_user
    ON game.match_players (user_id, match_id)
    WHERE user_id IS NOT NULL;

-- Glicko-2, one rating per account and subject key ('all' included).
CREATE TABLE IF NOT EXISTS game.ratings (
    user_id     bigint NOT NULL,
    subject     text NOT NULL
                CHECK (subject IN ('maths', 'physics', 'chemistry', 'computer-science', 'all')),
    rating      double precision NOT NULL,
    rd          double precision NOT NULL CHECK (rd > 0),
    volatility  double precision NOT NULL CHECK (volatility > 0),
    games       integer NOT NULL DEFAULT 0,
    wins        integer NOT NULL DEFAULT 0,
    losses      integer NOT NULL DEFAULT 0,
    draws       integer NOT NULL DEFAULT 0,
    updated_at  timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, subject),
    CHECK (games = wins + losses + draws)
);

CREATE INDEX IF NOT EXISTS ratings_leaderboard
    ON game.ratings (subject, rating DESC)
    WHERE games > 0;

-- Events for Laravel (docs/contract/webhooks.md §7), written in the
-- transaction that ends the match.
CREATE TABLE IF NOT EXISTS game.webhook_outbox (
    event_id         uuid PRIMARY KEY,           -- UUIDv7
    type             text NOT NULL,
    body             jsonb NOT NULL,
    status           text NOT NULL DEFAULT 'pending'
                     CHECK (status IN ('pending', 'sent', 'dead')),
    attempts         integer NOT NULL DEFAULT 0,
    next_attempt_at  timestamptz NOT NULL DEFAULT now(),
    last_error       text,
    created_at       timestamptz NOT NULL DEFAULT now(),
    sent_at          timestamptz
);

CREATE INDEX IF NOT EXISTS webhook_outbox_due
    ON game.webhook_outbox (next_attempt_at)
    WHERE status = 'pending';
