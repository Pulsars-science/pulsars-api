# What the API records about people, and for how long

What the site's privacy page says about the API, kept in step with the
code (`src/journal.rs`, `src/db/journal.rs`, `src/routes/telemetry.rs`).
Only admins read it, through the journal (`GET /v1/admin/people/*`,
`GET /v1/admin/activity`).

## Page views: only after consent

- The site shows a consent banner. Only when the visitor accepts the
  "mesure" cookies does it:
  - keep an anonymous visitor id (a random UUID v4) in the httpOnly
    cookie `pulsars_vid`, relayed to the API as `Pulsars-Visitor` on every
    request;
  - send `POST /v1/telemetry/page-view` for each page shown.
- The API records a page view only with a member's session or a visitor
  id; without either it writes nothing, whatever the site sends. Robots
  (by `User-Agent`) are ignored.
- What is kept: the time, the member and/or the visitor id, the page's
  path **without its query string or fragment**, and the referrer reduced
  to its origin (`https://www.qwant.com`), none when it is the site itself.
  No address, no browser.
- When a session opens (login, sign-up, Google, a session the site opens
  after a reset link) with a visitor id, that visitor's earlier anonymous
  page views become the member's.
- **Kept 13 months** (`site.page_views`).

## Proofs of consent

- Each answer to the banner (`POST /v1/consent`): accepted or refused,
  the banner's version, the time, the member and/or visitor id (both
  absent for a refusal before any), and the visitor fingerprint (SHA-256
  of address, browser and a secret salt), never the address itself.
- **Kept 25 months** (`site.consents`). An account's deletion keeps the
  proof without its member.

## Security events

Recorded for every account, without consent (legitimate interest: the
security of accounts), with the address and browser of the request:

| Event | When |
|---|---|
| `signup` | an account is created (form or Google, `detail.provider`) |
| `login` | a password login, or a session opened by the site (`detail.via = "site"`) |
| `login_failed` | a wrong password; on an unknown account, without member and with the identifier typed (`detail.identifier`) |
| `logout` | a session closed |
| `password_changed` | by the member (`detail.sessions_revoked`) |
| `password_reset_requested` | a reset link asked for an existing account (`detail.mail_sent`) |
| `password_reset` | through a reset link (`detail.via`: `link` or `site`) |
| `email_verified` | the address confirmed |
| `oauth_login`, `oauth_linked`, `oauth_unlinked` | Google or Discord (`detail.provider`) |

**Kept 12 months** (`site.security_events`). A failure to write one never
fails the request.

## Actions

What members do is already stored by the features themselves (lessons
completed, exercise attempts, enrollments, matches, forum posts, room
messages, brains, PDF downloads): the journal only reads it, as long as
those rows exist. A room message shows as its room's name, **never its
content**.

## Purge

The API deletes the rows past their retention once at start-up, then
every day.
