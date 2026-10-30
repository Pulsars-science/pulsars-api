-- The admin journal (`GET /v1/admin/people/*`, `/v1/admin/activity`): what a
-- person did on the site. Actions come from the existing tables; these add
-- the page views, the security events and the proofs of consent
-- (`docs/privacy.md`). Rows are purged daily past their retention
-- (`src/journal.rs`). In `site`: its default privileges give `pulsars_api`
-- the rows and the sequences.

-- Page views, recorded only after the visitor accepted the "mesure"
-- cookies: by a member, or by an anonymous visitor id (the site's
-- `pulsars_vid` cookie), linked to the member when they log in. 13 months.
CREATE TABLE site.page_views (
    id bigserial PRIMARY KEY,
    at timestamptz NOT NULL DEFAULT now(),
    user_id bigint REFERENCES site.users (id) ON DELETE CASCADE,
    visitor_id uuid,
    path text NOT NULL CHECK (length(path) <= 512),
    referrer text CHECK (length(referrer) <= 512),
    CHECK (user_id IS NOT NULL OR visitor_id IS NOT NULL)
);
CREATE INDEX page_views_user_index ON site.page_views (user_id, at DESC, id DESC);
CREATE INDEX page_views_visitor_index ON site.page_views (visitor_id, at DESC, id DESC);
CREATE INDEX page_views_at_index ON site.page_views (at);

-- Logins, failed logins, password and e-mail changes… with the address
-- and browser they came from. A failed login on an unknown account has no
-- user, and the identifier typed in `detail`. 12 months.
CREATE TABLE site.security_events (
    id bigserial PRIMARY KEY,
    at timestamptz NOT NULL DEFAULT now(),
    user_id bigint REFERENCES site.users (id) ON DELETE CASCADE,
    kind text NOT NULL CHECK (kind IN (
        'signup', 'login', 'login_failed', 'logout', 'password_changed',
        'password_reset_requested', 'password_reset', 'email_changed',
        'email_verified', 'oauth_login', 'oauth_linked', 'oauth_unlinked',
        'session_revoked', 'account_deleted')),
    ip text,
    user_agent text,
    detail jsonb NOT NULL DEFAULT '{}'
);
CREATE INDEX security_events_user_index ON site.security_events (user_id, at DESC, id DESC);
CREATE INDEX security_events_at_index ON site.security_events (at);

-- Proofs of the consent banner's answers: who (member and/or visitor id,
-- both null for a refusal without either), what (`measure`), which
-- version of the banner, and the visitor fingerprint, never the address.
-- Kept 25 months; an account's deletion keeps the proof, without its user.
CREATE TABLE site.consents (
    id bigserial PRIMARY KEY,
    at timestamptz NOT NULL DEFAULT now(),
    visitor_id uuid,
    user_id bigint REFERENCES site.users (id) ON DELETE SET NULL,
    measure boolean NOT NULL,
    version integer NOT NULL,
    ip_hash text
);
CREATE INDEX consents_user_index ON site.consents (user_id, at DESC, id DESC);
CREATE INDEX consents_visitor_index ON site.consents (visitor_id, at DESC, id DESC);
CREATE INDEX consents_at_index ON site.consents (at);
