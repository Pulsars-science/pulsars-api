-- Links sent by mail (phase 2, docs/contract/auth-sessions.md): the address
-- verification link and the password reset link. Only the SHA-256 of a
-- link's token is stored; a token works once, before `expires_at`, for the
-- address it was sent to.
--
-- The API also marks addresses verified, and resolves Google accounts:
-- it reads and writes `site.comptes_sociaux`, and may create an account
-- whose address the provider already verified.

DO $$
BEGIN
    IF to_regnamespace('contenu') IS NULL THEN
        RAISE EXCEPTION 'schema contenu does not exist: run deploy/sql/phase-1.sql first';
    END IF;
END
$$;

CREATE TABLE site.email_tokens (
    id uuid PRIMARY KEY,
    user_id bigint NOT NULL REFERENCES site.users (id) ON DELETE CASCADE,
    kind text NOT NULL CHECK (kind IN ('verify', 'reset')),
    token_hash bytea NOT NULL UNIQUE,
    email text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL,
    used_at timestamptz
);

CREATE INDEX email_tokens_user_id_kind_index ON site.email_tokens (user_id, kind);

GRANT SELECT, INSERT, UPDATE, DELETE ON site.email_tokens TO pulsars_api;
GRANT UPDATE (email_verified_at, updated_at) ON site.users TO pulsars_api;
GRANT INSERT (email_verified_at) ON site.users TO pulsars_api;
GRANT SELECT, INSERT, UPDATE, DELETE ON site.comptes_sociaux TO pulsars_api;
GRANT USAGE ON SEQUENCE site.comptes_sociaux_id_seq TO pulsars_api;

-- The site still reads and writes everything during phase 2.
GRANT SELECT, INSERT, UPDATE, DELETE ON site.email_tokens TO pulsars_site;
