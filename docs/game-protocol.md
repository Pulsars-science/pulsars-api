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

A generator with fewer than four distinct answers can give fewer choices; the
duel always needs four, so `duel_questions` pads by drawing again (documented
in the code) — in practice every generator reaches four.

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

Close codes: `4000` no auth in time, `4001` invalid token, `4003` `jti`
reused, `4029` rate limited, `4009` the account opened another session (the
old one is closed: one live session per account).

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
```

The outbox row is written in the same transaction as the match end
(`webhooks.md` §7) when at least one account played. Without
`LARAVEL_WEBHOOK_URL` the rows stay `pending`.

## 9. HTTP

| Endpoint | Auth | Body |
|---|---|---|
| `GET /v1/game/leaderboard?subject=maths&limit=50` | public | `{"subject", "data": [{rank, user_id, pseudo, avatar_url, rating, provisional, games, wins, losses, draws}]}` — players with at least one ranked match, best rating first. Pseudo and avatar from `api_read.members`; deleted accounts are skipped. |
| `GET /v1/game/me` | Bearer | `{"ratings": [{subject, rating, provisional, games, wins, losses, draws, rank}], "recent": [{match_id, mode, ranked, subject, rules, ended_at, outcome, score, rating_before, rating_after, opponents: [name]}]}` (last 20 matches) |

Both are JSON, `Cache-Control: no-store` for `/me`, `max-age=30` for the
leaderboard.
