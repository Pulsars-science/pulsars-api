# Webhooks: match results from the API to Laravel

When a match ends, the API (`pulsars-api`) tells Laravel. Laravel owns XP
and brains, so **Laravel decides what a result is worth**; the API only
reports what happened. Two event types exist: `match.completed` (§3) and
`season.ended`, the monthly duel standings (§3, defined in
[`brains.md`](brains.md) §3).

## 1. Endpoint

```
POST http://127.0.0.1:8000/internal/jeu/evenements
Host: pulsars.science
```

- **Private network only.** In the current hosting (one VPS, Octane listening
  on `127.0.0.1:8000`, see `docs/MISE-EN-PROD.md`), the API calls Octane
  directly on the loopback with `Host: pulsars.science`. If the API moves to
  another machine, it goes through a private network (WireGuard or the
  provider's VLAN), never through the public `https://pulsars.science`.
- **nginx must refuse the path publicly**: `location ^~ /internal/ { return 404; }`
  in `deploy/pulsars.science.conf`, before `location /`. Defence in depth: the
  signature already rejects anyone without the secret.
- The URL is configuration on the API side (`LARAVEL_WEBHOOK_URL`).
- The route sits outside the `web` group (no session, no CSRF, no cookies) and
  outside the `api` group (no `PublicApiHeaders`, no public throttle). See
  `laravel-tasks.md`.

## 2. Request

```http
POST /internal/jeu/evenements HTTP/1.1
Host: pulsars.science
Content-Type: application/json
User-Agent: pulsars-api/1.0.0
X-Pulsars-Event-Id: 01929a80-1b2c-7d3e-8f40-a1b2c3d4e5f6
X-Pulsars-Timestamp: 1792059631
X-Pulsars-Signature: v1=6d0f3c…(64 hex chars)
traceparent: 00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01

{ …body… }
```

### Signature

```
signed_payload = X-Pulsars-Timestamp + "." + raw_body
signature      = hex( HMAC-SHA256( WEBHOOK_SECRET, signed_payload ) )
header         = "v1=" + signature
```

- `raw_body` is the exact bytes sent. Laravel must verify against
  `$request->getContent()`, **before** any JSON decoding.
- The timestamp is **inside** the signed payload. Signing the body alone would
  let anyone who captured a request replay it with a fresh timestamp header.
  This is the only deviation from the original brief ("HMAC of the raw body"),
  and it is deliberate.
- Comparison with `hash_equals()` (constant time).
- The secret is at least 32 random bytes, shared through environment variables
  (`GAME_WEBHOOK_SECRET` in Laravel, `LARAVEL_WEBHOOK_SECRET` in the API).
- **Rotation**: Laravel accepts the header if it matches either
  `GAME_WEBHOOK_SECRET` or `GAME_WEBHOOK_SECRET_PREVIOUS`. Procedure: set the
  new secret as current and the old one as previous in Laravel, reload; switch
  the API to the new secret; after an hour, empty `..._PREVIOUS`.
- The header format `v1=` leaves room for a future scheme; Laravel ignores
  unknown versions and may receive several, comma-separated
  (`v1=abc,v1=def` during a rotation on the API side).

### Replay window

- Laravel rejects a request whose `X-Pulsars-Timestamp` is more than **300
  seconds** away from its clock, in either direction.
- The API computes a **new timestamp and signature for each attempt**, so a
  retry hours later is still accepted. The `event_id` stays the same, which is
  what makes retries safe (see idempotency).

## 3. Body

Common envelope:

```json
{
  "event_id": "01929a80-1b2c-7d3e-8f40-a1b2c3d4e5f6",
  "type": "match.completed",
  "version": 1,
  "occurred_at": "2026-10-13T10:20:31Z",
  "data": { }
}
```

| Field | Rule |
|---|---|
| `event_id` | UUIDv7, generated once when the event is written to the outbox. Unique forever. Also sent as `X-Pulsars-Event-Id` so it can be logged before parsing. |
| `type` | Event type. Laravel answers `422` to a type it does not know (so it is not silently dropped). |
| `version` | Version of `data` for this type. Starts at 1; incremented on breaking changes, during which the API sends the version Laravel declares it supports (`GAME_WEBHOOK_VERSION`). |
| `occurred_at` | When the match ended (`match.completed`) or the season was closed (`season.ended`), by the API's clock. |

### `match.completed`

Sent once per match that reached an end state (normal end, time out,
forfeit, including a disconnection past the grace period). Not sent for a
**void** match (nobody answered anything, `game-protocol.md` §4), nor for a
match without any account among its live players (guests only, or a guest
against a ghost).

```json
{
  "event_id": "01929a80-1b2c-7d3e-8f40-a1b2c3d4e5f6",
  "type": "match.completed",
  "version": 1,
  "occurred_at": "2026-10-13T10:20:31Z",
  "data": {
    "match_id": "01929a7e-9f00-7aa1-b2c3-d4e5f6a7b8c9",
    "mode": "flash-duel",
    "ranked": true,
    "started_at": "2026-10-13T10:15:02Z",
    "ended_at": "2026-10-13T10:20:31Z",
    "end_reason": "completed",
    "players": [
      {
        "user_id": "1",
        "team": null,
        "outcome": "win",
        "score": 12,
        "rank": 1,
        "rating_before": 1500,
        "rating_after": 1662
      },
      {
        "user_id": "3",
        "team": null,
        "outcome": "loss",
        "score": 9,
        "rank": 2,
        "rating_before": 1500,
        "rating_after": 1338
      }
    ]
  }
}
```

| Field | Meaning |
|---|---|
| `match_id` | UUIDv7 of the match in the `game` schema. |
| `mode` | `flash-duel` (1 vs 1 from matchmaking), `flash-room` (private room, 2–8 players), `flash-ghost` (against a recorded run). New modes get new keys. |
| `ranked` | Whether the match counted for the ranking: only `flash-duel` between two accounts, unless the anti-cheat rule made it unranked (`game-protocol.md` §4). |
| `started_at`, `ended_at` | ISO 8601 UTC. `started_at` is when the questions started (after the countdown). |
| `end_reason` | `completed` (time limit, target reached, or every question answered), `timeout` (target mode, nobody reached the target in 300 s), `forfeit` (a player left, or stayed disconnected past the 20 s grace period). `void` matches are never sent. |
| `players[].user_id` | Laravel user id as a string (the JWT `sub`). **Account players only**: guests and ghosts are not listed, so a room can list fewer players than it had. Never empty. |
| `players[].team` | Always `null`: no team mode (guilds do not exist, `game-design.md` Q10). |
| `players[].outcome` | `win` (rank 1 alone), `draw` (rank 1 shared), `loss`, `forfeit` (the player left; leavers rank last). |
| `players[].score` | Points of the flash race: +1 per right answer, −1 per wrong answer when the match has the penalty rule (else 0). Can be negative. |
| `players[].rank` | Final placement among all seats (guests and ghosts included), 1 = best; by score, then lower total answer time over right answers; equal ranks for a draw. |
| `players[].rating_*` | Glicko-2 rating of the match's subject before and after, rounded; `null` when `ranked` is false. |

The payload carries **raw results only**: no XP, no level, no reward.

No personal data travels: no pseudo (Laravel has it), no IP.

### `season.ended`

Sent once per UTC month and subject key (`maths`, `physics`, `chemistry`,
`computer-science`, `all`) whose standing is not empty, when the API closes
the month (`brains.md` §3): at its first check after the month ends (at
startup, then every 10 minutes).

```json
{
  "event_id": "01a3f1c0-0004-7b2e-9c1d-2f3e4a5b6c7d",
  "type": "season.ended",
  "version": 1,
  "occurred_at": "2026-11-01T00:00:04Z",
  "data": {
    "season": "2026-10",
    "subject": "maths",
    "standings": [
      {"user_id": "2", "rank": 1, "rating": 1720, "ranked_matches": 7},
      {"user_id": "1", "rank": 2, "rating": 1280, "ranked_matches": 5}
    ]
  }
}
```

| Field | Meaning |
|---|---|
| `season` | The closed month, `YYYY-MM`, UTC. |
| `subject` | The subject key of the rating. |
| `standings` | Never empty. Accounts with at least 5 ranked duels **ended** in the month in that subject, ordered by rating (desc), then more ranked duels in the month, then lower `user_id`; `rank` is 1..n without gaps (no shared ranks). |
| `standings[].rating` | Glicko-2 rating of the subject at the close, **before** the monthly decay, rounded. |
| `standings[].ranked_matches` | Ranked duels of the player that ended in the month in that subject. |

The API does not know which accounts were deleted: a `user_id` without an
account is in the standings, and Laravel skips it (rewards in
`brains.md` §3, key `saison:<season>:<subject>`). The `event_id` makes a
redelivery harmless (§4); the key `saison:<season>:<subject>` makes a second
event for the same month impossible to credit twice anyway.

## 4. Idempotency on the Laravel side

The API delivers **at least once**. Laravel must make sure an event delivered
twice grants XP once.

1. New table `game_events` (in `site`):

   | Column | Type | Note |
   |---|---|---|
   | `id` | bigint PK | |
   | `event_id` | uuid, **unique** | the API's `event_id` |
   | `type` | string | |
   | `payload` | json | the full body, for audit |
   | `received_at` | timestamp | |
   | `processed_at` | timestamp nullable | |

2. On receipt, after the signature check, in **one transaction**:
   - `INSERT` into `game_events`; on a unique violation, the event was already
     received → answer `200 {"status": "duplicate"}` without doing anything;
   - otherwise insert one `game_results` row per player (see §5) and set
     `processed_at`;
   - answer `200 {"status": "processed"}`.
3. `game_results` also has a unique `(match_id, user_id)`: a second line of
   defence if two different `event_id`s ever described the same match.

Because Laravel's XP is *computed from rows* (next section), the uniqueness of
those rows **is** the idempotency: there is no counter to increment twice.

## 5. How Laravel computes XP today

Source: `app/Services/Experience.php`. XP is **never stored**: it is recomputed
from activity tables each time it is displayed. Changing the scale is a
constant change, with no migration and no recomputation.

### Scale (`Experience::BAREME`)

| Key | Points | Counted from |
|---|---|---|
| `lecon` | 10 | `lesson_progress` rows with `completed_at` not null |
| `exercice` | 8 | `exercise_attempts` with `correct = true`, **distinct `exercise_id`** (an exercise pays once, however many attempts) |
| `cours` | 40 | `enrollments` with `completed_at` not null |
| `question` | 4 | `questions` authored by the member |
| `reponse` | 4 | `answers` authored by the member |
| `reponse_acceptee` | 12 | `answers` by the member that are some question's `accepted_answer_id` |

Note: hidden or soft-deleted forum posts (`masque_le`, `deleted_at`) still
count today, because `Experience` queries the tables directly without the
model scopes.

### Level

The cumulative XP needed to **start** level *n* is `50 × (n − 1) × n`:

| Level | 1 | 2 | 3 | 4 | 5 | 6 |
|---|---|---|---|---|---|---|
| Starts at | 0 | 100 | 300 | 600 | 1000 | 1500 |

`Experience::niveau()` loops until the next threshold exceeds the XP.
`Experience::pour()` returns `total`, `niveau`, `xp_dans_niveau`,
`xp_pour_niveau`, `xp_total_prochain` and the detail per key.
`Experience::classement()` aggregates every source grouped by user for the
ranking page (`/classement`) and the home page top 5.

### What match results add

To grant XP for matches, Laravel adds **one source** to `Experience`, reading
a new `game_results` table written by the webhook receiver:

| Column | Type |
|---|---|
| `id` | bigint PK |
| `game_event_id` | FK `game_events.id` |
| `match_id` | uuid |
| `user_id` | FK `users.id`, cascade on delete |
| `mode` | string |
| `ranked` | boolean |
| `outcome` | string (`win`, `loss`, `draw`, `forfeit`) |
| `ended_at` | timestamp |
| unique | `(match_id, user_id)` |

Implemented (2026-10-18) outside `BAREME`, because of the daily cap:
`Experience::PARTIE_JOUEE` (3 per match, whatever the outcome),
`PARTIE_GAGNEE` (5 more per win) and `PLAFOND_JEU_PAR_JOUR` (30), summed per
UTC day of `ended_at` (game-design.md Q9). Every mode counts; guests never
reach Laravel. The XP detail shows one line, `parties`.

`/v1/me` on the API side reproduces the same computation in the
`api_read.member_xp` view (`game_xp` in `views.sql`); Laravel's migration
builds it from the same constants.

## 6. Responses Laravel returns

| Status | Body | Meaning | API action |
|---|---|---|---|
| `200` | `{"status":"processed"}` | Stored and applied | Done. |
| `200` | `{"status":"duplicate"}` | Already received | Done. |
| `400` | problem+json | Body is not valid JSON or misses envelope fields | **No retry.** Dead letter + alert. |
| `401` | problem+json | Missing/invalid signature or timestamp outside the window | Retry with backoff (likely a secret rotation in progress or clock drift), **alert at the first one**. |
| `413` | | Body over 64 KiB | No retry. Dead letter + alert. |
| `422` | problem+json | Unknown `type`/`version`, unknown `user_id`, invalid `data` | **No retry.** Dead letter + alert. |
| `429` | | Throttled | Retry after `Retry-After`. |
| `5xx`, timeout, connection error | | Laravel down or failing | Retry with backoff. |

A `user_id` that no longer exists (account deleted during the match) is not
an error: Laravel stores the event, skips that player, and answers `200`.

Laravel answers within **2 seconds**: the work is a few inserts. If it ever
grows, Laravel stores the event, answers `200`, and processes it from its
queue; the contract does not change.

## 7. Delivery and retries on the API side

- **Outbox.** The API writes the event into `game.webhook_outbox` **in the
  same transaction** that marks the match finished (or, for `season.ended`,
  that closes the month in `game.seasons` and applies the decay). A worker
  sends pending rows. A crash between the two cannot lose a result.
- **Timeout** 10 seconds per attempt (connect 3 s).
- **Backoff**: exponential with full jitter, base 10 s, factor 3, ceiling 1 h:
  each delay is drawn uniformly below 10 s, 30 s, 1.5 min, 4.5 min,
  13.5 min, 40 min, then 1 h. A `429` waits its `Retry-After` (capped at
  1 h) instead.
- **Maximum**: 20 attempts or 24 hours, whichever comes first. Then the row
  becomes `dead`, an error is logged and the `webhook_dead_total` metric
  increases. A command (`pulsars-api webhooks replay <event_id>`) resends it:
  the row goes back to `pending` with its attempts and age reset. `400`,
  `413` and `422` answers make the row `dead` at once.
- **Order** is not guaranteed between matches. Laravel must not assume it
  (it does not need to: each event stands alone).
- **Concurrency**: one in-flight attempt per `event_id`
  (`SELECT … FOR UPDATE SKIP LOCKED`), so several API instances can run the
  worker.
- Metrics: `webhook_attempts_total{status}` (`sent`, `rejected`, `failed`),
  `webhook_pending`, `webhook_dead_total`, `webhook_latency_seconds`.
- **Configuration**: `LARAVEL_WEBHOOK_URL` and `LARAVEL_WEBHOOK_SECRET`
  (at least 32 characters) together, and optionally `LARAVEL_WEBHOOK_HOST`
  for the `Host` header (`pulsars.science` when the URL is
  `http://127.0.0.1:8000/…`). Without them the worker does not run and the
  rows stay `pending` until it does.

## 8. Known gaps

- Guilds do not exist in Laravel; `team` cannot refer to a guild today.
- XP is computed, not stored: "Laravel awards XP" really means "Laravel stores
  a result row that `Experience` counts".
- The `/classement` page and home page top 5 are built from
  `Experience::classement()`; adding a game source changes them too, as Q9
  decided (match XP counts in the general ranking).
