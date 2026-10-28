-- Reference data of `contenu` (phase 1): what the `api_read` views copied
-- from PHP, as tables the API reads (src/db/sources.rs).
--
-- Until the site writes content through the API (phase 4), the PHP copies
-- stay the source: a change to config/licences.php, to the subject names of
-- lang/*.json or to SujetOfficiel::SOURCES needs a migration here too.

DO $$
BEGIN
    IF to_regnamespace('contenu') IS NULL THEN
        RAISE EXCEPTION 'schema contenu does not exist: run deploy/sql/phase-1.sql first';
    END IF;
END
$$;

-- Same result as Laravel's Str::slug() for the French and English strings
-- of course themes ("Mathématiques" -> "mathematiques"), without the
-- `unaccent` extension. Copy of api_read.slugify().
CREATE FUNCTION contenu.slugify(value text)
RETURNS text
LANGUAGE sql
IMMUTABLE
PARALLEL SAFE
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

-- config/licences.php. `url` is null when there is no official text.
CREATE TABLE contenu.licences (
    cle text PRIMARY KEY,
    nom text NOT NULL,
    url text
);

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

-- Course subjects (App\Support\Matieres) and their name in each language
-- of the interface (lang/{langue}.json). `slug` is contenu.slugify() of the
-- French name, as courses.subject_slug.
CREATE TABLE contenu.matieres (
    slug   text NOT NULL,
    langue text NOT NULL,
    nom    text NOT NULL,
    PRIMARY KEY (slug, langue)
);

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

-- App\Support\SujetOfficiel::SOURCES: hosts an exam's official URL may point
-- to, and the path it must start with. The host is compared in lower case.
CREATE TABLE contenu.sources_officielles (
    hote    text PRIMARY KEY CHECK (hote = lower(hote)),
    prefixe text NOT NULL CHECK (prefixe LIKE '/%')
);

INSERT INTO contenu.sources_officielles (hote, prefixe) VALUES
    ('www.concours-centrale-supelec.fr', '/sites/default/files/documents/');
