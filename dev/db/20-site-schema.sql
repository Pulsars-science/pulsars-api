-- Stand-in for the Laravel `site` schema: only the tables and columns that
-- docs/contract/views.sql reads, with the types Laravel's PostgreSQL grammar
-- produces. The real schema belongs to the Laravel repository; when it
-- changes in a way the views notice, update this file too.

SET ROLE pulsars_site;
SET search_path = site;

CREATE TABLE users (
    id bigserial PRIMARY KEY,
    pseudo varchar(255) NOT NULL UNIQUE,
    name varchar(255) NOT NULL,
    email varchar(255) NOT NULL UNIQUE,
    avatar varchar(255),
    bio varchar(280),
    objectif json,
    niveau_scolaire varchar(255),
    classe varchar(255),
    onboarding_at timestamp(0),
    locale varchar(5),
    friend_token varchar(32),
    email_verified_at timestamp(0),
    password varchar(255),
    two_factor_secret text,
    two_factor_recovery_codes text,
    two_factor_confirmed_at timestamp(0),
    remember_token varchar(100),
    mute_jusqu_a timestamp(0),
    mute_motif varchar(300),
    mute_par bigint,
    cerveaux_publics boolean NOT NULL DEFAULT true,
    created_at timestamp(0),
    updated_at timestamp(0)
);

CREATE TABLE roles (
    id bigserial PRIMARY KEY,
    slug varchar(255) NOT NULL UNIQUE,
    nom varchar(255) NOT NULL,
    categorie varchar(255) NOT NULL DEFAULT 'distinction',
    couleur varchar(7) NOT NULL DEFAULT '#8a8a8f',
    niveau smallint NOT NULL DEFAULT 0,
    description varchar(255),
    ordre smallint NOT NULL DEFAULT 0
);

CREATE TABLE role_user (
    id bigserial PRIMARY KEY,
    role_id bigint NOT NULL REFERENCES roles ON DELETE CASCADE,
    user_id bigint NOT NULL REFERENCES users ON DELETE CASCADE,
    created_at timestamp(0) DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (role_id, user_id)
);

CREATE TABLE courses (
    id bigserial PRIMARY KEY,
    titre varchar(255) NOT NULL,
    slug varchar(255) NOT NULL UNIQUE,
    description text NOT NULL,
    theme varchar(255) NOT NULL DEFAULT 'Divers',
    difficulte varchar(255) NOT NULL DEFAULT 'debutant',
    duree_estimee smallint,
    tags json,
    auteur_id bigint NOT NULL REFERENCES users ON DELETE CASCADE,
    statut varchar(255) NOT NULL DEFAULT 'brouillon',
    published_at timestamp(0),
    vues bigint NOT NULL DEFAULT 0,
    created_at timestamp(0),
    updated_at timestamp(0)
);

CREATE TABLE chapters (
    id bigserial PRIMARY KEY,
    course_id bigint NOT NULL REFERENCES courses ON DELETE CASCADE,
    titre varchar(255) NOT NULL,
    ordre smallint NOT NULL DEFAULT 0,
    created_at timestamp(0),
    updated_at timestamp(0)
);

CREATE TABLE lessons (
    id bigserial PRIMARY KEY,
    chapter_id bigint NOT NULL REFERENCES chapters ON DELETE CASCADE,
    titre varchar(255) NOT NULL,
    slug varchar(255) NOT NULL,
    contenu_md text NOT NULL,
    ordre smallint NOT NULL DEFAULT 0,
    created_at timestamp(0),
    updated_at timestamp(0),
    UNIQUE (chapter_id, slug)
);

CREATE TABLE exercises (
    id bigserial PRIMARY KEY,
    chapter_id bigint NOT NULL REFERENCES chapters ON DELETE CASCADE,
    type varchar(255) NOT NULL DEFAULT 'qcm',
    titre varchar(255) NOT NULL,
    question_md text NOT NULL,
    donnees json,
    solution_md text,
    ordre smallint NOT NULL DEFAULT 0,
    created_at timestamp(0),
    updated_at timestamp(0)
);

CREATE TABLE exams (
    id bigserial PRIMARY KEY,
    titre varchar(255) NOT NULL,
    slug varchar(255) NOT NULL UNIQUE,
    type varchar(255) NOT NULL,
    concours varchar(255),
    matiere varchar(255) NOT NULL,
    pays varchar(2) NOT NULL DEFAULT 'FR',
    niveau varchar(255) NOT NULL,
    filieres json,
    session varchar(255),
    duree_minutes smallint,
    points_total smallint,
    description text,
    statut varchar(255) NOT NULL DEFAULT 'attente_import',
    fichier_pdf varchar(255),
    nom_fichier_original varchar(255),
    fichier_corrige varchar(255),
    url_officielle varchar(500),
    url_source varchar(500),
    prompt_ia text,
    erreur_ia text,
    analyse_at timestamp(0),
    published_at timestamp(0),
    auteur_id bigint REFERENCES users ON DELETE SET NULL,
    vues bigint NOT NULL DEFAULT 0,
    telechargements bigint NOT NULL DEFAULT 0,
    created_at timestamp(0),
    updated_at timestamp(0)
);

CREATE TABLE exam_parties (
    id bigserial PRIMARY KEY,
    exam_id bigint NOT NULL REFERENCES exams ON DELETE CASCADE,
    ordre smallint NOT NULL DEFAULT 0,
    titre varchar(255) NOT NULL,
    enonce_md text NOT NULL,
    corrige_md text NOT NULL,
    points decimal(5, 2)
);

CREATE TABLE ressources (
    id bigserial PRIMARY KEY,
    titre varchar(255) NOT NULL,
    slug varchar(255) NOT NULL UNIQUE,
    description text,
    categorie varchar(255),
    langue varchar(5),
    auteur_nom varchar(200),
    licence varchar(60),
    licence_personnalisee varchar(200),
    fichier varchar(255),
    nom_fichier_original varchar(255),
    taille bigint NOT NULL DEFAULT 0,
    lien varchar(500),
    statut varchar(255) NOT NULL DEFAULT 'publie',
    ajoute_par bigint REFERENCES users ON DELETE SET NULL,
    telechargements integer NOT NULL DEFAULT 0,
    vues bigint NOT NULL DEFAULT 0,
    created_at timestamp(0),
    updated_at timestamp(0)
);

CREATE TABLE contenu_translations (
    id bigserial PRIMARY KEY,
    traduisible_type varchar(255) NOT NULL,
    traduisible_id bigint NOT NULL,
    langue varchar(5) NOT NULL,
    champs json NOT NULL,
    source_hash varchar(64) NOT NULL,
    created_at timestamp(0),
    updated_at timestamp(0),
    UNIQUE (traduisible_type, traduisible_id, langue)
);

CREATE TABLE lesson_progress (
    id bigserial PRIMARY KEY,
    user_id bigint NOT NULL REFERENCES users ON DELETE CASCADE,
    lesson_id bigint NOT NULL REFERENCES lessons ON DELETE CASCADE,
    completed_at timestamp(0) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (user_id, lesson_id)
);

CREATE TABLE exercise_attempts (
    id bigserial PRIMARY KEY,
    user_id bigint NOT NULL REFERENCES users ON DELETE CASCADE,
    exercise_id bigint NOT NULL REFERENCES exercises ON DELETE CASCADE,
    reponse json NOT NULL,
    correct boolean,
    created_at timestamp(0) NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX ON exercise_attempts (user_id, exercise_id);

CREATE TABLE enrollments (
    id bigserial PRIMARY KEY,
    user_id bigint NOT NULL REFERENCES users ON DELETE CASCADE,
    course_id bigint NOT NULL REFERENCES courses ON DELETE CASCADE,
    started_at timestamp(0) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at timestamp(0),
    derniere_lesson_id bigint,
    created_at timestamp(0),
    updated_at timestamp(0),
    UNIQUE (user_id, course_id)
);

CREATE TABLE questions (
    id bigserial PRIMARY KEY,
    forum_theme_id bigint,
    user_id bigint REFERENCES users ON DELETE SET NULL,
    titre varchar(255) NOT NULL,
    slug varchar(255) NOT NULL UNIQUE,
    corps text NOT NULL,
    vues integer NOT NULL DEFAULT 0,
    accepted_answer_id bigint,
    masque_le timestamp(0),
    deleted_at timestamp(0),
    created_at timestamp(0),
    updated_at timestamp(0)
);

CREATE TABLE answers (
    id bigserial PRIMARY KEY,
    question_id bigint NOT NULL REFERENCES questions ON DELETE CASCADE,
    user_id bigint REFERENCES users ON DELETE SET NULL,
    parent_id bigint,
    corps text NOT NULL,
    masque_le timestamp(0),
    deleted_at timestamp(0),
    created_at timestamp(0),
    updated_at timestamp(0)
);

-- Game webhook receiver (docs/contract/webhooks.md §4 and §5).
CREATE TABLE game_events (
    id bigserial PRIMARY KEY,
    event_id uuid NOT NULL UNIQUE,
    type varchar(255) NOT NULL,
    payload json NOT NULL,
    received_at timestamp(0) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    processed_at timestamp(0)
);

CREATE TABLE game_results (
    id bigserial PRIMARY KEY,
    game_event_id bigint NOT NULL REFERENCES game_events ON DELETE CASCADE,
    match_id uuid NOT NULL,
    user_id bigint NOT NULL REFERENCES users ON DELETE CASCADE,
    mode varchar(255) NOT NULL,
    ranked boolean NOT NULL,
    outcome varchar(255) NOT NULL,
    ended_at timestamp(0) NOT NULL,
    UNIQUE (match_id, user_id)
);

-- Brains ledger (docs/contract/brains.md §1).
CREATE TABLE cerveaux_mouvements (
    id bigserial PRIMARY KEY,
    user_id bigint NOT NULL REFERENCES users ON DELETE CASCADE,
    montant integer NOT NULL,
    raison varchar(32) NOT NULL,
    cle varchar(120) NOT NULL,
    details json,
    created_at timestamp(0),
    UNIQUE (user_id, cle)
);

RESET ROLE;
