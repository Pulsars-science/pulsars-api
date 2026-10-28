-- Sessions of the site's members, opened and checked by the API (phase 2,
-- docs/contract/auth-sessions.md). A session is an opaque token; only its
-- SHA-256 is stored.
--
-- The API also reads the login columns of `site.users`, creates accounts and
-- changes passwords: column grants, so it never reads the two-factor
-- secrets or the remember token, and nothing else of `site`.

DO $$
BEGIN
    IF to_regnamespace('contenu') IS NULL THEN
        RAISE EXCEPTION 'schema contenu does not exist: run deploy/sql/phase-1.sql first';
    END IF;
END
$$;

CREATE TABLE site.auth_sessions (
    id uuid PRIMARY KEY,
    user_id bigint NOT NULL REFERENCES site.users (id) ON DELETE CASCADE,
    token_hash bytea NOT NULL UNIQUE,
    remember boolean NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    last_used_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL,
    revoked_at timestamptz,
    ip inet,
    user_agent text
);

CREATE INDEX auth_sessions_user_id_index ON site.auth_sessions (user_id);

GRANT USAGE ON SCHEMA site TO pulsars_api;
GRANT SELECT (id, pseudo, name, email, email_verified_at, password, locale,
              onboarding_at, created_at, updated_at)
    ON site.users TO pulsars_api;
GRANT INSERT (pseudo, name, email, password, locale, created_at, updated_at)
    ON site.users TO pulsars_api;
GRANT UPDATE (password, updated_at) ON site.users TO pulsars_api;
GRANT USAGE ON SEQUENCE site.users_id_seq TO pulsars_api;
GRANT SELECT, INSERT, UPDATE, DELETE ON site.auth_sessions TO pulsars_api;

-- The site still reads and writes everything during phase 2.
GRANT SELECT, INSERT, UPDATE, DELETE ON site.auth_sessions TO pulsars_site;
