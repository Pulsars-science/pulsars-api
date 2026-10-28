-- Demo data for local development and CI. NOT production data.
--
-- Content comes from the open-source Laravel seeders (same values as the
-- examples in docs/contract/openapi.yaml), plus deliberately invisible rows
-- (drafts, future publication, answers, corrections) so tests can check that
-- the api_read views hide them. Accounts are fictional.

SET ROLE pulsars_site;
SET search_path = site;

INSERT INTO users (id, pseudo, name, email, bio, niveau_scolaire, classe, locale, friend_token, password, created_at, updated_at) VALUES
    (1, 'jikuji', 'jikuji', 'demo-admin@example.invalid', NULL, NULL, NULL, 'fr', 'demofriendtokenadmin00000', 'not-a-real-hash', '2026-08-07 14:44:15', '2026-08-07 14:44:15'),
    (2, 'ada', 'ada', 'demo-ada@example.invalid', 'Mathématiques et café.', 'lycee', 'Terminale B (private)', 'en', 'demofriendtokenada0000000', 'not-a-real-hash', '2026-08-10 09:00:00', '2026-08-10 09:00:00'),
    (3, 'emmy', 'emmy', 'demo-emmy@example.invalid', NULL, 'superieur', NULL, NULL, 'demofriendtokenemmy000000', 'not-a-real-hash', '2026-08-11 10:00:00', '2026-08-11 10:00:00');

INSERT INTO roles (id, slug, nom, categorie, couleur, niveau, ordre) VALUES
    (1, 'super-admin', 'Super admin', 'permission', '#d1544f', 3, 0),
    (2, 'admin', 'Admin', 'permission', '#c98a34', 2, 1),
    (3, 'moderateur', 'Moderateur', 'permission', '#4f7df0', 1, 2),
    (4, 'mathematiques', 'Mathematiques', 'matiere', '#4f7df0', 0, 3),
    (5, 'membre-fondateur', 'Membre fondateur', 'distinction', '#c98a34', 0, 15);

INSERT INTO role_user (role_id, user_id) VALUES (1, 1), (4, 2), (5, 2), (3, 3);

-- Courses: two published, one draft, one scheduled in the future.
INSERT INTO courses (id, titre, slug, description, theme, difficulte, duree_estimee, tags, auteur_id, statut, published_at, created_at, updated_at) VALUES
    (1, 'Les bases du calcul', 'les-bases-du-calcul',
     'Priorités opératoires, fractions, puissances et pourcentages. De quoi calculer juste, sans calculatrice. Aucun prérequis.',
     'Mathématiques', 'debutant', 60, '["maths","calcul"]', 1, 'publie',
     '2026-08-17 09:27:34', '2026-08-07 14:44:24', '2026-08-17 09:27:34'),
    (2, 'Le tableau périodique des éléments', 'chimie-le-tableau-periodique',
     'Découvre comment les chimistes ont organisé tous les éléments connus en un tableau unique, et pourquoi ce classement permet encore aujourd''hui de prédire les propriétés de la matière.',
     'Chimie – La matière', 'intermediaire', 45, '["chimie","tableau-périodique","lycée"]', 1, 'publie',
     '2026-08-18 12:56:30', '2026-08-07 14:44:25', '2026-08-18 12:56:30'),
    (3, 'Brouillon de démonstration', 'demo-draft-course', 'Jamais visible par l''API.',
     'Mathématiques – Analyse', 'avance', 30, '["supérieur"]', 1, 'brouillon',
     NULL, '2026-08-20 10:00:00', '2026-08-20 10:00:00'),
    (4, 'Cours programmé', 'demo-future-course', 'Publié dans le futur, donc invisible.',
     'Physique – Optique', 'debutant', 30, '[]', 1, 'publie',
     now() + interval '30 days', '2026-08-20 10:00:00', '2026-08-20 10:00:00');

INSERT INTO chapters (id, course_id, titre, ordre) VALUES
    (1, 1, 'L''ordre des opérations', 1),
    (2, 1, 'Puissances et pourcentages', 2),
    (3, 2, 'Une table pour tous les éléments', 1),
    (4, 3, 'Chapitre brouillon', 1);

INSERT INTO lessons (id, chapter_id, titre, slug, contenu_md, ordre, updated_at) VALUES
    (1, 1, 'Les priorités opératoires', 'priorites-operatoires',
     E'Combien vaut `2 + 3 x 4` ?\n\n## La règle\n\n1. Ce qui est entre **parenthèses**\n2. Les **puissances**\n3. Les **multiplications et divisions**\n4. Les **additions et soustractions**\n\nDonc `2 + 3 x 4` = `2 + 12` = **14**.',
     1, '2026-08-13 18:59:10'),
    (2, 1, 'Les fractions', 'les-fractions',
     E'Une fraction, c''est une division qu''on n''a pas terminée. `3/4` veut dire « 3 divisé par 4 », soit 0,75.',
     2, '2026-08-13 18:50:57'),
    (3, 2, 'Les puissances', 'les-puissances', E'`2^3` = `2 x 2 x 2` = 8.', 1, '2026-08-13 18:57:45'),
    (4, 2, 'Les pourcentages', 'les-pourcentages', E'Prendre 30 % d''un nombre, c''est le multiplier par 0,3.', 2, '2026-08-13 18:50:57'),
    (5, 3, 'Mendeleïev et son tableau', 'mendeleiev', E'En 1869, Dmitri Mendeleïev classe les 63 éléments connus.', 1, '2026-08-18 12:56:30'),
    (6, 4, 'Leçon brouillon', 'brouillon', E'Invisible.', 1, '2026-08-20 10:00:00');

-- Every exercise carries its answer, so tests can prove the views drop it.
INSERT INTO exercises (id, chapter_id, type, titre, question_md, donnees, solution_md, ordre, updated_at) VALUES
    (1, 1, 'qcm', 'Appliquer les priorités', 'Combien vaut `12 - 2 x 3 + 1` ?',
     '{"options":[{"id":"a","texte":"31"},{"id":"b","texte":"7"},{"id":"c","texte":"11"},{"id":"d","texte":"4"}],"correctes":["b"]}',
     'La réponse est **7**. SOLUTION-MARKER-1', 1, '2026-08-13 18:59:10'),
    (2, 1, 'reponse_courte', 'Simplifier une fraction',
     E'Simplifie la fraction `18/24` au maximum.\n\nÉcris ta réponse sous la forme `a/b`.',
     '{"acceptees":["3/4","0.75"]}', 'La réponse est `3/4`. SOLUTION-MARKER-2', 2, '2026-08-13 18:59:10'),
    (3, 2, 'qcm', 'Vrai ou faux sur les puissances', 'Parmi ces égalités, lesquelles sont **vraies** ?',
     '{"options":[{"id":"a","texte":"2^3 x 2^4 = 2^7"},{"id":"b","texte":"2^3 x 2^4 = 2^12"},{"id":"c","texte":"(-3)^2 = 9"},{"id":"d","texte":"-3^2 = 9"},{"id":"e","texte":"5^0 = 1"}],"correctes":["a","c","e"]}',
     'Les égalités vraies sont a, c et e. SOLUTION-MARKER-3', 1, '2026-08-13 18:59:10'),
    (4, 2, 'reponse_courte', 'Calculer un pourcentage', 'Un article coûte 65 €. Il est soldé à -20 %. Quel est son nouveau prix ?',
     '{"acceptees":["52","52 euros","52e"]}', '65 x 0,8 = 52. SOLUTION-MARKER-4', 2, '2026-08-13 18:59:10'),
    (5, 2, 'reponse_libre', 'Le piège des pourcentages', 'Un prix augmente de 10 % puis baisse de 10 %. Revient-il au prix de départ ? Explique.',
     '[]', 'Non : 1,1 x 0,9 = 0,99. SOLUTION-MARKER-5', 3, '2026-08-13 18:59:10'),
    (6, 4, 'qcm', 'Exercice brouillon', 'Invisible ?',
     '{"options":[{"id":"a","texte":"oui"}],"correctes":["a"]}', 'SOLUTION-MARKER-6', 1, '2026-08-20 10:00:00');

-- Exams: one published and hosted, one external competitive paper, one draft.
INSERT INTO exams (id, titre, slug, type, concours, matiere, pays, niveau, filieres, session, duree_minutes, points_total, description, statut, fichier_pdf, nom_fichier_original, fichier_corrige, url_officielle, url_source, prompt_ia, published_at, auteur_id, created_at, updated_at) VALUES
    (2, 'Baccalauréat général 2026 — Mathématiques, spécialité (jour 1)', 'baccalaureat-general-2026-mathematiques-specialite-jour-1',
     'bac', NULL, 'mathematiques', 'FR', 'Terminale', NULL, 'Session 2026 — 16 juin 2026', 240, 20,
     'Sujet de spécialité mathématiques du baccalauréat général, session 2026, épreuve du 16 juin (sujet 26-MATJ1ME1).',
     'publie', 'examens/bac-2026-maths-jour-1.pdf', 'sujet.pdf', 'examens/bac-2026-maths-jour-1-corrige.pdf', NULL, NULL,
     'PROMPT-MARKER', '2026-08-13 09:32:04', 1, '2026-08-13 09:00:00', '2026-08-13 09:32:04'),
    (5, 'Centrale-Supélec 2026 — Mathématiques 1 (MP, MPI)', 'centrale-supelec-2026-mathematiques-1-mp-mpi',
     'concours', 'centrale-supelec', 'mathematiques', 'FR', 'CPGE', '["MP","MPI"]', '2026', NULL, NULL,
     'Sujet officiel de Mathématiques 1 du Concours Centrale-Supélec 2026, filières MP, MPI.',
     'publie', NULL, NULL, NULL,
     'https://www.concours-centrale-supelec.fr/sites/default/files/documents/M098.pdf',
     'https://www.concours-centrale-supelec.fr/sujets-rapports',
     NULL, '2026-08-13 11:03:10', 1, '2026-08-13 11:00:00', '2026-08-13 11:03:10'),
    (1, 'Diplôme national du brevet 2026 — Mathématiques, série générale', 'diplome-national-du-brevet-2026-mathematiques-serie-generale',
     'brevet', NULL, 'mathematiques', 'FR', '3e', NULL, '2026', 120, 20, 'Brouillon en attente de relecture.',
     'brouillon', 'examens/dnb-2026.pdf', 'mathsgen.pdf', NULL, NULL, NULL, NULL, NULL, 1, '2026-08-13 08:00:00', '2026-08-13 08:00:00');

INSERT INTO exam_parties (id, exam_id, ordre, titre, enonce_md, corrige_md, points) VALUES
    (46, 2, 1, 'Exercice 1 — Probabilités : traversée en navire',
     'Un navire assure la liaison entre deux ports de la Méditerranée.', 'CORRIGE-MARKER-46', 5),
    (47, 2, 2, 'Exercice 2 — Vrai/faux : géométrie dans l''espace et dénombrement',
     'Pour chacune des affirmations suivantes, indiquer si elle est vraie ou fausse.', 'CORRIGE-MARKER-47', 4),
    (10, 1, 1, 'Partie 1 — Automatismes', 'Donner une écriture du nombre `0,75` sous la forme d''une fraction.', 'CORRIGE-MARKER-10', 6);

-- Resources: hosted with licence, hosted without licence, link, pending.
INSERT INTO ressources (id, titre, slug, description, categorie, langue, auteur_nom, licence, fichier, nom_fichier_original, taille, lien, statut, ajoute_par, created_at, updated_at) VALUES
    (1, 'Tripodal 4-Pyridyl-Derived Host Ligands and Their Metallo-Supramolecular Chemistry: Stella Octangula and Bowl-Shaped Assemblies',
     'article-chimie-inorganique-ic901972h', 'Un article de recherche en chimie inorganique, à télécharger en PDF.',
     'Chimie', 'en', NULL, NULL, 'ressources/ic901972h.pdf', 'ic901972h.pdf', 3451711, NULL, 'publie', 1,
     '2026-08-07 14:44:26', '2026-08-07 14:44:26'),
    (2, 'Abstract Algebra', 'misselbooks-abstract-algebra',
     'Un manuel d’algèbre abstraite en anglais (220 pages) : groupes, anneaux, corps, avec des applications à la cryptographie. Écrit et publié en accès libre par Andrew Misseldine.',
     'Mathématiques', 'en', 'Andrew Misseldine', 'cc0-1.0', 'ressources/misselbooks-abstract-algebra.pdf', 'misselbooks-abstract-algebra.pdf', 1543021,
     'https://github.com/emisseldine/MisselBooks/blob/main/AbstractAlgebra/Abstract%20Algebra.pdf', 'publie', 1,
     '2026-08-07 14:44:26', '2026-08-07 14:44:26'),
    (3, 'Calculus I', 'misselbooks-calculus-i', 'Un cours d’analyse en anglais : limites, dérivées, intégrales.',
     'Mathématiques', 'en', 'Andrew Misseldine', 'cc0-1.0', NULL, NULL, 0,
     'https://github.com/emisseldine/MisselBooks/blob/main/Calculus/CalculusI.pdf', 'publie', 1,
     '2026-08-08 10:00:00', '2026-08-08 10:00:00'),
    (4, 'Ressource en attente', 'demo-pending-resource', 'Invisible.', 'Divers', 'fr', NULL, NULL,
     'ressources/pending.pdf', 'pending.pdf', 1000, NULL, 'en_attente', 2, '2026-08-20 10:00:00', '2026-08-20 10:00:00');

-- English translations, including fields the views must drop.
INSERT INTO contenu_translations (traduisible_type, traduisible_id, langue, champs, source_hash, created_at, updated_at) VALUES
    ('course', 1, 'en', '{"titre":"The basics of the calculation","description":"Operational priorities, fractions, powers and percentages. Everything you need to calculate correctly, without a calculator. No prior knowledge required."}', 'demo', now(), now()),
    ('course', 1, 'ru', '{"titre":"Основы вычислений","description":""}', 'demo', now(), now()),
    ('chapter', 1, 'en', '{"titre":"The order of operations"}', 'demo', now(), now()),
    ('lesson', 1, 'en', '{"titre":"Operational priorities","contenu_md":"What is `2 + 3 x 4` equal to?\n\n## The rule\n\n…"}', 'demo', now(), now()),
    ('lesson', 2, 'en', '{"titre":"Fractions","contenu_md":""}', 'demo', now(), now()),
    ('exercise', 1, 'en', '{"titre":"Set priorities","question_md":"What is the value of `12 - 2 x 3 + 1`?","solution_md":"The answer is **7**. SOLUTION-MARKER-EN-1"}', 'demo', now(), now()),
    -- Options partly translated (b empty, d and e missing: original text), and
    -- a stray `correctes` key the view must never read.
    ('exercise', 3, 'en', '{"titre":"True or false on powers","question_md":"Which of these equalities are **true**?","options":{"a":"OPTION-EN-A","b":"","c":"OPTION-EN-C"},"correctes":["a","c","e"]}', 'demo', now(), now()),
    ('exam_partie', 46, 'en', '{"titre":"Exercise 1 — Probability: ship crossing","enonce_md":"A ship links two Mediterranean ports.","corrige_md":"CORRIGE-MARKER-EN-46"}', 'demo', now(), now()),
    ('exam_partie', 10, 'en', '{"titre":"Part 1","enonce_md":"Write 0.75 as a fraction.","corrige_md":"CORRIGE-MARKER-EN-10"}', 'demo', now(), now()),
    ('ressource', 2, 'en', '{"titre":"Abstract Algebra","categorie":"Mathematics","description":"A 220-page abstract algebra textbook: groups, rings and fields, with applications to cryptography. Written and released openly by Andrew Misseldine."}', 'demo', now(), now());

-- Activity, so member_xp has something to compute.
-- ada: 3 lessons (30) + 2 distinct exercises solved (16) + 1 course (40)
--      + 1 answer (4) accepted (12) = 102 XP, level 2.
INSERT INTO lesson_progress (user_id, lesson_id) VALUES (2, 1), (2, 2), (2, 3);
INSERT INTO exercise_attempts (user_id, exercise_id, reponse, correct) VALUES
    (2, 1, '{"options":["b"]}', true),
    (2, 1, '{"options":["b"]}', true),
    (2, 2, '{"texte":"3/4"}', true),
    (2, 3, '{"options":["a"]}', false);
INSERT INTO enrollments (user_id, course_id, completed_at) VALUES (2, 1, now());
INSERT INTO forum_themes (id, nom, slug) VALUES (1, 'Mathématiques', 'mathematiques');
INSERT INTO questions (id, forum_theme_id, user_id, titre, slug, corps, created_at) VALUES
    (1, 1, 3, 'Pourquoi 5^0 = 1 ?', 'pourquoi-5-puissance-0', 'Je ne comprends pas.', now());
INSERT INTO answers (id, question_id, user_id, corps, created_at) VALUES
    (1, 1, 2, 'Parce que 5^1 / 5^1 = 5^(1-1).', now());
UPDATE questions SET accepted_answer_id = 1 WHERE id = 1;

-- Brains: 5 per completed lesson for ada (15); emmy hides her balance.
INSERT INTO cerveaux_mouvements (user_id, montant, raison, cle, details, created_at) VALUES
    (2, 5, 'lecon', 'lecon:1', '{"lesson_id": 1}', now()),
    (2, 5, 'lecon', 'lecon:2', '{"lesson_id": 2}', now()),
    (2, 5, 'lecon', 'lecon:3', '{"lesson_id": 3}', now());
UPDATE users SET cerveaux_publics = false WHERE id = 3;

-- Keep sequences ahead of the explicit ids above.
-- Stored files (bytea, as on the Laravel `local` disk): the published
-- statement and resource, and what must never be served: a correction, a
-- draft exam's PDF, a pending resource's file, a file on another disk.
INSERT INTO fichiers (id, disque, chemin, contenu, type_mime, taille, created_at, updated_at) VALUES
    (1, 'local', 'examens/bac-2026-maths-jour-1.pdf', convert_to('%PDF-1.4 STATEMENT-BAC-2026 0123456789', 'UTF8'), 'application/pdf', 38, '2026-08-13 09:00:00', '2026-08-13 09:32:04'),
    (2, 'local', 'examens/bac-2026-maths-jour-1-corrige.pdf', convert_to('%PDF-1.4 FILE-CORRIGE-MARKER', 'UTF8'), 'application/pdf', 28, '2026-08-13 09:00:00', '2026-08-13 09:00:00'),
    (3, 'local', 'examens/dnb-2026.pdf', convert_to('%PDF-1.4 FILE-DRAFT-MARKER', 'UTF8'), 'application/pdf', 26, '2026-08-13 08:00:00', '2026-08-13 08:00:00'),
    (4, 'local', 'ressources/ic901972h.pdf', convert_to('%PDF-1.4 RESOURCE-IC901972H', 'UTF8'), 'application/pdf', 27, '2026-08-07 14:44:26', '2026-08-07 14:44:26'),
    (5, 'local', 'ressources/pending.pdf', convert_to('%PDF-1.4 FILE-PENDING-MARKER', 'UTF8'), 'application/pdf', 28, '2026-08-20 10:00:00', '2026-08-20 10:00:00'),
    (6, 'public', 'ressources/ic901972h.pdf', convert_to('FILE-OTHER-DISK-MARKER', 'UTF8'), 'text/html', 22, '2026-08-07 14:44:26', '2026-08-07 14:44:26');

-- News: one published (translated), one draft, one scheduled in the future.
INSERT INTO actualites (id, titre, slug, chapeau, contenu_md, published_at, auteur_id, created_at, updated_at) VALUES
    (1, 'Le russe arrive sur Pulsars', 'le-russe-arrive-sur-pulsars',
     'L''interface parle désormais trois langues.', E'Après le français et l''anglais, **le russe**.\n\nBonne lecture !',
     '2026-10-20 08:00:00', 1, '2026-10-19 18:00:00', '2026-10-20 08:00:00'),
    (2, 'Annonce en préparation', 'demo-draft-news', 'NEWS-DRAFT-MARKER', 'NEWS-DRAFT-MARKER',
     NULL, 1, '2026-10-21 09:00:00', '2026-10-21 09:00:00'),
    (3, 'Annonce programmée', 'demo-future-news', 'NEWS-FUTURE-MARKER', 'NEWS-FUTURE-MARKER',
     '2099-01-01 00:00:00', 1, '2026-10-21 09:00:00', '2026-10-21 09:00:00');

INSERT INTO contenu_translations (traduisible_type, traduisible_id, langue, champs, source_hash, created_at, updated_at) VALUES
    ('actualite', 1, 'en', '{"titre":"Russian comes to Pulsars","chapeau":"","contenu_md":"After French and English, **Russian**.\n\nEnjoy!"}', 'demo', now(), now()),
    ('actualite', 2, 'en', '{"titre":"NEWS-DRAFT-MARKER","contenu_md":"NEWS-DRAFT-MARKER"}', 'demo', now(), now());

SELECT setval('site.users_id_seq', (SELECT max(id) FROM users));
SELECT setval('site.roles_id_seq', (SELECT max(id) FROM roles));
SELECT setval('site.courses_id_seq', (SELECT max(id) FROM courses));
SELECT setval('site.chapters_id_seq', (SELECT max(id) FROM chapters));
SELECT setval('site.lessons_id_seq', (SELECT max(id) FROM lessons));
SELECT setval('site.exercises_id_seq', (SELECT max(id) FROM exercises));
SELECT setval('site.exams_id_seq', (SELECT max(id) FROM exams));
SELECT setval('site.exam_parties_id_seq', (SELECT max(id) FROM exam_parties));
SELECT setval('site.ressources_id_seq', (SELECT max(id) FROM ressources));
SELECT setval('site.actualites_id_seq', (SELECT max(id) FROM actualites));
SELECT setval('site.fichiers_id_seq', (SELECT max(id) FROM fichiers));
SELECT setval('site.forum_themes_id_seq', (SELECT max(id) FROM forum_themes));
SELECT setval('site.questions_id_seq', (SELECT max(id) FROM questions));
SELECT setval('site.answers_id_seq', (SELECT max(id) FROM answers));

RESET ROLE;
