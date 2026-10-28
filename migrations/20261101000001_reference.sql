-- Reference data every server needs: roles, forum themes, licences,
-- subjects, official exam sources, and the first language taught.

-- The roles (Laravel's RoleSeeder): permissions in hierarchy, then
-- subjects, then distinctions. `niveau` orders the permissions.
INSERT INTO site.roles (slug, nom, categorie, couleur, niveau, description, ordre) VALUES
    ('super-admin', 'Super admin', 'permission', '#d1544f', 3, 'Controle total du site.', 0),
    ('admin', 'Admin', 'permission', '#c98a34', 2, 'Gestion des cours, des salons et des comptes.', 1),
    ('moderateur', 'Moderateur', 'permission', '#4f7df0', 1, 'Moderation des contenus et des salons.', 2),
    ('mathematiques', 'Mathematiques', 'matiere', '#4f7df0', 0, NULL, 3),
    ('physique', 'Physique', 'matiere', '#6a5acd', 0, NULL, 4),
    ('chimie', 'Chimie', 'matiere', '#3fa66b', 0, NULL, 5),
    ('svt', 'SVT', 'matiere', '#2e9e6b', 0, NULL, 6),
    ('informatique', 'Informatique', 'matiere', '#0f9b9b', 0, NULL, 7),
    ('francais', 'Francais', 'matiere', '#c0567a', 0, NULL, 8),
    ('langues', 'Langues', 'matiere', '#c98a34', 0, NULL, 9),
    ('histoire-geo', 'Histoire-Geo', 'matiere', '#a5743a', 0, NULL, 10),
    ('philosophie', 'Philosophie', 'matiere', '#8a8a8f', 0, NULL, 11),
    ('economie', 'Economie', 'matiere', '#3f8fa6', 0, NULL, 12),
    ('arts', 'Arts', 'matiere', '#c0567a', 0, NULL, 13),
    ('contributeur', 'Contributeur', 'distinction', '#3fa66b', 0, 'Redige des cours.', 14),
    ('membre-fondateur', 'Membre fondateur', 'distinction', '#c98a34', 0, 'Present depuis les debuts.', 15),
    ('beta-testeur', 'Beta-testeur', 'distinction', '#6a5acd', 0, NULL, 16),
    ('ambassadeur', 'Ambassadeur', 'distinction', '#4f7df0', 0, NULL, 17)
ON CONFLICT (slug) DO NOTHING;

-- The forum themes (Laravel's ForumSeeder).
INSERT INTO site.forum_themes (nom, slug, couleur, description, ordre) VALUES
    ('Mathematiques', 'mathematiques', '#4f7df0', 'Algebre, analyse, geometrie... toutes tes questions de maths.', 1),
    ('Physique', 'physique', '#6a5acd', 'Mecanique, electricite, ondes, physique moderne.', 2),
    ('Aide aux devoirs', 'aide-aux-devoirs', '#3fa66b', 'Bloque sur un exercice ? Demande un coup de main.', 3),
    ('Methodes et revisions', 'methodes-revisions', '#c98a34', 'Organisation, fiches, preparation aux examens.', 4),
    ('La plateforme', 'la-plateforme', '#8a8a8f', 'Retours, suggestions et questions sur Pulsars.', 5)
ON CONFLICT (slug) DO NOTHING;

INSERT INTO contenu.licences (cle, nom, url) VALUES
    ('cc-by-4.0',            'CC BY 4.0',                'https://creativecommons.org/licenses/by/4.0/'),
    ('cc-by-sa-4.0',         'CC BY-SA 4.0',             'https://creativecommons.org/licenses/by-sa/4.0/'),
    ('cc-by-nc-4.0',         'CC BY-NC 4.0',             'https://creativecommons.org/licenses/by-nc/4.0/'),
    ('cc-by-nc-sa-4.0',      'CC BY-NC-SA 4.0',          'https://creativecommons.org/licenses/by-nc-sa/4.0/'),
    ('cc-by-nd-4.0',         'CC BY-ND 4.0',             'https://creativecommons.org/licenses/by-nd/4.0/'),
    ('cc0-1.0',              'CC0 1.0 (domaine public)', 'https://creativecommons.org/publicdomain/zero/1.0/'),
    ('domaine-public',       'Domaine public',           NULL),
    ('tous-droits-reserves', 'Tous droits réservés',     NULL),
    ('autre',                'Autre (à préciser)',       NULL);

INSERT INTO contenu.matieres (slug, langue, nom) VALUES
    ('mathematiques', 'fr', 'Mathématiques'),
    ('physique',      'fr', 'Physique'),
    ('chimie',        'fr', 'Chimie'),
    ('biologie',      'fr', 'Biologie'),
    ('informatique',  'fr', 'Informatique'),
    ('cryptographie', 'fr', 'Cryptographie'),
    ('mathematiques', 'en', 'Mathematics'),
    ('physique',      'en', 'Physics'),
    ('chimie',        'en', 'Chemistry'),
    ('biologie',      'en', 'Biology'),
    ('informatique',  'en', 'Computer science'),
    ('cryptographie', 'en', 'Cryptography'),
    ('mathematiques', 'ru', 'Математика'),
    ('physique',      'ru', 'Физика'),
    ('chimie',        'ru', 'Химия'),
    ('biologie',      'ru', 'Биология'),
    ('informatique',  'ru', 'Информатика'),
    ('cryptographie', 'ru', 'Криптография');

INSERT INTO contenu.sources_officielles (hote, prefixe) VALUES
    ('www.concours-centrale-supelec.fr', '/sites/default/files/documents/');

-- Starting content: Mandarin, HSK 1 with its first words, HSK 2 announced.

INSERT INTO langues.languages (code, name, native_name, script, reading_system, position, published)
VALUES ('zh', '{"fr": "Mandarin", "en": "Mandarin Chinese"}', '中文', 'Hans', 'pinyin', 1, true)
ON CONFLICT (code) DO NOTHING;

INSERT INTO langues.levels (language_code, slug, framework, position, title, description)
VALUES
    ('zh', 'hsk-1', 'HSK', 1,
     '{"fr": "HSK 1", "en": "HSK 1"}',
     '{"fr": "Les premiers mots : se présenter, saluer, compter, la famille et les repas.",
       "en": "First words: introducing yourself, greetings, numbers, family and meals."}'),
    ('zh', 'hsk-2', 'HSK', 2,
     '{"fr": "HSK 2", "en": "HSK 2"}',
     '{"fr": "La vie de tous les jours : le temps, les déplacements, les achats.",
       "en": "Everyday life: time, getting around, shopping."}')
ON CONFLICT (language_code, slug) DO NOTHING;

INSERT INTO langues.words (level_id, position, text, reading, meaning)
SELECT l.id, w.position, w.text, w.reading, w.meaning::jsonb
  FROM langues.levels l
 CROSS JOIN (VALUES
    (1,  '我',   'wǒ',       '{"fr": "je, moi", "en": "I, me"}'),
    (2,  '你',   'nǐ',       '{"fr": "tu, toi", "en": "you"}'),
    (3,  '他',   'tā',       '{"fr": "il, lui", "en": "he, him"}'),
    (4,  '她',   'tā',       '{"fr": "elle", "en": "she, her"}'),
    (5,  '好',   'hǎo',      '{"fr": "bon, bien", "en": "good, well"}'),
    (6,  '是',   'shì',      '{"fr": "être", "en": "to be"}'),
    (7,  '不',   'bù',       '{"fr": "ne… pas", "en": "not"}'),
    (8,  '的',   'de',       '{"fr": "particule du complément du nom", "en": "possessive particle"}'),
    (9,  '谢谢', 'xièxie',   '{"fr": "merci", "en": "thank you"}'),
    (10, '再见', 'zàijiàn',  '{"fr": "au revoir", "en": "goodbye"}'),
    (11, '一',   'yī',       '{"fr": "un", "en": "one"}'),
    (12, '二',   'èr',       '{"fr": "deux", "en": "two"}'),
    (13, '三',   'sān',      '{"fr": "trois", "en": "three"}'),
    (14, '八',   'bā',       '{"fr": "huit", "en": "eight"}'),
    (15, '多',   'duō',      '{"fr": "beaucoup, nombreux", "en": "many, much"}'),
    (16, '大',   'dà',       '{"fr": "grand", "en": "big"}'),
    (17, '人',   'rén',      '{"fr": "personne, être humain", "en": "person"}'),
    (18, '爸爸', 'bàba',     '{"fr": "papa", "en": "dad"}'),
    (19, '妈妈', 'māma',     '{"fr": "maman", "en": "mum"}'),
    (20, '儿子', 'érzi',     '{"fr": "fils", "en": "son"}'),
    (21, '朋友', 'péngyou',  '{"fr": "ami", "en": "friend"}'),
    (22, '老师', 'lǎoshī',   '{"fr": "professeur", "en": "teacher"}'),
    (23, '学生', 'xuésheng', '{"fr": "élève, étudiant", "en": "student"}'),
    (24, '中国', 'Zhōngguó', '{"fr": "la Chine", "en": "China"}'),
    (25, '北京', 'Běijīng',  '{"fr": "Pékin", "en": "Beijing"}'),
    (26, '爱',   'ài',       '{"fr": "aimer", "en": "to love"}'),
    (27, '吃',   'chī',      '{"fr": "manger", "en": "to eat"}'),
    (28, '喝',   'hē',       '{"fr": "boire", "en": "to drink"}'),
    (29, '读',   'dú',       '{"fr": "lire", "en": "to read"}'),
    (30, '水',   'shuǐ',     '{"fr": "eau", "en": "water"}'),
    (31, '茶',   'chá',      '{"fr": "thé", "en": "tea"}'),
    (32, '菜',   'cài',      '{"fr": "plat, légume", "en": "dish, vegetable"}'),
    (33, '杯子', 'bēizi',    '{"fr": "verre, tasse", "en": "cup, glass"}'),
    (34, '饭店', 'fàndiàn',  '{"fr": "restaurant, hôtel", "en": "restaurant, hotel"}'),
    (35, '东西', 'dōngxi',   '{"fr": "chose, objet", "en": "thing"}'),
    (36, '电脑', 'diànnǎo',  '{"fr": "ordinateur", "en": "computer"}'),
    (37, '飞机', 'fēijī',    '{"fr": "avion", "en": "plane"}'),
    (38, '点',   'diǎn',     '{"fr": "heure (sur l’horloge)", "en": "o’clock"}')
 ) AS w (position, text, reading, meaning)
 WHERE l.language_code = 'zh' AND l.slug = 'hsk-1'
ON CONFLICT (level_id, position) DO NOTHING;

-- Mandarin is an official language of China, Taiwan and Singapore.
UPDATE langues.languages SET countries = '{CN,TW,SG}' WHERE code = 'zh' AND countries = '{}';
