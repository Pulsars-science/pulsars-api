# Game protocol: flash duels

The rules come from `docs/contract/game-design.md` (decided 2026-10-17). This
document fixes how they are built: the WebSocket messages, the match state
machine, the `game` schema, the HTTP endpoints and the link with the flash
generators. The front end is `pulsars-game` (`pulsars.science/jeux/duels`).

## 1. Vocabulary

| Term | Meaning |
|---|---|
| **subject** | `maths`, `physics`, `chemistry`, `computer-science`, or `all`. Selects the flash generators a match draws from. |
| **rules** | `{"duration": 60 \| 300 \| null, "target": 10 \| null, "penalty": bool}`. Exactly one of `duration` and `target` is set. `penalty`: a wrong answer is −1 (else 0). |
| **queue key** | `subject` + `rules`. Matchmaking only pairs players with the same key. |
| **mode** | `duel` (1 vs 1 matchmaking), `room` (private room, 2–8 players, by invitation code), `ghost` (against the recorded run of another player). Sent as `flash-duel`, `flash-room`, `flash-ghost` in the webhook. |
| **ranked** | Only `duel` between two accounts. Its rating is per subject (`all` has its own). |
| **seat** | A player's position in a match, 0-based. |

## 2. Questions

A match has a **seed** (`u32`, 1..=999999). From the seed, the subject and a
count, `flash::duel_questions` produces the list of questions every player of
the match answers, **in the same order, each at their own pace**. Each
question has four choices, in French and in English, with the right answer at
the same index in both languages. The server keeps the index; the client only
ever receives the text and the choices of the question it is on.

A match draws 240 questions (the flash "défi" cap). A player who reaches the
end is done; their score stays.

### Rust interface (`src/flash/`)

```rust
pub use hasard::Hasard;          // PHP Random\Randomizer(Mt19937) port

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Lang { Fr, En }
impl Lang { pub fn from_locale(locale: Option<&str>) -> Lang; pub fn code(self) -> &'static str; }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Subject { Maths, Physics, Chemistry, ComputerScience }
impl Subject { pub fn key(self) -> &'static str; pub fn from_key(key: &str) -> Option<Subject>; }

pub struct Card { pub question: String, pub answer: String }

pub trait Generator: Send + Sync {
    fn key(&self) -> &'static str;
    fn subject(&self) -> Subject;
    fn title(&self, lang: Lang) -> String;
    fn draw(&self, rng: &mut Hasard, lang: Lang) -> Card;
}

/// The 30 generators, in the PHP catalogue order.
pub fn catalogue() -> &'static [&'static dyn Generator];

pub struct Localized { pub question: String, pub series: String, pub choices: [String; 4] }
pub struct DuelQuestion { pub generator: &'static str, pub fr: Localized, pub en: Localized, pub correct: usize }
impl DuelQuestion { pub fn text(&self, lang: Lang) -> &Localized; }

/// `None` = every subject. Same algorithm as the "défi" (pick a generator
/// with `entier`, draw, decoys of the same shape first, shuffle), restricted
/// to the subject instead of maths only. `Some(Subject::Maths)` reproduces
/// the TypeScript `defi()` exactly, in both languages.
pub fn duel_questions(subject: Option<Subject>, seed: u32, count: usize) -> Vec<DuelQuestion>;
```

Rarely, the "défi" finds fewer than four distinct choices (1 question in
about 2,400 in the PHP fixtures, e.g. a limit whose only answers are `0`,
`+∞`, `−∞`). The duel always needs four: `duel_questions` then draws extra
decoys from a **clone** of the random stream and inserts them at random
positions, so the existing choices keep their order and every following
question stays identical to the "défi".

## 3. WebSocket

`GET /v1/game/ws`, upgraded. The `Origin` header must be in
`CORS_ALLOWED_ORIGINS`, otherwise `403` before the upgrade. Every frame is a
JSON text frame `{"type": "...", ...}`. Unknown types get an `error` and are
ignored. A client sends at most 20 frames per second (else close `4029`).

### 3.1 Authentication (first frame, within 5 s)

```json
{"type": "auth", "token": "eyJ…"}
{"type": "auth", "guest": true, "resume": "optional guest resume token"}
```

- **Account**: the token is checked as in `auth-jwt.md` §5, its `jti` is
  single-use (Redis). Name = `name` claim, language = `locale` claim.
- **Guest**: no token. The server names them `Invité 1234` / `Guest 1234`
  (4 random digits) and returns a `resume` token (random, 32 bytes base64url)
  valid while the guest has a live match. Guests never play ranked and earn
  no XP. The client sends its language with `{"type": "auth", "guest": true,
  "locale": "en"}`.

```json
{"type": "auth.ok", "player": {"user_id": "42", "name": "jikuji", "guest": false, "locale": "fr"}, "resume": null}
```

Close codes: `4000` no auth in time (or a first frame that is not `auth`),
`4001` invalid token (the close reason is the `reason` of `auth-jwt.md` §5),
`4003` `jti` reused, `4029` rate limited, `4009` the account opened another
session (the old one is closed: one live session per account). Standard
codes besides: `1008` the client does not read its frames (256 queued),
`1013` the keys or Redis are unavailable (try again later), `1001` server
stopping or idle connection (no frame for 90 s; the server pings every
30 s).

If the account (or the guest resume token) has a match in progress, the
server follows `auth.ok` with `match.start` (with `"resumed": true`), the
current scores and the player's current question.

### 3.2 Client → server

| Type | Fields | Effect |
|---|---|---|
| `queue.join` | `subject`, `rules` | Enter matchmaking. Accounts: ranked queue. Guests: casual queue, guests only. |
| `queue.leave` | | Leave matchmaking. |
| `ghost.start` | `subject`, `rules` | Start a match against a recorded run (§5). Leaves the queue if in it. |
| `room.create` | `subject`, `rules` | Create a private room; the sender is its host. |
| `room.join` | `code` | Join a room (max 8 players, before it starts). |
| `room.leave` | | Leave the room (the next player becomes host; empty room closes). |
| `room.start` | | Host only, at least 2 players. |
| `answer` | `match_id`, `index`, `choice` (0–3) | Answer the current question. Only the player's current `index` is accepted. |
| `match.leave` | `match_id` | Forfeit. |
| `ping` | | `pong`. |

### 3.3 Server → client

| Type | Fields |
|---|---|
| `error` | `code` (`bad_request`, `not_allowed`, `room_not_found`, `room_full`, `room_started`, `no_ghost`, `already_playing`), `message` |
| `queue.joined` | `subject`, `rules`, `ranked` |
| `queue.waiting` | `seconds` (every 5 s), `ghost_available` (true after 20 s if a ghost exists for the key) |
| `queue.left` | |
| `room.state` | `code`, `host_seat`, `players` `[{seat, name, user_id\|null, guest}]`, `subject`, `rules` (on every change) |
| `room.closed` | `reason` |
| `match.start` | `match_id`, `mode`, `ranked`, `subject`, `rules`, `seat` (yours), `players` `[{seat, name, user_id\|null, guest, ghost, rating\|null}]`, `starts_at` and `ends_at` (Unix ms; `ends_at` is the hard end, 300 s in target mode), `server_time` (Unix ms), `resumed` |
| `question` | `match_id`, `index`, `question`, `series`, `choices` (4 strings, the player's language). Sent at `starts_at` and right after each answer. |
| `answer.result` | `match_id`, `index`, `correct` (bool), `correct_choice`, `score` |
| `match.scores` | `match_id`, `scores` `[{seat, score, answered, correct, connected}]` (after every answer, to everyone) |
| `player.disconnected` | `match_id`, `seat`, `grace_until` (Unix ms) |
| `player.reconnected` | `match_id`, `seat` |
| `match.end` | `match_id`, `end_reason`, `ranked`, `results` `[{seat, name, user_id\|null, score, correct, answered, rank, outcome, rating_before\|null, rating_after\|null}]` |
| `pong` | `server_time` |

Strings meant for display (`question`, `series`, `choices`, guest names) are
plain text that may contain `$…$` maths, rendered by the front end like the
flash cards (KaTeX, no HTML).

## 4. A match

```
created ──(starts_at, 3 s countdown)──▶ running ──▶ finished
```

- **Start**: `starts_at` = now + 3 s. Questions are sent from `starts_at`.
- **Scoring**: correct +1; wrong −1 if `penalty`, else 0.
- **End** (`end_reason`):
  - `completed`: `duration` elapsed; or, in target mode, a player reaches
    `target` points (immediately); or every connected player answered all
    questions;
  - `timeout`: target mode, nobody reached the target within 300 s;
  - `forfeit`: a duel player left (`match.leave`, or disconnected past the
    grace period). In a room, a player who leaves gets `forfeit` and the match
    goes on while two players remain.
- **Disconnection**: the clock keeps running; the player has 20 s
  (`grace_until`) to reconnect, then forfeits.
- **Ranking**: by score, then by lower total answer time over correct answers
  (from the question being sent to the answer), else equal rank.
  `outcome`: `win` for rank 1 alone, `draw` when rank 1 is shared, `loss`
  otherwise, `forfeit` for leavers (who rank last).
- **Void match**: if nobody answered anything, the match is stored with
  `end_reason = "void"`, no rating change, no webhook.
- **Anti-cheat**: a correct answer under 300 ms after its question was sent is
  suspicious. More than 3 in a match → the match becomes unranked
  (`ranked: false`, no rating change; it still counts as played).

## 5. Ghosts

Every finished match stores each account player's answer log
`[{"i": index, "c": choice, "ok": bool, "t": ms_since_starts_at}]` with the
match seed, subject and rules.

`ghost.start` picks a recorded run of **another account**, same subject and
rules, not void, preferring a rating close to the player's (random among the
20 closest), and starts a match with the **same seed**: the player sees the
same questions the ghost saw. The server replays the ghost's answers at their
recorded times. If there is no run for the key: `error no_ghost`.

Ghost matches are unranked; the live player's run is recorded and can itself
become a ghost.

## 6. Matchmaking

In memory, in the API process (one instance in production). For each queue
key, the oldest waiting player is paired with the waiting player of closest
rating within a window of ±100, widened by 50 every 5 s. Guests are paired
among guests (casual, unranked). After 20 s, `queue.waiting` says whether a
ghost is available; the client offers it.

Moving matchmaking to Redis is needed only for several API instances.

## 7. Rating

Glicko-2 (Glickman), one rating period per match, `τ = 0.5`, new players at
rating 1500, RD 350, volatility 0.06. One rating per account and subject key
(`all` included). Shown rounded; a rating with RD > 110 is **provisional**.
Ranked forfeit: the leaver loses, the other wins.

**Monthly season** (`docs/contract/brains.md` §3): at the start of each UTC
month the API closes the previous one, per subject key, in one transaction:
standings of the accounts with at least 5 ranked duels ended in the month
(by rating, then ranked duels in the month, then lower `user_id`), a
`season.ended` webhook when they are not empty, then the decay of every
rating of the subject: `rating = 1500 + 0.8 × (rating − 1500)`,
`rd = min(350, rd + 50)`. `games`/`wins`/`losses`/`draws` are all-time and
stay. Laravel turns the standings into brains.

## 8. `game` schema

sqlx migrations in `migrations/`, run at startup (`sqlx::migrate!`).

```sql
game.matches (
  id uuid PK, mode text, ranked bool, subject text, rules jsonb, seed int,
  created_at timestamptz, started_at timestamptz, ended_at timestamptz null,
  end_reason text null
)
game.match_players (
  match_id uuid FK, seat smallint, user_id bigint null, guest_name text null,
  ghost_of uuid null,           -- (match_id of the replayed run)
  name text,                    -- name at match time
  score int, correct int, answered int, answer_time_ms bigint,
  rank smallint, outcome text, rating_before int null, rating_after int null,
  answers jsonb,                -- the log of §5
  PRIMARY KEY (match_id, seat)
)
game.ratings (
  user_id bigint, subject text, rating double precision, rd double precision,
  volatility double precision, games int, wins int, losses int, draws int,
  updated_at timestamptz, PRIMARY KEY (user_id, subject)
)
game.webhook_outbox (
  event_id uuid PK, type text, body jsonb, status text,  -- pending | sent | dead
  attempts int, next_attempt_at timestamptz, last_error text null,
  created_at timestamptz, sent_at timestamptz null
)
game.seasons (                  -- one row per closed month and subject key
  month text, subject text, closed_at timestamptz,
  standings jsonb,              -- the `standings` of `season.ended`, [] if none
  event_id uuid null,           -- the outbox row, when one was written
  PRIMARY KEY (month, subject)
)
```

The outbox row is written in the same transaction as the match end
(`webhooks.md` §7) when at least one account played. Without
`LARAVEL_WEBHOOK_URL` the rows stay `pending`.

## 9. HTTP

| Endpoint | Auth | Body |
|---|---|---|
| `GET /v1/game/leaderboard?subject=maths&limit=50` | public | `{"subject", "data": [{rank, user_id, pseudo, avatar_url, rating, provisional, games, wins, losses, draws}]}` — players with at least one ranked match, best rating first. Pseudo and avatar from `api_read.members`; deleted accounts are skipped. |
| `GET /v1/game/me` | Bearer | `{"brains": <balance>, "ratings": [{subject, rating, provisional, games, wins, losses, draws, rank}], "recent": [{match_id, mode, ranked, subject, rules, ended_at, outcome, score, rating_before, rating_after, opponents: [name]}]}` (last 20 matches) |
| `GET /v1/game/admin/stats?days=30` | Bearer, `roles` with `admin` or `super-admin` | Live counters and the activity of the last `days` UTC days (below). |

All are JSON, `Cache-Control: no-store` for `/me` and the statistics,
`max-age=30` for the leaderboard.

### 9.1 Admin statistics

For the Laravel admin dashboard, which calls it server to server (no
`Origin`) and shows the result. `days` is 1–90 (default 30). A token
without the `admin` or `super-admin` role gets `403` with
`reason: "insufficient_scope"` (and `WWW-Authenticate: Bearer …,
error="insufficient_scope"`); no or a bad token, `401` as everywhere.
Restricted CORS like `/v1/game/me`. All times are UTC; missing days and
hours are zeros.

```json
{
  "generated_at": "2026-10-17T13:40:00Z",
  "live": {"sessions": 12, "guests": 4, "queued": 2, "rooms_open": 1,
           "matches": {"duel": 3, "room": 1, "ghost": 2}, "players_in_matches": 9},
  "today": {"matches_started": 41, "by_hour": [0, 0, …24 integers], "accounts": 17, "guests": 6},
  "daily": [{"date": "2026-09-18", "duel": 0, "room": 0, "ghost": 0, "ranked": 0,
             "void": 0, "forfeit": 0, "accounts": 0, "guests": 0}, …],
  "subjects": [{"subject": "maths", "matches": 120}, …],
  "rules": [{"rules": {"duration": 60, "target": null, "penalty": false}, "matches": 80}, …],
  "quality": {"avg_match_seconds": 64.2, "avg_answers_per_player": 23.1, "accuracy": 0.71},
  "ratings": {"players": 25, "provisional": 18, "by_subject": [{"subject": "maths", "players": 20}, …]},
  "webhook": {"pending": 0, "dead": 0},
  "seasons": [{"month": "2026-09", "subject": "maths", "ranked_players": 0, "closed_at": "2026-10-01T00:00:12Z"}]
}
```

| Field | Meaning |
|---|---|
| `live` | From the hub's memory, now. `sessions`: open authenticated WebSockets, `guests` of them; `queued`: players waiting in matchmaking (every key); `rooms_open`: private rooms in their lobby (not started, or between matches); `matches`: matches running, by mode; `players_in_matches`: live players still seated (ghosts and forfeiters excluded). |
| `today` | The current UTC day: `matches_started` (= the sum of `by_hour`, = `duel + room + ghost` of the last `daily` entry), `accounts`: distinct accounts with a seat in a match started today, `guests`: guest seats. |
| `daily` | `days` entries, oldest first, the last one today. Matches by `started_at` day: per mode, `ranked` (as stored: a match made unranked by the anti-cheat or void is not counted), `void` and `forfeit` (end reasons), `accounts` (distinct) and `guests` (seats). |
| `subjects` | Matches of the period per subject key, all five listed, most played first (ties in the order `maths`, `physics`, `chemistry`, `computer-science`, `all`). |
| `rules` | Matches of the period per rules, most played first (ties by their JSON text). |
| `quality` | Finished non-void matches of the period: mean `ended_at − started_at` in seconds (one decimal), mean answers per live seat (one decimal, ghost seats excluded), `accuracy` = correct / answered over those seats (three decimals). Each is `null` without data. |
| `ratings` | All time: `players` with at least one ranked match in some subject, `provisional` = those whose every rating has RD > 110, `by_subject`: players per subject key (all five, most first). |
| `webhook` | Outbox rows `pending` and `dead` now. |
| `seasons` | The last 12 `game.seasons` rows, newest month first (subjects in key order); `ranked_players` = size of the standings. |

**Counting started matches.** The `game.matches` row is written as a
match is created (the runner inserts it a few milliseconds after
`match.start`, and the end updates it), so running matches already count
in `today` and `daily`; a start row that could not be written (database
down) appears at the end. Seats reach `game.match_players` only at the
end: today's `accounts` and `guests` add the seats of the live matches
started today, from the hub's snapshot. The snapshot is taken before the
queries, which run in one repeatable-read, read-only transaction: a match
that ends in between is counted once (distinct account ids; the database
seats of live matches are left out of the guest count). Indexes:
`matches_started` on `game.matches (started_at)` and the partial
`webhook_outbox_dead` (migration `20261019000000_admin_stats.sql`).

## 10. Implementation notes

Built on 2026-10-17 (`src/game/`). Choices the text above leaves open, and
the few additions:

- **Frames** are at most 4 KiB. `error` codes for `answer`: `bad_request`
  (not the current question, or another match), `not_allowed` (not started,
  over, or the player left).
- **Queue**: `queue.join` for the key one is already waiting in keeps the
  place; while in a match or a room it is `already_playing`. `ghost.start`,
  `room.create` and `room.join` leave the queue and send `queue.left`.
- **Rooms**: codes are 6 characters (`A–Z`, `2–9`, without `0 O 1 I L`),
  matched case-insensitively. The host is always seat 0 (`host_seat` is
  always 0). `room.leave` answers `room.closed {"reason": "left"}`; leaving
  (or `match.leave`) during the room's match is a forfeit. After the match
  the room goes back to its lobby with the players still connected (new
  `room.state`), ready for another start.
- **Anti-cheat** is counted per player: more than 3 suspicious answers from
  one player make the match unranked.
- **Void** looks at live players only: a ghost's answers do not make a match
  count. A void match takes precedence over a forfeit. Its results still
  carry ranks and outcomes (`forfeit` for leavers).
- **`match.end`** is also sent to a player who left, as the last frame of
  that match.
- **Ghosts**: the ghost seat has `user_id: null` (also in
  `game.match_players`, where `ghost_of` holds the source match). The
  ghost's recorded `ok` is replayed as is. "Every connected player answered
  all questions" also waits for the ghost to finish its run.
- **Ratings** are shown in `match.start` only in ranked matches. Glicko-2
  runs one period per match; there is no deviation growth for inactivity
  between matches, and RD is capped at 350.
- **Guests**: a `resume` token is honoured only while that guest has a
  live match; otherwise the guest gets a new identity. An account token
  without a `name` claim is named `#<id>`.
- **`/v1/game/me`** `recent` entries carry `end_reason` too. The
  leaderboard's `subject` defaults to `all` and `limit` to 50 (1–100);
  equal rounded ratings share a rank.
- **Shutdown**: matches in progress when the API stops are lost (neither
  stored nor sent to Laravel).
- **Season close** (`src/game/season.rs`, added with the brains): checked
  once the migrations are applied at startup, then every 10 minutes (1
  minute after a database error). Only the month before the current one is
  checked: if the API is down from before the end of a month until after
  the end of the next one, the earlier month is never closed. A transaction-scoped advisory lock on
  `(month, subject)` plus the `game.seasons` primary key make the close
  happen once across restarts and instances. The rating rows of the subject
  are locked in `user_id` order before the decay, the order a match end
  uses, so the two cannot deadlock; a match ending during the close waits
  and then applies its result to the decayed rating. Standings order by the
  exact rating, not the rounded one. `/v1/game/me` reads `brains` from
  `api_read.member_brains` (0 when the account has no row). Metric
  `game_seasons_closed_total{subject}`.
