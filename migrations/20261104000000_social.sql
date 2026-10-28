-- The social part of the site (step D): follows, friends, rooms and private
-- messages. The tables come from the starting schema, with Laravel's names;
-- this adds what the API's rules need and what Laravel only kept in PHP.

-- Private messages are sealed with AES-256-GCM under the API's
-- MESSAGES_KEY (`src/crypto.rs`). `contenu` holds base64(nonce ‖ ciphertext);
-- `key_version` names the key that sealed it, so the key can be rotated.
-- Nothing is migrated from Laravel: the table starts empty.
ALTER TABLE site.direct_messages
    ADD COLUMN key_version smallint NOT NULL,
    ADD CONSTRAINT direct_messages_key_version_check CHECK (key_version > 0);

-- A pair is stored once, smallest id first (Laravel's `Friendship::paire`).
ALTER TABLE site.friendships
    ADD CONSTRAINT friendships_pair_order_check CHECK (user_one_id < user_two_id),
    ADD CONSTRAINT friendships_origine_check CHECK (origine IN ('demande', 'abonnement'));
ALTER TABLE site.direct_conversations
    ADD CONSTRAINT direct_conversations_pair_order_check CHECK (user_one_id < user_two_id);
ALTER TABLE site.abonnements
    ADD CONSTRAINT abonnements_not_self_check CHECK (abonne_id <> suivi_id);

-- Laravel's enums TypeSalon and RoleSalon.
ALTER TABLE site.rooms
    ADD CONSTRAINT rooms_type_check CHECK (type IN ('public', 'prive'));
ALTER TABLE site.room_members
    ADD CONSTRAINT room_members_role_check CHECK (role IN ('proprietaire', 'membre'));
ALTER TABLE site.room_invites
    ADD CONSTRAINT room_invites_usages_check CHECK (usages >= 0),
    ADD CONSTRAINT room_invites_max_usages_check CHECK (max_usages IS NULL OR max_usages > 0);

-- Lookups the unique pairs do not cover: a member's friendships from the
-- second side, and the rooms a member belongs to.
CREATE INDEX friendships_user_two_id_index ON site.friendships USING btree (user_two_id);
CREATE INDEX room_members_user_id_index ON site.room_members USING btree (user_id);
-- Unread private messages of a conversation.
CREATE INDEX direct_messages_unread_index ON site.direct_messages
    USING btree (direct_conversation_id) WHERE read_at IS NULL;
