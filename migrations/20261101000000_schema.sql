-- The whole Pulsars schema, from scratch (PLAN-REFONTE.md, R3).
--
-- Four schemas, all owned by pulsars_owner (deploy/sql/roles.sql creates
-- the roles and the empty schemas; this runs as pulsars_migrate, whose
-- objects belong to pulsars_owner):
--
--   site     accounts, sessions, progression, forum, social, moderation;
--   contenu  courses, exams, resources, news, files, translations;
--   game     matches, ratings, seasons;
--   langues  the languages section.
--
-- It replaces the Laravel migrations and the API's earlier ones, which were
-- never installed together on a fresh server. Columns keep the names and
-- types Laravel gave them, so the content of the old database copies over
-- as is (`pulsars-api import-content`).

SET statement_timeout = 0;

CREATE SCHEMA IF NOT EXISTS contenu;

CREATE SCHEMA IF NOT EXISTS game;

CREATE SCHEMA IF NOT EXISTS langues;

CREATE SCHEMA IF NOT EXISTS site;

CREATE FUNCTION contenu.slugify(value text) RETURNS text
    LANGUAGE sql IMMUTABLE PARALLEL SAFE
    AS $$
    SELECT trim(BOTH '-' FROM regexp_replace(
        lower(translate(
            replace(replace(replace(replace(value, 'œ', 'oe'), 'Œ', 'oe'), 'æ', 'ae'), 'Æ', 'ae'),
            'ÀÂÄÁÃÅàâäáãåÇçÉÈÊËéèêëÍÌÎÏíìîïÑñÓÒÔÖÕóòôöõÚÙÛÜúùûüÝŸýÿ',
            'AAAAAAaaaaaaCcEEEEeeeeIIIIiiiiNnOOOOOoooooUUUUuuuuYYyy'
        )),
        '[^a-z0-9]+', '-', 'g'
    ))
$$;

CREATE TABLE contenu.chapters (
    id bigint NOT NULL,
    course_id bigint NOT NULL,
    titre character varying(255) NOT NULL,
    ordre smallint DEFAULT '0'::smallint NOT NULL,
    created_at timestamp(0) without time zone,
    updated_at timestamp(0) without time zone
);

CREATE TABLE contenu.courses (
    id bigint NOT NULL,
    titre character varying(255) NOT NULL,
    slug character varying(255) NOT NULL,
    description text NOT NULL,
    difficulte character varying(255) DEFAULT 'debutant'::character varying NOT NULL,
    duree_estimee smallint,
    tags json,
    auteur_id bigint NOT NULL,
    statut character varying(255) DEFAULT 'brouillon'::character varying NOT NULL,
    published_at timestamp(0) without time zone,
    created_at timestamp(0) without time zone,
    updated_at timestamp(0) without time zone,
    theme character varying(255) DEFAULT 'Divers'::character varying NOT NULL,
    vues bigint DEFAULT '0'::bigint NOT NULL
);

CREATE TABLE contenu.exercises (
    id bigint NOT NULL,
    chapter_id bigint NOT NULL,
    type character varying(255) DEFAULT 'qcm'::character varying NOT NULL,
    titre character varying(255) NOT NULL,
    question_md text NOT NULL,
    donnees json,
    solution_md text,
    ordre smallint DEFAULT '0'::smallint NOT NULL,
    created_at timestamp(0) without time zone,
    updated_at timestamp(0) without time zone
);

CREATE TABLE contenu.lessons (
    id bigint NOT NULL,
    chapter_id bigint NOT NULL,
    titre character varying(255) NOT NULL,
    slug character varying(255) NOT NULL,
    contenu_md text NOT NULL,
    ordre smallint DEFAULT '0'::smallint NOT NULL,
    created_at timestamp(0) without time zone,
    updated_at timestamp(0) without time zone
);

CREATE TABLE site.users (
    id bigint NOT NULL,
    name character varying(255) NOT NULL,
    email character varying(255) NOT NULL,
    email_verified_at timestamp(0) without time zone,
    password character varying(255),
    created_at timestamp(0) without time zone,
    updated_at timestamp(0) without time zone,
    pseudo character varying(255) NOT NULL,
    avatar character varying(255),
    bio character varying(280),
    objectif json,
    niveau_scolaire character varying(255),
    classe character varying(255),
    onboarding_at timestamp(0) without time zone,
    locale character varying(5),
    friend_token character varying(32),
    mute_jusqu_a timestamp(0) without time zone,
    mute_motif character varying(300),
    mute_par bigint,
    cerveaux_publics boolean DEFAULT true NOT NULL
);

CREATE TABLE contenu.contenu_translations (
    id bigint NOT NULL,
    traduisible_type character varying(255) NOT NULL,
    traduisible_id bigint NOT NULL,
    langue character varying(5) NOT NULL,
    champs json NOT NULL,
    source_hash character varying(64) NOT NULL,
    created_at timestamp(0) without time zone,
    updated_at timestamp(0) without time zone
);

CREATE TABLE contenu.exam_parties (
    id bigint NOT NULL,
    exam_id bigint NOT NULL,
    ordre smallint DEFAULT '0'::smallint NOT NULL,
    titre character varying(255) NOT NULL,
    enonce_md text NOT NULL,
    corrige_md text NOT NULL,
    points numeric(5,2)
);

CREATE TABLE contenu.exams (
    id bigint NOT NULL,
    titre character varying(255) NOT NULL,
    slug character varying(255) NOT NULL,
    type character varying(255) NOT NULL,
    matiere character varying(255) NOT NULL,
    niveau character varying(255) NOT NULL,
    session character varying(255),
    duree_minutes smallint,
    points_total smallint,
    description text,
    statut character varying(255) DEFAULT 'attente_import'::character varying NOT NULL,
    fichier_pdf character varying(255),
    nom_fichier_original character varying(255),
    prompt_ia text,
    erreur_ia text,
    analyse_at timestamp(0) without time zone,
    published_at timestamp(0) without time zone,
    auteur_id bigint,
    created_at timestamp(0) without time zone,
    updated_at timestamp(0) without time zone,
    pays character varying(2) DEFAULT 'FR'::character varying NOT NULL,
    vues bigint DEFAULT '0'::bigint NOT NULL,
    fichier_corrige character varying(255),
    concours character varying(255),
    url_officielle character varying(500),
    url_source character varying(500),
    filieres json,
    telechargements bigint DEFAULT '0'::bigint NOT NULL
);

CREATE TABLE site.cerveaux_mouvements (
    id bigint NOT NULL,
    user_id bigint NOT NULL,
    montant integer NOT NULL,
    raison character varying(32) NOT NULL,
    cle character varying(120) NOT NULL,
    details json,
    created_at timestamp(0) without time zone NOT NULL
);

CREATE TABLE site.role_user (
    id bigint NOT NULL,
    role_id bigint NOT NULL,
    user_id bigint NOT NULL,
    created_at timestamp(0) without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL
);

CREATE TABLE site.roles (
    id bigint NOT NULL,
    slug character varying(255) NOT NULL,
    nom character varying(255) NOT NULL,
    categorie character varying(255) DEFAULT 'distinction'::character varying NOT NULL,
    couleur character varying(7) DEFAULT '#8a8a8f'::character varying NOT NULL,
    niveau smallint DEFAULT '0'::smallint NOT NULL,
    description character varying(255),
    ordre smallint DEFAULT '0'::smallint NOT NULL
);

CREATE TABLE site.answers (
    id bigint NOT NULL,
    question_id bigint NOT NULL,
    user_id bigint,
    parent_id bigint,
    corps text NOT NULL,
    created_at timestamp(0) without time zone,
    updated_at timestamp(0) without time zone,
    masque_le timestamp(0) without time zone,
    masque_par bigint,
    masque_motif character varying(300),
    deleted_at timestamp(0) without time zone
);

CREATE TABLE site.enrollments (
    id bigint NOT NULL,
    user_id bigint NOT NULL,
    course_id bigint NOT NULL,
    started_at timestamp(0) without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL,
    completed_at timestamp(0) without time zone,
    derniere_lesson_id bigint,
    created_at timestamp(0) without time zone,
    updated_at timestamp(0) without time zone
);

CREATE TABLE site.exercise_attempts (
    id bigint NOT NULL,
    user_id bigint NOT NULL,
    exercise_id bigint NOT NULL,
    reponse json NOT NULL,
    correct boolean,
    created_at timestamp(0) without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL
);

CREATE TABLE site.game_results (
    id bigint NOT NULL,
    game_event_id bigint NOT NULL,
    match_id uuid NOT NULL,
    user_id bigint NOT NULL,
    mode character varying(64) NOT NULL,
    ranked boolean NOT NULL,
    outcome character varying(16) NOT NULL,
    ended_at timestamp(0) without time zone NOT NULL
);

CREATE TABLE site.lesson_progress (
    id bigint NOT NULL,
    user_id bigint NOT NULL,
    lesson_id bigint NOT NULL,
    completed_at timestamp(0) without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL
);

CREATE TABLE site.questions (
    id bigint NOT NULL,
    forum_theme_id bigint NOT NULL,
    user_id bigint,
    titre character varying(255) NOT NULL,
    slug character varying(255) NOT NULL,
    corps text NOT NULL,
    vues integer DEFAULT 0 NOT NULL,
    accepted_answer_id bigint,
    created_at timestamp(0) without time zone,
    updated_at timestamp(0) without time zone,
    masque_le timestamp(0) without time zone,
    masque_par bigint,
    masque_motif character varying(300),
    verrouille_le timestamp(0) without time zone,
    epingle_le timestamp(0) without time zone,
    deleted_at timestamp(0) without time zone
);

CREATE TABLE contenu.ressources (
    id bigint NOT NULL,
    titre character varying(255) NOT NULL,
    slug character varying(255) NOT NULL,
    description text,
    categorie character varying(255),
    fichier character varying(255),
    nom_fichier_original character varying(255),
    taille bigint DEFAULT '0'::bigint NOT NULL,
    statut character varying(255) DEFAULT 'publie'::character varying NOT NULL,
    ajoute_par bigint,
    telechargements integer DEFAULT 0 NOT NULL,
    created_at timestamp(0) without time zone,
    updated_at timestamp(0) without time zone,
    langue character varying(5),
    auteur_nom character varying(200),
    licence character varying(60),
    licence_personnalisee character varying(200),
    lien character varying(500),
    vues bigint DEFAULT '0'::bigint NOT NULL
);

CREATE TABLE contenu.actualites (
    id bigint NOT NULL,
    titre character varying(255) NOT NULL,
    slug character varying(255) NOT NULL,
    chapeau character varying(500),
    contenu_md text NOT NULL,
    published_at timestamp(0) without time zone,
    auteur_id bigint,
    created_at timestamp(0) without time zone,
    updated_at timestamp(0) without time zone
);

CREATE SEQUENCE contenu.actualites_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE contenu.actualites_id_seq OWNED BY contenu.actualites.id;

CREATE SEQUENCE contenu.chapters_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE contenu.chapters_id_seq OWNED BY contenu.chapters.id;

CREATE SEQUENCE contenu.contenu_translations_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE contenu.contenu_translations_id_seq OWNED BY contenu.contenu_translations.id;

CREATE SEQUENCE contenu.courses_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE contenu.courses_id_seq OWNED BY contenu.courses.id;

CREATE SEQUENCE contenu.exam_parties_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE contenu.exam_parties_id_seq OWNED BY contenu.exam_parties.id;

CREATE SEQUENCE contenu.exams_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE contenu.exams_id_seq OWNED BY contenu.exams.id;

CREATE SEQUENCE contenu.exercises_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE contenu.exercises_id_seq OWNED BY contenu.exercises.id;

CREATE TABLE contenu.fichiers (
    id bigint NOT NULL,
    disque character varying(32) NOT NULL,
    chemin character varying(512) NOT NULL,
    contenu bytea NOT NULL,
    type_mime character varying(127) NOT NULL,
    taille bigint NOT NULL,
    created_at timestamp(0) without time zone,
    updated_at timestamp(0) without time zone
);

CREATE SEQUENCE contenu.fichiers_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE contenu.fichiers_id_seq OWNED BY contenu.fichiers.id;

CREATE SEQUENCE contenu.lessons_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE contenu.lessons_id_seq OWNED BY contenu.lessons.id;

CREATE TABLE contenu.licences (
    cle text NOT NULL,
    nom text NOT NULL,
    url text
);

CREATE TABLE contenu.matieres (
    slug text NOT NULL,
    langue text NOT NULL,
    nom text NOT NULL
);

CREATE SEQUENCE contenu.ressources_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE contenu.ressources_id_seq OWNED BY contenu.ressources.id;

CREATE TABLE contenu.sources_officielles (
    hote text NOT NULL,
    prefixe text NOT NULL,
    CONSTRAINT sources_officielles_hote_check CHECK ((hote = lower(hote))),
    CONSTRAINT sources_officielles_prefixe_check CHECK ((prefixe ~~ '/%'::text))
);

CREATE TABLE contenu.telechargements (
    id bigint NOT NULL,
    telechargeable_type character varying(255) NOT NULL,
    telechargeable_id bigint NOT NULL,
    visiteur_hash character varying(64) NOT NULL,
    user_id bigint,
    created_at timestamp(0) without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL
);

CREATE SEQUENCE contenu.telechargements_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE contenu.telechargements_id_seq OWNED BY contenu.telechargements.id;

CREATE TABLE game.match_players (
    match_id uuid NOT NULL,
    seat smallint NOT NULL,
    user_id bigint,
    guest_name text,
    ghost_of uuid,
    name text NOT NULL,
    score integer NOT NULL,
    correct integer NOT NULL,
    answered integer NOT NULL,
    answer_time_ms bigint NOT NULL,
    rank smallint NOT NULL,
    outcome text NOT NULL,
    rating_before integer,
    rating_after integer,
    answers jsonb DEFAULT '[]'::jsonb NOT NULL,
    CONSTRAINT match_players_answer_time_ms_check CHECK ((answer_time_ms >= 0)),
    CONSTRAINT match_players_check CHECK ((answered >= correct)),
    CONSTRAINT match_players_check1 CHECK ((num_nonnulls(user_id, guest_name, ghost_of) = 1)),
    CONSTRAINT match_players_correct_check CHECK ((correct >= 0)),
    CONSTRAINT match_players_outcome_check CHECK ((outcome = ANY (ARRAY['win'::text, 'loss'::text, 'draw'::text, 'forfeit'::text]))),
    CONSTRAINT match_players_rank_check CHECK ((rank >= 1)),
    CONSTRAINT match_players_seat_check CHECK (((seat >= 0) AND (seat <= 7)))
);

CREATE TABLE game.matches (
    id uuid NOT NULL,
    mode text NOT NULL,
    ranked boolean NOT NULL,
    subject text NOT NULL,
    rules jsonb NOT NULL,
    seed integer NOT NULL,
    created_at timestamp with time zone NOT NULL,
    started_at timestamp with time zone NOT NULL,
    ended_at timestamp with time zone,
    end_reason text,
    CONSTRAINT matches_check CHECK (((ended_at IS NULL) = (end_reason IS NULL))),
    CONSTRAINT matches_end_reason_check CHECK ((end_reason = ANY (ARRAY['completed'::text, 'timeout'::text, 'forfeit'::text, 'void'::text]))),
    CONSTRAINT matches_mode_check CHECK ((mode = ANY (ARRAY['duel'::text, 'room'::text, 'ghost'::text]))),
    CONSTRAINT matches_seed_check CHECK (((seed >= 1) AND (seed <= 999999))),
    CONSTRAINT matches_subject_check CHECK ((subject = ANY (ARRAY['maths'::text, 'physics'::text, 'chemistry'::text, 'computer-science'::text, 'all'::text])))
);

CREATE TABLE game.ratings (
    user_id bigint NOT NULL,
    subject text NOT NULL,
    rating double precision NOT NULL,
    rd double precision NOT NULL,
    volatility double precision NOT NULL,
    games integer DEFAULT 0 NOT NULL,
    wins integer DEFAULT 0 NOT NULL,
    losses integer DEFAULT 0 NOT NULL,
    draws integer DEFAULT 0 NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT ratings_check CHECK ((games = ((wins + losses) + draws))),
    CONSTRAINT ratings_rd_check CHECK ((rd > (0)::double precision)),
    CONSTRAINT ratings_subject_check CHECK ((subject = ANY (ARRAY['maths'::text, 'physics'::text, 'chemistry'::text, 'computer-science'::text, 'all'::text]))),
    CONSTRAINT ratings_volatility_check CHECK ((volatility > (0)::double precision))
);

CREATE TABLE game.seasons (
    month text NOT NULL,
    subject text NOT NULL,
    closed_at timestamp with time zone DEFAULT now() NOT NULL,
    standings jsonb DEFAULT '[]'::jsonb NOT NULL,
    event_id uuid,
    CONSTRAINT seasons_month_check CHECK ((month ~ '^[0-9]{4}-(0[1-9]|1[0-2])$'::text)),
    CONSTRAINT seasons_subject_check CHECK ((subject = ANY (ARRAY['maths'::text, 'physics'::text, 'chemistry'::text, 'computer-science'::text, 'all'::text])))
);

CREATE TABLE game.webhook_outbox (
    event_id uuid NOT NULL,
    type text NOT NULL,
    body jsonb NOT NULL,
    status text DEFAULT 'pending'::text NOT NULL,
    attempts integer DEFAULT 0 NOT NULL,
    next_attempt_at timestamp with time zone DEFAULT now() NOT NULL,
    last_error text,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    sent_at timestamp with time zone,
    CONSTRAINT webhook_outbox_status_check CHECK ((status = ANY (ARRAY['pending'::text, 'sent'::text, 'dead'::text])))
);

CREATE TABLE langues.events (
    id bigint NOT NULL,
    received_at timestamp with time zone DEFAULT now() NOT NULL,
    kind text NOT NULL,
    learner uuid NOT NULL,
    language_code text NOT NULL,
    level_slug text NOT NULL,
    score integer,
    correct smallint,
    questions smallint,
    duration_ms integer,
    missed_words bigint[],
    "character" text,
    mode text,
    mistakes smallint,
    CONSTRAINT events_character_check CHECK ((char_length("character") = 1)),
    CONSTRAINT events_check CHECK ((((kind = 'lesson.completed'::text) AND (num_nonnulls(score, correct, questions, duration_ms, missed_words) = 5) AND (correct <= questions) AND (num_nonnulls("character", mode, mistakes) = 0)) OR ((kind = 'writing.completed'::text) AND (num_nonnulls("character", mode, mistakes) = 3) AND (num_nonnulls(score, correct, questions, duration_ms, missed_words) = 0)))),
    CONSTRAINT events_correct_check CHECK ((correct >= 0)),
    CONSTRAINT events_duration_ms_check CHECK (((duration_ms >= 0) AND (duration_ms <= 86400000))),
    CONSTRAINT events_kind_check CHECK ((kind = ANY (ARRAY['lesson.completed'::text, 'writing.completed'::text]))),
    CONSTRAINT events_mistakes_check CHECK (((mistakes >= 0) AND (mistakes <= 1000))),
    CONSTRAINT events_mode_check CHECK ((mode = ANY (ARRAY['trace'::text, 'memory'::text]))),
    CONSTRAINT events_questions_check CHECK (((questions >= 1) AND (questions <= 100))),
    CONSTRAINT events_score_check CHECK (((score >= 0) AND (score <= 10000)))
);

ALTER TABLE langues.events ALTER COLUMN id ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME langues.events_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);

CREATE TABLE langues.languages (
    code text NOT NULL,
    name jsonb NOT NULL,
    native_name text NOT NULL,
    script text NOT NULL,
    reading_system text,
    "position" smallint NOT NULL,
    published boolean DEFAULT false NOT NULL,
    countries text[] DEFAULT '{}'::text[] NOT NULL,
    CONSTRAINT languages_code_check CHECK ((code ~ '^[a-z]{2,3}(-[a-z0-9]{2,8})*$'::text)),
    CONSTRAINT languages_countries_check CHECK ((array_to_string(countries, ','::text) ~ '^([A-Z]{2}(,[A-Z]{2})*)?$'::text)),
    CONSTRAINT languages_name_check CHECK ((name ? 'fr'::text)),
    CONSTRAINT languages_script_check CHECK ((script ~ '^[A-Z][a-z]{3}$'::text))
);

CREATE TABLE langues.levels (
    id bigint NOT NULL,
    language_code text NOT NULL,
    slug text NOT NULL,
    framework text NOT NULL,
    "position" smallint NOT NULL,
    title jsonb NOT NULL,
    description jsonb NOT NULL,
    CONSTRAINT levels_description_check CHECK ((description ? 'fr'::text)),
    CONSTRAINT levels_slug_check CHECK ((slug ~ '^[a-z0-9]+(-[a-z0-9]+)*$'::text)),
    CONSTRAINT levels_title_check CHECK ((title ? 'fr'::text))
);

ALTER TABLE langues.levels ALTER COLUMN id ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME langues.levels_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);

CREATE TABLE langues.words (
    id bigint NOT NULL,
    level_id bigint NOT NULL,
    "position" integer NOT NULL,
    text text NOT NULL,
    reading text,
    meaning jsonb NOT NULL,
    CONSTRAINT words_meaning_check CHECK ((meaning ? 'fr'::text))
);

ALTER TABLE langues.words ALTER COLUMN id ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME langues.words_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);

CREATE TABLE site.abonnements (
    id bigint NOT NULL,
    abonne_id bigint NOT NULL,
    suivi_id bigint NOT NULL,
    created_at timestamp(0) without time zone,
    updated_at timestamp(0) without time zone
);

CREATE SEQUENCE site.abonnements_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE site.abonnements_id_seq OWNED BY site.abonnements.id;

CREATE SEQUENCE site.answers_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE site.answers_id_seq OWNED BY site.answers.id;

CREATE TABLE site.auth_sessions (
    id uuid NOT NULL,
    user_id bigint NOT NULL,
    token_hash bytea NOT NULL,
    remember boolean NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    last_used_at timestamp with time zone DEFAULT now() NOT NULL,
    expires_at timestamp with time zone NOT NULL,
    revoked_at timestamp with time zone,
    ip inet,
    user_agent text
);

CREATE SEQUENCE site.cerveaux_mouvements_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE site.cerveaux_mouvements_id_seq OWNED BY site.cerveaux_mouvements.id;

CREATE TABLE site.comptes_sociaux (
    id bigint NOT NULL,
    user_id bigint NOT NULL,
    fournisseur character varying(20) NOT NULL,
    identifiant_fournisseur character varying(255) NOT NULL,
    email character varying(255),
    created_at timestamp(0) without time zone,
    updated_at timestamp(0) without time zone
);

CREATE SEQUENCE site.comptes_sociaux_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE site.comptes_sociaux_id_seq OWNED BY site.comptes_sociaux.id;

CREATE TABLE site.demandes_ressources (
    id bigint NOT NULL,
    user_id bigint,
    titre character varying(200) NOT NULL,
    auteur_nom character varying(200) NOT NULL,
    licence character varying(60) NOT NULL,
    licence_personnalisee character varying(200),
    description text,
    categorie character varying(120),
    langue character varying(8),
    fichier character varying(255) NOT NULL,
    nom_fichier_original character varying(255) NOT NULL,
    taille bigint DEFAULT '0'::bigint NOT NULL,
    statut character varying(255) DEFAULT 'en_attente'::character varying NOT NULL,
    traite_par bigint,
    traite_le timestamp(0) without time zone,
    motif text,
    ressource_id bigint,
    created_at timestamp(0) without time zone,
    updated_at timestamp(0) without time zone
);

CREATE SEQUENCE site.demandes_ressources_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE site.demandes_ressources_id_seq OWNED BY site.demandes_ressources.id;

CREATE TABLE site.direct_conversations (
    id bigint NOT NULL,
    user_one_id bigint NOT NULL,
    user_two_id bigint NOT NULL,
    last_message_at timestamp(0) without time zone,
    created_at timestamp(0) without time zone,
    updated_at timestamp(0) without time zone
);

CREATE SEQUENCE site.direct_conversations_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE site.direct_conversations_id_seq OWNED BY site.direct_conversations.id;

CREATE TABLE site.direct_messages (
    id bigint NOT NULL,
    direct_conversation_id bigint NOT NULL,
    sender_id bigint,
    contenu text NOT NULL,
    read_at timestamp(0) without time zone,
    created_at timestamp(0) without time zone,
    updated_at timestamp(0) without time zone
);

CREATE SEQUENCE site.direct_messages_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE site.direct_messages_id_seq OWNED BY site.direct_messages.id;

CREATE TABLE site.email_tokens (
    id uuid NOT NULL,
    user_id bigint NOT NULL,
    kind text NOT NULL,
    token_hash bytea NOT NULL,
    email text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    expires_at timestamp with time zone NOT NULL,
    used_at timestamp with time zone,
    CONSTRAINT email_tokens_kind_check CHECK ((kind = ANY (ARRAY['verify'::text, 'reset'::text])))
);

CREATE SEQUENCE site.enrollments_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE site.enrollments_id_seq OWNED BY site.enrollments.id;

CREATE SEQUENCE site.exercise_attempts_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE site.exercise_attempts_id_seq OWNED BY site.exercise_attempts.id;

CREATE TABLE site.forum_themes (
    id bigint NOT NULL,
    nom character varying(255) NOT NULL,
    slug character varying(255) NOT NULL,
    description character varying(280),
    couleur character varying(7) DEFAULT '#4f7df0'::character varying NOT NULL,
    ordre smallint DEFAULT '0'::smallint NOT NULL
);

CREATE SEQUENCE site.forum_themes_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE site.forum_themes_id_seq OWNED BY site.forum_themes.id;

CREATE TABLE site.forum_translations (
    id bigint NOT NULL,
    traduisible_type character varying(255) NOT NULL,
    traduisible_id bigint NOT NULL,
    langue_source character varying(10),
    langue_cible character varying(10) NOT NULL,
    source_hash character varying(64) NOT NULL,
    titre character varying(255),
    contenu text NOT NULL,
    created_at timestamp(0) without time zone,
    updated_at timestamp(0) without time zone
);

CREATE SEQUENCE site.forum_translations_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE site.forum_translations_id_seq OWNED BY site.forum_translations.id;

CREATE TABLE site.forum_votes (
    id bigint NOT NULL,
    user_id bigint NOT NULL,
    votable_type character varying(255) NOT NULL,
    votable_id bigint NOT NULL,
    created_at timestamp(0) without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL
);

CREATE SEQUENCE site.forum_votes_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE site.forum_votes_id_seq OWNED BY site.forum_votes.id;

CREATE TABLE site.friendships (
    id bigint NOT NULL,
    user_one_id bigint NOT NULL,
    user_two_id bigint NOT NULL,
    requested_by_id bigint NOT NULL,
    accepted_at timestamp(0) without time zone,
    created_at timestamp(0) without time zone,
    updated_at timestamp(0) without time zone,
    origine character varying(255) DEFAULT 'demande'::character varying NOT NULL
);

CREATE SEQUENCE site.friendships_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE site.friendships_id_seq OWNED BY site.friendships.id;

CREATE SEQUENCE site.game_results_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE site.game_results_id_seq OWNED BY site.game_results.id;

CREATE SEQUENCE site.lesson_progress_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE site.lesson_progress_id_seq OWNED BY site.lesson_progress.id;

CREATE SEQUENCE site.questions_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE site.questions_id_seq OWNED BY site.questions.id;

CREATE TABLE site.reglages (
    id bigint NOT NULL,
    cle character varying(255) NOT NULL,
    valeur json NOT NULL,
    modifie_par bigint,
    created_at timestamp(0) without time zone,
    updated_at timestamp(0) without time zone
);

CREATE SEQUENCE site.reglages_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE site.reglages_id_seq OWNED BY site.reglages.id;

CREATE SEQUENCE site.role_user_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE site.role_user_id_seq OWNED BY site.role_user.id;

CREATE SEQUENCE site.roles_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE site.roles_id_seq OWNED BY site.roles.id;

CREATE TABLE site.room_invites (
    id bigint NOT NULL,
    room_id bigint NOT NULL,
    token character varying(40) NOT NULL,
    created_by bigint NOT NULL,
    expires_at timestamp(0) without time zone,
    max_usages integer,
    usages integer DEFAULT 0 NOT NULL,
    revoked_at timestamp(0) without time zone,
    created_at timestamp(0) without time zone,
    updated_at timestamp(0) without time zone
);

CREATE SEQUENCE site.room_invites_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE site.room_invites_id_seq OWNED BY site.room_invites.id;

CREATE TABLE site.room_members (
    id bigint NOT NULL,
    room_id bigint NOT NULL,
    user_id bigint NOT NULL,
    role character varying(255) DEFAULT 'membre'::character varying NOT NULL,
    last_read_at timestamp(0) without time zone,
    joined_at timestamp(0) without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL
);

CREATE SEQUENCE site.room_members_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE site.room_members_id_seq OWNED BY site.room_members.id;

CREATE TABLE site.room_messages (
    id bigint NOT NULL,
    room_id bigint NOT NULL,
    user_id bigint,
    contenu text NOT NULL,
    created_at timestamp(0) without time zone DEFAULT CURRENT_TIMESTAMP NOT NULL
);

CREATE SEQUENCE site.room_messages_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE site.room_messages_id_seq OWNED BY site.room_messages.id;

CREATE TABLE site.rooms (
    id bigint NOT NULL,
    nom character varying(255) NOT NULL,
    slug character varying(255) NOT NULL,
    description character varying(280),
    type character varying(255) DEFAULT 'public'::character varying NOT NULL,
    created_by bigint NOT NULL,
    created_at timestamp(0) without time zone,
    updated_at timestamp(0) without time zone
);

CREATE SEQUENCE site.rooms_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE site.rooms_id_seq OWNED BY site.rooms.id;

CREATE TABLE site.signalements (
    id bigint NOT NULL,
    signalable_type character varying(255) NOT NULL,
    signalable_id bigint NOT NULL,
    user_id bigint,
    type character varying(255) NOT NULL,
    message text NOT NULL,
    emplacement character varying(300),
    statut character varying(255) DEFAULT 'nouveau'::character varying NOT NULL,
    traite_par bigint,
    traite_le timestamp(0) without time zone,
    reponse text,
    created_at timestamp(0) without time zone,
    updated_at timestamp(0) without time zone
);

CREATE SEQUENCE site.signalements_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE site.signalements_id_seq OWNED BY site.signalements.id;

CREATE TABLE site.suggestions (
    id bigint NOT NULL,
    user_id bigint,
    titre character varying(150),
    message text NOT NULL,
    image character varying(2048),
    statut character varying(255) DEFAULT 'nouvelle'::character varying NOT NULL,
    traite_par bigint,
    traite_le timestamp(0) without time zone,
    reponse text,
    created_at timestamp(0) without time zone,
    updated_at timestamp(0) without time zone
);

CREATE SEQUENCE site.suggestions_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE site.suggestions_id_seq OWNED BY site.suggestions.id;

CREATE SEQUENCE site.users_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE site.users_id_seq OWNED BY site.users.id;

ALTER TABLE ONLY contenu.actualites ALTER COLUMN id SET DEFAULT nextval('contenu.actualites_id_seq'::regclass);

ALTER TABLE ONLY contenu.chapters ALTER COLUMN id SET DEFAULT nextval('contenu.chapters_id_seq'::regclass);

ALTER TABLE ONLY contenu.contenu_translations ALTER COLUMN id SET DEFAULT nextval('contenu.contenu_translations_id_seq'::regclass);

ALTER TABLE ONLY contenu.courses ALTER COLUMN id SET DEFAULT nextval('contenu.courses_id_seq'::regclass);

ALTER TABLE ONLY contenu.exam_parties ALTER COLUMN id SET DEFAULT nextval('contenu.exam_parties_id_seq'::regclass);

ALTER TABLE ONLY contenu.exams ALTER COLUMN id SET DEFAULT nextval('contenu.exams_id_seq'::regclass);

ALTER TABLE ONLY contenu.exercises ALTER COLUMN id SET DEFAULT nextval('contenu.exercises_id_seq'::regclass);

ALTER TABLE ONLY contenu.fichiers ALTER COLUMN id SET DEFAULT nextval('contenu.fichiers_id_seq'::regclass);

ALTER TABLE ONLY contenu.lessons ALTER COLUMN id SET DEFAULT nextval('contenu.lessons_id_seq'::regclass);

ALTER TABLE ONLY contenu.ressources ALTER COLUMN id SET DEFAULT nextval('contenu.ressources_id_seq'::regclass);

ALTER TABLE ONLY contenu.telechargements ALTER COLUMN id SET DEFAULT nextval('contenu.telechargements_id_seq'::regclass);

ALTER TABLE ONLY site.abonnements ALTER COLUMN id SET DEFAULT nextval('site.abonnements_id_seq'::regclass);

ALTER TABLE ONLY site.answers ALTER COLUMN id SET DEFAULT nextval('site.answers_id_seq'::regclass);

ALTER TABLE ONLY site.cerveaux_mouvements ALTER COLUMN id SET DEFAULT nextval('site.cerveaux_mouvements_id_seq'::regclass);

ALTER TABLE ONLY site.comptes_sociaux ALTER COLUMN id SET DEFAULT nextval('site.comptes_sociaux_id_seq'::regclass);

ALTER TABLE ONLY site.demandes_ressources ALTER COLUMN id SET DEFAULT nextval('site.demandes_ressources_id_seq'::regclass);

ALTER TABLE ONLY site.direct_conversations ALTER COLUMN id SET DEFAULT nextval('site.direct_conversations_id_seq'::regclass);

ALTER TABLE ONLY site.direct_messages ALTER COLUMN id SET DEFAULT nextval('site.direct_messages_id_seq'::regclass);

ALTER TABLE ONLY site.enrollments ALTER COLUMN id SET DEFAULT nextval('site.enrollments_id_seq'::regclass);

ALTER TABLE ONLY site.exercise_attempts ALTER COLUMN id SET DEFAULT nextval('site.exercise_attempts_id_seq'::regclass);

ALTER TABLE ONLY site.forum_themes ALTER COLUMN id SET DEFAULT nextval('site.forum_themes_id_seq'::regclass);

ALTER TABLE ONLY site.forum_translations ALTER COLUMN id SET DEFAULT nextval('site.forum_translations_id_seq'::regclass);

ALTER TABLE ONLY site.forum_votes ALTER COLUMN id SET DEFAULT nextval('site.forum_votes_id_seq'::regclass);

ALTER TABLE ONLY site.friendships ALTER COLUMN id SET DEFAULT nextval('site.friendships_id_seq'::regclass);

ALTER TABLE ONLY site.game_results ALTER COLUMN id SET DEFAULT nextval('site.game_results_id_seq'::regclass);

ALTER TABLE ONLY site.lesson_progress ALTER COLUMN id SET DEFAULT nextval('site.lesson_progress_id_seq'::regclass);

ALTER TABLE ONLY site.questions ALTER COLUMN id SET DEFAULT nextval('site.questions_id_seq'::regclass);

ALTER TABLE ONLY site.reglages ALTER COLUMN id SET DEFAULT nextval('site.reglages_id_seq'::regclass);

ALTER TABLE ONLY site.role_user ALTER COLUMN id SET DEFAULT nextval('site.role_user_id_seq'::regclass);

ALTER TABLE ONLY site.roles ALTER COLUMN id SET DEFAULT nextval('site.roles_id_seq'::regclass);

ALTER TABLE ONLY site.room_invites ALTER COLUMN id SET DEFAULT nextval('site.room_invites_id_seq'::regclass);

ALTER TABLE ONLY site.room_members ALTER COLUMN id SET DEFAULT nextval('site.room_members_id_seq'::regclass);

ALTER TABLE ONLY site.room_messages ALTER COLUMN id SET DEFAULT nextval('site.room_messages_id_seq'::regclass);

ALTER TABLE ONLY site.rooms ALTER COLUMN id SET DEFAULT nextval('site.rooms_id_seq'::regclass);

ALTER TABLE ONLY site.signalements ALTER COLUMN id SET DEFAULT nextval('site.signalements_id_seq'::regclass);

ALTER TABLE ONLY site.suggestions ALTER COLUMN id SET DEFAULT nextval('site.suggestions_id_seq'::regclass);

ALTER TABLE ONLY site.users ALTER COLUMN id SET DEFAULT nextval('site.users_id_seq'::regclass);

ALTER TABLE ONLY contenu.actualites
    ADD CONSTRAINT actualites_pkey PRIMARY KEY (id);

ALTER TABLE ONLY contenu.actualites
    ADD CONSTRAINT actualites_slug_unique UNIQUE (slug);

ALTER TABLE ONLY contenu.chapters
    ADD CONSTRAINT chapters_pkey PRIMARY KEY (id);

ALTER TABLE ONLY contenu.contenu_translations
    ADD CONSTRAINT contenu_trad_unique UNIQUE (traduisible_type, traduisible_id, langue);

ALTER TABLE ONLY contenu.contenu_translations
    ADD CONSTRAINT contenu_translations_pkey PRIMARY KEY (id);

ALTER TABLE ONLY contenu.courses
    ADD CONSTRAINT courses_pkey PRIMARY KEY (id);

ALTER TABLE ONLY contenu.courses
    ADD CONSTRAINT courses_slug_unique UNIQUE (slug);

ALTER TABLE ONLY contenu.exam_parties
    ADD CONSTRAINT exam_parties_pkey PRIMARY KEY (id);

ALTER TABLE ONLY contenu.exams
    ADD CONSTRAINT exams_pkey PRIMARY KEY (id);

ALTER TABLE ONLY contenu.exams
    ADD CONSTRAINT exams_slug_unique UNIQUE (slug);

ALTER TABLE ONLY contenu.exercises
    ADD CONSTRAINT exercises_pkey PRIMARY KEY (id);

ALTER TABLE ONLY contenu.fichiers
    ADD CONSTRAINT fichiers_disque_chemin_unique UNIQUE (disque, chemin);

ALTER TABLE ONLY contenu.fichiers
    ADD CONSTRAINT fichiers_pkey PRIMARY KEY (id);

ALTER TABLE ONLY contenu.lessons
    ADD CONSTRAINT lessons_chapter_id_slug_unique UNIQUE (chapter_id, slug);

ALTER TABLE ONLY contenu.lessons
    ADD CONSTRAINT lessons_pkey PRIMARY KEY (id);

ALTER TABLE ONLY contenu.licences
    ADD CONSTRAINT licences_pkey PRIMARY KEY (cle);

ALTER TABLE ONLY contenu.matieres
    ADD CONSTRAINT matieres_pkey PRIMARY KEY (slug, langue);

ALTER TABLE ONLY contenu.ressources
    ADD CONSTRAINT ressources_pkey PRIMARY KEY (id);

ALTER TABLE ONLY contenu.ressources
    ADD CONSTRAINT ressources_slug_unique UNIQUE (slug);

ALTER TABLE ONLY contenu.sources_officielles
    ADD CONSTRAINT sources_officielles_pkey PRIMARY KEY (hote);

ALTER TABLE ONLY contenu.telechargements
    ADD CONSTRAINT telechargement_unique UNIQUE (telechargeable_type, telechargeable_id, visiteur_hash);

ALTER TABLE ONLY contenu.telechargements
    ADD CONSTRAINT telechargements_pkey PRIMARY KEY (id);

ALTER TABLE ONLY game.match_players
    ADD CONSTRAINT match_players_pkey PRIMARY KEY (match_id, seat);

ALTER TABLE ONLY game.matches
    ADD CONSTRAINT matches_pkey PRIMARY KEY (id);

ALTER TABLE ONLY game.ratings
    ADD CONSTRAINT ratings_pkey PRIMARY KEY (user_id, subject);

ALTER TABLE ONLY game.seasons
    ADD CONSTRAINT seasons_pkey PRIMARY KEY (month, subject);

ALTER TABLE ONLY game.webhook_outbox
    ADD CONSTRAINT webhook_outbox_pkey PRIMARY KEY (event_id);

ALTER TABLE ONLY langues.events
    ADD CONSTRAINT events_pkey PRIMARY KEY (id);

ALTER TABLE ONLY langues.languages
    ADD CONSTRAINT languages_pkey PRIMARY KEY (code);

ALTER TABLE ONLY langues.languages
    ADD CONSTRAINT languages_position_key UNIQUE ("position");

ALTER TABLE ONLY langues.levels
    ADD CONSTRAINT levels_language_code_position_key UNIQUE (language_code, "position");

ALTER TABLE ONLY langues.levels
    ADD CONSTRAINT levels_language_code_slug_key UNIQUE (language_code, slug);

ALTER TABLE ONLY langues.levels
    ADD CONSTRAINT levels_pkey PRIMARY KEY (id);

ALTER TABLE ONLY langues.words
    ADD CONSTRAINT words_level_id_position_key UNIQUE (level_id, "position");

ALTER TABLE ONLY langues.words
    ADD CONSTRAINT words_pkey PRIMARY KEY (id);

ALTER TABLE ONLY site.abonnements
    ADD CONSTRAINT abonnements_abonne_id_suivi_id_unique UNIQUE (abonne_id, suivi_id);

ALTER TABLE ONLY site.abonnements
    ADD CONSTRAINT abonnements_pkey PRIMARY KEY (id);

ALTER TABLE ONLY site.answers
    ADD CONSTRAINT answers_pkey PRIMARY KEY (id);

ALTER TABLE ONLY site.auth_sessions
    ADD CONSTRAINT auth_sessions_pkey PRIMARY KEY (id);

ALTER TABLE ONLY site.auth_sessions
    ADD CONSTRAINT auth_sessions_token_hash_key UNIQUE (token_hash);

ALTER TABLE ONLY site.cerveaux_mouvements
    ADD CONSTRAINT cerveaux_mouvements_pkey PRIMARY KEY (id);

ALTER TABLE ONLY site.cerveaux_mouvements
    ADD CONSTRAINT cerveaux_mouvements_user_id_cle_unique UNIQUE (user_id, cle);

ALTER TABLE ONLY site.comptes_sociaux
    ADD CONSTRAINT comptes_sociaux_fournisseur_identifiant_fournisseur_unique UNIQUE (fournisseur, identifiant_fournisseur);

ALTER TABLE ONLY site.comptes_sociaux
    ADD CONSTRAINT comptes_sociaux_pkey PRIMARY KEY (id);

ALTER TABLE ONLY site.comptes_sociaux
    ADD CONSTRAINT comptes_sociaux_user_id_fournisseur_unique UNIQUE (user_id, fournisseur);

ALTER TABLE ONLY site.demandes_ressources
    ADD CONSTRAINT demandes_ressources_pkey PRIMARY KEY (id);

ALTER TABLE ONLY site.direct_conversations
    ADD CONSTRAINT direct_conversations_pkey PRIMARY KEY (id);

ALTER TABLE ONLY site.direct_conversations
    ADD CONSTRAINT direct_conversations_user_one_id_user_two_id_unique UNIQUE (user_one_id, user_two_id);

ALTER TABLE ONLY site.direct_messages
    ADD CONSTRAINT direct_messages_pkey PRIMARY KEY (id);

ALTER TABLE ONLY site.email_tokens
    ADD CONSTRAINT email_tokens_pkey PRIMARY KEY (id);

ALTER TABLE ONLY site.email_tokens
    ADD CONSTRAINT email_tokens_token_hash_key UNIQUE (token_hash);

ALTER TABLE ONLY site.enrollments
    ADD CONSTRAINT enrollments_pkey PRIMARY KEY (id);

ALTER TABLE ONLY site.enrollments
    ADD CONSTRAINT enrollments_user_id_course_id_unique UNIQUE (user_id, course_id);

ALTER TABLE ONLY site.exercise_attempts
    ADD CONSTRAINT exercise_attempts_pkey PRIMARY KEY (id);

ALTER TABLE ONLY site.forum_themes
    ADD CONSTRAINT forum_themes_pkey PRIMARY KEY (id);

ALTER TABLE ONLY site.forum_themes
    ADD CONSTRAINT forum_themes_slug_unique UNIQUE (slug);

ALTER TABLE ONLY site.forum_translations
    ADD CONSTRAINT forum_traduction_unique UNIQUE (traduisible_type, traduisible_id, langue_cible);

ALTER TABLE ONLY site.forum_translations
    ADD CONSTRAINT forum_translations_pkey PRIMARY KEY (id);

ALTER TABLE ONLY site.forum_votes
    ADD CONSTRAINT forum_votes_pkey PRIMARY KEY (id);

ALTER TABLE ONLY site.forum_votes
    ADD CONSTRAINT forum_votes_user_id_votable_type_votable_id_unique UNIQUE (user_id, votable_type, votable_id);

ALTER TABLE ONLY site.friendships
    ADD CONSTRAINT friendships_pkey PRIMARY KEY (id);

ALTER TABLE ONLY site.friendships
    ADD CONSTRAINT friendships_user_one_id_user_two_id_unique UNIQUE (user_one_id, user_two_id);

ALTER TABLE ONLY site.game_results
    ADD CONSTRAINT game_results_match_id_user_id_unique UNIQUE (match_id, user_id);

ALTER TABLE ONLY site.game_results
    ADD CONSTRAINT game_results_pkey PRIMARY KEY (id);

ALTER TABLE ONLY site.lesson_progress
    ADD CONSTRAINT lesson_progress_pkey PRIMARY KEY (id);

ALTER TABLE ONLY site.lesson_progress
    ADD CONSTRAINT lesson_progress_user_id_lesson_id_unique UNIQUE (user_id, lesson_id);

ALTER TABLE ONLY site.questions
    ADD CONSTRAINT questions_pkey PRIMARY KEY (id);

ALTER TABLE ONLY site.questions
    ADD CONSTRAINT questions_slug_unique UNIQUE (slug);

ALTER TABLE ONLY site.reglages
    ADD CONSTRAINT reglages_cle_unique UNIQUE (cle);

ALTER TABLE ONLY site.reglages
    ADD CONSTRAINT reglages_pkey PRIMARY KEY (id);

ALTER TABLE ONLY site.role_user
    ADD CONSTRAINT role_user_pkey PRIMARY KEY (id);

ALTER TABLE ONLY site.role_user
    ADD CONSTRAINT role_user_role_id_user_id_unique UNIQUE (role_id, user_id);

ALTER TABLE ONLY site.roles
    ADD CONSTRAINT roles_pkey PRIMARY KEY (id);

ALTER TABLE ONLY site.roles
    ADD CONSTRAINT roles_slug_unique UNIQUE (slug);

ALTER TABLE ONLY site.room_invites
    ADD CONSTRAINT room_invites_pkey PRIMARY KEY (id);

ALTER TABLE ONLY site.room_invites
    ADD CONSTRAINT room_invites_token_unique UNIQUE (token);

ALTER TABLE ONLY site.room_members
    ADD CONSTRAINT room_members_pkey PRIMARY KEY (id);

ALTER TABLE ONLY site.room_members
    ADD CONSTRAINT room_members_room_id_user_id_unique UNIQUE (room_id, user_id);

ALTER TABLE ONLY site.room_messages
    ADD CONSTRAINT room_messages_pkey PRIMARY KEY (id);

ALTER TABLE ONLY site.rooms
    ADD CONSTRAINT rooms_pkey PRIMARY KEY (id);

ALTER TABLE ONLY site.rooms
    ADD CONSTRAINT rooms_slug_unique UNIQUE (slug);

ALTER TABLE ONLY site.signalements
    ADD CONSTRAINT signalements_pkey PRIMARY KEY (id);

ALTER TABLE ONLY site.suggestions
    ADD CONSTRAINT suggestions_pkey PRIMARY KEY (id);

ALTER TABLE ONLY site.users
    ADD CONSTRAINT users_email_unique UNIQUE (email);

ALTER TABLE ONLY site.users
    ADD CONSTRAINT users_friend_token_unique UNIQUE (friend_token);

ALTER TABLE ONLY site.users
    ADD CONSTRAINT users_pkey PRIMARY KEY (id);

ALTER TABLE ONLY site.users
    ADD CONSTRAINT users_pseudo_unique UNIQUE (pseudo);

CREATE INDEX actualites_published_at_index ON contenu.actualites USING btree (published_at);

CREATE INDEX chapters_course_id_ordre_index ON contenu.chapters USING btree (course_id, ordre);

CREATE INDEX contenu_translations_traduisible_type_traduisible_id_index ON contenu.contenu_translations USING btree (traduisible_type, traduisible_id);

CREATE INDEX courses_difficulte_index ON contenu.courses USING btree (difficulte);

CREATE INDEX courses_statut_published_at_index ON contenu.courses USING btree (statut, published_at);

CREATE INDEX courses_theme_index ON contenu.courses USING btree (theme);

CREATE INDEX exam_parties_exam_id_ordre_index ON contenu.exam_parties USING btree (exam_id, ordre);

CREATE INDEX exams_concours_index ON contenu.exams USING btree (concours);

CREATE INDEX exams_niveau_index ON contenu.exams USING btree (niveau);

CREATE INDEX exams_pays_index ON contenu.exams USING btree (pays);

CREATE INDEX exams_statut_published_at_index ON contenu.exams USING btree (statut, published_at);

CREATE INDEX exams_type_matiere_index ON contenu.exams USING btree (type, matiere);

CREATE INDEX exercises_chapter_id_ordre_index ON contenu.exercises USING btree (chapter_id, ordre);

CREATE INDEX lessons_chapter_id_ordre_index ON contenu.lessons USING btree (chapter_id, ordre);

CREATE INDEX ressources_statut_created_at_index ON contenu.ressources USING btree (statut, created_at);

CREATE INDEX telechargements_telechargeable_type_telechargeable_id_index ON contenu.telechargements USING btree (telechargeable_type, telechargeable_id);

CREATE INDEX match_players_user ON game.match_players USING btree (user_id, match_id) WHERE (user_id IS NOT NULL);

CREATE INDEX matches_ghost_key ON game.matches USING btree (subject, rules) WHERE ((end_reason IS NOT NULL) AND (end_reason <> 'void'::text));

CREATE INDEX matches_ranked_ended ON game.matches USING btree (subject, ended_at) WHERE (ranked AND (end_reason IS NOT NULL));

CREATE INDEX matches_started ON game.matches USING btree (started_at);

CREATE INDEX ratings_leaderboard ON game.ratings USING btree (subject, rating DESC) WHERE (games > 0);

CREATE INDEX webhook_outbox_dead ON game.webhook_outbox USING btree (created_at) WHERE (status = 'dead'::text);

CREATE INDEX webhook_outbox_due ON game.webhook_outbox USING btree (next_attempt_at) WHERE (status = 'pending'::text);

CREATE INDEX events_received_at ON langues.events USING btree (received_at);

CREATE INDEX abonnements_suivi_id_index ON site.abonnements USING btree (suivi_id);

CREATE INDEX answers_question_id_index ON site.answers USING btree (question_id);

CREATE INDEX answers_user_id_index ON site.answers USING btree (user_id);

CREATE INDEX auth_sessions_user_id_index ON site.auth_sessions USING btree (user_id);

CREATE INDEX cerveaux_mouvements_user_id_created_at_index ON site.cerveaux_mouvements USING btree (user_id, created_at);

CREATE INDEX demandes_ressources_statut_created_at_index ON site.demandes_ressources USING btree (statut, created_at);

CREATE INDEX direct_conversations_user_one_id_last_message_at_index ON site.direct_conversations USING btree (user_one_id, last_message_at);

CREATE INDEX direct_conversations_user_two_id_last_message_at_index ON site.direct_conversations USING btree (user_two_id, last_message_at);

CREATE INDEX direct_messages_direct_conversation_id_id_index ON site.direct_messages USING btree (direct_conversation_id, id);

CREATE INDEX direct_messages_sender_id_read_at_index ON site.direct_messages USING btree (sender_id, read_at);

CREATE INDEX email_tokens_user_id_kind_index ON site.email_tokens USING btree (user_id, kind);

CREATE INDEX exercise_attempts_user_id_exercise_id_index ON site.exercise_attempts USING btree (user_id, exercise_id);

CREATE INDEX forum_translations_traduisible_type_traduisible_id_index ON site.forum_translations USING btree (traduisible_type, traduisible_id);

CREATE INDEX forum_votes_votable_type_votable_id_index ON site.forum_votes USING btree (votable_type, votable_id);

CREATE INDEX friendships_requested_by_id_accepted_at_index ON site.friendships USING btree (requested_by_id, accepted_at);

CREATE INDEX game_results_user_id_ended_at_index ON site.game_results USING btree (user_id, ended_at);

CREATE INDEX questions_forum_theme_id_created_at_index ON site.questions USING btree (forum_theme_id, created_at);

CREATE INDEX questions_user_id_index ON site.questions USING btree (user_id);

CREATE INDEX roles_categorie_index ON site.roles USING btree (categorie);

CREATE INDEX room_invites_room_id_index ON site.room_invites USING btree (room_id);

CREATE INDEX room_messages_room_id_id_index ON site.room_messages USING btree (room_id, id);

CREATE INDEX rooms_type_index ON site.rooms USING btree (type);

CREATE INDEX signalements_signalable_type_signalable_id_index ON site.signalements USING btree (signalable_type, signalable_id);

CREATE INDEX signalements_statut_created_at_index ON site.signalements USING btree (statut, created_at);

CREATE INDEX suggestions_statut_created_at_index ON site.suggestions USING btree (statut, created_at);

ALTER TABLE ONLY contenu.actualites
    ADD CONSTRAINT actualites_auteur_id_foreign FOREIGN KEY (auteur_id) REFERENCES site.users(id) ON DELETE SET NULL;

ALTER TABLE ONLY contenu.chapters
    ADD CONSTRAINT chapters_course_id_foreign FOREIGN KEY (course_id) REFERENCES contenu.courses(id) ON DELETE CASCADE;

ALTER TABLE ONLY contenu.courses
    ADD CONSTRAINT courses_auteur_id_foreign FOREIGN KEY (auteur_id) REFERENCES site.users(id) ON DELETE CASCADE;

ALTER TABLE ONLY contenu.exam_parties
    ADD CONSTRAINT exam_parties_exam_id_foreign FOREIGN KEY (exam_id) REFERENCES contenu.exams(id) ON DELETE CASCADE;

ALTER TABLE ONLY contenu.exams
    ADD CONSTRAINT exams_auteur_id_foreign FOREIGN KEY (auteur_id) REFERENCES site.users(id) ON DELETE SET NULL;

ALTER TABLE ONLY contenu.exercises
    ADD CONSTRAINT exercises_chapter_id_foreign FOREIGN KEY (chapter_id) REFERENCES contenu.chapters(id) ON DELETE CASCADE;

ALTER TABLE ONLY contenu.lessons
    ADD CONSTRAINT lessons_chapter_id_foreign FOREIGN KEY (chapter_id) REFERENCES contenu.chapters(id) ON DELETE CASCADE;

ALTER TABLE ONLY contenu.ressources
    ADD CONSTRAINT ressources_ajoute_par_foreign FOREIGN KEY (ajoute_par) REFERENCES site.users(id) ON DELETE SET NULL;

ALTER TABLE ONLY contenu.telechargements
    ADD CONSTRAINT telechargements_user_id_foreign FOREIGN KEY (user_id) REFERENCES site.users(id) ON DELETE SET NULL;

ALTER TABLE ONLY game.match_players
    ADD CONSTRAINT match_players_match_id_fkey FOREIGN KEY (match_id) REFERENCES game.matches(id) ON DELETE CASCADE;

ALTER TABLE ONLY langues.events
    ADD CONSTRAINT events_language_code_fkey FOREIGN KEY (language_code) REFERENCES langues.languages(code) ON UPDATE CASCADE;

ALTER TABLE ONLY langues.levels
    ADD CONSTRAINT levels_language_code_fkey FOREIGN KEY (language_code) REFERENCES langues.languages(code) ON UPDATE CASCADE;

ALTER TABLE ONLY langues.words
    ADD CONSTRAINT words_level_id_fkey FOREIGN KEY (level_id) REFERENCES langues.levels(id) ON DELETE CASCADE;

ALTER TABLE ONLY site.abonnements
    ADD CONSTRAINT abonnements_abonne_id_foreign FOREIGN KEY (abonne_id) REFERENCES site.users(id) ON DELETE CASCADE;

ALTER TABLE ONLY site.abonnements
    ADD CONSTRAINT abonnements_suivi_id_foreign FOREIGN KEY (suivi_id) REFERENCES site.users(id) ON DELETE CASCADE;

ALTER TABLE ONLY site.answers
    ADD CONSTRAINT answers_masque_par_foreign FOREIGN KEY (masque_par) REFERENCES site.users(id) ON DELETE SET NULL;

ALTER TABLE ONLY site.answers
    ADD CONSTRAINT answers_parent_id_foreign FOREIGN KEY (parent_id) REFERENCES site.answers(id) ON DELETE CASCADE;

ALTER TABLE ONLY site.answers
    ADD CONSTRAINT answers_question_id_foreign FOREIGN KEY (question_id) REFERENCES site.questions(id) ON DELETE CASCADE;

ALTER TABLE ONLY site.answers
    ADD CONSTRAINT answers_user_id_foreign FOREIGN KEY (user_id) REFERENCES site.users(id) ON DELETE SET NULL;

ALTER TABLE ONLY site.auth_sessions
    ADD CONSTRAINT auth_sessions_user_id_fkey FOREIGN KEY (user_id) REFERENCES site.users(id) ON DELETE CASCADE;

ALTER TABLE ONLY site.cerveaux_mouvements
    ADD CONSTRAINT cerveaux_mouvements_user_id_foreign FOREIGN KEY (user_id) REFERENCES site.users(id) ON DELETE CASCADE;

ALTER TABLE ONLY site.comptes_sociaux
    ADD CONSTRAINT comptes_sociaux_user_id_foreign FOREIGN KEY (user_id) REFERENCES site.users(id) ON DELETE CASCADE;

ALTER TABLE ONLY site.demandes_ressources
    ADD CONSTRAINT demandes_ressources_ressource_id_foreign FOREIGN KEY (ressource_id) REFERENCES contenu.ressources(id) ON DELETE SET NULL;

ALTER TABLE ONLY site.demandes_ressources
    ADD CONSTRAINT demandes_ressources_traite_par_foreign FOREIGN KEY (traite_par) REFERENCES site.users(id) ON DELETE SET NULL;

ALTER TABLE ONLY site.demandes_ressources
    ADD CONSTRAINT demandes_ressources_user_id_foreign FOREIGN KEY (user_id) REFERENCES site.users(id) ON DELETE SET NULL;

ALTER TABLE ONLY site.direct_conversations
    ADD CONSTRAINT direct_conversations_user_one_id_foreign FOREIGN KEY (user_one_id) REFERENCES site.users(id) ON DELETE CASCADE;

ALTER TABLE ONLY site.direct_conversations
    ADD CONSTRAINT direct_conversations_user_two_id_foreign FOREIGN KEY (user_two_id) REFERENCES site.users(id) ON DELETE CASCADE;

ALTER TABLE ONLY site.direct_messages
    ADD CONSTRAINT direct_messages_direct_conversation_id_foreign FOREIGN KEY (direct_conversation_id) REFERENCES site.direct_conversations(id) ON DELETE CASCADE;

ALTER TABLE ONLY site.direct_messages
    ADD CONSTRAINT direct_messages_sender_id_foreign FOREIGN KEY (sender_id) REFERENCES site.users(id) ON DELETE SET NULL;

ALTER TABLE ONLY site.email_tokens
    ADD CONSTRAINT email_tokens_user_id_fkey FOREIGN KEY (user_id) REFERENCES site.users(id) ON DELETE CASCADE;

ALTER TABLE ONLY site.enrollments
    ADD CONSTRAINT enrollments_course_id_foreign FOREIGN KEY (course_id) REFERENCES contenu.courses(id) ON DELETE CASCADE;

ALTER TABLE ONLY site.enrollments
    ADD CONSTRAINT enrollments_derniere_lesson_id_foreign FOREIGN KEY (derniere_lesson_id) REFERENCES contenu.lessons(id) ON DELETE SET NULL;

ALTER TABLE ONLY site.enrollments
    ADD CONSTRAINT enrollments_user_id_foreign FOREIGN KEY (user_id) REFERENCES site.users(id) ON DELETE CASCADE;

ALTER TABLE ONLY site.exercise_attempts
    ADD CONSTRAINT exercise_attempts_exercise_id_foreign FOREIGN KEY (exercise_id) REFERENCES contenu.exercises(id) ON DELETE CASCADE;

ALTER TABLE ONLY site.exercise_attempts
    ADD CONSTRAINT exercise_attempts_user_id_foreign FOREIGN KEY (user_id) REFERENCES site.users(id) ON DELETE CASCADE;

ALTER TABLE ONLY site.forum_votes
    ADD CONSTRAINT forum_votes_user_id_foreign FOREIGN KEY (user_id) REFERENCES site.users(id) ON DELETE CASCADE;

ALTER TABLE ONLY site.friendships
    ADD CONSTRAINT friendships_requested_by_id_foreign FOREIGN KEY (requested_by_id) REFERENCES site.users(id) ON DELETE CASCADE;

ALTER TABLE ONLY site.friendships
    ADD CONSTRAINT friendships_user_one_id_foreign FOREIGN KEY (user_one_id) REFERENCES site.users(id) ON DELETE CASCADE;

ALTER TABLE ONLY site.friendships
    ADD CONSTRAINT friendships_user_two_id_foreign FOREIGN KEY (user_two_id) REFERENCES site.users(id) ON DELETE CASCADE;

ALTER TABLE ONLY site.game_results
    ADD CONSTRAINT game_results_user_id_foreign FOREIGN KEY (user_id) REFERENCES site.users(id) ON DELETE CASCADE;

ALTER TABLE ONLY site.lesson_progress
    ADD CONSTRAINT lesson_progress_lesson_id_foreign FOREIGN KEY (lesson_id) REFERENCES contenu.lessons(id) ON DELETE CASCADE;

ALTER TABLE ONLY site.lesson_progress
    ADD CONSTRAINT lesson_progress_user_id_foreign FOREIGN KEY (user_id) REFERENCES site.users(id) ON DELETE CASCADE;

ALTER TABLE ONLY site.questions
    ADD CONSTRAINT questions_forum_theme_id_foreign FOREIGN KEY (forum_theme_id) REFERENCES site.forum_themes(id) ON DELETE CASCADE;

ALTER TABLE ONLY site.questions
    ADD CONSTRAINT questions_masque_par_foreign FOREIGN KEY (masque_par) REFERENCES site.users(id) ON DELETE SET NULL;

ALTER TABLE ONLY site.questions
    ADD CONSTRAINT questions_user_id_foreign FOREIGN KEY (user_id) REFERENCES site.users(id) ON DELETE SET NULL;

ALTER TABLE ONLY site.reglages
    ADD CONSTRAINT reglages_modifie_par_foreign FOREIGN KEY (modifie_par) REFERENCES site.users(id) ON DELETE SET NULL;

ALTER TABLE ONLY site.role_user
    ADD CONSTRAINT role_user_role_id_foreign FOREIGN KEY (role_id) REFERENCES site.roles(id) ON DELETE CASCADE;

ALTER TABLE ONLY site.role_user
    ADD CONSTRAINT role_user_user_id_foreign FOREIGN KEY (user_id) REFERENCES site.users(id) ON DELETE CASCADE;

ALTER TABLE ONLY site.room_invites
    ADD CONSTRAINT room_invites_created_by_foreign FOREIGN KEY (created_by) REFERENCES site.users(id) ON DELETE CASCADE;

ALTER TABLE ONLY site.room_invites
    ADD CONSTRAINT room_invites_room_id_foreign FOREIGN KEY (room_id) REFERENCES site.rooms(id) ON DELETE CASCADE;

ALTER TABLE ONLY site.room_members
    ADD CONSTRAINT room_members_room_id_foreign FOREIGN KEY (room_id) REFERENCES site.rooms(id) ON DELETE CASCADE;

ALTER TABLE ONLY site.room_members
    ADD CONSTRAINT room_members_user_id_foreign FOREIGN KEY (user_id) REFERENCES site.users(id) ON DELETE CASCADE;

ALTER TABLE ONLY site.room_messages
    ADD CONSTRAINT room_messages_room_id_foreign FOREIGN KEY (room_id) REFERENCES site.rooms(id) ON DELETE CASCADE;

ALTER TABLE ONLY site.room_messages
    ADD CONSTRAINT room_messages_user_id_foreign FOREIGN KEY (user_id) REFERENCES site.users(id) ON DELETE SET NULL;

ALTER TABLE ONLY site.rooms
    ADD CONSTRAINT rooms_created_by_foreign FOREIGN KEY (created_by) REFERENCES site.users(id) ON DELETE CASCADE;

ALTER TABLE ONLY site.signalements
    ADD CONSTRAINT signalements_traite_par_foreign FOREIGN KEY (traite_par) REFERENCES site.users(id) ON DELETE SET NULL;

ALTER TABLE ONLY site.signalements
    ADD CONSTRAINT signalements_user_id_foreign FOREIGN KEY (user_id) REFERENCES site.users(id) ON DELETE SET NULL;

ALTER TABLE ONLY site.suggestions
    ADD CONSTRAINT suggestions_traite_par_foreign FOREIGN KEY (traite_par) REFERENCES site.users(id) ON DELETE SET NULL;

ALTER TABLE ONLY site.suggestions
    ADD CONSTRAINT suggestions_user_id_foreign FOREIGN KEY (user_id) REFERENCES site.users(id) ON DELETE SET NULL;

ALTER TABLE ONLY site.users
    ADD CONSTRAINT users_mute_par_foreign FOREIGN KEY (mute_par) REFERENCES site.users(id) ON DELETE SET NULL;

-- What a member shows: profile, roles, brains and experience. The XP rules
-- (points per lesson, exercise, course, forum post, game) live here, in one
-- place, for every route that shows a level.

CREATE VIEW site.members AS
 SELECT id,
    pseudo,
    avatar AS avatar_path,
    bio,
    (created_at AT TIME ZONE 'UTC'::text) AS member_since
   FROM site.users u;

CREATE VIEW site.member_roles AS
 SELECT ru.user_id,
    r.slug,
    r.nom AS name,
    r.categorie AS category,
    r.couleur AS color,
    r.niveau AS permission_level,
    r.ordre AS display_order
   FROM (site.role_user ru
     JOIN site.roles r ON ((r.id = ru.role_id)));

CREATE VIEW site.member_brains AS
 SELECT id AS user_id,
    (COALESCE(( SELECT sum(m.montant) AS sum
           FROM site.cerveaux_mouvements m
          WHERE (m.user_id = u.id)), (0)::bigint))::integer AS balance,
    cerveaux_publics AS public
   FROM site.users u;

CREATE VIEW site.member_xp AS
 WITH counts AS (
         SELECT u.id AS user_id,
            ( SELECT count(*) AS count
                   FROM site.lesson_progress lp
                  WHERE ((lp.user_id = u.id) AND (lp.completed_at IS NOT NULL))) AS lessons,
            ( SELECT count(DISTINCT ea.exercise_id) AS count
                   FROM site.exercise_attempts ea
                  WHERE ((ea.user_id = u.id) AND (ea.correct = true))) AS exercises,
            ( SELECT count(*) AS count
                   FROM site.enrollments en
                  WHERE ((en.user_id = u.id) AND (en.completed_at IS NOT NULL))) AS courses,
            ( SELECT count(*) AS count
                   FROM site.questions q
                  WHERE (q.user_id = u.id)) AS questions,
            ( SELECT count(*) AS count
                   FROM site.answers a
                  WHERE (a.user_id = u.id)) AS answers,
            ( SELECT count(*) AS count
                   FROM (site.answers a
                     JOIN site.questions q ON ((q.accepted_answer_id = a.id)))
                  WHERE (a.user_id = u.id)) AS accepted_answers,
            ( SELECT COALESCE(sum(LEAST((30)::bigint, ((j.parties * 3) + (j.victoires * 5)))), (0)::numeric) AS "coalesce"
                   FROM ( SELECT count(*) AS parties,
                            count(*) FILTER (WHERE ((gr.outcome)::text = 'win'::text)) AS victoires
                           FROM site.game_results gr
                          WHERE (gr.user_id = u.id)
                          GROUP BY ((gr.ended_at)::date)) j) AS game_xp
           FROM site.users u
        ), totals AS (
         SELECT counts.user_id,
            (((((((((counts.lessons * 10) + (counts.exercises * 8)) + (counts.courses * 40)) + (counts.questions * 4)) + (counts.answers * 4)) + (counts.accepted_answers * 12)))::numeric + counts.game_xp))::integer AS total
           FROM counts
        ), levels AS (
         SELECT totals.user_id,
            totals.total,
            (floor((((1)::numeric + sqrt(((1)::numeric + ((totals.total)::numeric * 0.08)))) / (2)::numeric)))::integer AS n
           FROM totals
        )
 SELECT user_id,
    total,
        CASE
            WHEN (((50 * n) * (n + 1)) <= total) THEN (n + 1)
            WHEN (((50 * (n - 1)) * n) > total) THEN (n - 1)
            ELSE n
        END AS level
   FROM levels;

-- pulsars_api reads and writes every table; it creates nothing.
GRANT USAGE ON SCHEMA site, contenu, game, langues TO pulsars_api;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA site, contenu, game, langues TO pulsars_api;
GRANT USAGE, SELECT, UPDATE ON ALL SEQUENCES IN SCHEMA site, contenu, game, langues TO pulsars_api;
GRANT EXECUTE ON ALL FUNCTIONS IN SCHEMA contenu TO pulsars_api;
ALTER DEFAULT PRIVILEGES FOR ROLE pulsars_owner IN SCHEMA site, contenu, game, langues
    GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO pulsars_api;
ALTER DEFAULT PRIVILEGES FOR ROLE pulsars_owner IN SCHEMA site, contenu, game, langues
    GRANT USAGE, SELECT, UPDATE ON SEQUENCES TO pulsars_api;
