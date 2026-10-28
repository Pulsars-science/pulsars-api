# Brains: the Pulsars currency

Decided with the product owner on 2026-10-17. "Cerveaux" in French,
"brains" in English. A future shop will spend them on charms and badges; this
document covers earning, the monthly duel season, and display.

## 1. Ownership

**The API owns the currency** (`src/db/brains.rs`, a port of Laravel's
`App\Services\Cerveaux`). Unlike XP, brains are spendable, so they are
**stored as a ledger**, never recomputed: the balance is the sum of the
member's movements. Each credit is written in the transaction of what earns
it: a lesson completed (`PUT /v1/me/courses/{slug}/lessons/{lesson}/completion`),
a match ended (`game::store::finish`, with the `site.game_results` rows),
a season closed (`game::season::close_month`). There is no webhook any
more.

### Table `site.cerveaux_mouvements`

| Column | Type | Note |
|---|---|---|
| `id` | bigint PK | |
| `user_id` | FK `users.id`, cascade on delete | |
| `montant` | int | positive = earned, negative = spent (shop, later) |
| `raison` | string(32) | `lecon`, `partie`, `victoire`, `saison`, later `achat`, `ajustement` |
| `cle` | string(120) | idempotency key, **unique per user** `(user_id, cle)` |
| `details` | json nullable | e.g. `{"lesson_id": 42}`, `{"match_id": "…"}`, `{"season": "2026-10", "subject": "maths", "rank": 3}` |
| `created_at` | timestamp | |

The balance is never negative: a spend checks it in the same transaction
(`SELECT … FOR UPDATE` on the user row). Not needed until the shop.

### Visibility

`users.cerveaux_publics` boolean, **default true**. When false, the balance
is shown only to its owner (header, dashboard, own profile); other members
and the API's public endpoints do not see it.

## 2. Earning

| Event | Brains | Key (`cle`) | Rule |
|---|---|---|---|
| Lesson completed | **5** | `lecon:<lesson_id>` | Once per lesson, at the moment `lesson_progress.completed_at` is set. |
| Match played (any mode, accounts only) | **2** | `partie:<match_id>` | At the match end, void matches excepted. |
| Match won | **+3** | `victoire:<match_id>` | `outcome = win`. |
| Daily cap on matches | **20 / UTC day** | | `partie` + `victoire` of the day of `ended_at`, summed; the credit is cut to what is left (a cut credit is stored with the reduced amount, a zero credit is not stored). |
| Monthly duel season | see §3 | `saison:<YYYY-MM>:<subject>` | At the season close. |

Lessons completed before this feature are credited once by a migration
(backfill, same keys), and so are matches already in `game_results`, with the
daily cap applied in `ended_at` order.

## 3. Monthly duel season

Months are UTC calendar months. At the start of a month the API closes the
previous one, **for each subject key** (`maths`, `physics`, `chemistry`,
`computer-science`, `all`), in **one transaction**:

1. **Standings**: the accounts with **at least 5 ranked duels ended in that
   month** in that subject, ordered by their current rating (desc), ties by
   more ranked duels in the month, then lower `user_id`. Rank 1..n.
2. **Rewards** in brains (table below), credited in `user_id` order.
3. **Decay** of every rating of that subject (`game.ratings`):
   `rating = 1500 + 0.8 × (rating − 1500)`, `rd = min(350, rd + 50)` (the
   rating becomes a bit provisional again). Counters `games`/`wins`/… stay
   (they are all-time).
4. Record the month as closed in `game.seasons (month text, subject text,
   closed_at, standings jsonb, PRIMARY KEY (month, subject))`, so a restart
   never closes a month twice.

The API checks at startup and then every 10 minutes whether the previous
month is closed. A month in which the API did not exist yet is closed too
(empty standings, decay applies).

Implemented in `pulsars-api` (`src/game/season.rs`, details in
`docs/game-protocol.md` §10). `game.seasons` also keeps the `event_id` of
the standings. Ranks are never shared.

### Rewards

| Rank in the subject | Brains |
|---|---|
| 1 | 300 |
| 2–3 | 200 |
| 4–10 | 100 |
| 11–50 | 40 |
| 51 and beyond | 10 |

Credited per subject (a player ranked in maths and in "all" gets both), key
`saison:<season>:<subject>`. A deleted account is skipped.

## 4. Reading the balance from the API

New view in `api_read` (created by Laravel's migration, copied in
`views.sql`):

```sql
CREATE OR REPLACE VIEW api_read.member_brains AS
SELECT u.id AS user_id,
       coalesce((SELECT sum(m.montant) FROM site.cerveaux_mouvements m
                  WHERE m.user_id = u.id), 0)::int AS balance,
       u.cerveaux_publics AS public
FROM site.users u;
```

- `GET /v1/game/me` adds `"brains": <balance>` (the token's owner: always
  visible to themselves).
- `GET /v1/me` adds `"brains"` the same way.
- The public leaderboard does **not** show balances.

## 5. Display

- **Icon**: a coin — a circle stroked 1.5px in the coin colour with
  Phosphor's `Brain` icon (duotone) centred inside. Coin colour token
  `--p-coin`: `#d9a21b` on dark, `#a8760a` on light (a warm gold, distinct
  from the blue accent). Component `Cerveau` in both apps, sizes 16/20/24.
- **Site header**: coin + balance next to the profile menu, for signed-in
  members. Link to the dashboard's history.
- **Dashboard**: balance and the last 20 movements (date, reason, amount).
- **Public profile**: the balance when `cerveaux_publics`, else nothing for
  others (the owner still sees it, with "visible only to you").
- **Profile settings**: a checkbox "Afficher mes cerveaux sur mon profil
  public" (checked by default).
- **Game header** (`pulsars-game`): coin + balance for signed-in players
  (from `/v1/game/me`), refreshed after each match; the duel end screen
  shows the gain as the difference once the balance has updated.
- **Lesson completion**: the existing "+10 XP" feedback also announces
  "+5 cerveaux" when they were credited.
