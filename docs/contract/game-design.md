# Game design: decisions to make

**Status: decided on 2026-10-17**, except the defaults marked "to confirm" in Q4.

This document lists the questions the product owner must answer before
`pulsars-api` writes any game code (its Phase 4 is blocked while a question
below is open). Each question gives the options and their technical impact.
The "Decision" lines record the product owner's answers.

When a question is answered, write the decision and the date on its
"Decision" line, and remove the question from the "Open" list at the end.

## What already exists and could be reused

Facts from the Laravel code, to inform the choices:

| Asset | Volume | Auto-gradable | Notes |
|---|---|---|---|
| Exercises `qcm` | 536 | yes | 2 to 5 options, one or several correct. Option texts not translated. |
| Exercises `reponse_courte` | 327 | yes | Compared after normalising case, accents, spaces, final punctuation (`CorrecteurExercice`). |
| Exercises `reponse_libre` | 257 | **no** | Learner self-assesses. Unusable in a match. |
| Exam parts (published exams) | 98 | no | Long multi-question statements, graded by humans. |
| Flash question generators | 30 | yes | Computed from a seed, **not stored**: mental arithmetic, fractions, powers, derivatives, chemical symbols, binary… Collège to Lycée level. Already has a **"défi" mode**: 60 or 300 seconds, 4 choices per question, up to 240 questions. Written in PHP. |
| Courses | 272 | – | Maths 109, physics 83, crypto 33, computer science 19, chemistry 17, biology 11. |
| XP | – | – | Computed from activity (`Experience`). |
| Friendships, follows | – | – | `friendships`, `abonnements` tables: could serve "challenge a friend". |
| Guilds | **none** | – | Planned for v2 in `PROJET.md`, not built. |
| Code execution | **none** | – | Explicitly out of v1 in `PROJET.md`, planned v3. |

Counts are from a fresh local seed (1,120 exercises in total).

---

## Q1. What is a match?

| Option | Technical impact |
|---|---|
| **A. Timed quiz** (QCM and short answers) | No sandbox. Questions come from existing content or generators. Server grades instantly. Smallest scope: can ship first. |
| **B. Write code** (solve a problem, tests decide) | Needs a sandbox (Q7), a problem bank with hidden tests, per-language runtimes. Matches last minutes, not seconds. Weeks of extra work and a running cost. |
| **C. Both**, as separate modes | Build A first, B later on the same protocol. The protocol must allow mode-specific messages from day one. |
| **D. Mental-arithmetic race** using the flash generators | Like A, but questions are generated, unlimited and never repeat; the generators must be ported to Rust or exposed by Laravel (see Q6). |

Decision: **D, decided 2026-10-17.** A mental-arithmetic race on the flash generators. The protocol carries a `mode` field from day one so that other modes (quiz, code) can be added later without breaking clients.

## Q2. Which subjects (and, if code, which languages)?

- Quiz: all subjects, or start with maths only (most content, and the flash
  generators are mostly maths)?
- Code: which languages (Python only? Python + JavaScript + C?). Each language
  is a runtime to maintain in the sandbox.
- Is the subject chosen by the player before matchmaking (smaller queues), or
  random per match?

Impact: every subject or language chosen at queue time splits the player pool
(see Q3, waiting time).

Decision: **Decided 2026-10-17.** The player picks one subject or "all" before the match; no programming languages (no code mode). Each subject, and "all", is a separate ranked queue (see Q5).

## Q3. Format

| Option | Impact |
|---|---|
| **1 vs 1** | Simplest matchmaking and rating. Needs enough players online: with few players, waits get long. |
| **Free-for-all, 3–8 players** | Shorter waits with few players; rating is harder (multi-player Elo / TrueSkill). |
| **Teams** (2 vs 2, or guild vs guild) | Requires guilds (Q10) or ad-hoc teams; team rating; much more state. |
| **Solo against the clock**, scores compared asynchronously | No matchmaking, no real time; works with very few players; a "ghost" of another player's run can replace live opponents. |
| **Challenge a friend** (private match by invitation) | Uses the existing friendships; no waiting queue. |

Also: is there a bot/AI opponent when nobody is waiting?

Decision: **Decided 2026-10-17.** Four formats:

- **1 vs 1 online** with matchmaking: the only **ranked** format;
- **challenge a friend** (private match by invitation, existing friendships): unranked;
- **ghost**: play against the recorded run of another player, when nobody is waiting or for solo play: unranked;
- **free-for-all, 3–8 players**: unranked.

No bot opponent.

## Q4. Duration, victory, tie, forfeit, disconnection

For the chosen format:

- **Duration**: fixed time (60 s / 300 s like the flash "défi"), fixed number
  of questions, or first to N correct answers?
- **Scoring**: points per correct answer, speed bonus, penalty for a wrong
  answer (penalties stop random clicking on QCMs)?
- **Victory**: highest score; what breaks a tie (total answer time, the last
  correct answer, nothing → draw)?
- **Forfeit**: can a player leave? Is leaving a loss, and does the opponent win?
- **Disconnection**: how long a grace period to reconnect (15 s? 30 s?), does
  the clock keep running, after how long is it a forfeit?
- **Minimum**: a match where nobody answers anything: counted or void?

Impact: the state machine of a match in `docs/game-protocol.md`, the
`end_reason` and `outcome` values of `game.match_players` and `site.game_results`.

Decision: **Decided 2026-10-17: the rules are options chosen when starting a match** ("filters" of the game mode):

- duration: 60 s, 300 s, or first to 10 correct answers;
- wrong-answer penalty: on (+1 / −1) or off (+1 / 0).

Matchmaking only pairs players who chose the same options.

Defaults proposed by the developer, **to confirm**: a tie is broken by the lower total answer time, otherwise a draw; leaving is a forfeit (loss); after a disconnection the clock keeps running with a 20 s grace period, then forfeit; a match where nobody answered is void (no rating, no XP).

## Q5. Ranking

| Option | Impact |
|---|---|
| **No rating**, just wins and XP | Nothing to compute; no fair matchmaking by level. |
| **One global Elo/Glicko** | Simple; but a strong mathematician meets beginners in chemistry. |
| **One rating per subject** (or per language for code) | Fair; more tables; queues per subject. |
| **Seasons** (reset every N months) | More motivation, more code (archives, rewards). |

Also: provisional rating for new players? Public leaderboard on
`pulsars.science` (Laravel reads it from `/v1/game`)? Does a ranked match need
a verified account older than N days (against smurfs)?

Decision: **Decided 2026-10-17.** One Glicko-2 rating per subject (and one for "all"), for 1 vs 1 online only. Provisional rating for new players (Glicko's rating deviation). Public leaderboard on `pulsars.science`. No seasons for now.

## Q6. Where do the questions come from?

| Option | Impact |
|---|---|
| **Existing exercises** (`qcm`, `reponse_courte`) | 863 questions, already written and translated (except QCM options). But **they are public on the site and the solutions are visible after answering**, and the public site and the old `/api/v1` with the `catalogue:corrections` ability expose corrections: a player can look answers up during a match. |
| **Flash generators** | Unlimited, never the same question twice, so lookup is pointless. But they are PHP: port them to Rust (30 generators, seeded randomness must give the same series as PHP if shared links matter) or have Laravel generate and sign question sets for the API. |
| **A dedicated, private question bank** | Best against cheating; someone must write and review it; needs an admin UI (Laravel) and a new table, readable by the API through a new `api_read` view that **does** expose answers to the API role only. |
| **A mix** | Generators for speed modes, a private bank for ranked play. |

Also: how is a question's difficulty known (course difficulty, generator
level, or measured from answer rates)?

Impact: if answers must be available to the API, the API role needs a view
with answers — the only exception to "no answers in `api_read`". It must then
be a separate schema or view never used by the public endpoints.

Decision: **Decided 2026-10-17.** The flash generators, **ported to Rust** in the API: it generates and grades questions itself, nothing is sent in advance. The generators now live in TypeScript in `pulsars-game`, with parity fixtures produced by PHP; the Rust port must pass the same fixtures. Difficulty is the generator's level (collège, seconde, …). No exercise content and no answers are needed in `api_read`.

## Q7. Sandbox

Only if Q1 includes code.

- Judge0 or Piston, self-hosted on a separate VM without network (architecture
  rule: player code never runs on Pulsars' machine).
- Limits: time per run (2 s?), memory (128 MB?), output size, number of runs
  per match.
- Who pays for and operates the VM?
- What happens if the sandbox is down: code matches disabled, quiz still
  available?

Decision: **Not applicable (2026-10-17):** no code mode (Q1).

## Q8. Anti-cheat

The server already arbitrates (the client sends answers, never scores). What
else?

- Hide answers: which content may be used given that the site publishes
  solutions (Q6)?
- Send questions one at a time, only when they are asked (never the whole
  set in advance — today's flash "défi" sends all questions and answers to
  the browser).
- Detect impossibly fast correct answers (minimum human time per question)?
- Detect tab switching (the browser can report it; it proves little)?
- Multiple accounts: one match per account at a time; limit ranked play for
  new accounts?
- Collusion between friends (one loses on purpose): rate-limit matches
  between the same two accounts?
- Sanctions: who reviews, what happens (rating reset, ban from ranked)? There
  is no ban concept in Laravel today, only forum mute.

Decision: **Decided 2026-10-17.**

- The server sends one question at a time, when it is asked, never its answer (the 4 choices are sent, not which one is right).
- A minimum human time (about 300 ms) per correct answer: too many faster answers make the match unranked.

Not for now: limits between the same two accounts, restrictions on new accounts, tab-switch detection.

## Q9. Link with XP

XP is Laravel's (`Experience`). Today: lesson 10, exercise 8, course 40,
question 4, answer 4, accepted answer 12.

- Does a match give XP at all, or only a game rating?
- How much for a match played, won, drawn, lost, forfeited?
- Caps against farming: per day, per opponent, only ranked matches?
- Does match XP appear in the general ranking (`/classement`, home page top
  5), or in a separate game ranking?
- Does a correct answer to an existing exercise *during a match* count as
  "exercise solved" (8 XP, once per exercise), or not at all?

Impact: new `BAREME` keys in Laravel, the `game_results` table, the
`member_xp` view.

Decision: **Decided 2026-10-17.** A little XP, capped: about 3 XP per match played and 5 per win, at most 30 XP per day from the game. Accounts only (not guests). It counts in the general ranking.

## Q10. Link with guilds

Guilds do not exist.

- Is the game built without guilds first (recommended by the order of
  dependencies), with guild features later?
- If guilds come first: guild vs guild matches, guild ranking, guild XP?
  Then guilds must be built in Laravel before the game (v2 of `PROJET.md`),
  and exposed to the API through a new `api_read` view.

Decision: **Decided 2026-10-17.** The game is built without guilds; guild features come after guilds exist in Laravel.

## Q11. Who can play?

- Account required for every match, or a guest mode (no ranking, no XP)?
- Verified e-mail and completed onboarding required (the token endpoint
  assumes yes)?
- Minors: many members are collège/lycée students. Any chat between players?
  If yes: moderation, mute (the forum mute could apply), reporting.

Decision: **Decided 2026-10-17.** Guests and accounts. Guests play unranked and earn no XP; ranked 1 vs 1 needs an account. **No chat** between players. Impact: a guest token flow is needed besides `/v1/game/token` (see `auth-jwt.md`).

## Q12. Languages in the game

- The question's language follows the player's `locale` claim? What if two
  players in the same match have different locales: same question in two
  languages (only possible when translated), or one common language?
- QCM option texts are not translated today: acceptable in English matches?

Decision: **Decided 2026-10-17.** Each player sees the question in their own locale: a generator gives the same question in French and English from the same seed, with the same answers.

---

## Open

None. Q4's defaults for ties, forfeits and disconnections are to confirm.
