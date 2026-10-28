-- The API issues game tokens itself (phase 2, docs/contract/auth-jwt.md):
-- their `roles` claim lists the member's permission roles, read from
-- `site.roles` and `site.role_user`. Read only; nothing else of `site`.

GRANT SELECT (id, slug, categorie, niveau) ON site.roles TO pulsars_api;
GRANT SELECT (role_id, user_id) ON site.role_user TO pulsars_api;
