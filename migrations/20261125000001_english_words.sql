-- English vocabulary: the 3000 most common words (EF list), translated into
-- French and sorted by CEFR level (17 levels of at most 200 words, A1 to B2).
-- Replayable: upserts the levels and replaces their words, so the same file
-- can also be loaded by hand with psql (pulsars-travail/contenu-anglais-3000.sql).

INSERT INTO langues.languages (code, name, native_name, script, reading_system, countries, position, published)
VALUES ('en', '{"fr": "Anglais", "en": "English"}', 'English', 'Latn', NULL,
        '{GB,US,IE,CA,AU,NZ}', 2, true)
ON CONFLICT (code) DO NOTHING;

INSERT INTO langues.levels (language_code, slug, framework, position, title, description)
SELECT 'en', n.slug, 'CECR', n.position, n.title, n.description
  FROM jsonb_to_recordset($en3000$[
{
"slug": "a1-1",
"position": 1,
"title": {
"fr": "A1 · 1",
"en": "A1 · 1"
},
"description": {
"fr": "Les mots de base : se présenter, la famille, les chiffres, le quotidien. (1/4)",
"en": "Basic words: introducing yourself, family, numbers, daily life. (1/4)"
}
},
{
"slug": "a1-2",
"position": 2,
"title": {
"fr": "A1 · 2",
"en": "A1 · 2"
},
"description": {
"fr": "Les mots de base : se présenter, la famille, les chiffres, le quotidien. (2/4)",
"en": "Basic words: introducing yourself, family, numbers, daily life. (2/4)"
}
},
{
"slug": "a1-3",
"position": 3,
"title": {
"fr": "A1 · 3",
"en": "A1 · 3"
},
"description": {
"fr": "Les mots de base : se présenter, la famille, les chiffres, le quotidien. (3/4)",
"en": "Basic words: introducing yourself, family, numbers, daily life. (3/4)"
}
},
{
"slug": "a1-4",
"position": 4,
"title": {
"fr": "A1 · 4",
"en": "A1 · 4"
},
"description": {
"fr": "Les mots de base : se présenter, la famille, les chiffres, le quotidien. (4/4)",
"en": "Basic words: introducing yourself, family, numbers, daily life. (4/4)"
}
},
{
"slug": "a2-1",
"position": 5,
"title": {
"fr": "A2 · 1",
"en": "A2 · 1"
},
"description": {
"fr": "Les mots de tous les jours : les achats, les déplacements, le travail. (1/4)",
"en": "Everyday words: shopping, getting around, work. (1/4)"
}
},
{
"slug": "a2-2",
"position": 6,
"title": {
"fr": "A2 · 2",
"en": "A2 · 2"
},
"description": {
"fr": "Les mots de tous les jours : les achats, les déplacements, le travail. (2/4)",
"en": "Everyday words: shopping, getting around, work. (2/4)"
}
},
{
"slug": "a2-3",
"position": 7,
"title": {
"fr": "A2 · 3",
"en": "A2 · 3"
},
"description": {
"fr": "Les mots de tous les jours : les achats, les déplacements, le travail. (3/4)",
"en": "Everyday words: shopping, getting around, work. (3/4)"
}
},
{
"slug": "a2-4",
"position": 8,
"title": {
"fr": "A2 · 4",
"en": "A2 · 4"
},
"description": {
"fr": "Les mots de tous les jours : les achats, les déplacements, le travail. (4/4)",
"en": "Everyday words: shopping, getting around, work. (4/4)"
}
},
{
"slug": "b1-1",
"position": 9,
"title": {
"fr": "B1 · 1",
"en": "B1 · 1"
},
"description": {
"fr": "Pour parler de ses idées, de l’actualité et de ses expériences. (1/5)",
"en": "Talking about ideas, news and experiences. (1/5)"
}
},
{
"slug": "b1-2",
"position": 10,
"title": {
"fr": "B1 · 2",
"en": "B1 · 2"
},
"description": {
"fr": "Pour parler de ses idées, de l’actualité et de ses expériences. (2/5)",
"en": "Talking about ideas, news and experiences. (2/5)"
}
},
{
"slug": "b1-3",
"position": 11,
"title": {
"fr": "B1 · 3",
"en": "B1 · 3"
},
"description": {
"fr": "Pour parler de ses idées, de l’actualité et de ses expériences. (3/5)",
"en": "Talking about ideas, news and experiences. (3/5)"
}
},
{
"slug": "b1-4",
"position": 12,
"title": {
"fr": "B1 · 4",
"en": "B1 · 4"
},
"description": {
"fr": "Pour parler de ses idées, de l’actualité et de ses expériences. (4/5)",
"en": "Talking about ideas, news and experiences. (4/5)"
}
},
{
"slug": "b1-5",
"position": 13,
"title": {
"fr": "B1 · 5",
"en": "B1 · 5"
},
"description": {
"fr": "Pour parler de ses idées, de l’actualité et de ses expériences. (5/5)",
"en": "Talking about ideas, news and experiences. (5/5)"
}
},
{
"slug": "b2-1",
"position": 14,
"title": {
"fr": "B2 · 1",
"en": "B2 · 1"
},
"description": {
"fr": "Le vocabulaire des journaux, du travail et des débats. (1/4)",
"en": "The vocabulary of newspapers, work and debates. (1/4)"
}
},
{
"slug": "b2-2",
"position": 15,
"title": {
"fr": "B2 · 2",
"en": "B2 · 2"
},
"description": {
"fr": "Le vocabulaire des journaux, du travail et des débats. (2/4)",
"en": "The vocabulary of newspapers, work and debates. (2/4)"
}
},
{
"slug": "b2-3",
"position": 16,
"title": {
"fr": "B2 · 3",
"en": "B2 · 3"
},
"description": {
"fr": "Le vocabulaire des journaux, du travail et des débats. (3/4)",
"en": "The vocabulary of newspapers, work and debates. (3/4)"
}
},
{
"slug": "b2-4",
"position": 17,
"title": {
"fr": "B2 · 4",
"en": "B2 · 4"
},
"description": {
"fr": "Le vocabulaire des journaux, du travail et des débats. (4/4)",
"en": "The vocabulary of newspapers, work and debates. (4/4)"
}
}
]$en3000$)
       AS n(slug text, position smallint, title jsonb, description jsonb)
ON CONFLICT (language_code, slug) DO UPDATE
   SET framework = excluded.framework, position = excluded.position,
       title = excluded.title, description = excluded.description;

DELETE FROM langues.words w
 USING langues.levels l
 WHERE w.level_id = l.id AND l.language_code = 'en';

INSERT INTO langues.words (level_id, position, text, reading, meaning)
SELECT l.id, w.position, w.text, NULL, w.meaning
  FROM jsonb_to_recordset($en3000$[
{
"level": "a1-1",
"position": 1,
"text": "a",
"meaning": {
"fr": "un, une"
}
},
{
"level": "a1-1",
"position": 2,
"text": "about",
"meaning": {
"fr": "à propos de, environ"
}
},
{
"level": "a1-1",
"position": 3,
"text": "above",
"meaning": {
"fr": "au-dessus, ci-dessus"
}
},
{
"level": "a1-1",
"position": 4,
"text": "across",
"meaning": {
"fr": "à travers, de l'autre côté"
}
},
{
"level": "a1-1",
"position": 5,
"text": "activity",
"meaning": {
"fr": "activité"
}
},
{
"level": "a1-1",
"position": 6,
"text": "actor",
"meaning": {
"fr": "acteur"
}
},
{
"level": "a1-1",
"position": 7,
"text": "actress",
"meaning": {
"fr": "actrice"
}
},
{
"level": "a1-1",
"position": 8,
"text": "add",
"meaning": {
"fr": "ajouter"
}
},
{
"level": "a1-1",
"position": 9,
"text": "address",
"meaning": {
"fr": "(n.) adresse ; (v.) aborder"
}
},
{
"level": "a1-1",
"position": 10,
"text": "adult",
"meaning": {
"fr": "adulte"
}
},
{
"level": "a1-1",
"position": 11,
"text": "after",
"meaning": {
"fr": "après"
}
},
{
"level": "a1-1",
"position": 12,
"text": "afternoon",
"meaning": {
"fr": "après-midi"
}
},
{
"level": "a1-1",
"position": 13,
"text": "again",
"meaning": {
"fr": "encore, de nouveau"
}
},
{
"level": "a1-1",
"position": 14,
"text": "age",
"meaning": {
"fr": "âge"
}
},
{
"level": "a1-1",
"position": 15,
"text": "ago",
"meaning": {
"fr": "il y a (dans le temps)"
}
},
{
"level": "a1-1",
"position": 16,
"text": "agree",
"meaning": {
"fr": "être d'accord"
}
},
{
"level": "a1-1",
"position": 17,
"text": "air",
"meaning": {
"fr": "air"
}
},
{
"level": "a1-1",
"position": 18,
"text": "airport",
"meaning": {
"fr": "aéroport"
}
},
{
"level": "a1-1",
"position": 19,
"text": "all",
"meaning": {
"fr": "tout, tous"
}
},
{
"level": "a1-1",
"position": 20,
"text": "already",
"meaning": {
"fr": "déjà"
}
},
{
"level": "a1-1",
"position": 21,
"text": "also",
"meaning": {
"fr": "aussi, également"
}
},
{
"level": "a1-1",
"position": 22,
"text": "always",
"meaning": {
"fr": "toujours"
}
},
{
"level": "a1-1",
"position": 23,
"text": "amazing",
"meaning": {
"fr": "incroyable, étonnant"
}
},
{
"level": "a1-1",
"position": 24,
"text": "American",
"meaning": {
"fr": "américain"
}
},
{
"level": "a1-1",
"position": 25,
"text": "and",
"meaning": {
"fr": "et"
}
},
{
"level": "a1-1",
"position": 26,
"text": "angry",
"meaning": {
"fr": "en colère, fâché"
}
},
{
"level": "a1-1",
"position": 27,
"text": "animal",
"meaning": {
"fr": "animal"
}
},
{
"level": "a1-1",
"position": 28,
"text": "another",
"meaning": {
"fr": "un autre, encore un"
}
},
{
"level": "a1-1",
"position": 29,
"text": "answer",
"meaning": {
"fr": "(n.) réponse ; (v.) répondre"
}
},
{
"level": "a1-1",
"position": 30,
"text": "any",
"meaning": {
"fr": "n'importe quel, du, aucun"
}
},
{
"level": "a1-1",
"position": 31,
"text": "anything",
"meaning": {
"fr": "n'importe quoi, quelque chose"
}
},
{
"level": "a1-1",
"position": 32,
"text": "apartment",
"meaning": {
"fr": "appartement"
}
},
{
"level": "a1-1",
"position": 33,
"text": "apple",
"meaning": {
"fr": "pomme"
}
},
{
"level": "a1-1",
"position": 34,
"text": "area",
"meaning": {
"fr": "zone, région, domaine"
}
},
{
"level": "a1-1",
"position": 35,
"text": "arm",
"meaning": {
"fr": "(n.) bras ; (v.) armer"
}
},
{
"level": "a1-1",
"position": 36,
"text": "around",
"meaning": {
"fr": "autour, aux alentours"
}
},
{
"level": "a1-1",
"position": 37,
"text": "arrive",
"meaning": {
"fr": "arriver"
}
},
{
"level": "a1-1",
"position": 38,
"text": "art",
"meaning": {
"fr": "art"
}
},
{
"level": "a1-1",
"position": 39,
"text": "article",
"meaning": {
"fr": "article"
}
},
{
"level": "a1-1",
"position": 40,
"text": "artist",
"meaning": {
"fr": "artiste"
}
},
{
"level": "a1-1",
"position": 41,
"text": "as",
"meaning": {
"fr": "comme, en tant que"
}
},
{
"level": "a1-1",
"position": 42,
"text": "ask",
"meaning": {
"fr": "demander"
}
},
{
"level": "a1-1",
"position": 43,
"text": "at",
"meaning": {
"fr": "à, chez"
}
},
{
"level": "a1-1",
"position": 44,
"text": "away",
"meaning": {
"fr": "loin, absent"
}
},
{
"level": "a1-1",
"position": 45,
"text": "baby",
"meaning": {
"fr": "bébé"
}
},
{
"level": "a1-1",
"position": 46,
"text": "back",
"meaning": {
"fr": "(n.) dos ; (adv.) en arrière, de retour"
}
},
{
"level": "a1-1",
"position": 47,
"text": "bad",
"meaning": {
"fr": "mauvais"
}
},
{
"level": "a1-1",
"position": 48,
"text": "bag",
"meaning": {
"fr": "sac"
}
},
{
"level": "a1-1",
"position": 49,
"text": "ball",
"meaning": {
"fr": "balle, ballon, bal"
}
},
{
"level": "a1-1",
"position": 50,
"text": "band",
"meaning": {
"fr": "groupe (de musique), bande"
}
},
{
"level": "a1-1",
"position": 51,
"text": "bank",
"meaning": {
"fr": "banque, rive"
}
},
{
"level": "a1-1",
"position": 52,
"text": "baseball",
"meaning": {
"fr": "baseball"
}
},
{
"level": "a1-1",
"position": 53,
"text": "basketball",
"meaning": {
"fr": "basket-ball"
}
},
{
"level": "a1-1",
"position": 54,
"text": "bathroom",
"meaning": {
"fr": "salle de bain"
}
},
{
"level": "a1-1",
"position": 55,
"text": "be",
"meaning": {
"fr": "être"
}
},
{
"level": "a1-1",
"position": 56,
"text": "beach",
"meaning": {
"fr": "plage"
}
},
{
"level": "a1-1",
"position": 57,
"text": "beautiful",
"meaning": {
"fr": "beau, magnifique"
}
},
{
"level": "a1-1",
"position": 58,
"text": "because",
"meaning": {
"fr": "parce que"
}
},
{
"level": "a1-1",
"position": 59,
"text": "become",
"meaning": {
"fr": "devenir"
}
},
{
"level": "a1-1",
"position": 60,
"text": "bed",
"meaning": {
"fr": "lit"
}
},
{
"level": "a1-1",
"position": 61,
"text": "bedroom",
"meaning": {
"fr": "chambre (à coucher)"
}
},
{
"level": "a1-1",
"position": 62,
"text": "beer",
"meaning": {
"fr": "bière"
}
},
{
"level": "a1-1",
"position": 63,
"text": "before",
"meaning": {
"fr": "avant"
}
},
{
"level": "a1-1",
"position": 64,
"text": "begin",
"meaning": {
"fr": "commencer"
}
},
{
"level": "a1-1",
"position": 65,
"text": "beginning",
"meaning": {
"fr": "début, commencement"
}
},
{
"level": "a1-1",
"position": 66,
"text": "behind",
"meaning": {
"fr": "derrière"
}
},
{
"level": "a1-1",
"position": 67,
"text": "believe",
"meaning": {
"fr": "croire"
}
},
{
"level": "a1-1",
"position": 68,
"text": "below",
"meaning": {
"fr": "en dessous, ci-dessous"
}
},
{
"level": "a1-1",
"position": 69,
"text": "best",
"meaning": {
"fr": "le meilleur, le mieux"
}
},
{
"level": "a1-1",
"position": 70,
"text": "better",
"meaning": {
"fr": "meilleur, mieux"
}
},
{
"level": "a1-1",
"position": 71,
"text": "between",
"meaning": {
"fr": "entre"
}
},
{
"level": "a1-1",
"position": 72,
"text": "big",
"meaning": {
"fr": "grand, gros"
}
},
{
"level": "a1-1",
"position": 73,
"text": "bike",
"meaning": {
"fr": "vélo"
}
},
{
"level": "a1-1",
"position": 74,
"text": "bird",
"meaning": {
"fr": "oiseau"
}
},
{
"level": "a1-1",
"position": 75,
"text": "birthday",
"meaning": {
"fr": "anniversaire (de naissance)"
}
},
{
"level": "a1-1",
"position": 76,
"text": "black",
"meaning": {
"fr": "noir"
}
},
{
"level": "a1-1",
"position": 77,
"text": "blue",
"meaning": {
"fr": "bleu"
}
},
{
"level": "a1-1",
"position": 78,
"text": "boat",
"meaning": {
"fr": "bateau"
}
},
{
"level": "a1-1",
"position": 79,
"text": "body",
"meaning": {
"fr": "corps"
}
},
{
"level": "a1-1",
"position": 80,
"text": "book",
"meaning": {
"fr": "(n.) livre ; (v.) réserver"
}
},
{
"level": "a1-1",
"position": 81,
"text": "boot",
"meaning": {
"fr": "botte"
}
},
{
"level": "a1-1",
"position": 82,
"text": "born",
"meaning": {
"fr": "né"
}
},
{
"level": "a1-1",
"position": 83,
"text": "both",
"meaning": {
"fr": "les deux, tous les deux"
}
},
{
"level": "a1-1",
"position": 84,
"text": "bottle",
"meaning": {
"fr": "bouteille"
}
},
{
"level": "a1-1",
"position": 85,
"text": "box",
"meaning": {
"fr": "boîte, carton"
}
},
{
"level": "a1-1",
"position": 86,
"text": "boy",
"meaning": {
"fr": "garçon"
}
},
{
"level": "a1-1",
"position": 87,
"text": "boyfriend",
"meaning": {
"fr": "petit ami, copain"
}
},
{
"level": "a1-1",
"position": 88,
"text": "bread",
"meaning": {
"fr": "pain"
}
},
{
"level": "a1-1",
"position": 89,
"text": "break",
"meaning": {
"fr": "(v.) casser ; (n.) pause"
}
},
{
"level": "a1-1",
"position": 90,
"text": "breakfast",
"meaning": {
"fr": "petit-déjeuner"
}
},
{
"level": "a1-1",
"position": 91,
"text": "bring",
"meaning": {
"fr": "apporter, amener"
}
},
{
"level": "a1-1",
"position": 92,
"text": "British",
"meaning": {
"fr": "britannique"
}
},
{
"level": "a1-1",
"position": 93,
"text": "brother",
"meaning": {
"fr": "frère"
}
},
{
"level": "a1-1",
"position": 94,
"text": "brown",
"meaning": {
"fr": "marron, brun"
}
},
{
"level": "a1-1",
"position": 95,
"text": "build",
"meaning": {
"fr": "construire"
}
},
{
"level": "a1-1",
"position": 96,
"text": "building",
"meaning": {
"fr": "bâtiment, immeuble"
}
},
{
"level": "a1-1",
"position": 97,
"text": "bus",
"meaning": {
"fr": "bus"
}
},
{
"level": "a1-1",
"position": 98,
"text": "business",
"meaning": {
"fr": "affaires, entreprise"
}
},
{
"level": "a1-1",
"position": 99,
"text": "busy",
"meaning": {
"fr": "occupé"
}
},
{
"level": "a1-1",
"position": 100,
"text": "but",
"meaning": {
"fr": "mais"
}
},
{
"level": "a1-1",
"position": 101,
"text": "butter",
"meaning": {
"fr": "beurre"
}
},
{
"level": "a1-1",
"position": 102,
"text": "buy",
"meaning": {
"fr": "acheter"
}
},
{
"level": "a1-1",
"position": 103,
"text": "by",
"meaning": {
"fr": "par, près de"
}
},
{
"level": "a1-1",
"position": 104,
"text": "cake",
"meaning": {
"fr": "gâteau"
}
},
{
"level": "a1-1",
"position": 105,
"text": "call",
"meaning": {
"fr": "(v.) appeler ; (n.) appel"
}
},
{
"level": "a1-1",
"position": 106,
"text": "camera",
"meaning": {
"fr": "appareil photo, caméra"
}
},
{
"level": "a1-1",
"position": 107,
"text": "can",
"meaning": {
"fr": "(v.) pouvoir ; (n.) boîte de conserve"
}
},
{
"level": "a1-1",
"position": 108,
"text": "car",
"meaning": {
"fr": "voiture"
}
},
{
"level": "a1-1",
"position": 109,
"text": "card",
"meaning": {
"fr": "carte"
}
},
{
"level": "a1-1",
"position": 110,
"text": "cat",
"meaning": {
"fr": "chat"
}
},
{
"level": "a1-1",
"position": 111,
"text": "center",
"meaning": {
"fr": "centre"
}
},
{
"level": "a1-1",
"position": 112,
"text": "chair",
"meaning": {
"fr": "chaise"
}
},
{
"level": "a1-1",
"position": 113,
"text": "change",
"meaning": {
"fr": "(v.) changer ; (n.) changement, monnaie"
}
},
{
"level": "a1-1",
"position": 114,
"text": "cheap",
"meaning": {
"fr": "bon marché, pas cher"
}
},
{
"level": "a1-1",
"position": 115,
"text": "check",
"meaning": {
"fr": "(v.) vérifier ; (n.) chèque"
}
},
{
"level": "a1-1",
"position": 116,
"text": "cheese",
"meaning": {
"fr": "fromage"
}
},
{
"level": "a1-1",
"position": 117,
"text": "chicken",
"meaning": {
"fr": "poulet"
}
},
{
"level": "a1-1",
"position": 118,
"text": "child",
"meaning": {
"fr": "enfant"
}
},
{
"level": "a1-1",
"position": 119,
"text": "Chinese",
"meaning": {
"fr": "chinois"
}
},
{
"level": "a1-1",
"position": 120,
"text": "chocolate",
"meaning": {
"fr": "chocolat"
}
},
{
"level": "a1-1",
"position": 121,
"text": "choose",
"meaning": {
"fr": "choisir"
}
},
{
"level": "a1-1",
"position": 122,
"text": "Christmas",
"meaning": {
"fr": "Noël"
}
},
{
"level": "a1-1",
"position": 123,
"text": "city",
"meaning": {
"fr": "ville"
}
},
{
"level": "a1-1",
"position": 124,
"text": "class",
"meaning": {
"fr": "classe, cours"
}
},
{
"level": "a1-1",
"position": 125,
"text": "classroom",
"meaning": {
"fr": "salle de classe"
}
},
{
"level": "a1-1",
"position": 126,
"text": "clean",
"meaning": {
"fr": "(adj.) propre ; (v.) nettoyer"
}
},
{
"level": "a1-1",
"position": 127,
"text": "climb",
"meaning": {
"fr": "grimper, escalader"
}
},
{
"level": "a1-1",
"position": 128,
"text": "clock",
"meaning": {
"fr": "horloge"
}
},
{
"level": "a1-1",
"position": 129,
"text": "close",
"meaning": {
"fr": "(v.) fermer ; (adj.) proche"
}
},
{
"level": "a1-1",
"position": 130,
"text": "clothes",
"meaning": {
"fr": "vêtements, habits"
}
},
{
"level": "a1-1",
"position": 131,
"text": "club",
"meaning": {
"fr": "club"
}
},
{
"level": "a1-1",
"position": 132,
"text": "coat",
"meaning": {
"fr": "manteau"
}
},
{
"level": "a1-1",
"position": 133,
"text": "coffee",
"meaning": {
"fr": "café"
}
},
{
"level": "a1-1",
"position": 134,
"text": "cold",
"meaning": {
"fr": "(adj.) froid ; (n.) rhume"
}
},
{
"level": "a1-1",
"position": 135,
"text": "college",
"meaning": {
"fr": "université, faculté"
}
},
{
"level": "a1-1",
"position": 136,
"text": "color",
"meaning": {
"fr": "couleur ; (v.) colorier"
}
},
{
"level": "a1-1",
"position": 137,
"text": "come",
"meaning": {
"fr": "venir, arriver"
}
},
{
"level": "a1-1",
"position": 138,
"text": "computer",
"meaning": {
"fr": "ordinateur"
}
},
{
"level": "a1-1",
"position": 139,
"text": "concert",
"meaning": {
"fr": "concert"
}
},
{
"level": "a1-1",
"position": 140,
"text": "conversation",
"meaning": {
"fr": "conversation"
}
},
{
"level": "a1-1",
"position": 141,
"text": "cook",
"meaning": {
"fr": "cuisiner ; (n.) cuisinier"
}
},
{
"level": "a1-1",
"position": 142,
"text": "cookie",
"meaning": {
"fr": "biscuit, cookie"
}
},
{
"level": "a1-1",
"position": 143,
"text": "cooking",
"meaning": {
"fr": "cuisine (art culinaire)"
}
},
{
"level": "a1-1",
"position": 144,
"text": "cool",
"meaning": {
"fr": "frais ; (fam.) cool, génial"
}
},
{
"level": "a1-1",
"position": 145,
"text": "correct",
"meaning": {
"fr": "correct, juste ; (v.) corriger"
}
},
{
"level": "a1-1",
"position": 146,
"text": "could",
"meaning": {
"fr": "pourrait, pouvait"
}
},
{
"level": "a1-1",
"position": 147,
"text": "country",
"meaning": {
"fr": "pays, campagne"
}
},
{
"level": "a1-1",
"position": 148,
"text": "cousin",
"meaning": {
"fr": "cousin, cousine"
}
},
{
"level": "a1-1",
"position": 149,
"text": "cow",
"meaning": {
"fr": "vache"
}
},
{
"level": "a1-1",
"position": 150,
"text": "cup",
"meaning": {
"fr": "tasse, coupe"
}
},
{
"level": "a1-1",
"position": 151,
"text": "dad",
"meaning": {
"fr": "papa"
}
},
{
"level": "a1-1",
"position": 152,
"text": "dance",
"meaning": {
"fr": "danser ; (n.) danse"
}
},
{
"level": "a1-1",
"position": 153,
"text": "dark",
"meaning": {
"fr": "sombre, foncé ; (n.) obscurité"
}
},
{
"level": "a1-1",
"position": 154,
"text": "date",
"meaning": {
"fr": "date, rendez-vous"
}
},
{
"level": "a1-1",
"position": 155,
"text": "daughter",
"meaning": {
"fr": "fille (enfant)"
}
},
{
"level": "a1-1",
"position": 156,
"text": "day",
"meaning": {
"fr": "jour, journée"
}
},
{
"level": "a1-1",
"position": 157,
"text": "dear",
"meaning": {
"fr": "cher, chère"
}
},
{
"level": "a1-1",
"position": 158,
"text": "describe",
"meaning": {
"fr": "décrire"
}
},
{
"level": "a1-1",
"position": 159,
"text": "description",
"meaning": {
"fr": "description"
}
},
{
"level": "a1-1",
"position": 160,
"text": "desk",
"meaning": {
"fr": "bureau (meuble)"
}
},
{
"level": "a1-1",
"position": 161,
"text": "difference",
"meaning": {
"fr": "différence"
}
},
{
"level": "a1-1",
"position": 162,
"text": "different",
"meaning": {
"fr": "différent"
}
},
{
"level": "a1-1",
"position": 163,
"text": "difficult",
"meaning": {
"fr": "difficile"
}
},
{
"level": "a1-1",
"position": 164,
"text": "dinner",
"meaning": {
"fr": "dîner"
}
},
{
"level": "a1-1",
"position": 165,
"text": "dirty",
"meaning": {
"fr": "sale"
}
},
{
"level": "a1-1",
"position": 166,
"text": "do",
"meaning": {
"fr": "faire"
}
},
{
"level": "a1-1",
"position": 167,
"text": "doctor",
"meaning": {
"fr": "médecin, docteur"
}
},
{
"level": "a1-1",
"position": 168,
"text": "dog",
"meaning": {
"fr": "chien"
}
},
{
"level": "a1-1",
"position": 169,
"text": "door",
"meaning": {
"fr": "porte"
}
},
{
"level": "a1-1",
"position": 170,
"text": "down",
"meaning": {
"fr": "en bas, vers le bas"
}
},
{
"level": "a1-1",
"position": 171,
"text": "draw",
"meaning": {
"fr": "dessiner, tirer"
}
},
{
"level": "a1-1",
"position": 172,
"text": "drawing",
"meaning": {
"fr": "dessin"
}
},
{
"level": "a1-1",
"position": 173,
"text": "dream",
"meaning": {
"fr": "rêve ; (v.) rêver"
}
},
{
"level": "a1-2",
"position": 1,
"text": "dress",
"meaning": {
"fr": "robe ; (v.) s'habiller"
}
},
{
"level": "a1-2",
"position": 2,
"text": "drink",
"meaning": {
"fr": "boire ; (n.) boisson"
}
},
{
"level": "a1-2",
"position": 3,
"text": "drive",
"meaning": {
"fr": "conduire ; (n.) trajet"
}
},
{
"level": "a1-2",
"position": 4,
"text": "dry",
"meaning": {
"fr": "sec ; (v.) sécher"
}
},
{
"level": "a1-2",
"position": 5,
"text": "during",
"meaning": {
"fr": "pendant, au cours de"
}
},
{
"level": "a1-2",
"position": 6,
"text": "e-mail",
"meaning": {
"fr": "e-mail, courriel"
}
},
{
"level": "a1-2",
"position": 7,
"text": "each",
"meaning": {
"fr": "chaque, chacun"
}
},
{
"level": "a1-2",
"position": 8,
"text": "ear",
"meaning": {
"fr": "oreille"
}
},
{
"level": "a1-2",
"position": 9,
"text": "early",
"meaning": {
"fr": "tôt, en avance, précoce"
}
},
{
"level": "a1-2",
"position": 10,
"text": "east",
"meaning": {
"fr": "est"
}
},
{
"level": "a1-2",
"position": 11,
"text": "easy",
"meaning": {
"fr": "facile"
}
},
{
"level": "a1-2",
"position": 12,
"text": "eat",
"meaning": {
"fr": "manger"
}
},
{
"level": "a1-2",
"position": 13,
"text": "egg",
"meaning": {
"fr": "œuf"
}
},
{
"level": "a1-2",
"position": 14,
"text": "eight",
"meaning": {
"fr": "huit"
}
},
{
"level": "a1-2",
"position": 15,
"text": "end",
"meaning": {
"fr": "fin ; (v.) finir, terminer"
}
},
{
"level": "a1-2",
"position": 16,
"text": "English",
"meaning": {
"fr": "anglais"
}
},
{
"level": "a1-2",
"position": 17,
"text": "enjoy",
"meaning": {
"fr": "aimer, apprécier, profiter de"
}
},
{
"level": "a1-2",
"position": 18,
"text": "enough",
"meaning": {
"fr": "assez, suffisamment"
}
},
{
"level": "a1-2",
"position": 19,
"text": "enter",
"meaning": {
"fr": "entrer, saisir"
}
},
{
"level": "a1-2",
"position": 20,
"text": "evening",
"meaning": {
"fr": "soir, soirée"
}
},
{
"level": "a1-2",
"position": 21,
"text": "every",
"meaning": {
"fr": "chaque, tous les"
}
},
{
"level": "a1-2",
"position": 22,
"text": "everybody",
"meaning": {
"fr": "tout le monde"
}
},
{
"level": "a1-2",
"position": 23,
"text": "everyone",
"meaning": {
"fr": "tout le monde, chacun"
}
},
{
"level": "a1-2",
"position": 24,
"text": "everything",
"meaning": {
"fr": "tout"
}
},
{
"level": "a1-2",
"position": 25,
"text": "example",
"meaning": {
"fr": "exemple"
}
},
{
"level": "a1-2",
"position": 26,
"text": "excellent",
"meaning": {
"fr": "excellent"
}
},
{
"level": "a1-2",
"position": 27,
"text": "exciting",
"meaning": {
"fr": "passionnant, excitant"
}
},
{
"level": "a1-2",
"position": 28,
"text": "exercise",
"meaning": {
"fr": "exercice ; (v.) faire de l'exercice"
}
},
{
"level": "a1-2",
"position": 29,
"text": "expensive",
"meaning": {
"fr": "cher, coûteux"
}
},
{
"level": "a1-2",
"position": 30,
"text": "explain",
"meaning": {
"fr": "expliquer"
}
},
{
"level": "a1-2",
"position": 31,
"text": "eye",
"meaning": {
"fr": "œil"
}
},
{
"level": "a1-2",
"position": 32,
"text": "face",
"meaning": {
"fr": "visage ; faire face à"
}
},
{
"level": "a1-2",
"position": 33,
"text": "fact",
"meaning": {
"fr": "fait"
}
},
{
"level": "a1-2",
"position": 34,
"text": "fall",
"meaning": {
"fr": "tomber ; chute, automne"
}
},
{
"level": "a1-2",
"position": 35,
"text": "false",
"meaning": {
"fr": "faux"
}
},
{
"level": "a1-2",
"position": 36,
"text": "family",
"meaning": {
"fr": "famille"
}
},
{
"level": "a1-2",
"position": 37,
"text": "famous",
"meaning": {
"fr": "célèbre"
}
},
{
"level": "a1-2",
"position": 38,
"text": "far",
"meaning": {
"fr": "loin"
}
},
{
"level": "a1-2",
"position": 39,
"text": "farm",
"meaning": {
"fr": "ferme, exploitation agricole"
}
},
{
"level": "a1-2",
"position": 40,
"text": "farmer",
"meaning": {
"fr": "agriculteur, fermier"
}
},
{
"level": "a1-2",
"position": 41,
"text": "fast",
"meaning": {
"fr": "rapide, vite"
}
},
{
"level": "a1-2",
"position": 42,
"text": "father",
"meaning": {
"fr": "père"
}
},
{
"level": "a1-2",
"position": 43,
"text": "favorite",
"meaning": {
"fr": "préféré, favori"
}
},
{
"level": "a1-2",
"position": 44,
"text": "feel",
"meaning": {
"fr": "sentir, ressentir"
}
},
{
"level": "a1-2",
"position": 45,
"text": "feeling",
"meaning": {
"fr": "sentiment, sensation"
}
},
{
"level": "a1-2",
"position": 46,
"text": "few",
"meaning": {
"fr": "peu de, quelques"
}
},
{
"level": "a1-2",
"position": 47,
"text": "fifteen",
"meaning": {
"fr": "quinze"
}
},
{
"level": "a1-2",
"position": 48,
"text": "fifth",
"meaning": {
"fr": "cinquième"
}
},
{
"level": "a1-2",
"position": 49,
"text": "fifty",
"meaning": {
"fr": "cinquante"
}
},
{
"level": "a1-2",
"position": 50,
"text": "fill",
"meaning": {
"fr": "remplir"
}
},
{
"level": "a1-2",
"position": 51,
"text": "film",
"meaning": {
"fr": "film ; filmer"
}
},
{
"level": "a1-2",
"position": 52,
"text": "find",
"meaning": {
"fr": "trouver"
}
},
{
"level": "a1-2",
"position": 53,
"text": "fine",
"meaning": {
"fr": "bien, fin ; amende"
}
},
{
"level": "a1-2",
"position": 54,
"text": "finish",
"meaning": {
"fr": "finir, terminer"
}
},
{
"level": "a1-2",
"position": 55,
"text": "first",
"meaning": {
"fr": "premier, d'abord"
}
},
{
"level": "a1-2",
"position": 56,
"text": "fish",
"meaning": {
"fr": "poisson ; pêcher"
}
},
{
"level": "a1-2",
"position": 57,
"text": "five",
"meaning": {
"fr": "cinq"
}
},
{
"level": "a1-2",
"position": 58,
"text": "flight",
"meaning": {
"fr": "vol (avion)"
}
},
{
"level": "a1-2",
"position": 59,
"text": "floor",
"meaning": {
"fr": "sol, étage"
}
},
{
"level": "a1-2",
"position": 60,
"text": "flower",
"meaning": {
"fr": "fleur"
}
},
{
"level": "a1-2",
"position": 61,
"text": "fly",
"meaning": {
"fr": "voler ; mouche"
}
},
{
"level": "a1-2",
"position": 62,
"text": "follow",
"meaning": {
"fr": "suivre"
}
},
{
"level": "a1-2",
"position": 63,
"text": "following",
"meaning": {
"fr": "suivant"
}
},
{
"level": "a1-2",
"position": 64,
"text": "food",
"meaning": {
"fr": "nourriture"
}
},
{
"level": "a1-2",
"position": 65,
"text": "foot",
"meaning": {
"fr": "pied"
}
},
{
"level": "a1-2",
"position": 66,
"text": "football",
"meaning": {
"fr": "football"
}
},
{
"level": "a1-2",
"position": 67,
"text": "for",
"meaning": {
"fr": "pour, pendant"
}
},
{
"level": "a1-2",
"position": 68,
"text": "forget",
"meaning": {
"fr": "oublier"
}
},
{
"level": "a1-2",
"position": 69,
"text": "form",
"meaning": {
"fr": "forme, formulaire ; former"
}
},
{
"level": "a1-2",
"position": 70,
"text": "four",
"meaning": {
"fr": "quatre"
}
},
{
"level": "a1-2",
"position": 71,
"text": "fourth",
"meaning": {
"fr": "quatrième"
}
},
{
"level": "a1-2",
"position": 72,
"text": "free",
"meaning": {
"fr": "libre, gratuit"
}
},
{
"level": "a1-2",
"position": 73,
"text": "French",
"meaning": {
"fr": "français"
}
},
{
"level": "a1-2",
"position": 74,
"text": "fresh",
"meaning": {
"fr": "frais (aliment), neuf"
}
},
{
"level": "a1-2",
"position": 75,
"text": "friend",
"meaning": {
"fr": "ami"
}
},
{
"level": "a1-2",
"position": 76,
"text": "friendly",
"meaning": {
"fr": "amical, sympathique"
}
},
{
"level": "a1-2",
"position": 77,
"text": "from",
"meaning": {
"fr": "de, depuis"
}
},
{
"level": "a1-2",
"position": 78,
"text": "front",
"meaning": {
"fr": "avant, devant ; façade"
}
},
{
"level": "a1-2",
"position": 79,
"text": "fruit",
"meaning": {
"fr": "fruit"
}
},
{
"level": "a1-2",
"position": 80,
"text": "full",
"meaning": {
"fr": "plein, complet"
}
},
{
"level": "a1-2",
"position": 81,
"text": "fun",
"meaning": {
"fr": "amusement ; amusant"
}
},
{
"level": "a1-2",
"position": 82,
"text": "funny",
"meaning": {
"fr": "drôle, marrant"
}
},
{
"level": "a1-2",
"position": 83,
"text": "future",
"meaning": {
"fr": "avenir, futur"
}
},
{
"level": "a1-2",
"position": 84,
"text": "game",
"meaning": {
"fr": "jeu, match"
}
},
{
"level": "a1-2",
"position": 85,
"text": "garden",
"meaning": {
"fr": "jardin"
}
},
{
"level": "a1-2",
"position": 86,
"text": "German",
"meaning": {
"fr": "allemand"
}
},
{
"level": "a1-2",
"position": 87,
"text": "get",
"meaning": {
"fr": "obtenir, recevoir, devenir"
}
},
{
"level": "a1-2",
"position": 88,
"text": "gift",
"meaning": {
"fr": "cadeau, don"
}
},
{
"level": "a1-2",
"position": 89,
"text": "girl",
"meaning": {
"fr": "fille"
}
},
{
"level": "a1-2",
"position": 90,
"text": "girlfriend",
"meaning": {
"fr": "petite amie"
}
},
{
"level": "a1-2",
"position": 91,
"text": "give",
"meaning": {
"fr": "donner"
}
},
{
"level": "a1-2",
"position": 92,
"text": "glass",
"meaning": {
"fr": "verre"
}
},
{
"level": "a1-2",
"position": 93,
"text": "go",
"meaning": {
"fr": "aller"
}
},
{
"level": "a1-2",
"position": 94,
"text": "good",
"meaning": {
"fr": "bon, bien"
}
},
{
"level": "a1-2",
"position": 95,
"text": "grandfather",
"meaning": {
"fr": "grand-père"
}
},
{
"level": "a1-2",
"position": 96,
"text": "grandmother",
"meaning": {
"fr": "grand-mère"
}
},
{
"level": "a1-2",
"position": 97,
"text": "gray",
"meaning": {
"fr": "gris"
}
},
{
"level": "a1-2",
"position": 98,
"text": "great",
"meaning": {
"fr": "grand, génial"
}
},
{
"level": "a1-2",
"position": 99,
"text": "green",
"meaning": {
"fr": "vert"
}
},
{
"level": "a1-2",
"position": 100,
"text": "group",
"meaning": {
"fr": "groupe"
}
},
{
"level": "a1-2",
"position": 101,
"text": "grow",
"meaning": {
"fr": "grandir, pousser, cultiver"
}
},
{
"level": "a1-2",
"position": 102,
"text": "guess",
"meaning": {
"fr": "deviner"
}
},
{
"level": "a1-2",
"position": 103,
"text": "hair",
"meaning": {
"fr": "cheveux, poils"
}
},
{
"level": "a1-2",
"position": 104,
"text": "half",
"meaning": {
"fr": "moitié, demi"
}
},
{
"level": "a1-2",
"position": 105,
"text": "hand",
"meaning": {
"fr": "main"
}
},
{
"level": "a1-2",
"position": 106,
"text": "happen",
"meaning": {
"fr": "arriver, se produire"
}
},
{
"level": "a1-2",
"position": 107,
"text": "happy",
"meaning": {
"fr": "heureux"
}
},
{
"level": "a1-2",
"position": 108,
"text": "hard",
"meaning": {
"fr": "dur, difficile"
}
},
{
"level": "a1-2",
"position": 109,
"text": "hat",
"meaning": {
"fr": "chapeau"
}
},
{
"level": "a1-2",
"position": 110,
"text": "hate",
"meaning": {
"fr": "détester, haïr"
}
},
{
"level": "a1-2",
"position": 111,
"text": "have",
"meaning": {
"fr": "avoir"
}
},
{
"level": "a1-2",
"position": 112,
"text": "he",
"meaning": {
"fr": "il"
}
},
{
"level": "a1-2",
"position": 113,
"text": "head",
"meaning": {
"fr": "tête ; diriger"
}
},
{
"level": "a1-2",
"position": 114,
"text": "healthy",
"meaning": {
"fr": "en bonne santé, sain"
}
},
{
"level": "a1-2",
"position": 115,
"text": "hear",
"meaning": {
"fr": "entendre"
}
},
{
"level": "a1-2",
"position": 116,
"text": "hello",
"meaning": {
"fr": "bonjour, allô"
}
},
{
"level": "a1-2",
"position": 117,
"text": "help",
"meaning": {
"fr": "aider ; aide"
}
},
{
"level": "a1-2",
"position": 118,
"text": "her",
"meaning": {
"fr": "la, lui (elle) ; son, sa (à elle)"
}
},
{
"level": "a1-2",
"position": 119,
"text": "here",
"meaning": {
"fr": "ici"
}
},
{
"level": "a1-2",
"position": 120,
"text": "hi",
"meaning": {
"fr": "salut"
}
},
{
"level": "a1-2",
"position": 121,
"text": "high",
"meaning": {
"fr": "haut, élevé"
}
},
{
"level": "a1-2",
"position": 122,
"text": "him",
"meaning": {
"fr": "le, lui"
}
},
{
"level": "a1-2",
"position": 123,
"text": "his",
"meaning": {
"fr": "son, sa, ses (à lui)"
}
},
{
"level": "a1-2",
"position": 124,
"text": "history",
"meaning": {
"fr": "histoire (passé)"
}
},
{
"level": "a1-2",
"position": 125,
"text": "holiday",
"meaning": {
"fr": "vacances, jour férié"
}
},
{
"level": "a1-2",
"position": 126,
"text": "home",
"meaning": {
"fr": "maison, foyer, chez soi"
}
},
{
"level": "a1-2",
"position": 127,
"text": "hope",
"meaning": {
"fr": "espoir ; espérer"
}
},
{
"level": "a1-2",
"position": 128,
"text": "horse",
"meaning": {
"fr": "cheval"
}
},
{
"level": "a1-2",
"position": 129,
"text": "hospital",
"meaning": {
"fr": "hôpital"
}
},
{
"level": "a1-2",
"position": 130,
"text": "hot",
"meaning": {
"fr": "chaud"
}
},
{
"level": "a1-2",
"position": 131,
"text": "hotel",
"meaning": {
"fr": "hôtel"
}
},
{
"level": "a1-2",
"position": 132,
"text": "hour",
"meaning": {
"fr": "heure"
}
},
{
"level": "a1-2",
"position": 133,
"text": "house",
"meaning": {
"fr": "maison"
}
},
{
"level": "a1-2",
"position": 134,
"text": "how",
"meaning": {
"fr": "comment"
}
},
{
"level": "a1-2",
"position": 135,
"text": "hundred",
"meaning": {
"fr": "cent"
}
},
{
"level": "a1-2",
"position": 136,
"text": "hungry",
"meaning": {
"fr": "affamé, qui a faim"
}
},
{
"level": "a1-2",
"position": 137,
"text": "husband",
"meaning": {
"fr": "mari"
}
},
{
"level": "a1-2",
"position": 138,
"text": "I",
"meaning": {
"fr": "je, moi"
}
},
{
"level": "a1-2",
"position": 139,
"text": "ice",
"meaning": {
"fr": "glace"
}
},
{
"level": "a1-2",
"position": 140,
"text": "idea",
"meaning": {
"fr": "idée"
}
},
{
"level": "a1-2",
"position": 141,
"text": "if",
"meaning": {
"fr": "si"
}
},
{
"level": "a1-2",
"position": 142,
"text": "imagine",
"meaning": {
"fr": "imaginer"
}
},
{
"level": "a1-2",
"position": 143,
"text": "important",
"meaning": {
"fr": "important"
}
},
{
"level": "a1-2",
"position": 144,
"text": "in",
"meaning": {
"fr": "dans, en"
}
},
{
"level": "a1-2",
"position": 145,
"text": "information",
"meaning": {
"fr": "information(s), renseignements"
}
},
{
"level": "a1-2",
"position": 146,
"text": "inside",
"meaning": {
"fr": "dedans, à l'intérieur ; intérieur"
}
},
{
"level": "a1-2",
"position": 147,
"text": "interest",
"meaning": {
"fr": "intérêt ; intéresser"
}
},
{
"level": "a1-2",
"position": 148,
"text": "interested",
"meaning": {
"fr": "intéressé"
}
},
{
"level": "a1-2",
"position": 149,
"text": "interesting",
"meaning": {
"fr": "intéressant"
}
},
{
"level": "a1-2",
"position": 150,
"text": "Internet",
"meaning": {
"fr": "Internet"
}
},
{
"level": "a1-2",
"position": 151,
"text": "interview",
"meaning": {
"fr": "entretien, interview"
}
},
{
"level": "a1-2",
"position": 152,
"text": "into",
"meaning": {
"fr": "dans, en (mouvement vers)"
}
},
{
"level": "a1-2",
"position": 153,
"text": "invite",
"meaning": {
"fr": "inviter"
}
},
{
"level": "a1-2",
"position": 154,
"text": "island",
"meaning": {
"fr": "île"
}
},
{
"level": "a1-2",
"position": 155,
"text": "it",
"meaning": {
"fr": "il, elle, ce (chose)"
}
},
{
"level": "a1-2",
"position": 156,
"text": "Italian",
"meaning": {
"fr": "italien"
}
},
{
"level": "a1-2",
"position": 157,
"text": "its",
"meaning": {
"fr": "son, sa, ses (chose)"
}
},
{
"level": "a1-2",
"position": 158,
"text": "jacket",
"meaning": {
"fr": "veste"
}
},
{
"level": "a1-2",
"position": 159,
"text": "Japanese",
"meaning": {
"fr": "japonais"
}
},
{
"level": "a1-2",
"position": 160,
"text": "job",
"meaning": {
"fr": "emploi, travail"
}
},
{
"level": "a1-2",
"position": 161,
"text": "join",
"meaning": {
"fr": "rejoindre, joindre"
}
},
{
"level": "a1-2",
"position": 162,
"text": "juice",
"meaning": {
"fr": "jus"
}
},
{
"level": "a1-2",
"position": 163,
"text": "just",
"meaning": {
"fr": "juste, seulement"
}
},
{
"level": "a1-2",
"position": 164,
"text": "keep",
"meaning": {
"fr": "garder, continuer"
}
},
{
"level": "a1-2",
"position": 165,
"text": "key",
"meaning": {
"fr": "clé ; essentiel"
}
},
{
"level": "a1-2",
"position": 166,
"text": "kid",
"meaning": {
"fr": "gamin, gosse"
}
},
{
"level": "a1-2",
"position": 167,
"text": "kind",
"meaning": {
"fr": "gentil ; sorte"
}
},
{
"level": "a1-2",
"position": 168,
"text": "kitchen",
"meaning": {
"fr": "cuisine (pièce)"
}
},
{
"level": "a1-2",
"position": 169,
"text": "know",
"meaning": {
"fr": "savoir, connaître"
}
},
{
"level": "a1-2",
"position": 170,
"text": "lake",
"meaning": {
"fr": "lac"
}
},
{
"level": "a1-2",
"position": 171,
"text": "language",
"meaning": {
"fr": "langue, langage"
}
},
{
"level": "a1-2",
"position": 172,
"text": "large",
"meaning": {
"fr": "grand, gros"
}
},
{
"level": "a1-2",
"position": 173,
"text": "last",
"meaning": {
"fr": "dernier ; durer"
}
},
{
"level": "a1-3",
"position": 1,
"text": "late",
"meaning": {
"fr": "en retard, tard"
}
},
{
"level": "a1-3",
"position": 2,
"text": "later",
"meaning": {
"fr": "plus tard"
}
},
{
"level": "a1-3",
"position": 3,
"text": "learn",
"meaning": {
"fr": "apprendre"
}
},
{
"level": "a1-3",
"position": 4,
"text": "leave",
"meaning": {
"fr": "partir, quitter, laisser"
}
},
{
"level": "a1-3",
"position": 5,
"text": "left",
"meaning": {
"fr": "gauche ; (v.) parti (passé de leave)"
}
},
{
"level": "a1-3",
"position": 6,
"text": "leg",
"meaning": {
"fr": "jambe, patte"
}
},
{
"level": "a1-3",
"position": 7,
"text": "less",
"meaning": {
"fr": "moins"
}
},
{
"level": "a1-3",
"position": 8,
"text": "lesson",
"meaning": {
"fr": "leçon, cours"
}
},
{
"level": "a1-3",
"position": 9,
"text": "letter",
"meaning": {
"fr": "lettre"
}
},
{
"level": "a1-3",
"position": 10,
"text": "library",
"meaning": {
"fr": "bibliothèque"
}
},
{
"level": "a1-3",
"position": 11,
"text": "life",
"meaning": {
"fr": "vie"
}
},
{
"level": "a1-3",
"position": 12,
"text": "light",
"meaning": {
"fr": "lumière ; (adj.) léger, clair"
}
},
{
"level": "a1-3",
"position": 13,
"text": "like",
"meaning": {
"fr": "aimer ; (prép.) comme"
}
},
{
"level": "a1-3",
"position": 14,
"text": "line",
"meaning": {
"fr": "ligne, file"
}
},
{
"level": "a1-3",
"position": 15,
"text": "list",
"meaning": {
"fr": "liste ; (v.) énumérer"
}
},
{
"level": "a1-3",
"position": 16,
"text": "listen",
"meaning": {
"fr": "écouter"
}
},
{
"level": "a1-3",
"position": 17,
"text": "little",
"meaning": {
"fr": "petit, peu"
}
},
{
"level": "a1-3",
"position": 18,
"text": "live",
"meaning": {
"fr": "vivre, habiter ; (adj.) en direct"
}
},
{
"level": "a1-3",
"position": 19,
"text": "long",
"meaning": {
"fr": "long ; (adv.) longtemps"
}
},
{
"level": "a1-3",
"position": 20,
"text": "look",
"meaning": {
"fr": "regarder, sembler ; (n.) regard"
}
},
{
"level": "a1-3",
"position": 21,
"text": "lot",
"meaning": {
"fr": "beaucoup (a lot), lot, terrain"
}
},
{
"level": "a1-3",
"position": 22,
"text": "lots",
"meaning": {
"fr": "des tas, plein de"
}
},
{
"level": "a1-3",
"position": 23,
"text": "love",
"meaning": {
"fr": "amour ; (v.) aimer, adorer"
}
},
{
"level": "a1-3",
"position": 24,
"text": "lunch",
"meaning": {
"fr": "déjeuner"
}
},
{
"level": "a1-3",
"position": 25,
"text": "magazine",
"meaning": {
"fr": "magazine, revue"
}
},
{
"level": "a1-3",
"position": 26,
"text": "make",
"meaning": {
"fr": "faire, fabriquer"
}
},
{
"level": "a1-3",
"position": 27,
"text": "man",
"meaning": {
"fr": "homme"
}
},
{
"level": "a1-3",
"position": 28,
"text": "many",
"meaning": {
"fr": "beaucoup de, nombreux"
}
},
{
"level": "a1-3",
"position": 29,
"text": "map",
"meaning": {
"fr": "carte, plan"
}
},
{
"level": "a1-3",
"position": 30,
"text": "market",
"meaning": {
"fr": "marché"
}
},
{
"level": "a1-3",
"position": 31,
"text": "married",
"meaning": {
"fr": "marié"
}
},
{
"level": "a1-3",
"position": 32,
"text": "math",
"meaning": {
"fr": "maths"
}
},
{
"level": "a1-3",
"position": 33,
"text": "maybe",
"meaning": {
"fr": "peut-être"
}
},
{
"level": "a1-3",
"position": 34,
"text": "me",
"meaning": {
"fr": "moi, me"
}
},
{
"level": "a1-3",
"position": 35,
"text": "meal",
"meaning": {
"fr": "repas"
}
},
{
"level": "a1-3",
"position": 36,
"text": "meat",
"meaning": {
"fr": "viande"
}
},
{
"level": "a1-3",
"position": 37,
"text": "meet",
"meaning": {
"fr": "rencontrer, retrouver"
}
},
{
"level": "a1-3",
"position": 38,
"text": "menu",
"meaning": {
"fr": "menu, carte"
}
},
{
"level": "a1-3",
"position": 39,
"text": "message",
"meaning": {
"fr": "message"
}
},
{
"level": "a1-3",
"position": 40,
"text": "Mexican",
"meaning": {
"fr": "mexicain"
}
},
{
"level": "a1-3",
"position": 41,
"text": "milk",
"meaning": {
"fr": "lait"
}
},
{
"level": "a1-3",
"position": 42,
"text": "million",
"meaning": {
"fr": "million"
}
},
{
"level": "a1-3",
"position": 43,
"text": "minute",
"meaning": {
"fr": "minute"
}
},
{
"level": "a1-3",
"position": 44,
"text": "mistake",
"meaning": {
"fr": "erreur, faute"
}
},
{
"level": "a1-3",
"position": 45,
"text": "mom",
"meaning": {
"fr": "maman"
}
},
{
"level": "a1-3",
"position": 46,
"text": "money",
"meaning": {
"fr": "argent"
}
},
{
"level": "a1-3",
"position": 47,
"text": "month",
"meaning": {
"fr": "mois"
}
},
{
"level": "a1-3",
"position": 48,
"text": "more",
"meaning": {
"fr": "plus, davantage"
}
},
{
"level": "a1-3",
"position": 49,
"text": "morning",
"meaning": {
"fr": "matin, matinée"
}
},
{
"level": "a1-3",
"position": 50,
"text": "most",
"meaning": {
"fr": "le plus, la plupart"
}
},
{
"level": "a1-3",
"position": 51,
"text": "mother",
"meaning": {
"fr": "mère"
}
},
{
"level": "a1-3",
"position": 52,
"text": "mountain",
"meaning": {
"fr": "montagne"
}
},
{
"level": "a1-3",
"position": 53,
"text": "mouse",
"meaning": {
"fr": "souris"
}
},
{
"level": "a1-3",
"position": 54,
"text": "mouth",
"meaning": {
"fr": "bouche"
}
},
{
"level": "a1-3",
"position": 55,
"text": "move",
"meaning": {
"fr": "bouger, déménager ; (n.) coup"
}
},
{
"level": "a1-3",
"position": 56,
"text": "movie",
"meaning": {
"fr": "film"
}
},
{
"level": "a1-3",
"position": 57,
"text": "Mr",
"meaning": {
"fr": "monsieur (M.)"
}
},
{
"level": "a1-3",
"position": 58,
"text": "Mrs",
"meaning": {
"fr": "madame (Mme)"
}
},
{
"level": "a1-3",
"position": 59,
"text": "Ms",
"meaning": {
"fr": "madame (Mme, sans statut marital)"
}
},
{
"level": "a1-3",
"position": 60,
"text": "much",
"meaning": {
"fr": "beaucoup, très"
}
},
{
"level": "a1-3",
"position": 61,
"text": "museum",
"meaning": {
"fr": "musée"
}
},
{
"level": "a1-3",
"position": 62,
"text": "music",
"meaning": {
"fr": "musique"
}
},
{
"level": "a1-3",
"position": 63,
"text": "must",
"meaning": {
"fr": "devoir (obligation)"
}
},
{
"level": "a1-3",
"position": 64,
"text": "my",
"meaning": {
"fr": "mon, ma, mes"
}
},
{
"level": "a1-3",
"position": 65,
"text": "n't",
"meaning": {
"fr": "ne... pas (forme contractée)"
}
},
{
"level": "a1-3",
"position": 66,
"text": "name",
"meaning": {
"fr": "nom ; (v.) nommer"
}
},
{
"level": "a1-3",
"position": 67,
"text": "near",
"meaning": {
"fr": "près de, proche"
}
},
{
"level": "a1-3",
"position": 68,
"text": "need",
"meaning": {
"fr": "avoir besoin de ; (n.) besoin"
}
},
{
"level": "a1-3",
"position": 69,
"text": "never",
"meaning": {
"fr": "jamais"
}
},
{
"level": "a1-3",
"position": 70,
"text": "new",
"meaning": {
"fr": "nouveau, neuf"
}
},
{
"level": "a1-3",
"position": 71,
"text": "news",
"meaning": {
"fr": "nouvelles, informations"
}
},
{
"level": "a1-3",
"position": 72,
"text": "newspaper",
"meaning": {
"fr": "journal"
}
},
{
"level": "a1-3",
"position": 73,
"text": "next",
"meaning": {
"fr": "prochain, suivant"
}
},
{
"level": "a1-3",
"position": 74,
"text": "nice",
"meaning": {
"fr": "gentil, agréable, joli"
}
},
{
"level": "a1-3",
"position": 75,
"text": "night",
"meaning": {
"fr": "nuit, soir"
}
},
{
"level": "a1-3",
"position": 76,
"text": "nine",
"meaning": {
"fr": "neuf (9)"
}
},
{
"level": "a1-3",
"position": 77,
"text": "no",
"meaning": {
"fr": "non, aucun"
}
},
{
"level": "a1-3",
"position": 78,
"text": "north",
"meaning": {
"fr": "nord"
}
},
{
"level": "a1-3",
"position": 79,
"text": "nose",
"meaning": {
"fr": "nez"
}
},
{
"level": "a1-3",
"position": 80,
"text": "not",
"meaning": {
"fr": "ne... pas"
}
},
{
"level": "a1-3",
"position": 81,
"text": "nothing",
"meaning": {
"fr": "rien"
}
},
{
"level": "a1-3",
"position": 82,
"text": "now",
"meaning": {
"fr": "maintenant"
}
},
{
"level": "a1-3",
"position": 83,
"text": "number",
"meaning": {
"fr": "nombre, numéro"
}
},
{
"level": "a1-3",
"position": 84,
"text": "nurse",
"meaning": {
"fr": "infirmier, infirmière"
}
},
{
"level": "a1-3",
"position": 85,
"text": "of",
"meaning": {
"fr": "de"
}
},
{
"level": "a1-3",
"position": 86,
"text": "off",
"meaning": {
"fr": "éteint ; (adv.) hors de, loin"
}
},
{
"level": "a1-3",
"position": 87,
"text": "office",
"meaning": {
"fr": "bureau"
}
},
{
"level": "a1-3",
"position": 88,
"text": "often",
"meaning": {
"fr": "souvent"
}
},
{
"level": "a1-3",
"position": 89,
"text": "oh",
"meaning": {
"fr": "oh, ah"
}
},
{
"level": "a1-3",
"position": 90,
"text": "ok",
"meaning": {
"fr": "d'accord, OK"
}
},
{
"level": "a1-3",
"position": 91,
"text": "okay",
"meaning": {
"fr": "d'accord, bien"
}
},
{
"level": "a1-3",
"position": 92,
"text": "old",
"meaning": {
"fr": "vieux, âgé, ancien"
}
},
{
"level": "a1-3",
"position": 93,
"text": "on",
"meaning": {
"fr": "sur ; (adj.) allumé"
}
},
{
"level": "a1-3",
"position": 94,
"text": "one",
"meaning": {
"fr": "un, une (chiffre 1)"
}
},
{
"level": "a1-3",
"position": 95,
"text": "only",
"meaning": {
"fr": "seulement ; (adj.) seul, unique"
}
},
{
"level": "a1-3",
"position": 96,
"text": "open",
"meaning": {
"fr": "ouvrir ; (adj.) ouvert"
}
},
{
"level": "a1-3",
"position": 97,
"text": "or",
"meaning": {
"fr": "ou"
}
},
{
"level": "a1-3",
"position": 98,
"text": "orange",
"meaning": {
"fr": "orange"
}
},
{
"level": "a1-3",
"position": 99,
"text": "other",
"meaning": {
"fr": "autre"
}
},
{
"level": "a1-3",
"position": 100,
"text": "others",
"meaning": {
"fr": "les autres"
}
},
{
"level": "a1-3",
"position": 101,
"text": "our",
"meaning": {
"fr": "notre, nos"
}
},
{
"level": "a1-3",
"position": 102,
"text": "out",
"meaning": {
"fr": "dehors, hors de"
}
},
{
"level": "a1-3",
"position": 103,
"text": "outside",
"meaning": {
"fr": "à l'extérieur, dehors"
}
},
{
"level": "a1-3",
"position": 104,
"text": "over",
"meaning": {
"fr": "au-dessus de, plus de, fini"
}
},
{
"level": "a1-3",
"position": 105,
"text": "page",
"meaning": {
"fr": "page"
}
},
{
"level": "a1-3",
"position": 106,
"text": "paint",
"meaning": {
"fr": "peindre ; (n.) peinture"
}
},
{
"level": "a1-3",
"position": 107,
"text": "painting",
"meaning": {
"fr": "tableau, peinture (art)"
}
},
{
"level": "a1-3",
"position": 108,
"text": "pair",
"meaning": {
"fr": "paire"
}
},
{
"level": "a1-3",
"position": 109,
"text": "paper",
"meaning": {
"fr": "papier, journal"
}
},
{
"level": "a1-3",
"position": 110,
"text": "parent",
"meaning": {
"fr": "parent"
}
},
{
"level": "a1-3",
"position": 111,
"text": "park",
"meaning": {
"fr": "parc ; (v.) se garer"
}
},
{
"level": "a1-3",
"position": 112,
"text": "part",
"meaning": {
"fr": "partie, rôle, pièce"
}
},
{
"level": "a1-3",
"position": 113,
"text": "party",
"meaning": {
"fr": "fête, soirée, parti"
}
},
{
"level": "a1-3",
"position": 114,
"text": "pay",
"meaning": {
"fr": "payer ; (n.) salaire, paie"
}
},
{
"level": "a1-3",
"position": 115,
"text": "people",
"meaning": {
"fr": "gens, personnes"
}
},
{
"level": "a1-3",
"position": 116,
"text": "perfect",
"meaning": {
"fr": "parfait"
}
},
{
"level": "a1-3",
"position": 117,
"text": "person",
"meaning": {
"fr": "personne"
}
},
{
"level": "a1-3",
"position": 118,
"text": "phone",
"meaning": {
"fr": "téléphone ; (v.) appeler"
}
},
{
"level": "a1-3",
"position": 119,
"text": "photo",
"meaning": {
"fr": "photo"
}
},
{
"level": "a1-3",
"position": 120,
"text": "piano",
"meaning": {
"fr": "piano"
}
},
{
"level": "a1-3",
"position": 121,
"text": "picture",
"meaning": {
"fr": "image, photo, tableau"
}
},
{
"level": "a1-3",
"position": 122,
"text": "piece",
"meaning": {
"fr": "morceau, pièce"
}
},
{
"level": "a1-3",
"position": 123,
"text": "pink",
"meaning": {
"fr": "rose (couleur)"
}
},
{
"level": "a1-3",
"position": 124,
"text": "place",
"meaning": {
"fr": "endroit, lieu, place"
}
},
{
"level": "a1-3",
"position": 125,
"text": "plan",
"meaning": {
"fr": "plan, projet ; (v.) prévoir"
}
},
{
"level": "a1-3",
"position": 126,
"text": "plane",
"meaning": {
"fr": "avion"
}
},
{
"level": "a1-3",
"position": 127,
"text": "plant",
"meaning": {
"fr": "plante, usine ; (v.) planter"
}
},
{
"level": "a1-3",
"position": 128,
"text": "play",
"meaning": {
"fr": "jouer ; (n.) pièce de théâtre"
}
},
{
"level": "a1-3",
"position": 129,
"text": "player",
"meaning": {
"fr": "joueur"
}
},
{
"level": "a1-3",
"position": 130,
"text": "please",
"meaning": {
"fr": "s'il vous plaît ; (v.) plaire"
}
},
{
"level": "a1-3",
"position": 131,
"text": "police",
"meaning": {
"fr": "police"
}
},
{
"level": "a1-3",
"position": 132,
"text": "pool",
"meaning": {
"fr": "piscine, flaque"
}
},
{
"level": "a1-3",
"position": 133,
"text": "poor",
"meaning": {
"fr": "pauvre, médiocre"
}
},
{
"level": "a1-3",
"position": 134,
"text": "popular",
"meaning": {
"fr": "populaire"
}
},
{
"level": "a1-3",
"position": 135,
"text": "potato",
"meaning": {
"fr": "pomme de terre"
}
},
{
"level": "a1-3",
"position": 136,
"text": "price",
"meaning": {
"fr": "prix"
}
},
{
"level": "a1-3",
"position": 137,
"text": "problem",
"meaning": {
"fr": "problème"
}
},
{
"level": "a1-3",
"position": 138,
"text": "put",
"meaning": {
"fr": "mettre, poser"
}
},
{
"level": "a1-3",
"position": 139,
"text": "question",
"meaning": {
"fr": "question"
}
},
{
"level": "a1-3",
"position": 140,
"text": "radio",
"meaning": {
"fr": "radio"
}
},
{
"level": "a1-3",
"position": 141,
"text": "rain",
"meaning": {
"fr": "(n.) pluie ; (v.) pleuvoir"
}
},
{
"level": "a1-3",
"position": 142,
"text": "read",
"meaning": {
"fr": "lire"
}
},
{
"level": "a1-3",
"position": 143,
"text": "reading",
"meaning": {
"fr": "lecture"
}
},
{
"level": "a1-3",
"position": 144,
"text": "ready",
"meaning": {
"fr": "prêt"
}
},
{
"level": "a1-3",
"position": 145,
"text": "really",
"meaning": {
"fr": "vraiment"
}
},
{
"level": "a1-3",
"position": 146,
"text": "red",
"meaning": {
"fr": "rouge"
}
},
{
"level": "a1-3",
"position": 147,
"text": "remember",
"meaning": {
"fr": "se souvenir de"
}
},
{
"level": "a1-3",
"position": 148,
"text": "repeat",
"meaning": {
"fr": "répéter"
}
},
{
"level": "a1-3",
"position": 149,
"text": "restaurant",
"meaning": {
"fr": "restaurant"
}
},
{
"level": "a1-3",
"position": 150,
"text": "rice",
"meaning": {
"fr": "riz"
}
},
{
"level": "a1-3",
"position": 151,
"text": "ride",
"meaning": {
"fr": "(v.) monter, aller (à vélo) ; (n.) trajet"
}
},
{
"level": "a1-3",
"position": 152,
"text": "right",
"meaning": {
"fr": "droit, correct, droite"
}
},
{
"level": "a1-3",
"position": 153,
"text": "river",
"meaning": {
"fr": "rivière, fleuve"
}
},
{
"level": "a1-3",
"position": 154,
"text": "road",
"meaning": {
"fr": "route"
}
},
{
"level": "a1-3",
"position": 155,
"text": "room",
"meaning": {
"fr": "pièce, chambre, place"
}
},
{
"level": "a1-3",
"position": 156,
"text": "run",
"meaning": {
"fr": "courir, diriger"
}
},
{
"level": "a1-3",
"position": 157,
"text": "running",
"meaning": {
"fr": "course à pied"
}
},
{
"level": "a1-3",
"position": 158,
"text": "Russian",
"meaning": {
"fr": "russe"
}
},
{
"level": "a1-3",
"position": 159,
"text": "sad",
"meaning": {
"fr": "triste"
}
},
{
"level": "a1-3",
"position": 160,
"text": "salad",
"meaning": {
"fr": "salade"
}
},
{
"level": "a1-3",
"position": 161,
"text": "same",
"meaning": {
"fr": "même, pareil"
}
},
{
"level": "a1-3",
"position": 162,
"text": "say",
"meaning": {
"fr": "dire"
}
},
{
"level": "a1-3",
"position": 163,
"text": "school",
"meaning": {
"fr": "école"
}
},
{
"level": "a1-3",
"position": 164,
"text": "science",
"meaning": {
"fr": "science"
}
},
{
"level": "a1-3",
"position": 165,
"text": "sea",
"meaning": {
"fr": "mer"
}
},
{
"level": "a1-3",
"position": 166,
"text": "second",
"meaning": {
"fr": "deuxième, seconde"
}
},
{
"level": "a1-3",
"position": 167,
"text": "see",
"meaning": {
"fr": "voir"
}
},
{
"level": "a1-3",
"position": 168,
"text": "sell",
"meaning": {
"fr": "vendre"
}
},
{
"level": "a1-3",
"position": 169,
"text": "send",
"meaning": {
"fr": "envoyer"
}
},
{
"level": "a1-3",
"position": 170,
"text": "sentence",
"meaning": {
"fr": "phrase, peine (judiciaire)"
}
},
{
"level": "a1-3",
"position": 171,
"text": "seven",
"meaning": {
"fr": "sept"
}
},
{
"level": "a1-3",
"position": 172,
"text": "share",
"meaning": {
"fr": "(v.) partager ; (n.) part, action"
}
},
{
"level": "a1-3",
"position": 173,
"text": "she",
"meaning": {
"fr": "elle"
}
},
{
"level": "a1-4",
"position": 1,
"text": "shirt",
"meaning": {
"fr": "chemise"
}
},
{
"level": "a1-4",
"position": 2,
"text": "shoe",
"meaning": {
"fr": "chaussure"
}
},
{
"level": "a1-4",
"position": 3,
"text": "shop",
"meaning": {
"fr": "magasin, boutique"
}
},
{
"level": "a1-4",
"position": 4,
"text": "shopping",
"meaning": {
"fr": "courses, achats"
}
},
{
"level": "a1-4",
"position": 5,
"text": "short",
"meaning": {
"fr": "court, petit"
}
},
{
"level": "a1-4",
"position": 6,
"text": "should",
"meaning": {
"fr": "devoir (conseil), devrait"
}
},
{
"level": "a1-4",
"position": 7,
"text": "show",
"meaning": {
"fr": "(v.) montrer ; (n.) spectacle"
}
},
{
"level": "a1-4",
"position": 8,
"text": "shower",
"meaning": {
"fr": "douche, averse"
}
},
{
"level": "a1-4",
"position": 9,
"text": "sing",
"meaning": {
"fr": "chanter"
}
},
{
"level": "a1-4",
"position": 10,
"text": "singer",
"meaning": {
"fr": "chanteur, chanteuse"
}
},
{
"level": "a1-4",
"position": 11,
"text": "sister",
"meaning": {
"fr": "sœur"
}
},
{
"level": "a1-4",
"position": 12,
"text": "sit",
"meaning": {
"fr": "s'asseoir"
}
},
{
"level": "a1-4",
"position": 13,
"text": "six",
"meaning": {
"fr": "six"
}
},
{
"level": "a1-4",
"position": 14,
"text": "sleep",
"meaning": {
"fr": "(v.) dormir ; (n.) sommeil"
}
},
{
"level": "a1-4",
"position": 15,
"text": "small",
"meaning": {
"fr": "petit"
}
},
{
"level": "a1-4",
"position": 16,
"text": "snow",
"meaning": {
"fr": "neige"
}
},
{
"level": "a1-4",
"position": 17,
"text": "so",
"meaning": {
"fr": "donc, si, tellement"
}
},
{
"level": "a1-4",
"position": 18,
"text": "some",
"meaning": {
"fr": "quelques, du, de la"
}
},
{
"level": "a1-4",
"position": 19,
"text": "someone",
"meaning": {
"fr": "quelqu'un"
}
},
{
"level": "a1-4",
"position": 20,
"text": "something",
"meaning": {
"fr": "quelque chose"
}
},
{
"level": "a1-4",
"position": 21,
"text": "sometimes",
"meaning": {
"fr": "parfois"
}
},
{
"level": "a1-4",
"position": 22,
"text": "son",
"meaning": {
"fr": "fils"
}
},
{
"level": "a1-4",
"position": 23,
"text": "song",
"meaning": {
"fr": "chanson"
}
},
{
"level": "a1-4",
"position": 24,
"text": "soon",
"meaning": {
"fr": "bientôt, tôt"
}
},
{
"level": "a1-4",
"position": 25,
"text": "sorry",
"meaning": {
"fr": "désolé, pardon"
}
},
{
"level": "a1-4",
"position": 26,
"text": "soup",
"meaning": {
"fr": "soupe"
}
},
{
"level": "a1-4",
"position": 27,
"text": "south",
"meaning": {
"fr": "sud"
}
},
{
"level": "a1-4",
"position": 28,
"text": "Spanish",
"meaning": {
"fr": "espagnol"
}
},
{
"level": "a1-4",
"position": 29,
"text": "speak",
"meaning": {
"fr": "parler"
}
},
{
"level": "a1-4",
"position": 30,
"text": "spend",
"meaning": {
"fr": "dépenser, passer (du temps)"
}
},
{
"level": "a1-4",
"position": 31,
"text": "sport",
"meaning": {
"fr": "sport"
}
},
{
"level": "a1-4",
"position": 32,
"text": "spring",
"meaning": {
"fr": "printemps, ressort, source"
}
},
{
"level": "a1-4",
"position": 33,
"text": "stand",
"meaning": {
"fr": "être debout, se lever, supporter"
}
},
{
"level": "a1-4",
"position": 34,
"text": "star",
"meaning": {
"fr": "étoile, vedette"
}
},
{
"level": "a1-4",
"position": 35,
"text": "start",
"meaning": {
"fr": "commencer, démarrer ; (n.) début"
}
},
{
"level": "a1-4",
"position": 36,
"text": "station",
"meaning": {
"fr": "gare, station"
}
},
{
"level": "a1-4",
"position": 37,
"text": "stay",
"meaning": {
"fr": "rester, séjourner ; (n.) séjour"
}
},
{
"level": "a1-4",
"position": 38,
"text": "still",
"meaning": {
"fr": "encore, toujours ; (adj.) immobile"
}
},
{
"level": "a1-4",
"position": 39,
"text": "stop",
"meaning": {
"fr": "arrêter, s'arrêter ; (n.) arrêt"
}
},
{
"level": "a1-4",
"position": 40,
"text": "store",
"meaning": {
"fr": "magasin ; (v.) stocker, ranger"
}
},
{
"level": "a1-4",
"position": 41,
"text": "story",
"meaning": {
"fr": "histoire, récit"
}
},
{
"level": "a1-4",
"position": 42,
"text": "street",
"meaning": {
"fr": "rue"
}
},
{
"level": "a1-4",
"position": 43,
"text": "strong",
"meaning": {
"fr": "fort, solide"
}
},
{
"level": "a1-4",
"position": 44,
"text": "student",
"meaning": {
"fr": "étudiant, élève"
}
},
{
"level": "a1-4",
"position": 45,
"text": "study",
"meaning": {
"fr": "étudier ; (n.) étude, bureau"
}
},
{
"level": "a1-4",
"position": 46,
"text": "subject",
"meaning": {
"fr": "sujet, matière"
}
},
{
"level": "a1-4",
"position": 47,
"text": "sugar",
"meaning": {
"fr": "sucre"
}
},
{
"level": "a1-4",
"position": 48,
"text": "summer",
"meaning": {
"fr": "été (saison)"
}
},
{
"level": "a1-4",
"position": 49,
"text": "sun",
"meaning": {
"fr": "soleil"
}
},
{
"level": "a1-4",
"position": 50,
"text": "sure",
"meaning": {
"fr": "sûr, certain"
}
},
{
"level": "a1-4",
"position": 51,
"text": "swim",
"meaning": {
"fr": "nager"
}
},
{
"level": "a1-4",
"position": 52,
"text": "table",
"meaning": {
"fr": "table, tableau"
}
},
{
"level": "a1-4",
"position": 53,
"text": "take",
"meaning": {
"fr": "prendre, emmener"
}
},
{
"level": "a1-4",
"position": 54,
"text": "talk",
"meaning": {
"fr": "parler ; (n.) discussion, exposé"
}
},
{
"level": "a1-4",
"position": 55,
"text": "tall",
"meaning": {
"fr": "grand (de taille), haut"
}
},
{
"level": "a1-4",
"position": 56,
"text": "tea",
"meaning": {
"fr": "thé"
}
},
{
"level": "a1-4",
"position": 57,
"text": "teach",
"meaning": {
"fr": "enseigner, apprendre à"
}
},
{
"level": "a1-4",
"position": 58,
"text": "teacher",
"meaning": {
"fr": "professeur, enseignant"
}
},
{
"level": "a1-4",
"position": 59,
"text": "team",
"meaning": {
"fr": "équipe"
}
},
{
"level": "a1-4",
"position": 60,
"text": "telephone",
"meaning": {
"fr": "téléphone"
}
},
{
"level": "a1-4",
"position": 61,
"text": "television",
"meaning": {
"fr": "télévision"
}
},
{
"level": "a1-4",
"position": 62,
"text": "tell",
"meaning": {
"fr": "dire, raconter"
}
},
{
"level": "a1-4",
"position": 63,
"text": "ten",
"meaning": {
"fr": "dix"
}
},
{
"level": "a1-4",
"position": 64,
"text": "tennis",
"meaning": {
"fr": "tennis"
}
},
{
"level": "a1-4",
"position": 65,
"text": "test",
"meaning": {
"fr": "test, examen ; (v.) tester"
}
},
{
"level": "a1-4",
"position": 66,
"text": "text",
"meaning": {
"fr": "texte ; (v.) envoyer un SMS"
}
},
{
"level": "a1-4",
"position": 67,
"text": "than",
"meaning": {
"fr": "que (comparaison)"
}
},
{
"level": "a1-4",
"position": 68,
"text": "thank",
"meaning": {
"fr": "remercier"
}
},
{
"level": "a1-4",
"position": 69,
"text": "thanks",
"meaning": {
"fr": "merci"
}
},
{
"level": "a1-4",
"position": 70,
"text": "that",
"meaning": {
"fr": "ce, cela, que"
}
},
{
"level": "a1-4",
"position": 71,
"text": "the",
"meaning": {
"fr": "le, la, les"
}
},
{
"level": "a1-4",
"position": 72,
"text": "their",
"meaning": {
"fr": "leur, leurs"
}
},
{
"level": "a1-4",
"position": 73,
"text": "them",
"meaning": {
"fr": "les, eux, elles"
}
},
{
"level": "a1-4",
"position": 74,
"text": "then",
"meaning": {
"fr": "puis, ensuite, alors"
}
},
{
"level": "a1-4",
"position": 75,
"text": "there",
"meaning": {
"fr": "là, y"
}
},
{
"level": "a1-4",
"position": 76,
"text": "these",
"meaning": {
"fr": "ces, ceux-ci"
}
},
{
"level": "a1-4",
"position": 77,
"text": "they",
"meaning": {
"fr": "ils, elles"
}
},
{
"level": "a1-4",
"position": 78,
"text": "thing",
"meaning": {
"fr": "chose"
}
},
{
"level": "a1-4",
"position": 79,
"text": "think",
"meaning": {
"fr": "penser, croire"
}
},
{
"level": "a1-4",
"position": 80,
"text": "third",
"meaning": {
"fr": "troisième, tiers"
}
},
{
"level": "a1-4",
"position": 81,
"text": "thirty",
"meaning": {
"fr": "trente"
}
},
{
"level": "a1-4",
"position": 82,
"text": "this",
"meaning": {
"fr": "ce, ceci, cette"
}
},
{
"level": "a1-4",
"position": 83,
"text": "those",
"meaning": {
"fr": "ceux-là, ces"
}
},
{
"level": "a1-4",
"position": 84,
"text": "thousand",
"meaning": {
"fr": "mille"
}
},
{
"level": "a1-4",
"position": 85,
"text": "three",
"meaning": {
"fr": "trois"
}
},
{
"level": "a1-4",
"position": 86,
"text": "ticket",
"meaning": {
"fr": "billet, ticket"
}
},
{
"level": "a1-4",
"position": 87,
"text": "time",
"meaning": {
"fr": "temps, heure, fois"
}
},
{
"level": "a1-4",
"position": 88,
"text": "tired",
"meaning": {
"fr": "fatigué"
}
},
{
"level": "a1-4",
"position": 89,
"text": "to",
"meaning": {
"fr": "à, vers, pour"
}
},
{
"level": "a1-4",
"position": 90,
"text": "today",
"meaning": {
"fr": "aujourd'hui"
}
},
{
"level": "a1-4",
"position": 91,
"text": "together",
"meaning": {
"fr": "ensemble"
}
},
{
"level": "a1-4",
"position": 92,
"text": "tomato",
"meaning": {
"fr": "tomate"
}
},
{
"level": "a1-4",
"position": 93,
"text": "tomorrow",
"meaning": {
"fr": "demain"
}
},
{
"level": "a1-4",
"position": 94,
"text": "tonight",
"meaning": {
"fr": "ce soir"
}
},
{
"level": "a1-4",
"position": 95,
"text": "too",
"meaning": {
"fr": "aussi, trop"
}
},
{
"level": "a1-4",
"position": 96,
"text": "tourist",
"meaning": {
"fr": "touriste"
}
},
{
"level": "a1-4",
"position": 97,
"text": "town",
"meaning": {
"fr": "ville"
}
},
{
"level": "a1-4",
"position": 98,
"text": "train",
"meaning": {
"fr": "train ; (v.) entraîner, former"
}
},
{
"level": "a1-4",
"position": 99,
"text": "travel",
"meaning": {
"fr": "voyager ; (n.) voyages"
}
},
{
"level": "a1-4",
"position": 100,
"text": "tree",
"meaning": {
"fr": "arbre"
}
},
{
"level": "a1-4",
"position": 101,
"text": "trip",
"meaning": {
"fr": "voyage, excursion, trajet"
}
},
{
"level": "a1-4",
"position": 102,
"text": "true",
"meaning": {
"fr": "vrai"
}
},
{
"level": "a1-4",
"position": 103,
"text": "try",
"meaning": {
"fr": "essayer"
}
},
{
"level": "a1-4",
"position": 104,
"text": "turn",
"meaning": {
"fr": "tourner ; (n.) tour (à jouer)"
}
},
{
"level": "a1-4",
"position": 105,
"text": "TV",
"meaning": {
"fr": "télé"
}
},
{
"level": "a1-4",
"position": 106,
"text": "twelve",
"meaning": {
"fr": "douze"
}
},
{
"level": "a1-4",
"position": 107,
"text": "twenty",
"meaning": {
"fr": "vingt"
}
},
{
"level": "a1-4",
"position": 108,
"text": "two",
"meaning": {
"fr": "deux"
}
},
{
"level": "a1-4",
"position": 109,
"text": "uncle",
"meaning": {
"fr": "oncle"
}
},
{
"level": "a1-4",
"position": 110,
"text": "under",
"meaning": {
"fr": "sous, au-dessous de"
}
},
{
"level": "a1-4",
"position": 111,
"text": "understand",
"meaning": {
"fr": "comprendre"
}
},
{
"level": "a1-4",
"position": 112,
"text": "university",
"meaning": {
"fr": "université"
}
},
{
"level": "a1-4",
"position": 113,
"text": "until",
"meaning": {
"fr": "jusqu'à"
}
},
{
"level": "a1-4",
"position": 114,
"text": "up",
"meaning": {
"fr": "en haut, vers le haut"
}
},
{
"level": "a1-4",
"position": 115,
"text": "us",
"meaning": {
"fr": "nous (complément)"
}
},
{
"level": "a1-4",
"position": 116,
"text": "use",
"meaning": {
"fr": "utiliser ; (n.) usage"
}
},
{
"level": "a1-4",
"position": 117,
"text": "usually",
"meaning": {
"fr": "habituellement, d'habitude"
}
},
{
"level": "a1-4",
"position": 118,
"text": "vacation",
"meaning": {
"fr": "vacances"
}
},
{
"level": "a1-4",
"position": 119,
"text": "vegetable",
"meaning": {
"fr": "légume"
}
},
{
"level": "a1-4",
"position": 120,
"text": "very",
"meaning": {
"fr": "très"
}
},
{
"level": "a1-4",
"position": 121,
"text": "video",
"meaning": {
"fr": "vidéo"
}
},
{
"level": "a1-4",
"position": 122,
"text": "village",
"meaning": {
"fr": "village"
}
},
{
"level": "a1-4",
"position": 123,
"text": "visit",
"meaning": {
"fr": "visiter, rendre visite ; (n.) visite"
}
},
{
"level": "a1-4",
"position": 124,
"text": "wait",
"meaning": {
"fr": "attendre"
}
},
{
"level": "a1-4",
"position": 125,
"text": "walk",
"meaning": {
"fr": "marcher ; (n.) promenade"
}
},
{
"level": "a1-4",
"position": 126,
"text": "wall",
"meaning": {
"fr": "mur"
}
},
{
"level": "a1-4",
"position": 127,
"text": "want",
"meaning": {
"fr": "vouloir"
}
},
{
"level": "a1-4",
"position": 128,
"text": "warm",
"meaning": {
"fr": "chaud, chaleureux"
}
},
{
"level": "a1-4",
"position": 129,
"text": "wash",
"meaning": {
"fr": "laver, se laver"
}
},
{
"level": "a1-4",
"position": 130,
"text": "watch",
"meaning": {
"fr": "regarder ; (n.) montre"
}
},
{
"level": "a1-4",
"position": 131,
"text": "water",
"meaning": {
"fr": "(n.) eau ; (v.) arroser"
}
},
{
"level": "a1-4",
"position": 132,
"text": "way",
"meaning": {
"fr": "chemin, façon, manière"
}
},
{
"level": "a1-4",
"position": 133,
"text": "we",
"meaning": {
"fr": "nous"
}
},
{
"level": "a1-4",
"position": 134,
"text": "wear",
"meaning": {
"fr": "porter (un vêtement)"
}
},
{
"level": "a1-4",
"position": 135,
"text": "weather",
"meaning": {
"fr": "temps, météo"
}
},
{
"level": "a1-4",
"position": 136,
"text": "week",
"meaning": {
"fr": "semaine"
}
},
{
"level": "a1-4",
"position": 137,
"text": "weekend",
"meaning": {
"fr": "week-end"
}
},
{
"level": "a1-4",
"position": 138,
"text": "welcome",
"meaning": {
"fr": "bienvenue ; (v.) accueillir"
}
},
{
"level": "a1-4",
"position": 139,
"text": "well",
"meaning": {
"fr": "bien ; (n.) puits"
}
},
{
"level": "a1-4",
"position": 140,
"text": "west",
"meaning": {
"fr": "ouest"
}
},
{
"level": "a1-4",
"position": 141,
"text": "what",
"meaning": {
"fr": "quoi, que, quel"
}
},
{
"level": "a1-4",
"position": 142,
"text": "when",
"meaning": {
"fr": "quand, lorsque"
}
},
{
"level": "a1-4",
"position": 143,
"text": "where",
"meaning": {
"fr": "où"
}
},
{
"level": "a1-4",
"position": 144,
"text": "which",
"meaning": {
"fr": "quel, lequel, qui"
}
},
{
"level": "a1-4",
"position": 145,
"text": "white",
"meaning": {
"fr": "blanc"
}
},
{
"level": "a1-4",
"position": 146,
"text": "who",
"meaning": {
"fr": "qui"
}
},
{
"level": "a1-4",
"position": 147,
"text": "why",
"meaning": {
"fr": "pourquoi"
}
},
{
"level": "a1-4",
"position": 148,
"text": "wife",
"meaning": {
"fr": "épouse, femme (mariée)"
}
},
{
"level": "a1-4",
"position": 149,
"text": "will",
"meaning": {
"fr": "volonté, testament ; (aux.) futur"
}
},
{
"level": "a1-4",
"position": 150,
"text": "win",
"meaning": {
"fr": "gagner ; (n.) victoire (match)"
}
},
{
"level": "a1-4",
"position": 151,
"text": "window",
"meaning": {
"fr": "fenêtre"
}
},
{
"level": "a1-4",
"position": 152,
"text": "wine",
"meaning": {
"fr": "vin"
}
},
{
"level": "a1-4",
"position": 153,
"text": "winter",
"meaning": {
"fr": "hiver"
}
},
{
"level": "a1-4",
"position": 154,
"text": "with",
"meaning": {
"fr": "avec"
}
},
{
"level": "a1-4",
"position": 155,
"text": "without",
"meaning": {
"fr": "sans"
}
},
{
"level": "a1-4",
"position": 156,
"text": "woman",
"meaning": {
"fr": "femme"
}
},
{
"level": "a1-4",
"position": 157,
"text": "wonderful",
"meaning": {
"fr": "merveilleux, formidable"
}
},
{
"level": "a1-4",
"position": 158,
"text": "word",
"meaning": {
"fr": "mot, parole"
}
},
{
"level": "a1-4",
"position": 159,
"text": "work",
"meaning": {
"fr": "travailler ; (n.) travail"
}
},
{
"level": "a1-4",
"position": 160,
"text": "world",
"meaning": {
"fr": "monde"
}
},
{
"level": "a1-4",
"position": 161,
"text": "would",
"meaning": {
"fr": "conditionnel (aux.)"
}
},
{
"level": "a1-4",
"position": 162,
"text": "write",
"meaning": {
"fr": "écrire"
}
},
{
"level": "a1-4",
"position": 163,
"text": "writer",
"meaning": {
"fr": "écrivain, auteur"
}
},
{
"level": "a1-4",
"position": 164,
"text": "wrong",
"meaning": {
"fr": "faux, mauvais, erroné"
}
},
{
"level": "a1-4",
"position": 165,
"text": "yeah",
"meaning": {
"fr": "ouais"
}
},
{
"level": "a1-4",
"position": 166,
"text": "year",
"meaning": {
"fr": "année, an"
}
},
{
"level": "a1-4",
"position": 167,
"text": "yellow",
"meaning": {
"fr": "jaune"
}
},
{
"level": "a1-4",
"position": 168,
"text": "yes",
"meaning": {
"fr": "oui"
}
},
{
"level": "a1-4",
"position": 169,
"text": "yesterday",
"meaning": {
"fr": "hier"
}
},
{
"level": "a1-4",
"position": 170,
"text": "you",
"meaning": {
"fr": "tu, vous"
}
},
{
"level": "a1-4",
"position": 171,
"text": "young",
"meaning": {
"fr": "jeune"
}
},
{
"level": "a1-4",
"position": 172,
"text": "your",
"meaning": {
"fr": "ton, ta, votre"
}
},
{
"level": "a2-1",
"position": 1,
"text": "able",
"meaning": {
"fr": "capable de, apte"
}
},
{
"level": "a2-1",
"position": 2,
"text": "abroad",
"meaning": {
"fr": "à l'étranger"
}
},
{
"level": "a2-1",
"position": 3,
"text": "accept",
"meaning": {
"fr": "accepter"
}
},
{
"level": "a2-1",
"position": 4,
"text": "accident",
"meaning": {
"fr": "accident"
}
},
{
"level": "a2-1",
"position": 5,
"text": "according",
"meaning": {
"fr": "selon (according to)"
}
},
{
"level": "a2-1",
"position": 6,
"text": "act",
"meaning": {
"fr": "(v.) agir, jouer ; (n.) acte"
}
},
{
"level": "a2-1",
"position": 7,
"text": "action",
"meaning": {
"fr": "action"
}
},
{
"level": "a2-1",
"position": 8,
"text": "active",
"meaning": {
"fr": "actif"
}
},
{
"level": "a2-1",
"position": 9,
"text": "actually",
"meaning": {
"fr": "en fait, vraiment"
}
},
{
"level": "a2-1",
"position": 10,
"text": "advantage",
"meaning": {
"fr": "avantage"
}
},
{
"level": "a2-1",
"position": 11,
"text": "adventure",
"meaning": {
"fr": "aventure"
}
},
{
"level": "a2-1",
"position": 12,
"text": "advice",
"meaning": {
"fr": "conseil(s)"
}
},
{
"level": "a2-1",
"position": 13,
"text": "afraid",
"meaning": {
"fr": "effrayé, avoir peur"
}
},
{
"level": "a2-1",
"position": 14,
"text": "African",
"meaning": {
"fr": "africain"
}
},
{
"level": "a2-1",
"position": 15,
"text": "against",
"meaning": {
"fr": "contre"
}
},
{
"level": "a2-1",
"position": 16,
"text": "ah",
"meaning": {
"fr": "ah"
}
},
{
"level": "a2-1",
"position": 17,
"text": "album",
"meaning": {
"fr": "album"
}
},
{
"level": "a2-1",
"position": 18,
"text": "alive",
"meaning": {
"fr": "vivant, en vie"
}
},
{
"level": "a2-1",
"position": 19,
"text": "allow",
"meaning": {
"fr": "permettre, autoriser"
}
},
{
"level": "a2-1",
"position": 20,
"text": "almost",
"meaning": {
"fr": "presque"
}
},
{
"level": "a2-1",
"position": 21,
"text": "alone",
"meaning": {
"fr": "seul"
}
},
{
"level": "a2-1",
"position": 22,
"text": "along",
"meaning": {
"fr": "le long de"
}
},
{
"level": "a2-1",
"position": 23,
"text": "although",
"meaning": {
"fr": "bien que, quoique"
}
},
{
"level": "a2-1",
"position": 24,
"text": "AM",
"meaning": {
"fr": "du matin (heure)"
}
},
{
"level": "a2-1",
"position": 25,
"text": "ancient",
"meaning": {
"fr": "ancien, antique"
}
},
{
"level": "a2-1",
"position": 26,
"text": "anybody",
"meaning": {
"fr": "n'importe qui, personne (nég.)"
}
},
{
"level": "a2-1",
"position": 27,
"text": "anymore",
"meaning": {
"fr": "ne... plus"
}
},
{
"level": "a2-1",
"position": 28,
"text": "anyone",
"meaning": {
"fr": "quelqu'un, qui que ce soit"
}
},
{
"level": "a2-1",
"position": 29,
"text": "anyway",
"meaning": {
"fr": "de toute façon, quand même"
}
},
{
"level": "a2-1",
"position": 30,
"text": "anywhere",
"meaning": {
"fr": "n'importe où"
}
},
{
"level": "a2-1",
"position": 31,
"text": "appear",
"meaning": {
"fr": "apparaître, sembler"
}
},
{
"level": "a2-1",
"position": 32,
"text": "architect",
"meaning": {
"fr": "architecte"
}
},
{
"level": "a2-1",
"position": 33,
"text": "army",
"meaning": {
"fr": "armée"
}
},
{
"level": "a2-1",
"position": 34,
"text": "arrange",
"meaning": {
"fr": "organiser, arranger"
}
},
{
"level": "a2-1",
"position": 35,
"text": "Asian",
"meaning": {
"fr": "asiatique"
}
},
{
"level": "a2-1",
"position": 36,
"text": "asleep",
"meaning": {
"fr": "endormi"
}
},
{
"level": "a2-1",
"position": 37,
"text": "assistant",
"meaning": {
"fr": "assistant, adjoint"
}
},
{
"level": "a2-1",
"position": 38,
"text": "athlete",
"meaning": {
"fr": "athlète, sportif"
}
},
{
"level": "a2-1",
"position": 39,
"text": "attack",
"meaning": {
"fr": "(n.) attaque ; (v.) attaquer"
}
},
{
"level": "a2-1",
"position": 40,
"text": "attend",
"meaning": {
"fr": "assister à, fréquenter"
}
},
{
"level": "a2-1",
"position": 41,
"text": "attention",
"meaning": {
"fr": "attention"
}
},
{
"level": "a2-1",
"position": 42,
"text": "audience",
"meaning": {
"fr": "public, auditoire"
}
},
{
"level": "a2-1",
"position": 43,
"text": "author",
"meaning": {
"fr": "auteur"
}
},
{
"level": "a2-1",
"position": 44,
"text": "available",
"meaning": {
"fr": "disponible"
}
},
{
"level": "a2-1",
"position": 45,
"text": "average",
"meaning": {
"fr": "(adj.) moyen ; (n.) moyenne"
}
},
{
"level": "a2-1",
"position": 46,
"text": "avoid",
"meaning": {
"fr": "éviter"
}
},
{
"level": "a2-1",
"position": 47,
"text": "awful",
"meaning": {
"fr": "affreux, terrible"
}
},
{
"level": "a2-1",
"position": 48,
"text": "background",
"meaning": {
"fr": "arrière-plan, contexte, milieu"
}
},
{
"level": "a2-1",
"position": 49,
"text": "badly",
"meaning": {
"fr": "mal, gravement"
}
},
{
"level": "a2-1",
"position": 50,
"text": "bake",
"meaning": {
"fr": "cuire au four"
}
},
{
"level": "a2-1",
"position": 51,
"text": "bar",
"meaning": {
"fr": "bar, barre"
}
},
{
"level": "a2-1",
"position": 52,
"text": "basic",
"meaning": {
"fr": "de base, élémentaire"
}
},
{
"level": "a2-1",
"position": 53,
"text": "basket",
"meaning": {
"fr": "panier"
}
},
{
"level": "a2-1",
"position": 54,
"text": "battery",
"meaning": {
"fr": "pile, batterie"
}
},
{
"level": "a2-1",
"position": 55,
"text": "bean",
"meaning": {
"fr": "haricot, fève"
}
},
{
"level": "a2-1",
"position": 56,
"text": "bear",
"meaning": {
"fr": "(n.) ours ; (v.) supporter, porter"
}
},
{
"level": "a2-1",
"position": 57,
"text": "beat",
"meaning": {
"fr": "(v.) battre ; (n.) rythme"
}
},
{
"level": "a2-1",
"position": 58,
"text": "beauty",
"meaning": {
"fr": "beauté"
}
},
{
"level": "a2-1",
"position": 59,
"text": "bell",
"meaning": {
"fr": "cloche, sonnette"
}
},
{
"level": "a2-1",
"position": 60,
"text": "belong",
"meaning": {
"fr": "appartenir"
}
},
{
"level": "a2-1",
"position": 61,
"text": "belt",
"meaning": {
"fr": "ceinture"
}
},
{
"level": "a2-1",
"position": 62,
"text": "beside",
"meaning": {
"fr": "à côté de"
}
},
{
"level": "a2-1",
"position": 63,
"text": "bill",
"meaning": {
"fr": "facture, addition, projet de loi"
}
},
{
"level": "a2-1",
"position": 64,
"text": "billion",
"meaning": {
"fr": "milliard"
}
},
{
"level": "a2-1",
"position": 65,
"text": "birth",
"meaning": {
"fr": "naissance"
}
},
{
"level": "a2-1",
"position": 66,
"text": "bit",
"meaning": {
"fr": "morceau, un peu"
}
},
{
"level": "a2-1",
"position": 67,
"text": "bite",
"meaning": {
"fr": "(v.) mordre ; (n.) morsure, bouchée"
}
},
{
"level": "a2-1",
"position": 68,
"text": "block",
"meaning": {
"fr": "(n.) bloc, pâté de maisons ; (v.) bloquer"
}
},
{
"level": "a2-1",
"position": 69,
"text": "blood",
"meaning": {
"fr": "sang"
}
},
{
"level": "a2-1",
"position": 70,
"text": "board",
"meaning": {
"fr": "planche, tableau, conseil"
}
},
{
"level": "a2-1",
"position": 71,
"text": "bone",
"meaning": {
"fr": "os"
}
},
{
"level": "a2-1",
"position": 72,
"text": "borrow",
"meaning": {
"fr": "emprunter"
}
},
{
"level": "a2-1",
"position": 73,
"text": "boss",
"meaning": {
"fr": "patron"
}
},
{
"level": "a2-1",
"position": 74,
"text": "bottom",
"meaning": {
"fr": "fond, bas"
}
},
{
"level": "a2-1",
"position": 75,
"text": "bowl",
"meaning": {
"fr": "bol"
}
},
{
"level": "a2-1",
"position": 76,
"text": "brain",
"meaning": {
"fr": "cerveau"
}
},
{
"level": "a2-1",
"position": 77,
"text": "bridge",
"meaning": {
"fr": "pont"
}
},
{
"level": "a2-1",
"position": 78,
"text": "bright",
"meaning": {
"fr": "lumineux, vif"
}
},
{
"level": "a2-1",
"position": 79,
"text": "brilliant",
"meaning": {
"fr": "brillant, génial"
}
},
{
"level": "a2-1",
"position": 80,
"text": "broken",
"meaning": {
"fr": "cassé, brisé"
}
},
{
"level": "a2-1",
"position": 81,
"text": "brush",
"meaning": {
"fr": "(n.) brosse ; (v.) brosser"
}
},
{
"level": "a2-1",
"position": 82,
"text": "burn",
"meaning": {
"fr": "brûler"
}
},
{
"level": "a2-1",
"position": 83,
"text": "button",
"meaning": {
"fr": "bouton"
}
},
{
"level": "a2-1",
"position": 84,
"text": "camp",
"meaning": {
"fr": "camp"
}
},
{
"level": "a2-1",
"position": 85,
"text": "Canadian",
"meaning": {
"fr": "canadien"
}
},
{
"level": "a2-1",
"position": 86,
"text": "cap",
"meaning": {
"fr": "casquette, bouchon"
}
},
{
"level": "a2-1",
"position": 87,
"text": "capital",
"meaning": {
"fr": "capitale, capital"
}
},
{
"level": "a2-1",
"position": 88,
"text": "captain",
"meaning": {
"fr": "capitaine"
}
},
{
"level": "a2-1",
"position": 89,
"text": "care",
"meaning": {
"fr": "(n.) soin ; (v.) se soucier"
}
},
{
"level": "a2-1",
"position": 90,
"text": "career",
"meaning": {
"fr": "carrière"
}
},
{
"level": "a2-1",
"position": 91,
"text": "careful",
"meaning": {
"fr": "prudent, attentif"
}
},
{
"level": "a2-1",
"position": 92,
"text": "carefully",
"meaning": {
"fr": "soigneusement, prudemment"
}
},
{
"level": "a2-1",
"position": 93,
"text": "carry",
"meaning": {
"fr": "porter, transporter"
}
},
{
"level": "a2-1",
"position": 94,
"text": "case",
"meaning": {
"fr": "cas, affaire, étui"
}
},
{
"level": "a2-1",
"position": 95,
"text": "cash",
"meaning": {
"fr": "espèces, liquide"
}
},
{
"level": "a2-1",
"position": 96,
"text": "catch",
"meaning": {
"fr": "attraper"
}
},
{
"level": "a2-1",
"position": 97,
"text": "cause",
"meaning": {
"fr": "(n.) cause ; (v.) causer"
}
},
{
"level": "a2-1",
"position": 98,
"text": "celebrate",
"meaning": {
"fr": "célébrer, fêter"
}
},
{
"level": "a2-1",
"position": 99,
"text": "central",
"meaning": {
"fr": "central"
}
},
{
"level": "a2-1",
"position": 100,
"text": "century",
"meaning": {
"fr": "siècle"
}
},
{
"level": "a2-1",
"position": 101,
"text": "certain",
"meaning": {
"fr": "certain, sûr"
}
},
{
"level": "a2-1",
"position": 102,
"text": "certainly",
"meaning": {
"fr": "certainement"
}
},
{
"level": "a2-1",
"position": 103,
"text": "chance",
"meaning": {
"fr": "chance, occasion, hasard"
}
},
{
"level": "a2-1",
"position": 104,
"text": "chapter",
"meaning": {
"fr": "chapitre"
}
},
{
"level": "a2-1",
"position": 105,
"text": "character",
"meaning": {
"fr": "personnage, caractère"
}
},
{
"level": "a2-1",
"position": 106,
"text": "chart",
"meaning": {
"fr": "graphique, tableau"
}
},
{
"level": "a2-1",
"position": 107,
"text": "chef",
"meaning": {
"fr": "chef cuisinier"
}
},
{
"level": "a2-1",
"position": 108,
"text": "childhood",
"meaning": {
"fr": "enfance"
}
},
{
"level": "a2-1",
"position": 109,
"text": "chip",
"meaning": {
"fr": "frite, puce, éclat"
}
},
{
"level": "a2-1",
"position": 110,
"text": "choice",
"meaning": {
"fr": "choix"
}
},
{
"level": "a2-1",
"position": 111,
"text": "church",
"meaning": {
"fr": "église"
}
},
{
"level": "a2-1",
"position": 112,
"text": "cigarette",
"meaning": {
"fr": "cigarette"
}
},
{
"level": "a2-1",
"position": 113,
"text": "circle",
"meaning": {
"fr": "cercle"
}
},
{
"level": "a2-1",
"position": 114,
"text": "clear",
"meaning": {
"fr": "(adj.) clair ; (v.) dégager"
}
},
{
"level": "a2-1",
"position": 115,
"text": "clearly",
"meaning": {
"fr": "clairement"
}
},
{
"level": "a2-1",
"position": 116,
"text": "climate",
"meaning": {
"fr": "climat"
}
},
{
"level": "a2-1",
"position": 117,
"text": "closer",
"meaning": {
"fr": "plus proche, plus près"
}
},
{
"level": "a2-1",
"position": 118,
"text": "clothing",
"meaning": {
"fr": "habillement, vêtements (collectif)"
}
},
{
"level": "a2-1",
"position": 119,
"text": "cloud",
"meaning": {
"fr": "nuage"
}
},
{
"level": "a2-1",
"position": 120,
"text": "coach",
"meaning": {
"fr": "(n.) entraîneur ; (v.) entraîner"
}
},
{
"level": "a2-1",
"position": 121,
"text": "coast",
"meaning": {
"fr": "côte, littoral"
}
},
{
"level": "a2-1",
"position": 122,
"text": "colleague",
"meaning": {
"fr": "collègue"
}
},
{
"level": "a2-1",
"position": 123,
"text": "collect",
"meaning": {
"fr": "collecter, ramasser"
}
},
{
"level": "a2-1",
"position": 124,
"text": "collection",
"meaning": {
"fr": "collection, collecte"
}
},
{
"level": "a2-1",
"position": 125,
"text": "comedy",
"meaning": {
"fr": "comédie"
}
},
{
"level": "a2-1",
"position": 126,
"text": "comfortable",
"meaning": {
"fr": "confortable, à l'aise"
}
},
{
"level": "a2-1",
"position": 127,
"text": "comment",
"meaning": {
"fr": "commentaire ; (v.) commenter"
}
},
{
"level": "a2-1",
"position": 128,
"text": "common",
"meaning": {
"fr": "commun, courant, fréquent"
}
},
{
"level": "a2-1",
"position": 129,
"text": "company",
"meaning": {
"fr": "entreprise, société, compagnie"
}
},
{
"level": "a2-1",
"position": 130,
"text": "compare",
"meaning": {
"fr": "comparer"
}
},
{
"level": "a2-1",
"position": 131,
"text": "competition",
"meaning": {
"fr": "compétition, concours, concurrence"
}
},
{
"level": "a2-1",
"position": 132,
"text": "complain",
"meaning": {
"fr": "se plaindre, porter plainte"
}
},
{
"level": "a2-1",
"position": 133,
"text": "complete",
"meaning": {
"fr": "complet ; (v.) terminer, compléter"
}
},
{
"level": "a2-1",
"position": 134,
"text": "completely",
"meaning": {
"fr": "complètement, entièrement"
}
},
{
"level": "a2-1",
"position": 135,
"text": "condition",
"meaning": {
"fr": "condition, état"
}
},
{
"level": "a2-1",
"position": 136,
"text": "connect",
"meaning": {
"fr": "connecter, relier"
}
},
{
"level": "a2-1",
"position": 137,
"text": "consider",
"meaning": {
"fr": "envisager, considérer"
}
},
{
"level": "a2-1",
"position": 138,
"text": "contact",
"meaning": {
"fr": "contact ; (v.) contacter"
}
},
{
"level": "a2-1",
"position": 139,
"text": "contain",
"meaning": {
"fr": "contenir"
}
},
{
"level": "a2-1",
"position": 140,
"text": "continue",
"meaning": {
"fr": "continuer, poursuivre"
}
},
{
"level": "a2-1",
"position": 141,
"text": "control",
"meaning": {
"fr": "contrôle, maîtrise ; (v.) contrôler"
}
},
{
"level": "a2-1",
"position": 142,
"text": "copy",
"meaning": {
"fr": "copie, exemplaire ; (v.) copier"
}
},
{
"level": "a2-1",
"position": 143,
"text": "corner",
"meaning": {
"fr": "coin, angle, virage"
}
},
{
"level": "a2-1",
"position": 144,
"text": "cost",
"meaning": {
"fr": "coût ; (v.) coûter"
}
},
{
"level": "a2-1",
"position": 145,
"text": "count",
"meaning": {
"fr": "compter ; (n.) compte, comte"
}
},
{
"level": "a2-1",
"position": 146,
"text": "couple",
"meaning": {
"fr": "couple, deux ou trois"
}
},
{
"level": "a2-1",
"position": 147,
"text": "course",
"meaning": {
"fr": "cours, stage, parcours"
}
},
{
"level": "a2-1",
"position": 148,
"text": "cover",
"meaning": {
"fr": "couvrir ; (n.) couverture"
}
},
{
"level": "a2-1",
"position": 149,
"text": "crazy",
"meaning": {
"fr": "fou, dingue"
}
},
{
"level": "a2-1",
"position": 150,
"text": "cream",
"meaning": {
"fr": "crème"
}
},
{
"level": "a2-1",
"position": 151,
"text": "create",
"meaning": {
"fr": "créer"
}
},
{
"level": "a2-1",
"position": 152,
"text": "crime",
"meaning": {
"fr": "crime, criminalité"
}
},
{
"level": "a2-1",
"position": 153,
"text": "cross",
"meaning": {
"fr": "traverser ; (n.) croix"
}
},
{
"level": "a2-1",
"position": 154,
"text": "crowd",
"meaning": {
"fr": "foule"
}
},
{
"level": "a2-1",
"position": 155,
"text": "cry",
"meaning": {
"fr": "pleurer, crier ; (n.) cri"
}
},
{
"level": "a2-1",
"position": 156,
"text": "culture",
"meaning": {
"fr": "culture"
}
},
{
"level": "a2-1",
"position": 157,
"text": "customer",
"meaning": {
"fr": "client"
}
},
{
"level": "a2-1",
"position": 158,
"text": "cut",
"meaning": {
"fr": "couper ; (n.) coupure, réduction"
}
},
{
"level": "a2-1",
"position": 159,
"text": "daily",
"meaning": {
"fr": "quotidien, journalier"
}
},
{
"level": "a2-1",
"position": 160,
"text": "danger",
"meaning": {
"fr": "danger"
}
},
{
"level": "a2-1",
"position": 161,
"text": "dangerous",
"meaning": {
"fr": "dangereux"
}
},
{
"level": "a2-1",
"position": 162,
"text": "dead",
"meaning": {
"fr": "mort (adj.), décédé"
}
},
{
"level": "a2-1",
"position": 163,
"text": "death",
"meaning": {
"fr": "mort, décès"
}
},
{
"level": "a2-1",
"position": 164,
"text": "decide",
"meaning": {
"fr": "décider"
}
},
{
"level": "a2-2",
"position": 1,
"text": "decision",
"meaning": {
"fr": "décision"
}
},
{
"level": "a2-2",
"position": 2,
"text": "deep",
"meaning": {
"fr": "profond"
}
},
{
"level": "a2-2",
"position": 3,
"text": "definitely",
"meaning": {
"fr": "sans aucun doute, absolument"
}
},
{
"level": "a2-2",
"position": 4,
"text": "definition",
"meaning": {
"fr": "définition"
}
},
{
"level": "a2-2",
"position": 5,
"text": "degree",
"meaning": {
"fr": "degré, diplôme"
}
},
{
"level": "a2-2",
"position": 6,
"text": "department",
"meaning": {
"fr": "département, service, rayon"
}
},
{
"level": "a2-2",
"position": 7,
"text": "depend",
"meaning": {
"fr": "dépendre"
}
},
{
"level": "a2-2",
"position": 8,
"text": "desert",
"meaning": {
"fr": "désert ; (v.) abandonner"
}
},
{
"level": "a2-2",
"position": 9,
"text": "design",
"meaning": {
"fr": "conception, design ; (v.) concevoir"
}
},
{
"level": "a2-2",
"position": 10,
"text": "designer",
"meaning": {
"fr": "designer, créateur"
}
},
{
"level": "a2-2",
"position": 11,
"text": "destroy",
"meaning": {
"fr": "détruire"
}
},
{
"level": "a2-2",
"position": 12,
"text": "detail",
"meaning": {
"fr": "détail"
}
},
{
"level": "a2-2",
"position": 13,
"text": "develop",
"meaning": {
"fr": "développer"
}
},
{
"level": "a2-2",
"position": 14,
"text": "dialogue",
"meaning": {
"fr": "dialogue"
}
},
{
"level": "a2-2",
"position": 15,
"text": "die",
"meaning": {
"fr": "mourir"
}
},
{
"level": "a2-2",
"position": 16,
"text": "diet",
"meaning": {
"fr": "alimentation, régime"
}
},
{
"level": "a2-2",
"position": 17,
"text": "differently",
"meaning": {
"fr": "différemment"
}
},
{
"level": "a2-2",
"position": 18,
"text": "direct",
"meaning": {
"fr": "direct ; (v.) diriger"
}
},
{
"level": "a2-2",
"position": 19,
"text": "direction",
"meaning": {
"fr": "direction, sens"
}
},
{
"level": "a2-2",
"position": 20,
"text": "director",
"meaning": {
"fr": "directeur, réalisateur"
}
},
{
"level": "a2-2",
"position": 21,
"text": "disagree",
"meaning": {
"fr": "ne pas être d'accord"
}
},
{
"level": "a2-2",
"position": 22,
"text": "discover",
"meaning": {
"fr": "découvrir"
}
},
{
"level": "a2-2",
"position": 23,
"text": "discuss",
"meaning": {
"fr": "discuter de, débattre de"
}
},
{
"level": "a2-2",
"position": 24,
"text": "discussion",
"meaning": {
"fr": "discussion"
}
},
{
"level": "a2-2",
"position": 25,
"text": "dish",
"meaning": {
"fr": "plat, assiette"
}
},
{
"level": "a2-2",
"position": 26,
"text": "distance",
"meaning": {
"fr": "distance"
}
},
{
"level": "a2-2",
"position": 27,
"text": "document",
"meaning": {
"fr": "document"
}
},
{
"level": "a2-2",
"position": 28,
"text": "double",
"meaning": {
"fr": "double ; (v.) doubler"
}
},
{
"level": "a2-2",
"position": 29,
"text": "dozen",
"meaning": {
"fr": "douzaine"
}
},
{
"level": "a2-2",
"position": 30,
"text": "drama",
"meaning": {
"fr": "drame, théâtre"
}
},
{
"level": "a2-2",
"position": 31,
"text": "driver",
"meaning": {
"fr": "conducteur, chauffeur"
}
},
{
"level": "a2-2",
"position": 32,
"text": "drop",
"meaning": {
"fr": "laisser tomber ; (n.) goutte, baisse"
}
},
{
"level": "a2-2",
"position": 33,
"text": "drug",
"meaning": {
"fr": "drogue, médicament"
}
},
{
"level": "a2-2",
"position": 34,
"text": "earn",
"meaning": {
"fr": "gagner (de l'argent)"
}
},
{
"level": "a2-2",
"position": 35,
"text": "earth",
"meaning": {
"fr": "terre, Terre"
}
},
{
"level": "a2-2",
"position": 36,
"text": "easily",
"meaning": {
"fr": "facilement"
}
},
{
"level": "a2-2",
"position": 37,
"text": "education",
"meaning": {
"fr": "éducation, enseignement"
}
},
{
"level": "a2-2",
"position": 38,
"text": "effect",
"meaning": {
"fr": "effet"
}
},
{
"level": "a2-2",
"position": 39,
"text": "effort",
"meaning": {
"fr": "effort"
}
},
{
"level": "a2-2",
"position": 40,
"text": "either",
"meaning": {
"fr": "l'un ou l'autre, non plus"
}
},
{
"level": "a2-2",
"position": 41,
"text": "electric",
"meaning": {
"fr": "électrique"
}
},
{
"level": "a2-2",
"position": 42,
"text": "electricity",
"meaning": {
"fr": "électricité"
}
},
{
"level": "a2-2",
"position": 43,
"text": "else",
"meaning": {
"fr": "autre, d'autre, sinon"
}
},
{
"level": "a2-2",
"position": 44,
"text": "employee",
"meaning": {
"fr": "employé, salarié"
}
},
{
"level": "a2-2",
"position": 45,
"text": "empty",
"meaning": {
"fr": "vide ; (v.) vider"
}
},
{
"level": "a2-2",
"position": 46,
"text": "energy",
"meaning": {
"fr": "énergie"
}
},
{
"level": "a2-2",
"position": 47,
"text": "engine",
"meaning": {
"fr": "moteur"
}
},
{
"level": "a2-2",
"position": 48,
"text": "engineer",
"meaning": {
"fr": "ingénieur"
}
},
{
"level": "a2-2",
"position": 49,
"text": "entrance",
"meaning": {
"fr": "entrée (porte, accès)"
}
},
{
"level": "a2-2",
"position": 50,
"text": "environment",
"meaning": {
"fr": "environnement"
}
},
{
"level": "a2-2",
"position": 51,
"text": "equipment",
"meaning": {
"fr": "équipement, matériel"
}
},
{
"level": "a2-2",
"position": 52,
"text": "error",
"meaning": {
"fr": "erreur"
}
},
{
"level": "a2-2",
"position": 53,
"text": "especially",
"meaning": {
"fr": "surtout, particulièrement"
}
},
{
"level": "a2-2",
"position": 54,
"text": "essay",
"meaning": {
"fr": "dissertation, essai"
}
},
{
"level": "a2-2",
"position": 55,
"text": "etc",
"meaning": {
"fr": "etc."
}
},
{
"level": "a2-2",
"position": 56,
"text": "European",
"meaning": {
"fr": "européen"
}
},
{
"level": "a2-2",
"position": 57,
"text": "even",
"meaning": {
"fr": "même ; (adj.) pair, plat"
}
},
{
"level": "a2-2",
"position": 58,
"text": "event",
"meaning": {
"fr": "événement"
}
},
{
"level": "a2-2",
"position": 59,
"text": "ever",
"meaning": {
"fr": "jamais (question), déjà"
}
},
{
"level": "a2-2",
"position": 60,
"text": "everywhere",
"meaning": {
"fr": "partout"
}
},
{
"level": "a2-2",
"position": 61,
"text": "exactly",
"meaning": {
"fr": "exactement"
}
},
{
"level": "a2-2",
"position": 62,
"text": "except",
"meaning": {
"fr": "sauf, excepté"
}
},
{
"level": "a2-2",
"position": 63,
"text": "exhibition",
"meaning": {
"fr": "exposition"
}
},
{
"level": "a2-2",
"position": 64,
"text": "expect",
"meaning": {
"fr": "s'attendre à, attendre"
}
},
{
"level": "a2-2",
"position": 65,
"text": "experience",
"meaning": {
"fr": "expérience ; (v.) vivre, connaître"
}
},
{
"level": "a2-2",
"position": 66,
"text": "expert",
"meaning": {
"fr": "expert"
}
},
{
"level": "a2-2",
"position": 67,
"text": "explanation",
"meaning": {
"fr": "explication"
}
},
{
"level": "a2-2",
"position": 68,
"text": "extra",
"meaning": {
"fr": "supplémentaire, en plus"
}
},
{
"level": "a2-2",
"position": 69,
"text": "extremely",
"meaning": {
"fr": "extrêmement"
}
},
{
"level": "a2-2",
"position": 70,
"text": "factory",
"meaning": {
"fr": "usine"
}
},
{
"level": "a2-2",
"position": 71,
"text": "fail",
"meaning": {
"fr": "échouer, rater"
}
},
{
"level": "a2-2",
"position": 72,
"text": "fair",
"meaning": {
"fr": "juste, équitable ; foire"
}
},
{
"level": "a2-2",
"position": 73,
"text": "fan",
"meaning": {
"fr": "fan ; ventilateur"
}
},
{
"level": "a2-2",
"position": 74,
"text": "fashion",
"meaning": {
"fr": "mode"
}
},
{
"level": "a2-2",
"position": 75,
"text": "fat",
"meaning": {
"fr": "gros ; graisse"
}
},
{
"level": "a2-2",
"position": 76,
"text": "feature",
"meaning": {
"fr": "caractéristique ; présenter"
}
},
{
"level": "a2-2",
"position": 77,
"text": "feed",
"meaning": {
"fr": "nourrir, alimenter"
}
},
{
"level": "a2-2",
"position": 78,
"text": "female",
"meaning": {
"fr": "femelle, féminin"
}
},
{
"level": "a2-2",
"position": 79,
"text": "fewer",
"meaning": {
"fr": "moins de (nombre)"
}
},
{
"level": "a2-2",
"position": 80,
"text": "fiction",
"meaning": {
"fr": "fiction"
}
},
{
"level": "a2-2",
"position": 81,
"text": "field",
"meaning": {
"fr": "champ, domaine"
}
},
{
"level": "a2-2",
"position": 82,
"text": "fight",
"meaning": {
"fr": "se battre ; combat"
}
},
{
"level": "a2-2",
"position": 83,
"text": "figure",
"meaning": {
"fr": "chiffre, silhouette, personnage"
}
},
{
"level": "a2-2",
"position": 84,
"text": "file",
"meaning": {
"fr": "dossier, fichier ; classer"
}
},
{
"level": "a2-2",
"position": 85,
"text": "final",
"meaning": {
"fr": "final ; finale"
}
},
{
"level": "a2-2",
"position": 86,
"text": "finally",
"meaning": {
"fr": "enfin, finalement"
}
},
{
"level": "a2-2",
"position": 87,
"text": "finger",
"meaning": {
"fr": "doigt"
}
},
{
"level": "a2-2",
"position": 88,
"text": "fire",
"meaning": {
"fr": "feu, incendie ; licencier"
}
},
{
"level": "a2-2",
"position": 89,
"text": "fishing",
"meaning": {
"fr": "pêche"
}
},
{
"level": "a2-2",
"position": 90,
"text": "fit",
"meaning": {
"fr": "aller (taille) ; en forme"
}
},
{
"level": "a2-2",
"position": 91,
"text": "fix",
"meaning": {
"fr": "réparer, fixer"
}
},
{
"level": "a2-2",
"position": 92,
"text": "flag",
"meaning": {
"fr": "drapeau"
}
},
{
"level": "a2-2",
"position": 93,
"text": "flat",
"meaning": {
"fr": "plat ; appartement"
}
},
{
"level": "a2-2",
"position": 94,
"text": "focus",
"meaning": {
"fr": "se concentrer ; centre"
}
},
{
"level": "a2-2",
"position": 95,
"text": "foreign",
"meaning": {
"fr": "étranger"
}
},
{
"level": "a2-2",
"position": 96,
"text": "forest",
"meaning": {
"fr": "forêt"
}
},
{
"level": "a2-2",
"position": 97,
"text": "forward",
"meaning": {
"fr": "en avant ; transmettre"
}
},
{
"level": "a2-2",
"position": 98,
"text": "friendship",
"meaning": {
"fr": "amitié"
}
},
{
"level": "a2-2",
"position": 99,
"text": "furniture",
"meaning": {
"fr": "meubles, mobilier"
}
},
{
"level": "a2-2",
"position": 100,
"text": "gallery",
"meaning": {
"fr": "galerie"
}
},
{
"level": "a2-2",
"position": 101,
"text": "garage",
"meaning": {
"fr": "garage"
}
},
{
"level": "a2-2",
"position": 102,
"text": "gas",
"meaning": {
"fr": "gaz, essence"
}
},
{
"level": "a2-2",
"position": 103,
"text": "gate",
"meaning": {
"fr": "portail, porte (embarquement)"
}
},
{
"level": "a2-2",
"position": 104,
"text": "general",
"meaning": {
"fr": "général"
}
},
{
"level": "a2-2",
"position": 105,
"text": "generally",
"meaning": {
"fr": "généralement"
}
},
{
"level": "a2-2",
"position": 106,
"text": "ghost",
"meaning": {
"fr": "fantôme"
}
},
{
"level": "a2-2",
"position": 107,
"text": "glad",
"meaning": {
"fr": "content, ravi"
}
},
{
"level": "a2-2",
"position": 108,
"text": "glove",
"meaning": {
"fr": "gant"
}
},
{
"level": "a2-2",
"position": 109,
"text": "goal",
"meaning": {
"fr": "but, objectif"
}
},
{
"level": "a2-2",
"position": 110,
"text": "God",
"meaning": {
"fr": "Dieu"
}
},
{
"level": "a2-2",
"position": 111,
"text": "gold",
"meaning": {
"fr": "or"
}
},
{
"level": "a2-2",
"position": 112,
"text": "golf",
"meaning": {
"fr": "golf"
}
},
{
"level": "a2-2",
"position": 113,
"text": "government",
"meaning": {
"fr": "gouvernement"
}
},
{
"level": "a2-2",
"position": 114,
"text": "grade",
"meaning": {
"fr": "note, classe"
}
},
{
"level": "a2-2",
"position": 115,
"text": "grass",
"meaning": {
"fr": "herbe"
}
},
{
"level": "a2-2",
"position": 116,
"text": "greatest",
"meaning": {
"fr": "le plus grand, le meilleur"
}
},
{
"level": "a2-2",
"position": 117,
"text": "ground",
"meaning": {
"fr": "sol, terrain"
}
},
{
"level": "a2-2",
"position": 118,
"text": "guest",
"meaning": {
"fr": "invité"
}
},
{
"level": "a2-2",
"position": 119,
"text": "guide",
"meaning": {
"fr": "guide ; guider"
}
},
{
"level": "a2-2",
"position": 120,
"text": "gun",
"meaning": {
"fr": "arme à feu, pistolet"
}
},
{
"level": "a2-2",
"position": 121,
"text": "guy",
"meaning": {
"fr": "type, gars"
}
},
{
"level": "a2-2",
"position": 122,
"text": "habit",
"meaning": {
"fr": "habitude"
}
},
{
"level": "a2-2",
"position": 123,
"text": "hall",
"meaning": {
"fr": "salle, couloir, hall"
}
},
{
"level": "a2-2",
"position": 124,
"text": "hang",
"meaning": {
"fr": "accrocher, pendre"
}
},
{
"level": "a2-2",
"position": 125,
"text": "health",
"meaning": {
"fr": "santé"
}
},
{
"level": "a2-2",
"position": 126,
"text": "heart",
"meaning": {
"fr": "cœur"
}
},
{
"level": "a2-2",
"position": 127,
"text": "heat",
"meaning": {
"fr": "chaleur ; chauffer"
}
},
{
"level": "a2-2",
"position": 128,
"text": "heavy",
"meaning": {
"fr": "lourd"
}
},
{
"level": "a2-2",
"position": 129,
"text": "height",
"meaning": {
"fr": "hauteur, taille"
}
},
{
"level": "a2-2",
"position": 130,
"text": "helicopter",
"meaning": {
"fr": "hélicoptère"
}
},
{
"level": "a2-2",
"position": 131,
"text": "helpful",
"meaning": {
"fr": "utile, serviable"
}
},
{
"level": "a2-2",
"position": 132,
"text": "hero",
"meaning": {
"fr": "héros"
}
},
{
"level": "a2-2",
"position": 133,
"text": "herself",
"meaning": {
"fr": "elle-même"
}
},
{
"level": "a2-2",
"position": 134,
"text": "hey",
"meaning": {
"fr": "hé, eh"
}
},
{
"level": "a2-2",
"position": 135,
"text": "hide",
"meaning": {
"fr": "cacher, se cacher"
}
},
{
"level": "a2-2",
"position": 136,
"text": "hill",
"meaning": {
"fr": "colline"
}
},
{
"level": "a2-2",
"position": 137,
"text": "himself",
"meaning": {
"fr": "lui-même"
}
},
{
"level": "a2-2",
"position": 138,
"text": "hit",
"meaning": {
"fr": "frapper ; succès"
}
},
{
"level": "a2-2",
"position": 139,
"text": "hold",
"meaning": {
"fr": "tenir, détenir"
}
},
{
"level": "a2-2",
"position": 140,
"text": "hole",
"meaning": {
"fr": "trou"
}
},
{
"level": "a2-2",
"position": 141,
"text": "honest",
"meaning": {
"fr": "honnête"
}
},
{
"level": "a2-2",
"position": 142,
"text": "however",
"meaning": {
"fr": "cependant, toutefois"
}
},
{
"level": "a2-2",
"position": 143,
"text": "huge",
"meaning": {
"fr": "énorme"
}
},
{
"level": "a2-2",
"position": 144,
"text": "human",
"meaning": {
"fr": "humain"
}
},
{
"level": "a2-2",
"position": 145,
"text": "hurt",
"meaning": {
"fr": "blesser, faire mal"
}
},
{
"level": "a2-2",
"position": 146,
"text": "ill",
"meaning": {
"fr": "malade"
}
},
{
"level": "a2-2",
"position": 147,
"text": "image",
"meaning": {
"fr": "image"
}
},
{
"level": "a2-2",
"position": 148,
"text": "immediately",
"meaning": {
"fr": "immédiatement"
}
},
{
"level": "a2-2",
"position": 149,
"text": "impossible",
"meaning": {
"fr": "impossible"
}
},
{
"level": "a2-2",
"position": 150,
"text": "improve",
"meaning": {
"fr": "améliorer"
}
},
{
"level": "a2-2",
"position": 151,
"text": "include",
"meaning": {
"fr": "inclure, comprendre"
}
},
{
"level": "a2-2",
"position": 152,
"text": "including",
"meaning": {
"fr": "y compris"
}
},
{
"level": "a2-2",
"position": 153,
"text": "increase",
"meaning": {
"fr": "augmenter ; augmentation"
}
},
{
"level": "a2-2",
"position": 154,
"text": "incredible",
"meaning": {
"fr": "incroyable"
}
},
{
"level": "a2-2",
"position": 155,
"text": "Indian",
"meaning": {
"fr": "indien"
}
},
{
"level": "a2-2",
"position": 156,
"text": "industry",
"meaning": {
"fr": "industrie"
}
},
{
"level": "a2-2",
"position": 157,
"text": "ingredient",
"meaning": {
"fr": "ingrédient"
}
},
{
"level": "a2-2",
"position": 158,
"text": "instead",
"meaning": {
"fr": "à la place, plutôt"
}
},
{
"level": "a2-2",
"position": 159,
"text": "instruction",
"meaning": {
"fr": "instruction, consigne"
}
},
{
"level": "a2-2",
"position": 160,
"text": "instrument",
"meaning": {
"fr": "instrument"
}
},
{
"level": "a2-2",
"position": 161,
"text": "international",
"meaning": {
"fr": "international"
}
},
{
"level": "a2-2",
"position": 162,
"text": "introduce",
"meaning": {
"fr": "présenter, introduire"
}
},
{
"level": "a2-2",
"position": 163,
"text": "Irish",
"meaning": {
"fr": "irlandais"
}
},
{
"level": "a2-2",
"position": 164,
"text": "item",
"meaning": {
"fr": "article, élément"
}
},
{
"level": "a2-3",
"position": 1,
"text": "itself",
"meaning": {
"fr": "lui-même, elle-même (chose)"
}
},
{
"level": "a2-3",
"position": 2,
"text": "joke",
"meaning": {
"fr": "blague ; plaisanter"
}
},
{
"level": "a2-3",
"position": 3,
"text": "journalist",
"meaning": {
"fr": "journaliste"
}
},
{
"level": "a2-3",
"position": 4,
"text": "journey",
"meaning": {
"fr": "voyage, trajet"
}
},
{
"level": "a2-3",
"position": 5,
"text": "jump",
"meaning": {
"fr": "sauter ; saut"
}
},
{
"level": "a2-3",
"position": 6,
"text": "kill",
"meaning": {
"fr": "tuer"
}
},
{
"level": "a2-3",
"position": 7,
"text": "king",
"meaning": {
"fr": "roi"
}
},
{
"level": "a2-3",
"position": 8,
"text": "kiss",
"meaning": {
"fr": "baiser ; embrasser"
}
},
{
"level": "a2-3",
"position": 9,
"text": "knee",
"meaning": {
"fr": "genou"
}
},
{
"level": "a2-3",
"position": 10,
"text": "knife",
"meaning": {
"fr": "couteau"
}
},
{
"level": "a2-3",
"position": 11,
"text": "lady",
"meaning": {
"fr": "dame"
}
},
{
"level": "a2-3",
"position": 12,
"text": "land",
"meaning": {
"fr": "terre ; atterrir"
}
},
{
"level": "a2-3",
"position": 13,
"text": "laugh",
"meaning": {
"fr": "rire"
}
},
{
"level": "a2-3",
"position": 14,
"text": "law",
"meaning": {
"fr": "loi, droit"
}
},
{
"level": "a2-3",
"position": 15,
"text": "leader",
"meaning": {
"fr": "chef, dirigeant, leader"
}
},
{
"level": "a2-3",
"position": 16,
"text": "leaf",
"meaning": {
"fr": "feuille (d'arbre)"
}
},
{
"level": "a2-3",
"position": 17,
"text": "least",
"meaning": {
"fr": "le moins"
}
},
{
"level": "a2-3",
"position": 18,
"text": "lemon",
"meaning": {
"fr": "citron"
}
},
{
"level": "a2-3",
"position": 19,
"text": "let",
"meaning": {
"fr": "laisser, permettre"
}
},
{
"level": "a2-3",
"position": 20,
"text": "level",
"meaning": {
"fr": "niveau"
}
},
{
"level": "a2-3",
"position": 21,
"text": "local",
"meaning": {
"fr": "local"
}
},
{
"level": "a2-3",
"position": 22,
"text": "lose",
"meaning": {
"fr": "perdre"
}
},
{
"level": "a2-3",
"position": 23,
"text": "lost",
"meaning": {
"fr": "perdu"
}
},
{
"level": "a2-3",
"position": 24,
"text": "loud",
"meaning": {
"fr": "fort, bruyant"
}
},
{
"level": "a2-3",
"position": 25,
"text": "lovely",
"meaning": {
"fr": "charmant, ravissant"
}
},
{
"level": "a2-3",
"position": 26,
"text": "low",
"meaning": {
"fr": "bas, faible"
}
},
{
"level": "a2-3",
"position": 27,
"text": "luck",
"meaning": {
"fr": "chance"
}
},
{
"level": "a2-3",
"position": 28,
"text": "lucky",
"meaning": {
"fr": "chanceux, qui a de la chance"
}
},
{
"level": "a2-3",
"position": 29,
"text": "machine",
"meaning": {
"fr": "machine"
}
},
{
"level": "a2-3",
"position": 30,
"text": "mail",
"meaning": {
"fr": "courrier, poste"
}
},
{
"level": "a2-3",
"position": 31,
"text": "main",
"meaning": {
"fr": "principal"
}
},
{
"level": "a2-3",
"position": 32,
"text": "mall",
"meaning": {
"fr": "centre commercial"
}
},
{
"level": "a2-3",
"position": 33,
"text": "manager",
"meaning": {
"fr": "responsable, gérant, directeur"
}
},
{
"level": "a2-3",
"position": 34,
"text": "marry",
"meaning": {
"fr": "épouser, se marier"
}
},
{
"level": "a2-3",
"position": 35,
"text": "match",
"meaning": {
"fr": "match, allumette ; (v.) assortir"
}
},
{
"level": "a2-3",
"position": 36,
"text": "may",
"meaning": {
"fr": "pouvoir (permission, éventualité)"
}
},
{
"level": "a2-3",
"position": 37,
"text": "mean",
"meaning": {
"fr": "signifier, vouloir dire ; (adj.) méchant"
}
},
{
"level": "a2-3",
"position": 38,
"text": "meaning",
"meaning": {
"fr": "sens, signification"
}
},
{
"level": "a2-3",
"position": 39,
"text": "medicine",
"meaning": {
"fr": "médecine, médicament"
}
},
{
"level": "a2-3",
"position": 40,
"text": "meeting",
"meaning": {
"fr": "réunion, rencontre"
}
},
{
"level": "a2-3",
"position": 41,
"text": "member",
"meaning": {
"fr": "membre"
}
},
{
"level": "a2-3",
"position": 42,
"text": "metal",
"meaning": {
"fr": "métal"
}
},
{
"level": "a2-3",
"position": 43,
"text": "method",
"meaning": {
"fr": "méthode"
}
},
{
"level": "a2-3",
"position": 44,
"text": "middle",
"meaning": {
"fr": "milieu ; (adj.) du milieu"
}
},
{
"level": "a2-3",
"position": 45,
"text": "might",
"meaning": {
"fr": "pourrait (faible possibilité)"
}
},
{
"level": "a2-3",
"position": 46,
"text": "mirror",
"meaning": {
"fr": "miroir"
}
},
{
"level": "a2-3",
"position": 47,
"text": "miss",
"meaning": {
"fr": "manquer, rater"
}
},
{
"level": "a2-3",
"position": 48,
"text": "mm-hmm",
"meaning": {
"fr": "hum hum, oui"
}
},
{
"level": "a2-3",
"position": 49,
"text": "model",
"meaning": {
"fr": "modèle, mannequin"
}
},
{
"level": "a2-3",
"position": 50,
"text": "modern",
"meaning": {
"fr": "moderne"
}
},
{
"level": "a2-3",
"position": 51,
"text": "moment",
"meaning": {
"fr": "moment, instant"
}
},
{
"level": "a2-3",
"position": 52,
"text": "moon",
"meaning": {
"fr": "lune"
}
},
{
"level": "a2-3",
"position": 53,
"text": "musical",
"meaning": {
"fr": "musical ; (n.) comédie musicale"
}
},
{
"level": "a2-3",
"position": 54,
"text": "musician",
"meaning": {
"fr": "musicien"
}
},
{
"level": "a2-3",
"position": 55,
"text": "myself",
"meaning": {
"fr": "moi-même, me"
}
},
{
"level": "a2-3",
"position": 56,
"text": "national",
"meaning": {
"fr": "national"
}
},
{
"level": "a2-3",
"position": 57,
"text": "natural",
"meaning": {
"fr": "naturel"
}
},
{
"level": "a2-3",
"position": 58,
"text": "nature",
"meaning": {
"fr": "nature"
}
},
{
"level": "a2-3",
"position": 59,
"text": "nearly",
"meaning": {
"fr": "presque"
}
},
{
"level": "a2-3",
"position": 60,
"text": "necessary",
"meaning": {
"fr": "nécessaire"
}
},
{
"level": "a2-3",
"position": 61,
"text": "neck",
"meaning": {
"fr": "cou"
}
},
{
"level": "a2-3",
"position": 62,
"text": "neighbor",
"meaning": {
"fr": "voisin"
}
},
{
"level": "a2-3",
"position": 63,
"text": "nervous",
"meaning": {
"fr": "nerveux, anxieux"
}
},
{
"level": "a2-3",
"position": 64,
"text": "nobody",
"meaning": {
"fr": "personne (aucun)"
}
},
{
"level": "a2-3",
"position": 65,
"text": "noise",
"meaning": {
"fr": "bruit"
}
},
{
"level": "a2-3",
"position": 66,
"text": "normal",
"meaning": {
"fr": "normal"
}
},
{
"level": "a2-3",
"position": 67,
"text": "normally",
"meaning": {
"fr": "normalement"
}
},
{
"level": "a2-3",
"position": 68,
"text": "note",
"meaning": {
"fr": "note, mot ; (v.) noter"
}
},
{
"level": "a2-3",
"position": 69,
"text": "notice",
"meaning": {
"fr": "remarquer ; (n.) avis, préavis"
}
},
{
"level": "a2-3",
"position": 70,
"text": "novel",
"meaning": {
"fr": "roman ; (adj.) nouveau, inédit"
}
},
{
"level": "a2-3",
"position": 71,
"text": "ocean",
"meaning": {
"fr": "océan"
}
},
{
"level": "a2-3",
"position": 72,
"text": "offer",
"meaning": {
"fr": "offrir, proposer ; (n.) offre"
}
},
{
"level": "a2-3",
"position": 73,
"text": "oil",
"meaning": {
"fr": "huile, pétrole"
}
},
{
"level": "a2-3",
"position": 74,
"text": "once",
"meaning": {
"fr": "une fois, autrefois"
}
},
{
"level": "a2-3",
"position": 75,
"text": "onion",
"meaning": {
"fr": "oignon"
}
},
{
"level": "a2-3",
"position": 76,
"text": "online",
"meaning": {
"fr": "en ligne"
}
},
{
"level": "a2-3",
"position": 77,
"text": "opinion",
"meaning": {
"fr": "opinion, avis"
}
},
{
"level": "a2-3",
"position": 78,
"text": "opposite",
"meaning": {
"fr": "opposé, contraire ; (prép.) en face de"
}
},
{
"level": "a2-3",
"position": 79,
"text": "order",
"meaning": {
"fr": "(v.) commander, ordonner ; (n.) ordre"
}
},
{
"level": "a2-3",
"position": 80,
"text": "original",
"meaning": {
"fr": "original, d'origine"
}
},
{
"level": "a2-3",
"position": 81,
"text": "ourselves",
"meaning": {
"fr": "nous-mêmes"
}
},
{
"level": "a2-3",
"position": 82,
"text": "own",
"meaning": {
"fr": "propre ; (v.) posséder"
}
},
{
"level": "a2-3",
"position": 83,
"text": "pain",
"meaning": {
"fr": "douleur"
}
},
{
"level": "a2-3",
"position": 84,
"text": "painter",
"meaning": {
"fr": "peintre"
}
},
{
"level": "a2-3",
"position": 85,
"text": "pale",
"meaning": {
"fr": "pâle"
}
},
{
"level": "a2-3",
"position": 86,
"text": "parking",
"meaning": {
"fr": "stationnement, parking"
}
},
{
"level": "a2-3",
"position": 87,
"text": "partner",
"meaning": {
"fr": "partenaire, conjoint"
}
},
{
"level": "a2-3",
"position": 88,
"text": "pass",
"meaning": {
"fr": "passer, réussir (examen)"
}
},
{
"level": "a2-3",
"position": 89,
"text": "passenger",
"meaning": {
"fr": "passager"
}
},
{
"level": "a2-3",
"position": 90,
"text": "past",
"meaning": {
"fr": "passé ; (prép.) après, devant"
}
},
{
"level": "a2-3",
"position": 91,
"text": "PC",
"meaning": {
"fr": "PC, ordinateur personnel"
}
},
{
"level": "a2-3",
"position": 92,
"text": "pepper",
"meaning": {
"fr": "poivre, poivron"
}
},
{
"level": "a2-3",
"position": 93,
"text": "perhaps",
"meaning": {
"fr": "peut-être, sans doute"
}
},
{
"level": "a2-3",
"position": 94,
"text": "period",
"meaning": {
"fr": "période, règles, point"
}
},
{
"level": "a2-3",
"position": 95,
"text": "personal",
"meaning": {
"fr": "personnel"
}
},
{
"level": "a2-3",
"position": 96,
"text": "pet",
"meaning": {
"fr": "animal de compagnie"
}
},
{
"level": "a2-3",
"position": 97,
"text": "photograph",
"meaning": {
"fr": "photographie"
}
},
{
"level": "a2-3",
"position": 98,
"text": "photographer",
"meaning": {
"fr": "photographe"
}
},
{
"level": "a2-3",
"position": 99,
"text": "phrase",
"meaning": {
"fr": "expression, locution"
}
},
{
"level": "a2-3",
"position": 100,
"text": "pick",
"meaning": {
"fr": "choisir, cueillir, ramasser"
}
},
{
"level": "a2-3",
"position": 101,
"text": "pie",
"meaning": {
"fr": "tourte, tarte"
}
},
{
"level": "a2-3",
"position": 102,
"text": "planet",
"meaning": {
"fr": "planète"
}
},
{
"level": "a2-3",
"position": 103,
"text": "plastic",
"meaning": {
"fr": "plastique"
}
},
{
"level": "a2-3",
"position": 104,
"text": "plate",
"meaning": {
"fr": "assiette, plaque"
}
},
{
"level": "a2-3",
"position": 105,
"text": "PM",
"meaning": {
"fr": "de l'après-midi (heure)"
}
},
{
"level": "a2-3",
"position": 106,
"text": "pocket",
"meaning": {
"fr": "poche"
}
},
{
"level": "a2-3",
"position": 107,
"text": "poem",
"meaning": {
"fr": "poème"
}
},
{
"level": "a2-3",
"position": 108,
"text": "point",
"meaning": {
"fr": "point, pointe ; (v.) montrer du doigt"
}
},
{
"level": "a2-3",
"position": 109,
"text": "pollution",
"meaning": {
"fr": "pollution"
}
},
{
"level": "a2-3",
"position": 110,
"text": "positive",
"meaning": {
"fr": "positif"
}
},
{
"level": "a2-3",
"position": 111,
"text": "possible",
"meaning": {
"fr": "possible"
}
},
{
"level": "a2-3",
"position": 112,
"text": "post",
"meaning": {
"fr": "(n.) poste, courrier ; (v.) poster"
}
},
{
"level": "a2-3",
"position": 113,
"text": "pound",
"meaning": {
"fr": "livre (monnaie, poids)"
}
},
{
"level": "a2-3",
"position": 114,
"text": "power",
"meaning": {
"fr": "pouvoir, puissance, énergie"
}
},
{
"level": "a2-3",
"position": 115,
"text": "practice",
"meaning": {
"fr": "(n.) pratique, entraînement ; (v.) s'exercer"
}
},
{
"level": "a2-3",
"position": 116,
"text": "prefer",
"meaning": {
"fr": "préférer"
}
},
{
"level": "a2-3",
"position": 117,
"text": "prepare",
"meaning": {
"fr": "préparer"
}
},
{
"level": "a2-3",
"position": 118,
"text": "present",
"meaning": {
"fr": "(adj.) présent ; (n.) cadeau"
}
},
{
"level": "a2-3",
"position": 119,
"text": "president",
"meaning": {
"fr": "président"
}
},
{
"level": "a2-3",
"position": 120,
"text": "pretty",
"meaning": {
"fr": "(adv.) assez ; (adj.) joli"
}
},
{
"level": "a2-3",
"position": 121,
"text": "print",
"meaning": {
"fr": "imprimer"
}
},
{
"level": "a2-3",
"position": 122,
"text": "probably",
"meaning": {
"fr": "probablement"
}
},
{
"level": "a2-3",
"position": 123,
"text": "product",
"meaning": {
"fr": "produit"
}
},
{
"level": "a2-3",
"position": 124,
"text": "program",
"meaning": {
"fr": "programme, émission"
}
},
{
"level": "a2-3",
"position": 125,
"text": "project",
"meaning": {
"fr": "projet"
}
},
{
"level": "a2-3",
"position": 126,
"text": "protect",
"meaning": {
"fr": "protéger"
}
},
{
"level": "a2-3",
"position": 127,
"text": "public",
"meaning": {
"fr": "public"
}
},
{
"level": "a2-3",
"position": 128,
"text": "pull",
"meaning": {
"fr": "tirer (vers soi)"
}
},
{
"level": "a2-3",
"position": 129,
"text": "push",
"meaning": {
"fr": "pousser"
}
},
{
"level": "a2-3",
"position": 130,
"text": "quality",
"meaning": {
"fr": "qualité"
}
},
{
"level": "a2-3",
"position": 131,
"text": "quarter",
"meaning": {
"fr": "quart, trimestre, quartier"
}
},
{
"level": "a2-3",
"position": 132,
"text": "quick",
"meaning": {
"fr": "rapide, vif"
}
},
{
"level": "a2-3",
"position": 133,
"text": "quickly",
"meaning": {
"fr": "vite, rapidement"
}
},
{
"level": "a2-3",
"position": 134,
"text": "quiet",
"meaning": {
"fr": "calme, tranquille"
}
},
{
"level": "a2-3",
"position": 135,
"text": "quite",
"meaning": {
"fr": "assez, tout à fait"
}
},
{
"level": "a2-3",
"position": 136,
"text": "race",
"meaning": {
"fr": "course, race"
}
},
{
"level": "a2-3",
"position": 137,
"text": "rather",
"meaning": {
"fr": "plutôt"
}
},
{
"level": "a2-3",
"position": 138,
"text": "reader",
"meaning": {
"fr": "lecteur"
}
},
{
"level": "a2-3",
"position": 139,
"text": "real",
"meaning": {
"fr": "réel, vrai"
}
},
{
"level": "a2-3",
"position": 140,
"text": "reason",
"meaning": {
"fr": "raison"
}
},
{
"level": "a2-3",
"position": 141,
"text": "receive",
"meaning": {
"fr": "recevoir"
}
},
{
"level": "a2-3",
"position": 142,
"text": "recently",
"meaning": {
"fr": "récemment"
}
},
{
"level": "a2-3",
"position": 143,
"text": "recipe",
"meaning": {
"fr": "recette"
}
},
{
"level": "a2-3",
"position": 144,
"text": "record",
"meaning": {
"fr": "(n.) record, dossier ; (v.) enregistrer"
}
},
{
"level": "a2-3",
"position": 145,
"text": "regular",
"meaning": {
"fr": "régulier, habituel"
}
},
{
"level": "a2-3",
"position": 146,
"text": "relax",
"meaning": {
"fr": "se détendre"
}
},
{
"level": "a2-3",
"position": 147,
"text": "reply",
"meaning": {
"fr": "(v.) répondre ; (n.) réponse"
}
},
{
"level": "a2-3",
"position": 148,
"text": "report",
"meaning": {
"fr": "(n.) rapport ; (v.) signaler"
}
},
{
"level": "a2-3",
"position": 149,
"text": "research",
"meaning": {
"fr": "recherche (scientifique)"
}
},
{
"level": "a2-3",
"position": 150,
"text": "rest",
"meaning": {
"fr": "(n.) repos, reste ; (v.) se reposer"
}
},
{
"level": "a2-3",
"position": 151,
"text": "result",
"meaning": {
"fr": "résultat"
}
},
{
"level": "a2-3",
"position": 152,
"text": "return",
"meaning": {
"fr": "(v.) revenir, rendre ; (n.) retour"
}
},
{
"level": "a2-3",
"position": 153,
"text": "rich",
"meaning": {
"fr": "riche"
}
},
{
"level": "a2-3",
"position": 154,
"text": "ring",
"meaning": {
"fr": "(n.) bague, anneau ; (v.) sonner"
}
},
{
"level": "a2-3",
"position": 155,
"text": "rock",
"meaning": {
"fr": "rocher, roche"
}
},
{
"level": "a2-3",
"position": 156,
"text": "round",
"meaning": {
"fr": "(adj.) rond ; (n.) tour, manche"
}
},
{
"level": "a2-3",
"position": 157,
"text": "routine",
"meaning": {
"fr": "routine"
}
},
{
"level": "a2-3",
"position": 158,
"text": "rule",
"meaning": {
"fr": "(n.) règle ; (v.) gouverner"
}
},
{
"level": "a2-3",
"position": 159,
"text": "safe",
"meaning": {
"fr": "sûr, en sécurité"
}
},
{
"level": "a2-3",
"position": 160,
"text": "sale",
"meaning": {
"fr": "vente, soldes"
}
},
{
"level": "a2-3",
"position": 161,
"text": "salt",
"meaning": {
"fr": "sel"
}
},
{
"level": "a2-3",
"position": 162,
"text": "sand",
"meaning": {
"fr": "sable"
}
},
{
"level": "a2-3",
"position": 163,
"text": "sauce",
"meaning": {
"fr": "sauce"
}
},
{
"level": "a2-3",
"position": 164,
"text": "save",
"meaning": {
"fr": "sauver, économiser"
}
},
{
"level": "a2-4",
"position": 1,
"text": "scared",
"meaning": {
"fr": "effrayé, qui a peur"
}
},
{
"level": "a2-4",
"position": 2,
"text": "scientist",
"meaning": {
"fr": "scientifique (n.)"
}
},
{
"level": "a2-4",
"position": 3,
"text": "score",
"meaning": {
"fr": "score, note"
}
},
{
"level": "a2-4",
"position": 4,
"text": "screen",
"meaning": {
"fr": "écran"
}
},
{
"level": "a2-4",
"position": 5,
"text": "search",
"meaning": {
"fr": "(v.) chercher ; (n.) recherche"
}
},
{
"level": "a2-4",
"position": 6,
"text": "season",
"meaning": {
"fr": "saison"
}
},
{
"level": "a2-4",
"position": 7,
"text": "seat",
"meaning": {
"fr": "siège, place"
}
},
{
"level": "a2-4",
"position": 8,
"text": "secret",
"meaning": {
"fr": "secret"
}
},
{
"level": "a2-4",
"position": 9,
"text": "secretary",
"meaning": {
"fr": "secrétaire"
}
},
{
"level": "a2-4",
"position": 10,
"text": "section",
"meaning": {
"fr": "section, partie"
}
},
{
"level": "a2-4",
"position": 11,
"text": "seem",
"meaning": {
"fr": "sembler, paraître"
}
},
{
"level": "a2-4",
"position": 12,
"text": "series",
"meaning": {
"fr": "série"
}
},
{
"level": "a2-4",
"position": 13,
"text": "serious",
"meaning": {
"fr": "sérieux, grave"
}
},
{
"level": "a2-4",
"position": 14,
"text": "serve",
"meaning": {
"fr": "servir"
}
},
{
"level": "a2-4",
"position": 15,
"text": "service",
"meaning": {
"fr": "service"
}
},
{
"level": "a2-4",
"position": 16,
"text": "set",
"meaning": {
"fr": "(v.) fixer, régler ; (n.) ensemble"
}
},
{
"level": "a2-4",
"position": 17,
"text": "several",
"meaning": {
"fr": "plusieurs"
}
},
{
"level": "a2-4",
"position": 18,
"text": "shape",
"meaning": {
"fr": "forme"
}
},
{
"level": "a2-4",
"position": 19,
"text": "shelf",
"meaning": {
"fr": "étagère"
}
},
{
"level": "a2-4",
"position": 20,
"text": "ship",
"meaning": {
"fr": "navire, bateau"
}
},
{
"level": "a2-4",
"position": 21,
"text": "shoulder",
"meaning": {
"fr": "épaule"
}
},
{
"level": "a2-4",
"position": 22,
"text": "shout",
"meaning": {
"fr": "crier, appeler"
}
},
{
"level": "a2-4",
"position": 23,
"text": "sick",
"meaning": {
"fr": "malade"
}
},
{
"level": "a2-4",
"position": 24,
"text": "side",
"meaning": {
"fr": "côté"
}
},
{
"level": "a2-4",
"position": 25,
"text": "sign",
"meaning": {
"fr": "(n.) signe, panneau ; (v.) signer"
}
},
{
"level": "a2-4",
"position": 26,
"text": "silver",
"meaning": {
"fr": "argent (métal)"
}
},
{
"level": "a2-4",
"position": 27,
"text": "similar",
"meaning": {
"fr": "similaire, semblable"
}
},
{
"level": "a2-4",
"position": 28,
"text": "simple",
"meaning": {
"fr": "simple"
}
},
{
"level": "a2-4",
"position": 29,
"text": "since",
"meaning": {
"fr": "depuis, puisque"
}
},
{
"level": "a2-4",
"position": 30,
"text": "single",
"meaning": {
"fr": "seul, unique, célibataire"
}
},
{
"level": "a2-4",
"position": 31,
"text": "situation",
"meaning": {
"fr": "situation"
}
},
{
"level": "a2-4",
"position": 32,
"text": "size",
"meaning": {
"fr": "taille"
}
},
{
"level": "a2-4",
"position": 33,
"text": "ski",
"meaning": {
"fr": "(n.) ski ; (v.) skier"
}
},
{
"level": "a2-4",
"position": 34,
"text": "skill",
"meaning": {
"fr": "compétence, habileté"
}
},
{
"level": "a2-4",
"position": 35,
"text": "skin",
"meaning": {
"fr": "peau"
}
},
{
"level": "a2-4",
"position": 36,
"text": "sky",
"meaning": {
"fr": "ciel"
}
},
{
"level": "a2-4",
"position": 37,
"text": "slow",
"meaning": {
"fr": "lent"
}
},
{
"level": "a2-4",
"position": 38,
"text": "slowly",
"meaning": {
"fr": "lentement"
}
},
{
"level": "a2-4",
"position": 39,
"text": "smell",
"meaning": {
"fr": "(n.) odeur ; (v.) sentir"
}
},
{
"level": "a2-4",
"position": 40,
"text": "smile",
"meaning": {
"fr": "(n.) sourire ; (v.) sourire"
}
},
{
"level": "a2-4",
"position": 41,
"text": "smoke",
"meaning": {
"fr": "(n.) fumée ; (v.) fumer"
}
},
{
"level": "a2-4",
"position": 42,
"text": "soccer",
"meaning": {
"fr": "football"
}
},
{
"level": "a2-4",
"position": 43,
"text": "social",
"meaning": {
"fr": "social"
}
},
{
"level": "a2-4",
"position": 44,
"text": "soft",
"meaning": {
"fr": "doux, mou"
}
},
{
"level": "a2-4",
"position": 45,
"text": "software",
"meaning": {
"fr": "logiciel"
}
},
{
"level": "a2-4",
"position": 46,
"text": "solution",
"meaning": {
"fr": "solution"
}
},
{
"level": "a2-4",
"position": 47,
"text": "solve",
"meaning": {
"fr": "résoudre (un problème)"
}
},
{
"level": "a2-4",
"position": 48,
"text": "somebody",
"meaning": {
"fr": "quelqu'un, une personne"
}
},
{
"level": "a2-4",
"position": 49,
"text": "somewhere",
"meaning": {
"fr": "quelque part"
}
},
{
"level": "a2-4",
"position": 50,
"text": "sort",
"meaning": {
"fr": "(n.) sorte ; (v.) trier"
}
},
{
"level": "a2-4",
"position": 51,
"text": "sound",
"meaning": {
"fr": "(n.) son, bruit ; (v.) sembler"
}
},
{
"level": "a2-4",
"position": 52,
"text": "space",
"meaning": {
"fr": "espace"
}
},
{
"level": "a2-4",
"position": 53,
"text": "special",
"meaning": {
"fr": "spécial"
}
},
{
"level": "a2-4",
"position": 54,
"text": "speed",
"meaning": {
"fr": "vitesse ; (v.) aller vite"
}
},
{
"level": "a2-4",
"position": 55,
"text": "square",
"meaning": {
"fr": "carré, place (de ville)"
}
},
{
"level": "a2-4",
"position": 56,
"text": "stage",
"meaning": {
"fr": "étape, scène, stade"
}
},
{
"level": "a2-4",
"position": 57,
"text": "stair",
"meaning": {
"fr": "marche (d'escalier)"
}
},
{
"level": "a2-4",
"position": 58,
"text": "state",
"meaning": {
"fr": "état ; (v.) déclarer"
}
},
{
"level": "a2-4",
"position": 59,
"text": "steal",
"meaning": {
"fr": "voler, dérober"
}
},
{
"level": "a2-4",
"position": 60,
"text": "step",
"meaning": {
"fr": "pas, étape, marche"
}
},
{
"level": "a2-4",
"position": 61,
"text": "stomach",
"meaning": {
"fr": "estomac, ventre"
}
},
{
"level": "a2-4",
"position": 62,
"text": "stone",
"meaning": {
"fr": "pierre"
}
},
{
"level": "a2-4",
"position": 63,
"text": "storm",
"meaning": {
"fr": "tempête, orage"
}
},
{
"level": "a2-4",
"position": 64,
"text": "straight",
"meaning": {
"fr": "droit, directement, tout droit"
}
},
{
"level": "a2-4",
"position": 65,
"text": "strange",
"meaning": {
"fr": "étrange, bizarre"
}
},
{
"level": "a2-4",
"position": 66,
"text": "stress",
"meaning": {
"fr": "stress, insistance ; (v.) souligner"
}
},
{
"level": "a2-4",
"position": 67,
"text": "studio",
"meaning": {
"fr": "studio, atelier"
}
},
{
"level": "a2-4",
"position": 68,
"text": "stuff",
"meaning": {
"fr": "trucs, choses, affaires"
}
},
{
"level": "a2-4",
"position": 69,
"text": "stupid",
"meaning": {
"fr": "stupide, bête"
}
},
{
"level": "a2-4",
"position": 70,
"text": "style",
"meaning": {
"fr": "style"
}
},
{
"level": "a2-4",
"position": 71,
"text": "succeed",
"meaning": {
"fr": "réussir, succéder"
}
},
{
"level": "a2-4",
"position": 72,
"text": "success",
"meaning": {
"fr": "succès, réussite"
}
},
{
"level": "a2-4",
"position": 73,
"text": "successful",
"meaning": {
"fr": "réussi, qui a du succès"
}
},
{
"level": "a2-4",
"position": 74,
"text": "such",
"meaning": {
"fr": "tel, si, pareil"
}
},
{
"level": "a2-4",
"position": 75,
"text": "suddenly",
"meaning": {
"fr": "soudainement, tout à coup"
}
},
{
"level": "a2-4",
"position": 76,
"text": "suggest",
"meaning": {
"fr": "suggérer, proposer"
}
},
{
"level": "a2-4",
"position": 77,
"text": "suggestion",
"meaning": {
"fr": "suggestion, proposition"
}
},
{
"level": "a2-4",
"position": 78,
"text": "suit",
"meaning": {
"fr": "costume, tailleur ; (v.) convenir"
}
},
{
"level": "a2-4",
"position": 79,
"text": "super",
"meaning": {
"fr": "super, génial"
}
},
{
"level": "a2-4",
"position": 80,
"text": "support",
"meaning": {
"fr": "soutien ; (v.) soutenir"
}
},
{
"level": "a2-4",
"position": 81,
"text": "suppose",
"meaning": {
"fr": "supposer"
}
},
{
"level": "a2-4",
"position": 82,
"text": "surprise",
"meaning": {
"fr": "surprise ; (v.) surprendre"
}
},
{
"level": "a2-4",
"position": 83,
"text": "surprised",
"meaning": {
"fr": "surpris"
}
},
{
"level": "a2-4",
"position": 84,
"text": "surprising",
"meaning": {
"fr": "surprenant"
}
},
{
"level": "a2-4",
"position": 85,
"text": "sweet",
"meaning": {
"fr": "sucré, doux, mignon"
}
},
{
"level": "a2-4",
"position": 86,
"text": "system",
"meaning": {
"fr": "système"
}
},
{
"level": "a2-4",
"position": 87,
"text": "task",
"meaning": {
"fr": "tâche"
}
},
{
"level": "a2-4",
"position": 88,
"text": "taste",
"meaning": {
"fr": "goût ; (v.) goûter"
}
},
{
"level": "a2-4",
"position": 89,
"text": "teaching",
"meaning": {
"fr": "enseignement"
}
},
{
"level": "a2-4",
"position": 90,
"text": "technology",
"meaning": {
"fr": "technologie"
}
},
{
"level": "a2-4",
"position": 91,
"text": "teenager",
"meaning": {
"fr": "adolescent"
}
},
{
"level": "a2-4",
"position": 92,
"text": "temperature",
"meaning": {
"fr": "température"
}
},
{
"level": "a2-4",
"position": 93,
"text": "tent",
"meaning": {
"fr": "tente"
}
},
{
"level": "a2-4",
"position": 94,
"text": "terrible",
"meaning": {
"fr": "terrible, affreux"
}
},
{
"level": "a2-4",
"position": 95,
"text": "theater",
"meaning": {
"fr": "théâtre"
}
},
{
"level": "a2-4",
"position": 96,
"text": "themselves",
"meaning": {
"fr": "eux-mêmes, se"
}
},
{
"level": "a2-4",
"position": 97,
"text": "thin",
"meaning": {
"fr": "mince, fin"
}
},
{
"level": "a2-4",
"position": 98,
"text": "thought",
"meaning": {
"fr": "pensée, idée"
}
},
{
"level": "a2-4",
"position": 99,
"text": "through",
"meaning": {
"fr": "à travers, par"
}
},
{
"level": "a2-4",
"position": 100,
"text": "throw",
"meaning": {
"fr": "lancer, jeter"
}
},
{
"level": "a2-4",
"position": 101,
"text": "tiny",
"meaning": {
"fr": "minuscule"
}
},
{
"level": "a2-4",
"position": 102,
"text": "title",
"meaning": {
"fr": "titre"
}
},
{
"level": "a2-4",
"position": 103,
"text": "tool",
"meaning": {
"fr": "outil"
}
},
{
"level": "a2-4",
"position": 104,
"text": "tooth",
"meaning": {
"fr": "dent"
}
},
{
"level": "a2-4",
"position": 105,
"text": "top",
"meaning": {
"fr": "sommet, haut ; (adj.) meilleur"
}
},
{
"level": "a2-4",
"position": 106,
"text": "topic",
"meaning": {
"fr": "sujet, thème"
}
},
{
"level": "a2-4",
"position": 107,
"text": "total",
"meaning": {
"fr": "total"
}
},
{
"level": "a2-4",
"position": 108,
"text": "totally",
"meaning": {
"fr": "totalement, complètement"
}
},
{
"level": "a2-4",
"position": 109,
"text": "touch",
"meaning": {
"fr": "toucher ; (n.) contact"
}
},
{
"level": "a2-4",
"position": 110,
"text": "tour",
"meaning": {
"fr": "visite, tournée, circuit"
}
},
{
"level": "a2-4",
"position": 111,
"text": "tower",
"meaning": {
"fr": "tour (bâtiment)"
}
},
{
"level": "a2-4",
"position": 112,
"text": "toy",
"meaning": {
"fr": "jouet"
}
},
{
"level": "a2-4",
"position": 113,
"text": "tradition",
"meaning": {
"fr": "tradition"
}
},
{
"level": "a2-4",
"position": 114,
"text": "traditional",
"meaning": {
"fr": "traditionnel"
}
},
{
"level": "a2-4",
"position": 115,
"text": "traffic",
"meaning": {
"fr": "circulation, trafic"
}
},
{
"level": "a2-4",
"position": 116,
"text": "training",
"meaning": {
"fr": "formation, entraînement"
}
},
{
"level": "a2-4",
"position": 117,
"text": "translate",
"meaning": {
"fr": "traduire"
}
},
{
"level": "a2-4",
"position": 118,
"text": "trouble",
"meaning": {
"fr": "problèmes, ennuis"
}
},
{
"level": "a2-4",
"position": 119,
"text": "truck",
"meaning": {
"fr": "camion"
}
},
{
"level": "a2-4",
"position": 120,
"text": "truth",
"meaning": {
"fr": "vérité"
}
},
{
"level": "a2-4",
"position": 121,
"text": "twice",
"meaning": {
"fr": "deux fois"
}
},
{
"level": "a2-4",
"position": 122,
"text": "twin",
"meaning": {
"fr": "jumeau"
}
},
{
"level": "a2-4",
"position": 123,
"text": "type",
"meaning": {
"fr": "type, genre ; (v.) taper"
}
},
{
"level": "a2-4",
"position": 124,
"text": "typical",
"meaning": {
"fr": "typique"
}
},
{
"level": "a2-4",
"position": 125,
"text": "ugly",
"meaning": {
"fr": "laid, moche"
}
},
{
"level": "a2-4",
"position": 126,
"text": "unfortunately",
"meaning": {
"fr": "malheureusement"
}
},
{
"level": "a2-4",
"position": 127,
"text": "unit",
"meaning": {
"fr": "unité"
}
},
{
"level": "a2-4",
"position": 128,
"text": "United",
"meaning": {
"fr": "uni"
}
},
{
"level": "a2-4",
"position": 129,
"text": "unusual",
"meaning": {
"fr": "inhabituel, insolite"
}
},
{
"level": "a2-4",
"position": 130,
"text": "used",
"meaning": {
"fr": "habitué ; (adj.) d'occasion"
}
},
{
"level": "a2-4",
"position": 131,
"text": "useful",
"meaning": {
"fr": "utile"
}
},
{
"level": "a2-4",
"position": 132,
"text": "usual",
"meaning": {
"fr": "habituel"
}
},
{
"level": "a2-4",
"position": 133,
"text": "valley",
"meaning": {
"fr": "vallée"
}
},
{
"level": "a2-4",
"position": 134,
"text": "view",
"meaning": {
"fr": "vue, avis ; (v.) regarder"
}
},
{
"level": "a2-4",
"position": 135,
"text": "visitor",
"meaning": {
"fr": "visiteur"
}
},
{
"level": "a2-4",
"position": 136,
"text": "voice",
"meaning": {
"fr": "voix"
}
},
{
"level": "a2-4",
"position": 137,
"text": "wake",
"meaning": {
"fr": "se réveiller, réveiller"
}
},
{
"level": "a2-4",
"position": 138,
"text": "war",
"meaning": {
"fr": "guerre"
}
},
{
"level": "a2-4",
"position": 139,
"text": "waste",
"meaning": {
"fr": "gaspiller ; (n.) déchets, gaspillage"
}
},
{
"level": "a2-4",
"position": 140,
"text": "weak",
"meaning": {
"fr": "faible"
}
},
{
"level": "a2-4",
"position": 141,
"text": "wedding",
"meaning": {
"fr": "mariage, noces"
}
},
{
"level": "a2-4",
"position": 142,
"text": "weight",
"meaning": {
"fr": "poids"
}
},
{
"level": "a2-4",
"position": 143,
"text": "western",
"meaning": {
"fr": "occidental, de l'ouest"
}
},
{
"level": "a2-4",
"position": 144,
"text": "wet",
"meaning": {
"fr": "mouillé, humide"
}
},
{
"level": "a2-4",
"position": 145,
"text": "wheel",
"meaning": {
"fr": "roue, volant"
}
},
{
"level": "a2-4",
"position": 146,
"text": "while",
"meaning": {
"fr": "pendant que ; (n.) moment"
}
},
{
"level": "a2-4",
"position": 147,
"text": "whole",
"meaning": {
"fr": "entier, tout"
}
},
{
"level": "a2-4",
"position": 148,
"text": "wide",
"meaning": {
"fr": "large, vaste"
}
},
{
"level": "a2-4",
"position": 149,
"text": "wild",
"meaning": {
"fr": "sauvage, fou"
}
},
{
"level": "a2-4",
"position": 150,
"text": "wind",
"meaning": {
"fr": "vent ; (v.) enrouler, serpenter"
}
},
{
"level": "a2-4",
"position": 151,
"text": "winner",
"meaning": {
"fr": "gagnant, vainqueur"
}
},
{
"level": "a2-4",
"position": 152,
"text": "wish",
"meaning": {
"fr": "souhaiter ; (n.) souhait, vœu"
}
},
{
"level": "a2-4",
"position": 153,
"text": "wonder",
"meaning": {
"fr": "se demander ; (n.) merveille"
}
},
{
"level": "a2-4",
"position": 154,
"text": "wood",
"meaning": {
"fr": "bois (matière), forêt"
}
},
{
"level": "a2-4",
"position": 155,
"text": "wooden",
"meaning": {
"fr": "en bois"
}
},
{
"level": "a2-4",
"position": 156,
"text": "worker",
"meaning": {
"fr": "travailleur, ouvrier"
}
},
{
"level": "a2-4",
"position": 157,
"text": "working",
"meaning": {
"fr": "qui travaille, en état de marche"
}
},
{
"level": "a2-4",
"position": 158,
"text": "worried",
"meaning": {
"fr": "inquiet"
}
},
{
"level": "a2-4",
"position": 159,
"text": "worry",
"meaning": {
"fr": "s'inquiéter ; (n.) souci"
}
},
{
"level": "a2-4",
"position": 160,
"text": "writing",
"meaning": {
"fr": "écriture, écrits"
}
},
{
"level": "a2-4",
"position": 161,
"text": "yet",
"meaning": {
"fr": "encore, déjà ; (conj.) pourtant"
}
},
{
"level": "a2-4",
"position": 162,
"text": "yours",
"meaning": {
"fr": "le tien, le vôtre"
}
},
{
"level": "a2-4",
"position": 163,
"text": "yourself",
"meaning": {
"fr": "toi-même, vous-même"
}
},
{
"level": "b1-1",
"position": 1,
"text": "ability",
"meaning": {
"fr": "capacité, compétence"
}
},
{
"level": "b1-1",
"position": 2,
"text": "absolutely",
"meaning": {
"fr": "absolument"
}
},
{
"level": "b1-1",
"position": 3,
"text": "academic",
"meaning": {
"fr": "universitaire, scolaire"
}
},
{
"level": "b1-1",
"position": 4,
"text": "access",
"meaning": {
"fr": "(n.) accès ; (v.) accéder"
}
},
{
"level": "b1-1",
"position": 5,
"text": "account",
"meaning": {
"fr": "compte, récit"
}
},
{
"level": "b1-1",
"position": 6,
"text": "achieve",
"meaning": {
"fr": "atteindre, réussir"
}
},
{
"level": "b1-1",
"position": 7,
"text": "achievement",
"meaning": {
"fr": "réussite, exploit"
}
},
{
"level": "b1-1",
"position": 8,
"text": "actual",
"meaning": {
"fr": "réel, effectif"
}
},
{
"level": "b1-1",
"position": 9,
"text": "ad",
"meaning": {
"fr": "pub, annonce"
}
},
{
"level": "b1-1",
"position": 10,
"text": "addition",
"meaning": {
"fr": "ajout, addition"
}
},
{
"level": "b1-1",
"position": 11,
"text": "additional",
"meaning": {
"fr": "supplémentaire"
}
},
{
"level": "b1-1",
"position": 12,
"text": "admire",
"meaning": {
"fr": "admirer"
}
},
{
"level": "b1-1",
"position": 13,
"text": "admit",
"meaning": {
"fr": "admettre, avouer"
}
},
{
"level": "b1-1",
"position": 14,
"text": "advance",
"meaning": {
"fr": "(v.) avancer ; (n.) progrès, avance"
}
},
{
"level": "b1-1",
"position": 15,
"text": "advanced",
"meaning": {
"fr": "avancé, supérieur"
}
},
{
"level": "b1-1",
"position": 16,
"text": "advertising",
"meaning": {
"fr": "publicité"
}
},
{
"level": "b1-1",
"position": 17,
"text": "advise",
"meaning": {
"fr": "conseiller, recommander"
}
},
{
"level": "b1-1",
"position": 18,
"text": "affect",
"meaning": {
"fr": "affecter, toucher"
}
},
{
"level": "b1-1",
"position": 19,
"text": "afford",
"meaning": {
"fr": "avoir les moyens de"
}
},
{
"level": "b1-1",
"position": 20,
"text": "African-American",
"meaning": {
"fr": "afro-américain"
}
},
{
"level": "b1-1",
"position": 21,
"text": "agency",
"meaning": {
"fr": "agence, organisme"
}
},
{
"level": "b1-1",
"position": 22,
"text": "agent",
"meaning": {
"fr": "agent"
}
},
{
"level": "b1-1",
"position": 23,
"text": "aggressive",
"meaning": {
"fr": "agressif"
}
},
{
"level": "b1-1",
"position": 24,
"text": "agreement",
"meaning": {
"fr": "accord"
}
},
{
"level": "b1-1",
"position": 25,
"text": "ahead",
"meaning": {
"fr": "en avant, devant"
}
},
{
"level": "b1-1",
"position": 26,
"text": "aim",
"meaning": {
"fr": "(n.) but ; (v.) viser"
}
},
{
"level": "b1-1",
"position": 27,
"text": "aircraft",
"meaning": {
"fr": "avion, aéronef"
}
},
{
"level": "b1-1",
"position": 28,
"text": "airline",
"meaning": {
"fr": "compagnie aérienne"
}
},
{
"level": "b1-1",
"position": 29,
"text": "alcohol",
"meaning": {
"fr": "alcool"
}
},
{
"level": "b1-1",
"position": 30,
"text": "alternative",
"meaning": {
"fr": "alternative, autre option"
}
},
{
"level": "b1-1",
"position": 31,
"text": "among",
"meaning": {
"fr": "parmi"
}
},
{
"level": "b1-1",
"position": 32,
"text": "amount",
"meaning": {
"fr": "quantité, montant"
}
},
{
"level": "b1-1",
"position": 33,
"text": "analysis",
"meaning": {
"fr": "analyse"
}
},
{
"level": "b1-1",
"position": 34,
"text": "analyze",
"meaning": {
"fr": "analyser"
}
},
{
"level": "b1-1",
"position": 35,
"text": "anger",
"meaning": {
"fr": "colère"
}
},
{
"level": "b1-1",
"position": 36,
"text": "anniversary",
"meaning": {
"fr": "anniversaire (d'un événement)"
}
},
{
"level": "b1-1",
"position": 37,
"text": "announce",
"meaning": {
"fr": "annoncer"
}
},
{
"level": "b1-1",
"position": 38,
"text": "apart",
"meaning": {
"fr": "à part, séparément"
}
},
{
"level": "b1-1",
"position": 39,
"text": "apparently",
"meaning": {
"fr": "apparemment"
}
},
{
"level": "b1-1",
"position": 40,
"text": "appearance",
"meaning": {
"fr": "apparence, apparition"
}
},
{
"level": "b1-1",
"position": 41,
"text": "application",
"meaning": {
"fr": "candidature, demande, appli"
}
},
{
"level": "b1-1",
"position": 42,
"text": "apply",
"meaning": {
"fr": "postuler, appliquer"
}
},
{
"level": "b1-1",
"position": 43,
"text": "appointment",
"meaning": {
"fr": "rendez-vous, nomination"
}
},
{
"level": "b1-1",
"position": 44,
"text": "appreciate",
"meaning": {
"fr": "apprécier, être reconnaissant"
}
},
{
"level": "b1-1",
"position": 45,
"text": "approach",
"meaning": {
"fr": "(n.) approche ; (v.) s'approcher"
}
},
{
"level": "b1-1",
"position": 46,
"text": "appropriate",
"meaning": {
"fr": "approprié, adéquat"
}
},
{
"level": "b1-1",
"position": 47,
"text": "approximately",
"meaning": {
"fr": "approximativement, environ"
}
},
{
"level": "b1-1",
"position": 48,
"text": "Arab",
"meaning": {
"fr": "arabe"
}
},
{
"level": "b1-1",
"position": 49,
"text": "argue",
"meaning": {
"fr": "se disputer, soutenir que"
}
},
{
"level": "b1-1",
"position": 50,
"text": "argument",
"meaning": {
"fr": "dispute, argument"
}
},
{
"level": "b1-1",
"position": 51,
"text": "arrangement",
"meaning": {
"fr": "arrangement, disposition"
}
},
{
"level": "b1-1",
"position": 52,
"text": "arrest",
"meaning": {
"fr": "(v.) arrêter ; (n.) arrestation"
}
},
{
"level": "b1-1",
"position": 53,
"text": "arrival",
"meaning": {
"fr": "arrivée"
}
},
{
"level": "b1-1",
"position": 54,
"text": "artistic",
"meaning": {
"fr": "artistique"
}
},
{
"level": "b1-1",
"position": 55,
"text": "aspect",
"meaning": {
"fr": "aspect"
}
},
{
"level": "b1-1",
"position": 56,
"text": "assume",
"meaning": {
"fr": "supposer, présumer"
}
},
{
"level": "b1-1",
"position": 57,
"text": "atmosphere",
"meaning": {
"fr": "atmosphère, ambiance"
}
},
{
"level": "b1-1",
"position": 58,
"text": "attach",
"meaning": {
"fr": "attacher, joindre"
}
},
{
"level": "b1-1",
"position": 59,
"text": "attempt",
"meaning": {
"fr": "(n.) tentative ; (v.) tenter"
}
},
{
"level": "b1-1",
"position": 60,
"text": "attitude",
"meaning": {
"fr": "attitude"
}
},
{
"level": "b1-1",
"position": 61,
"text": "attract",
"meaning": {
"fr": "attirer"
}
},
{
"level": "b1-1",
"position": 62,
"text": "attractive",
"meaning": {
"fr": "attirant, séduisant"
}
},
{
"level": "b1-1",
"position": 63,
"text": "authority",
"meaning": {
"fr": "autorité"
}
},
{
"level": "b1-1",
"position": 64,
"text": "award",
"meaning": {
"fr": "(n.) prix, récompense ; (v.) décerner"
}
},
{
"level": "b1-1",
"position": 65,
"text": "aware",
"meaning": {
"fr": "conscient, au courant"
}
},
{
"level": "b1-1",
"position": 66,
"text": "balance",
"meaning": {
"fr": "équilibre, solde"
}
},
{
"level": "b1-1",
"position": 67,
"text": "ban",
"meaning": {
"fr": "(v.) interdire ; (n.) interdiction"
}
},
{
"level": "b1-1",
"position": 68,
"text": "base",
"meaning": {
"fr": "(n.) base ; (v.) baser"
}
},
{
"level": "b1-1",
"position": 69,
"text": "basically",
"meaning": {
"fr": "en gros, fondamentalement"
}
},
{
"level": "b1-1",
"position": 70,
"text": "basis",
"meaning": {
"fr": "fondement, base de référence"
}
},
{
"level": "b1-1",
"position": 71,
"text": "battle",
"meaning": {
"fr": "bataille, combat"
}
},
{
"level": "b1-1",
"position": 72,
"text": "behavior",
"meaning": {
"fr": "comportement"
}
},
{
"level": "b1-1",
"position": 73,
"text": "being",
"meaning": {
"fr": "être (n.), existence"
}
},
{
"level": "b1-1",
"position": 74,
"text": "belief",
"meaning": {
"fr": "croyance, conviction"
}
},
{
"level": "b1-1",
"position": 75,
"text": "bench",
"meaning": {
"fr": "banc"
}
},
{
"level": "b1-1",
"position": 76,
"text": "bend",
"meaning": {
"fr": "plier, se pencher"
}
},
{
"level": "b1-1",
"position": 77,
"text": "benefit",
"meaning": {
"fr": "(n.) bénéfice, avantage ; (v.) profiter"
}
},
{
"level": "b1-1",
"position": 78,
"text": "besides",
"meaning": {
"fr": "en outre, d'ailleurs"
}
},
{
"level": "b1-1",
"position": 79,
"text": "bet",
"meaning": {
"fr": "(v.) parier ; (n.) pari"
}
},
{
"level": "b1-1",
"position": 80,
"text": "beyond",
"meaning": {
"fr": "au-delà de"
}
},
{
"level": "b1-1",
"position": 81,
"text": "Bible",
"meaning": {
"fr": "Bible"
}
},
{
"level": "b1-1",
"position": 82,
"text": "blame",
"meaning": {
"fr": "blâmer, reprocher"
}
},
{
"level": "b1-1",
"position": 83,
"text": "blanket",
"meaning": {
"fr": "couverture"
}
},
{
"level": "b1-1",
"position": 84,
"text": "blind",
"meaning": {
"fr": "(adj.) aveugle ; (n.) store"
}
},
{
"level": "b1-1",
"position": 85,
"text": "blow",
"meaning": {
"fr": "(v.) souffler ; (n.) coup"
}
},
{
"level": "b1-1",
"position": 86,
"text": "bomb",
"meaning": {
"fr": "bombe"
}
},
{
"level": "b1-1",
"position": 87,
"text": "border",
"meaning": {
"fr": "frontière, bordure"
}
},
{
"level": "b1-1",
"position": 88,
"text": "bother",
"meaning": {
"fr": "déranger, ennuyer"
}
},
{
"level": "b1-1",
"position": 89,
"text": "branch",
"meaning": {
"fr": "branche, succursale"
}
},
{
"level": "b1-1",
"position": 90,
"text": "brand",
"meaning": {
"fr": "marque"
}
},
{
"level": "b1-1",
"position": 91,
"text": "breath",
"meaning": {
"fr": "souffle, haleine"
}
},
{
"level": "b1-1",
"position": 92,
"text": "breathe",
"meaning": {
"fr": "respirer"
}
},
{
"level": "b1-1",
"position": 93,
"text": "brick",
"meaning": {
"fr": "brique"
}
},
{
"level": "b1-1",
"position": 94,
"text": "brief",
"meaning": {
"fr": "bref, court"
}
},
{
"level": "b1-1",
"position": 95,
"text": "briefly",
"meaning": {
"fr": "brièvement"
}
},
{
"level": "b1-1",
"position": 96,
"text": "budget",
"meaning": {
"fr": "budget"
}
},
{
"level": "b1-1",
"position": 97,
"text": "bury",
"meaning": {
"fr": "enterrer"
}
},
{
"level": "b1-1",
"position": 98,
"text": "buyer",
"meaning": {
"fr": "acheteur"
}
},
{
"level": "b1-1",
"position": 99,
"text": "cabin",
"meaning": {
"fr": "cabane, cabine"
}
},
{
"level": "b1-1",
"position": 100,
"text": "cable",
"meaning": {
"fr": "câble"
}
},
{
"level": "b1-1",
"position": 101,
"text": "calculate",
"meaning": {
"fr": "calculer"
}
},
{
"level": "b1-1",
"position": 102,
"text": "campaign",
"meaning": {
"fr": "campagne"
}
},
{
"level": "b1-1",
"position": 103,
"text": "campus",
"meaning": {
"fr": "campus"
}
},
{
"level": "b1-1",
"position": 104,
"text": "cancer",
"meaning": {
"fr": "cancer"
}
},
{
"level": "b1-1",
"position": 105,
"text": "candidate",
"meaning": {
"fr": "candidat"
}
},
{
"level": "b1-1",
"position": 106,
"text": "capable",
"meaning": {
"fr": "capable"
}
},
{
"level": "b1-1",
"position": 107,
"text": "capture",
"meaning": {
"fr": "capturer"
}
},
{
"level": "b1-1",
"position": 108,
"text": "carbon",
"meaning": {
"fr": "carbone"
}
},
{
"level": "b1-1",
"position": 109,
"text": "cast",
"meaning": {
"fr": "(v.) jeter, lancer ; (n.) distribution"
}
},
{
"level": "b1-1",
"position": 110,
"text": "category",
"meaning": {
"fr": "catégorie"
}
},
{
"level": "b1-1",
"position": 111,
"text": "Catholic",
"meaning": {
"fr": "catholique"
}
},
{
"level": "b1-1",
"position": 112,
"text": "ceiling",
"meaning": {
"fr": "plafond"
}
},
{
"level": "b1-1",
"position": 113,
"text": "celebration",
"meaning": {
"fr": "célébration, fête"
}
},
{
"level": "b1-1",
"position": 114,
"text": "celebrity",
"meaning": {
"fr": "célébrité"
}
},
{
"level": "b1-1",
"position": 115,
"text": "cell",
"meaning": {
"fr": "cellule"
}
},
{
"level": "b1-1",
"position": 116,
"text": "ceremony",
"meaning": {
"fr": "cérémonie"
}
},
{
"level": "b1-1",
"position": 117,
"text": "chain",
"meaning": {
"fr": "chaîne (maillons)"
}
},
{
"level": "b1-1",
"position": 118,
"text": "challenge",
"meaning": {
"fr": "(n.) défi ; (v.) contester"
}
},
{
"level": "b1-1",
"position": 119,
"text": "champion",
"meaning": {
"fr": "champion"
}
},
{
"level": "b1-1",
"position": 120,
"text": "championship",
"meaning": {
"fr": "championnat"
}
},
{
"level": "b1-1",
"position": 121,
"text": "changing",
"meaning": {
"fr": "changeant, en évolution"
}
},
{
"level": "b1-1",
"position": 122,
"text": "channel",
"meaning": {
"fr": "chaîne (télé), canal"
}
},
{
"level": "b1-1",
"position": 123,
"text": "characteristic",
"meaning": {
"fr": "caractéristique"
}
},
{
"level": "b1-1",
"position": 124,
"text": "charge",
"meaning": {
"fr": "(n.) frais, accusation ; (v.) facturer"
}
},
{
"level": "b1-1",
"position": 125,
"text": "charity",
"meaning": {
"fr": "association caritative, charité"
}
},
{
"level": "b1-1",
"position": 126,
"text": "chase",
"meaning": {
"fr": "poursuivre, pourchasser"
}
},
{
"level": "b1-1",
"position": 127,
"text": "cheek",
"meaning": {
"fr": "joue"
}
},
{
"level": "b1-1",
"position": 128,
"text": "chemical",
"meaning": {
"fr": "(adj.) chimique ; (n.) produit chimique"
}
},
{
"level": "b1-1",
"position": 129,
"text": "chest",
"meaning": {
"fr": "poitrine, coffre"
}
},
{
"level": "b1-1",
"position": 130,
"text": "chief",
"meaning": {
"fr": "(n.) chef ; (adj.) principal"
}
},
{
"level": "b1-1",
"position": 131,
"text": "Christian",
"meaning": {
"fr": "chrétien"
}
},
{
"level": "b1-1",
"position": 132,
"text": "citizen",
"meaning": {
"fr": "citoyen"
}
},
{
"level": "b1-1",
"position": 133,
"text": "civil",
"meaning": {
"fr": "civil, civique"
}
},
{
"level": "b1-1",
"position": 134,
"text": "claim",
"meaning": {
"fr": "(v.) affirmer, réclamer ; (n.) revendication"
}
},
{
"level": "b1-1",
"position": 135,
"text": "classic",
"meaning": {
"fr": "classique"
}
},
{
"level": "b1-1",
"position": 136,
"text": "client",
"meaning": {
"fr": "client"
}
},
{
"level": "b1-1",
"position": 137,
"text": "clinic",
"meaning": {
"fr": "clinique"
}
},
{
"level": "b1-1",
"position": 138,
"text": "closely",
"meaning": {
"fr": "de près, étroitement"
}
},
{
"level": "b1-1",
"position": 139,
"text": "clue",
"meaning": {
"fr": "indice"
}
},
{
"level": "b1-1",
"position": 140,
"text": "coal",
"meaning": {
"fr": "charbon"
}
},
{
"level": "b1-1",
"position": 141,
"text": "code",
"meaning": {
"fr": "code"
}
},
{
"level": "b1-1",
"position": 142,
"text": "column",
"meaning": {
"fr": "colonne, rubrique"
}
},
{
"level": "b1-1",
"position": 143,
"text": "combination",
"meaning": {
"fr": "combinaison, association"
}
},
{
"level": "b1-1",
"position": 144,
"text": "combine",
"meaning": {
"fr": "combiner, associer"
}
},
{
"level": "b1-1",
"position": 145,
"text": "comfort",
"meaning": {
"fr": "confort, réconfort ; (v.) réconforter"
}
},
{
"level": "b1-1",
"position": 146,
"text": "commercial",
"meaning": {
"fr": "commercial ; (n.) spot publicitaire"
}
},
{
"level": "b1-1",
"position": 147,
"text": "committee",
"meaning": {
"fr": "comité"
}
},
{
"level": "b1-1",
"position": 148,
"text": "communicate",
"meaning": {
"fr": "communiquer"
}
},
{
"level": "b1-1",
"position": 149,
"text": "communication",
"meaning": {
"fr": "communication"
}
},
{
"level": "b1-1",
"position": 150,
"text": "community",
"meaning": {
"fr": "communauté, collectivité"
}
},
{
"level": "b1-1",
"position": 151,
"text": "comparison",
"meaning": {
"fr": "comparaison"
}
},
{
"level": "b1-1",
"position": 152,
"text": "compete",
"meaning": {
"fr": "rivaliser, concourir"
}
},
{
"level": "b1-1",
"position": 153,
"text": "competitive",
"meaning": {
"fr": "compétitif, de compétition"
}
},
{
"level": "b1-1",
"position": 154,
"text": "competitor",
"meaning": {
"fr": "concurrent, compétiteur"
}
},
{
"level": "b1-1",
"position": 155,
"text": "complaint",
"meaning": {
"fr": "plainte, réclamation"
}
},
{
"level": "b1-1",
"position": 156,
"text": "complex",
"meaning": {
"fr": "complexe"
}
},
{
"level": "b1-1",
"position": 157,
"text": "complicated",
"meaning": {
"fr": "compliqué"
}
},
{
"level": "b1-1",
"position": 158,
"text": "concentrate",
"meaning": {
"fr": "se concentrer, concentrer"
}
},
{
"level": "b1-1",
"position": 159,
"text": "concentration",
"meaning": {
"fr": "concentration"
}
},
{
"level": "b1-1",
"position": 160,
"text": "concept",
"meaning": {
"fr": "concept, notion"
}
},
{
"level": "b1-1",
"position": 161,
"text": "concern",
"meaning": {
"fr": "inquiétude ; (v.) concerner, inquiéter"
}
},
{
"level": "b1-1",
"position": 162,
"text": "concerned",
"meaning": {
"fr": "inquiet, concerné"
}
},
{
"level": "b1-1",
"position": 163,
"text": "conclusion",
"meaning": {
"fr": "conclusion"
}
},
{
"level": "b1-1",
"position": 164,
"text": "conference",
"meaning": {
"fr": "conférence, congrès"
}
},
{
"level": "b1-1",
"position": 165,
"text": "confidence",
"meaning": {
"fr": "confiance (en soi), assurance"
}
},
{
"level": "b1-1",
"position": 166,
"text": "confident",
"meaning": {
"fr": "sûr de soi, confiant"
}
},
{
"level": "b1-1",
"position": 167,
"text": "confirm",
"meaning": {
"fr": "confirmer"
}
},
{
"level": "b1-1",
"position": 168,
"text": "conflict",
"meaning": {
"fr": "conflit"
}
},
{
"level": "b1-1",
"position": 169,
"text": "confusion",
"meaning": {
"fr": "confusion, désarroi"
}
},
{
"level": "b1-1",
"position": 170,
"text": "connection",
"meaning": {
"fr": "lien, connexion, rapport"
}
},
{
"level": "b1-1",
"position": 171,
"text": "consequence",
"meaning": {
"fr": "conséquence"
}
},
{
"level": "b1-1",
"position": 172,
"text": "conservative",
"meaning": {
"fr": "conservateur"
}
},
{
"level": "b1-1",
"position": 173,
"text": "consist",
"meaning": {
"fr": "consister (en), se composer de"
}
},
{
"level": "b1-1",
"position": 174,
"text": "constant",
"meaning": {
"fr": "constant, permanent, continuel"
}
},
{
"level": "b1-1",
"position": 175,
"text": "constantly",
"meaning": {
"fr": "constamment, sans cesse"
}
},
{
"level": "b1-1",
"position": 176,
"text": "construction",
"meaning": {
"fr": "construction, bâtiment"
}
},
{
"level": "b1-1",
"position": 177,
"text": "consumer",
"meaning": {
"fr": "consommateur"
}
},
{
"level": "b1-1",
"position": 178,
"text": "container",
"meaning": {
"fr": "récipient, conteneur"
}
},
{
"level": "b1-1",
"position": 179,
"text": "content",
"meaning": {
"fr": "contenu ; (adj.) satisfait, content"
}
},
{
"level": "b1-1",
"position": 180,
"text": "contest",
"meaning": {
"fr": "concours, lutte ; (v.) contester"
}
},
{
"level": "b1-1",
"position": 181,
"text": "context",
"meaning": {
"fr": "contexte"
}
},
{
"level": "b1-1",
"position": 182,
"text": "contract",
"meaning": {
"fr": "contrat ; (v.) contracter"
}
},
{
"level": "b1-1",
"position": 183,
"text": "contrast",
"meaning": {
"fr": "contraste ; (v.) contraster"
}
},
{
"level": "b1-1",
"position": 184,
"text": "convince",
"meaning": {
"fr": "convaincre, persuader"
}
},
{
"level": "b1-1",
"position": 185,
"text": "corn",
"meaning": {
"fr": "maïs, céréales"
}
},
{
"level": "b1-1",
"position": 186,
"text": "cotton",
"meaning": {
"fr": "coton"
}
},
{
"level": "b1-1",
"position": 187,
"text": "couch",
"meaning": {
"fr": "canapé"
}
},
{
"level": "b1-1",
"position": 188,
"text": "council",
"meaning": {
"fr": "conseil (municipal)"
}
},
{
"level": "b1-1",
"position": 189,
"text": "courage",
"meaning": {
"fr": "courage"
}
},
{
"level": "b1-1",
"position": 190,
"text": "court",
"meaning": {
"fr": "tribunal, cour, terrain (de sport)"
}
},
{
"level": "b1-2",
"position": 1,
"text": "crash",
"meaning": {
"fr": "accident, collision ; (v.) s'écraser"
}
},
{
"level": "b1-2",
"position": 2,
"text": "creation",
"meaning": {
"fr": "création"
}
},
{
"level": "b1-2",
"position": 3,
"text": "creative",
"meaning": {
"fr": "créatif"
}
},
{
"level": "b1-2",
"position": 4,
"text": "creature",
"meaning": {
"fr": "créature, être"
}
},
{
"level": "b1-2",
"position": 5,
"text": "credit",
"meaning": {
"fr": "crédit, mérite"
}
},
{
"level": "b1-2",
"position": 6,
"text": "crew",
"meaning": {
"fr": "équipage, équipe"
}
},
{
"level": "b1-2",
"position": 7,
"text": "criminal",
"meaning": {
"fr": "criminel"
}
},
{
"level": "b1-2",
"position": 8,
"text": "crisis",
"meaning": {
"fr": "crise"
}
},
{
"level": "b1-2",
"position": 9,
"text": "critic",
"meaning": {
"fr": "critique (personne)"
}
},
{
"level": "b1-2",
"position": 10,
"text": "critical",
"meaning": {
"fr": "critique, crucial"
}
},
{
"level": "b1-2",
"position": 11,
"text": "criticism",
"meaning": {
"fr": "critique (reproche)"
}
},
{
"level": "b1-2",
"position": 12,
"text": "criticize",
"meaning": {
"fr": "critiquer"
}
},
{
"level": "b1-2",
"position": 13,
"text": "cultural",
"meaning": {
"fr": "culturel"
}
},
{
"level": "b1-2",
"position": 14,
"text": "curious",
"meaning": {
"fr": "curieux"
}
},
{
"level": "b1-2",
"position": 15,
"text": "current",
"meaning": {
"fr": "actuel ; (n.) courant"
}
},
{
"level": "b1-2",
"position": 16,
"text": "currently",
"meaning": {
"fr": "actuellement"
}
},
{
"level": "b1-2",
"position": 17,
"text": "cycle",
"meaning": {
"fr": "cycle ; (v.) faire du vélo"
}
},
{
"level": "b1-2",
"position": 18,
"text": "damage",
"meaning": {
"fr": "dégâts, dommages ; (v.) endommager"
}
},
{
"level": "b1-2",
"position": 19,
"text": "darkness",
"meaning": {
"fr": "obscurité, ténèbres"
}
},
{
"level": "b1-2",
"position": 20,
"text": "data",
"meaning": {
"fr": "données"
}
},
{
"level": "b1-2",
"position": 21,
"text": "deal",
"meaning": {
"fr": "accord, affaire ; (v.) distribuer"
}
},
{
"level": "b1-2",
"position": 22,
"text": "debate",
"meaning": {
"fr": "débat ; (v.) débattre"
}
},
{
"level": "b1-2",
"position": 23,
"text": "debt",
"meaning": {
"fr": "dette"
}
},
{
"level": "b1-2",
"position": 24,
"text": "decade",
"meaning": {
"fr": "décennie"
}
},
{
"level": "b1-2",
"position": 25,
"text": "decrease",
"meaning": {
"fr": "diminuer ; (n.) diminution"
}
},
{
"level": "b1-2",
"position": 26,
"text": "deeply",
"meaning": {
"fr": "profondément"
}
},
{
"level": "b1-2",
"position": 27,
"text": "deer",
"meaning": {
"fr": "cerf, chevreuil"
}
},
{
"level": "b1-2",
"position": 28,
"text": "defeat",
"meaning": {
"fr": "défaite ; (v.) vaincre"
}
},
{
"level": "b1-2",
"position": 29,
"text": "defend",
"meaning": {
"fr": "défendre"
}
},
{
"level": "b1-2",
"position": 30,
"text": "defense",
"meaning": {
"fr": "défense"
}
},
{
"level": "b1-2",
"position": 31,
"text": "define",
"meaning": {
"fr": "définir"
}
},
{
"level": "b1-2",
"position": 32,
"text": "delay",
"meaning": {
"fr": "retard ; (v.) retarder"
}
},
{
"level": "b1-2",
"position": 33,
"text": "deliver",
"meaning": {
"fr": "livrer, remettre"
}
},
{
"level": "b1-2",
"position": 34,
"text": "delivery",
"meaning": {
"fr": "livraison"
}
},
{
"level": "b1-2",
"position": 35,
"text": "demand",
"meaning": {
"fr": "demande, exigence ; (v.) exiger"
}
},
{
"level": "b1-2",
"position": 36,
"text": "democracy",
"meaning": {
"fr": "démocratie"
}
},
{
"level": "b1-2",
"position": 37,
"text": "democratic",
"meaning": {
"fr": "démocratique"
}
},
{
"level": "b1-2",
"position": 38,
"text": "demonstrate",
"meaning": {
"fr": "démontrer, manifester"
}
},
{
"level": "b1-2",
"position": 39,
"text": "demonstration",
"meaning": {
"fr": "démonstration, manifestation"
}
},
{
"level": "b1-2",
"position": 40,
"text": "deny",
"meaning": {
"fr": "nier, refuser"
}
},
{
"level": "b1-2",
"position": 41,
"text": "dependent",
"meaning": {
"fr": "dépendant"
}
},
{
"level": "b1-2",
"position": 42,
"text": "depending",
"meaning": {
"fr": "selon, en fonction de"
}
},
{
"level": "b1-2",
"position": 43,
"text": "depth",
"meaning": {
"fr": "profondeur"
}
},
{
"level": "b1-2",
"position": 44,
"text": "deserve",
"meaning": {
"fr": "mériter"
}
},
{
"level": "b1-2",
"position": 45,
"text": "desperate",
"meaning": {
"fr": "désespéré"
}
},
{
"level": "b1-2",
"position": 46,
"text": "despite",
"meaning": {
"fr": "malgré"
}
},
{
"level": "b1-2",
"position": 47,
"text": "destruction",
"meaning": {
"fr": "destruction"
}
},
{
"level": "b1-2",
"position": 48,
"text": "detailed",
"meaning": {
"fr": "détaillé"
}
},
{
"level": "b1-2",
"position": 49,
"text": "determine",
"meaning": {
"fr": "déterminer"
}
},
{
"level": "b1-2",
"position": 50,
"text": "development",
"meaning": {
"fr": "développement, évolution"
}
},
{
"level": "b1-2",
"position": 51,
"text": "device",
"meaning": {
"fr": "appareil, dispositif"
}
},
{
"level": "b1-2",
"position": 52,
"text": "difficulty",
"meaning": {
"fr": "difficulté"
}
},
{
"level": "b1-2",
"position": 53,
"text": "dig",
"meaning": {
"fr": "creuser"
}
},
{
"level": "b1-2",
"position": 54,
"text": "digital",
"meaning": {
"fr": "numérique"
}
},
{
"level": "b1-2",
"position": 55,
"text": "dining",
"meaning": {
"fr": "repas, (salle) à manger"
}
},
{
"level": "b1-2",
"position": 56,
"text": "directly",
"meaning": {
"fr": "directement"
}
},
{
"level": "b1-2",
"position": 57,
"text": "disappear",
"meaning": {
"fr": "disparaître"
}
},
{
"level": "b1-2",
"position": 58,
"text": "disaster",
"meaning": {
"fr": "catastrophe, désastre"
}
},
{
"level": "b1-2",
"position": 59,
"text": "discovery",
"meaning": {
"fr": "découverte"
}
},
{
"level": "b1-2",
"position": 60,
"text": "disease",
"meaning": {
"fr": "maladie"
}
},
{
"level": "b1-2",
"position": 61,
"text": "display",
"meaning": {
"fr": "exposition, affichage ; (v.) afficher"
}
},
{
"level": "b1-2",
"position": 62,
"text": "district",
"meaning": {
"fr": "district, quartier"
}
},
{
"level": "b1-2",
"position": 63,
"text": "divide",
"meaning": {
"fr": "diviser, partager"
}
},
{
"level": "b1-2",
"position": 64,
"text": "division",
"meaning": {
"fr": "division, service"
}
},
{
"level": "b1-2",
"position": 65,
"text": "divorce",
"meaning": {
"fr": "divorce ; (v.) divorcer"
}
},
{
"level": "b1-2",
"position": 66,
"text": "doubt",
"meaning": {
"fr": "doute ; (v.) douter"
}
},
{
"level": "b1-2",
"position": 67,
"text": "downtown",
"meaning": {
"fr": "centre-ville"
}
},
{
"level": "b1-2",
"position": 68,
"text": "dramatic",
"meaning": {
"fr": "spectaculaire, dramatique"
}
},
{
"level": "b1-2",
"position": 69,
"text": "dramatically",
"meaning": {
"fr": "de façon spectaculaire"
}
},
{
"level": "b1-2",
"position": 70,
"text": "due",
"meaning": {
"fr": "dû, prévu"
}
},
{
"level": "b1-2",
"position": 71,
"text": "dust",
"meaning": {
"fr": "poussière"
}
},
{
"level": "b1-2",
"position": 72,
"text": "duty",
"meaning": {
"fr": "devoir, obligation, taxe"
}
},
{
"level": "b1-2",
"position": 73,
"text": "eastern",
"meaning": {
"fr": "oriental, de l'est"
}
},
{
"level": "b1-2",
"position": 74,
"text": "economic",
"meaning": {
"fr": "économique"
}
},
{
"level": "b1-2",
"position": 75,
"text": "economy",
"meaning": {
"fr": "économie"
}
},
{
"level": "b1-2",
"position": 76,
"text": "edge",
"meaning": {
"fr": "bord, lisière, tranchant"
}
},
{
"level": "b1-2",
"position": 77,
"text": "editor",
"meaning": {
"fr": "rédacteur, éditeur"
}
},
{
"level": "b1-2",
"position": 78,
"text": "educate",
"meaning": {
"fr": "éduquer, instruire"
}
},
{
"level": "b1-2",
"position": 79,
"text": "educational",
"meaning": {
"fr": "éducatif, pédagogique"
}
},
{
"level": "b1-2",
"position": 80,
"text": "effective",
"meaning": {
"fr": "efficace, effectif"
}
},
{
"level": "b1-2",
"position": 81,
"text": "effectively",
"meaning": {
"fr": "efficacement, en fait"
}
},
{
"level": "b1-2",
"position": 82,
"text": "efficient",
"meaning": {
"fr": "efficace, performant"
}
},
{
"level": "b1-2",
"position": 83,
"text": "elderly",
"meaning": {
"fr": "âgé, personnes âgées"
}
},
{
"level": "b1-2",
"position": 84,
"text": "elect",
"meaning": {
"fr": "élire"
}
},
{
"level": "b1-2",
"position": 85,
"text": "election",
"meaning": {
"fr": "élection"
}
},
{
"level": "b1-2",
"position": 86,
"text": "electronic",
"meaning": {
"fr": "électronique"
}
},
{
"level": "b1-2",
"position": 87,
"text": "element",
"meaning": {
"fr": "élément"
}
},
{
"level": "b1-2",
"position": 88,
"text": "elsewhere",
"meaning": {
"fr": "ailleurs"
}
},
{
"level": "b1-2",
"position": 89,
"text": "emergency",
"meaning": {
"fr": "urgence"
}
},
{
"level": "b1-2",
"position": 90,
"text": "emotion",
"meaning": {
"fr": "émotion"
}
},
{
"level": "b1-2",
"position": 91,
"text": "emotional",
"meaning": {
"fr": "émotionnel, émouvant"
}
},
{
"level": "b1-2",
"position": 92,
"text": "employ",
"meaning": {
"fr": "employer"
}
},
{
"level": "b1-2",
"position": 93,
"text": "employer",
"meaning": {
"fr": "employeur"
}
},
{
"level": "b1-2",
"position": 94,
"text": "employment",
"meaning": {
"fr": "emploi"
}
},
{
"level": "b1-2",
"position": 95,
"text": "encourage",
"meaning": {
"fr": "encourager"
}
},
{
"level": "b1-2",
"position": 96,
"text": "enemy",
"meaning": {
"fr": "ennemi"
}
},
{
"level": "b1-2",
"position": 97,
"text": "engineering",
"meaning": {
"fr": "ingénierie"
}
},
{
"level": "b1-2",
"position": 98,
"text": "enormous",
"meaning": {
"fr": "énorme"
}
},
{
"level": "b1-2",
"position": 99,
"text": "entertainment",
"meaning": {
"fr": "divertissement"
}
},
{
"level": "b1-2",
"position": 100,
"text": "entire",
"meaning": {
"fr": "entier, tout"
}
},
{
"level": "b1-2",
"position": 101,
"text": "entirely",
"meaning": {
"fr": "entièrement, tout à fait"
}
},
{
"level": "b1-2",
"position": 102,
"text": "entry",
"meaning": {
"fr": "entrée, inscription, article"
}
},
{
"level": "b1-2",
"position": 103,
"text": "environmental",
"meaning": {
"fr": "environnemental"
}
},
{
"level": "b1-2",
"position": 104,
"text": "episode",
"meaning": {
"fr": "épisode"
}
},
{
"level": "b1-2",
"position": 105,
"text": "equal",
"meaning": {
"fr": "égal ; (v.) égaler"
}
},
{
"level": "b1-2",
"position": 106,
"text": "equally",
"meaning": {
"fr": "également, de manière égale"
}
},
{
"level": "b1-2",
"position": 107,
"text": "escape",
"meaning": {
"fr": "s'échapper ; (n.) évasion"
}
},
{
"level": "b1-2",
"position": 108,
"text": "essential",
"meaning": {
"fr": "essentiel, indispensable"
}
},
{
"level": "b1-2",
"position": 109,
"text": "establish",
"meaning": {
"fr": "établir, fonder"
}
},
{
"level": "b1-2",
"position": 110,
"text": "estimate",
"meaning": {
"fr": "estimation ; (v.) estimer"
}
},
{
"level": "b1-2",
"position": 111,
"text": "eventually",
"meaning": {
"fr": "finalement, à la longue"
}
},
{
"level": "b1-2",
"position": 112,
"text": "everyday",
"meaning": {
"fr": "quotidien, de tous les jours"
}
},
{
"level": "b1-2",
"position": 113,
"text": "evidence",
"meaning": {
"fr": "preuve(s), témoignage"
}
},
{
"level": "b1-2",
"position": 114,
"text": "exact",
"meaning": {
"fr": "exact, précis"
}
},
{
"level": "b1-2",
"position": 115,
"text": "examination",
"meaning": {
"fr": "examen"
}
},
{
"level": "b1-2",
"position": 116,
"text": "examine",
"meaning": {
"fr": "examiner"
}
},
{
"level": "b1-2",
"position": 117,
"text": "exception",
"meaning": {
"fr": "exception"
}
},
{
"level": "b1-2",
"position": 118,
"text": "exchange",
"meaning": {
"fr": "échange ; (v.) échanger"
}
},
{
"level": "b1-2",
"position": 119,
"text": "exist",
"meaning": {
"fr": "exister"
}
},
{
"level": "b1-2",
"position": 120,
"text": "experiment",
"meaning": {
"fr": "expérience (scientifique)"
}
},
{
"level": "b1-2",
"position": 121,
"text": "explode",
"meaning": {
"fr": "exploser"
}
},
{
"level": "b1-2",
"position": 122,
"text": "explore",
"meaning": {
"fr": "explorer"
}
},
{
"level": "b1-2",
"position": 123,
"text": "explosion",
"meaning": {
"fr": "explosion"
}
},
{
"level": "b1-2",
"position": 124,
"text": "express",
"meaning": {
"fr": "exprimer ; (adj.) express"
}
},
{
"level": "b1-2",
"position": 125,
"text": "expression",
"meaning": {
"fr": "expression"
}
},
{
"level": "b1-2",
"position": 126,
"text": "extraordinary",
"meaning": {
"fr": "extraordinaire"
}
},
{
"level": "b1-2",
"position": 127,
"text": "extreme",
"meaning": {
"fr": "extrême"
}
},
{
"level": "b1-2",
"position": 128,
"text": "facility",
"meaning": {
"fr": "installation, équipement"
}
},
{
"level": "b1-2",
"position": 129,
"text": "factor",
"meaning": {
"fr": "facteur"
}
},
{
"level": "b1-2",
"position": 130,
"text": "failure",
"meaning": {
"fr": "échec"
}
},
{
"level": "b1-2",
"position": 131,
"text": "fairly",
"meaning": {
"fr": "assez, équitablement"
}
},
{
"level": "b1-2",
"position": 132,
"text": "faith",
"meaning": {
"fr": "foi, confiance"
}
},
{
"level": "b1-2",
"position": 133,
"text": "familiar",
"meaning": {
"fr": "familier"
}
},
{
"level": "b1-2",
"position": 134,
"text": "fantasy",
"meaning": {
"fr": "fantasme, fantaisie"
}
},
{
"level": "b1-2",
"position": 135,
"text": "fault",
"meaning": {
"fr": "faute, défaut"
}
},
{
"level": "b1-2",
"position": 136,
"text": "favor",
"meaning": {
"fr": "faveur, service"
}
},
{
"level": "b1-2",
"position": 137,
"text": "fear",
"meaning": {
"fr": "peur ; craindre"
}
},
{
"level": "b1-2",
"position": 138,
"text": "fee",
"meaning": {
"fr": "frais, honoraires"
}
},
{
"level": "b1-2",
"position": 139,
"text": "fence",
"meaning": {
"fr": "clôture, barrière"
}
},
{
"level": "b1-2",
"position": 140,
"text": "fighting",
"meaning": {
"fr": "combats, lutte"
}
},
{
"level": "b1-2",
"position": 141,
"text": "finance",
"meaning": {
"fr": "finance(s) ; financer"
}
},
{
"level": "b1-2",
"position": 142,
"text": "financial",
"meaning": {
"fr": "financier"
}
},
{
"level": "b1-2",
"position": 143,
"text": "firm",
"meaning": {
"fr": "entreprise ; (adj.) ferme"
}
},
{
"level": "b1-2",
"position": 144,
"text": "fitness",
"meaning": {
"fr": "forme physique"
}
},
{
"level": "b1-2",
"position": 145,
"text": "flavor",
"meaning": {
"fr": "saveur, goût"
}
},
{
"level": "b1-2",
"position": 146,
"text": "float",
"meaning": {
"fr": "flotter"
}
},
{
"level": "b1-2",
"position": 147,
"text": "flow",
"meaning": {
"fr": "couler ; flux"
}
},
{
"level": "b1-2",
"position": 148,
"text": "folk",
"meaning": {
"fr": "gens ; musique folk"
}
},
{
"level": "b1-2",
"position": 149,
"text": "force",
"meaning": {
"fr": "force ; forcer"
}
},
{
"level": "b1-2",
"position": 150,
"text": "forever",
"meaning": {
"fr": "pour toujours"
}
},
{
"level": "b1-2",
"position": 151,
"text": "formal",
"meaning": {
"fr": "formel, officiel"
}
},
{
"level": "b1-2",
"position": 152,
"text": "former",
"meaning": {
"fr": "ancien, précédent"
}
},
{
"level": "b1-2",
"position": 153,
"text": "fortune",
"meaning": {
"fr": "fortune, chance"
}
},
{
"level": "b1-2",
"position": 154,
"text": "found",
"meaning": {
"fr": "fonder"
}
},
{
"level": "b1-2",
"position": 155,
"text": "frame",
"meaning": {
"fr": "cadre, monture"
}
},
{
"level": "b1-2",
"position": 156,
"text": "freedom",
"meaning": {
"fr": "liberté"
}
},
{
"level": "b1-2",
"position": 157,
"text": "freeze",
"meaning": {
"fr": "geler, congeler"
}
},
{
"level": "b1-2",
"position": 158,
"text": "frequent",
"meaning": {
"fr": "fréquent"
}
},
{
"level": "b1-2",
"position": 159,
"text": "frequently",
"meaning": {
"fr": "fréquemment, souvent"
}
},
{
"level": "b1-2",
"position": 160,
"text": "fuel",
"meaning": {
"fr": "carburant, combustible"
}
},
{
"level": "b1-2",
"position": 161,
"text": "fully",
"meaning": {
"fr": "entièrement, pleinement"
}
},
{
"level": "b1-2",
"position": 162,
"text": "function",
"meaning": {
"fr": "fonction ; fonctionner"
}
},
{
"level": "b1-2",
"position": 163,
"text": "fund",
"meaning": {
"fr": "fonds ; financer"
}
},
{
"level": "b1-2",
"position": 164,
"text": "funeral",
"meaning": {
"fr": "funérailles, enterrement"
}
},
{
"level": "b1-2",
"position": 165,
"text": "gain",
"meaning": {
"fr": "gagner, obtenir ; gain"
}
},
{
"level": "b1-2",
"position": 166,
"text": "gang",
"meaning": {
"fr": "bande, gang"
}
},
{
"level": "b1-2",
"position": 167,
"text": "gap",
"meaning": {
"fr": "écart, trou, lacune"
}
},
{
"level": "b1-2",
"position": 168,
"text": "garlic",
"meaning": {
"fr": "ail"
}
},
{
"level": "b1-2",
"position": 169,
"text": "gather",
"meaning": {
"fr": "rassembler, recueillir"
}
},
{
"level": "b1-2",
"position": 170,
"text": "gay",
"meaning": {
"fr": "homosexuel, gay"
}
},
{
"level": "b1-2",
"position": 171,
"text": "gear",
"meaning": {
"fr": "équipement, vitesse (véhicule)"
}
},
{
"level": "b1-2",
"position": 172,
"text": "gender",
"meaning": {
"fr": "genre, sexe"
}
},
{
"level": "b1-2",
"position": 173,
"text": "generation",
"meaning": {
"fr": "génération"
}
},
{
"level": "b1-2",
"position": 174,
"text": "gentleman",
"meaning": {
"fr": "monsieur, gentleman"
}
},
{
"level": "b1-2",
"position": 175,
"text": "gently",
"meaning": {
"fr": "doucement"
}
},
{
"level": "b1-2",
"position": 176,
"text": "giant",
"meaning": {
"fr": "géant"
}
},
{
"level": "b1-2",
"position": 177,
"text": "given",
"meaning": {
"fr": "donné ; étant donné"
}
},
{
"level": "b1-2",
"position": 178,
"text": "global",
"meaning": {
"fr": "mondial, global"
}
},
{
"level": "b1-2",
"position": 179,
"text": "golden",
"meaning": {
"fr": "doré, en or"
}
},
{
"level": "b1-2",
"position": 180,
"text": "gradually",
"meaning": {
"fr": "progressivement"
}
},
{
"level": "b1-2",
"position": 181,
"text": "graduate",
"meaning": {
"fr": "diplômé ; obtenir son diplôme"
}
},
{
"level": "b1-2",
"position": 182,
"text": "grocery",
"meaning": {
"fr": "épicerie"
}
},
{
"level": "b1-2",
"position": 183,
"text": "growing",
"meaning": {
"fr": "en croissance, qui grandit"
}
},
{
"level": "b1-2",
"position": 184,
"text": "growth",
"meaning": {
"fr": "croissance"
}
},
{
"level": "b1-2",
"position": 185,
"text": "guarantee",
"meaning": {
"fr": "garantie ; garantir"
}
},
{
"level": "b1-2",
"position": 186,
"text": "guard",
"meaning": {
"fr": "garde ; surveiller"
}
},
{
"level": "b1-2",
"position": 187,
"text": "guilty",
"meaning": {
"fr": "coupable"
}
},
{
"level": "b1-2",
"position": 188,
"text": "handle",
"meaning": {
"fr": "gérer, manier ; poignée"
}
},
{
"level": "b1-2",
"position": 189,
"text": "hardly",
"meaning": {
"fr": "à peine, guère"
}
},
{
"level": "b1-2",
"position": 190,
"text": "headline",
"meaning": {
"fr": "gros titre"
}
},
{
"level": "b1-3",
"position": 1,
"text": "heaven",
"meaning": {
"fr": "paradis, ciel"
}
},
{
"level": "b1-3",
"position": 2,
"text": "heavily",
"meaning": {
"fr": "lourdement, fortement"
}
},
{
"level": "b1-3",
"position": 3,
"text": "hell",
"meaning": {
"fr": "enfer"
}
},
{
"level": "b1-3",
"position": 4,
"text": "highlight",
"meaning": {
"fr": "souligner ; temps fort"
}
},
{
"level": "b1-3",
"position": 5,
"text": "highly",
"meaning": {
"fr": "très, hautement"
}
},
{
"level": "b1-3",
"position": 6,
"text": "highway",
"meaning": {
"fr": "autoroute"
}
},
{
"level": "b1-3",
"position": 7,
"text": "hire",
"meaning": {
"fr": "embaucher, louer"
}
},
{
"level": "b1-3",
"position": 8,
"text": "historic",
"meaning": {
"fr": "historique (marquant)"
}
},
{
"level": "b1-3",
"position": 9,
"text": "historical",
"meaning": {
"fr": "historique (du passé)"
}
},
{
"level": "b1-3",
"position": 10,
"text": "homeless",
"meaning": {
"fr": "sans-abri"
}
},
{
"level": "b1-3",
"position": 11,
"text": "honey",
"meaning": {
"fr": "miel"
}
},
{
"level": "b1-3",
"position": 12,
"text": "honor",
"meaning": {
"fr": "honneur ; honorer"
}
},
{
"level": "b1-3",
"position": 13,
"text": "horror",
"meaning": {
"fr": "horreur"
}
},
{
"level": "b1-3",
"position": 14,
"text": "host",
"meaning": {
"fr": "hôte, animateur ; accueillir"
}
},
{
"level": "b1-3",
"position": 15,
"text": "humor",
"meaning": {
"fr": "humour"
}
},
{
"level": "b1-3",
"position": 16,
"text": "hunting",
"meaning": {
"fr": "chasse"
}
},
{
"level": "b1-3",
"position": 17,
"text": "ideal",
"meaning": {
"fr": "idéal"
}
},
{
"level": "b1-3",
"position": 18,
"text": "identify",
"meaning": {
"fr": "identifier"
}
},
{
"level": "b1-3",
"position": 19,
"text": "identity",
"meaning": {
"fr": "identité"
}
},
{
"level": "b1-3",
"position": 20,
"text": "ie",
"meaning": {
"fr": "c.-à-d., c'est-à-dire"
}
},
{
"level": "b1-3",
"position": 21,
"text": "ignore",
"meaning": {
"fr": "ignorer"
}
},
{
"level": "b1-3",
"position": 22,
"text": "illegal",
"meaning": {
"fr": "illégal"
}
},
{
"level": "b1-3",
"position": 23,
"text": "illness",
"meaning": {
"fr": "maladie"
}
},
{
"level": "b1-3",
"position": 24,
"text": "illustrate",
"meaning": {
"fr": "illustrer"
}
},
{
"level": "b1-3",
"position": 25,
"text": "imagination",
"meaning": {
"fr": "imagination"
}
},
{
"level": "b1-3",
"position": 26,
"text": "immediate",
"meaning": {
"fr": "immédiat"
}
},
{
"level": "b1-3",
"position": 27,
"text": "immigrant",
"meaning": {
"fr": "immigré, immigrant"
}
},
{
"level": "b1-3",
"position": 28,
"text": "impact",
"meaning": {
"fr": "impact ; affecter"
}
},
{
"level": "b1-3",
"position": 29,
"text": "importance",
"meaning": {
"fr": "importance"
}
},
{
"level": "b1-3",
"position": 30,
"text": "impress",
"meaning": {
"fr": "impressionner"
}
},
{
"level": "b1-3",
"position": 31,
"text": "impression",
"meaning": {
"fr": "impression"
}
},
{
"level": "b1-3",
"position": 32,
"text": "impressive",
"meaning": {
"fr": "impressionnant"
}
},
{
"level": "b1-3",
"position": 33,
"text": "improvement",
"meaning": {
"fr": "amélioration"
}
},
{
"level": "b1-3",
"position": 34,
"text": "income",
"meaning": {
"fr": "revenu(s)"
}
},
{
"level": "b1-3",
"position": 35,
"text": "increased",
"meaning": {
"fr": "accru"
}
},
{
"level": "b1-3",
"position": 36,
"text": "indeed",
"meaning": {
"fr": "en effet, vraiment"
}
},
{
"level": "b1-3",
"position": 37,
"text": "independence",
"meaning": {
"fr": "indépendance"
}
},
{
"level": "b1-3",
"position": 38,
"text": "independent",
"meaning": {
"fr": "indépendant"
}
},
{
"level": "b1-3",
"position": 39,
"text": "indicate",
"meaning": {
"fr": "indiquer"
}
},
{
"level": "b1-3",
"position": 40,
"text": "individual",
"meaning": {
"fr": "individu ; individuel"
}
},
{
"level": "b1-3",
"position": 41,
"text": "industrial",
"meaning": {
"fr": "industriel"
}
},
{
"level": "b1-3",
"position": 42,
"text": "infection",
"meaning": {
"fr": "infection"
}
},
{
"level": "b1-3",
"position": 43,
"text": "influence",
"meaning": {
"fr": "influence ; influencer"
}
},
{
"level": "b1-3",
"position": 44,
"text": "inform",
"meaning": {
"fr": "informer"
}
},
{
"level": "b1-3",
"position": 45,
"text": "initial",
"meaning": {
"fr": "initial"
}
},
{
"level": "b1-3",
"position": 46,
"text": "initially",
"meaning": {
"fr": "initialement, au début"
}
},
{
"level": "b1-3",
"position": 47,
"text": "injury",
"meaning": {
"fr": "blessure"
}
},
{
"level": "b1-3",
"position": 48,
"text": "innocent",
"meaning": {
"fr": "innocent"
}
},
{
"level": "b1-3",
"position": 49,
"text": "insist",
"meaning": {
"fr": "insister"
}
},
{
"level": "b1-3",
"position": 50,
"text": "inspire",
"meaning": {
"fr": "inspirer"
}
},
{
"level": "b1-3",
"position": 51,
"text": "install",
"meaning": {
"fr": "installer"
}
},
{
"level": "b1-3",
"position": 52,
"text": "instructor",
"meaning": {
"fr": "moniteur, instructeur"
}
},
{
"level": "b1-3",
"position": 53,
"text": "insurance",
"meaning": {
"fr": "assurance"
}
},
{
"level": "b1-3",
"position": 54,
"text": "intelligence",
"meaning": {
"fr": "intelligence"
}
},
{
"level": "b1-3",
"position": 55,
"text": "intend",
"meaning": {
"fr": "avoir l'intention de"
}
},
{
"level": "b1-3",
"position": 56,
"text": "intention",
"meaning": {
"fr": "intention"
}
},
{
"level": "b1-3",
"position": 57,
"text": "introduction",
"meaning": {
"fr": "introduction"
}
},
{
"level": "b1-3",
"position": 58,
"text": "invest",
"meaning": {
"fr": "investir"
}
},
{
"level": "b1-3",
"position": 59,
"text": "investigate",
"meaning": {
"fr": "enquêter sur"
}
},
{
"level": "b1-3",
"position": 60,
"text": "investigation",
"meaning": {
"fr": "enquête, investigation"
}
},
{
"level": "b1-3",
"position": 61,
"text": "investment",
"meaning": {
"fr": "investissement"
}
},
{
"level": "b1-3",
"position": 62,
"text": "involve",
"meaning": {
"fr": "impliquer, nécessiter"
}
},
{
"level": "b1-3",
"position": 63,
"text": "involved",
"meaning": {
"fr": "impliqué, concerné"
}
},
{
"level": "b1-3",
"position": 64,
"text": "iron",
"meaning": {
"fr": "fer ; repasser"
}
},
{
"level": "b1-3",
"position": 65,
"text": "issue",
"meaning": {
"fr": "question, problème ; publier"
}
},
{
"level": "b1-3",
"position": 66,
"text": "jail",
"meaning": {
"fr": "prison"
}
},
{
"level": "b1-3",
"position": 67,
"text": "jet",
"meaning": {
"fr": "avion à réaction, jet"
}
},
{
"level": "b1-3",
"position": 68,
"text": "joy",
"meaning": {
"fr": "joie"
}
},
{
"level": "b1-3",
"position": 69,
"text": "judge",
"meaning": {
"fr": "juge ; juger"
}
},
{
"level": "b1-3",
"position": 70,
"text": "junior",
"meaning": {
"fr": "cadet, subalterne"
}
},
{
"level": "b1-3",
"position": 71,
"text": "kick",
"meaning": {
"fr": "coup de pied ; donner un coup de pied"
}
},
{
"level": "b1-3",
"position": 72,
"text": "killer",
"meaning": {
"fr": "tueur, meurtrier"
}
},
{
"level": "b1-3",
"position": 73,
"text": "knock",
"meaning": {
"fr": "frapper ; coup"
}
},
{
"level": "b1-3",
"position": 74,
"text": "knowledge",
"meaning": {
"fr": "connaissance(s), savoir"
}
},
{
"level": "b1-3",
"position": 75,
"text": "lab",
"meaning": {
"fr": "labo"
}
},
{
"level": "b1-3",
"position": 76,
"text": "label",
"meaning": {
"fr": "étiquette ; étiqueter"
}
},
{
"level": "b1-3",
"position": 77,
"text": "laboratory",
"meaning": {
"fr": "laboratoire"
}
},
{
"level": "b1-3",
"position": 78,
"text": "lack",
"meaning": {
"fr": "manque ; manquer de"
}
},
{
"level": "b1-3",
"position": 79,
"text": "landscape",
"meaning": {
"fr": "paysage"
}
},
{
"level": "b1-3",
"position": 80,
"text": "Latin",
"meaning": {
"fr": "latin"
}
},
{
"level": "b1-3",
"position": 81,
"text": "launch",
"meaning": {
"fr": "lancer ; lancement"
}
},
{
"level": "b1-3",
"position": 82,
"text": "lawn",
"meaning": {
"fr": "pelouse"
}
},
{
"level": "b1-3",
"position": 83,
"text": "lawyer",
"meaning": {
"fr": "avocat"
}
},
{
"level": "b1-3",
"position": 84,
"text": "lay",
"meaning": {
"fr": "poser, étendre ; (adj.) laïc"
}
},
{
"level": "b1-3",
"position": 85,
"text": "layer",
"meaning": {
"fr": "couche, strate"
}
},
{
"level": "b1-3",
"position": 86,
"text": "lead",
"meaning": {
"fr": "(v.) mener, conduire ; (n.) avance, plomb"
}
},
{
"level": "b1-3",
"position": 87,
"text": "league",
"meaning": {
"fr": "ligue, championnat"
}
},
{
"level": "b1-3",
"position": 88,
"text": "learning",
"meaning": {
"fr": "apprentissage, savoir"
}
},
{
"level": "b1-3",
"position": 89,
"text": "leather",
"meaning": {
"fr": "cuir"
}
},
{
"level": "b1-3",
"position": 90,
"text": "legal",
"meaning": {
"fr": "légal, juridique"
}
},
{
"level": "b1-3",
"position": 91,
"text": "legend",
"meaning": {
"fr": "légende"
}
},
{
"level": "b1-3",
"position": 92,
"text": "length",
"meaning": {
"fr": "longueur, durée"
}
},
{
"level": "b1-3",
"position": 93,
"text": "license",
"meaning": {
"fr": "permis, licence"
}
},
{
"level": "b1-3",
"position": 94,
"text": "lie",
"meaning": {
"fr": "(v.) mentir, être allongé ; (n.) mensonge"
}
},
{
"level": "b1-3",
"position": 95,
"text": "lifestyle",
"meaning": {
"fr": "mode de vie"
}
},
{
"level": "b1-3",
"position": 96,
"text": "lift",
"meaning": {
"fr": "soulever, lever ; (n.) ascenseur"
}
},
{
"level": "b1-3",
"position": 97,
"text": "likely",
"meaning": {
"fr": "probable"
}
},
{
"level": "b1-3",
"position": 98,
"text": "limit",
"meaning": {
"fr": "limite ; (v.) limiter"
}
},
{
"level": "b1-3",
"position": 99,
"text": "limited",
"meaning": {
"fr": "limité"
}
},
{
"level": "b1-3",
"position": 100,
"text": "link",
"meaning": {
"fr": "lien ; (v.) relier"
}
},
{
"level": "b1-3",
"position": 101,
"text": "lip",
"meaning": {
"fr": "lèvre"
}
},
{
"level": "b1-3",
"position": 102,
"text": "literature",
"meaning": {
"fr": "littérature"
}
},
{
"level": "b1-3",
"position": 103,
"text": "living",
"meaning": {
"fr": "(adj.) vivant ; (n.) gagne-pain"
}
},
{
"level": "b1-3",
"position": 104,
"text": "load",
"meaning": {
"fr": "charge, chargement ; (v.) charger"
}
},
{
"level": "b1-3",
"position": 105,
"text": "loan",
"meaning": {
"fr": "prêt, emprunt"
}
},
{
"level": "b1-3",
"position": 106,
"text": "locate",
"meaning": {
"fr": "localiser, situer"
}
},
{
"level": "b1-3",
"position": 107,
"text": "location",
"meaning": {
"fr": "emplacement, lieu"
}
},
{
"level": "b1-3",
"position": 108,
"text": "lock",
"meaning": {
"fr": "serrure ; (v.) fermer à clé"
}
},
{
"level": "b1-3",
"position": 109,
"text": "loss",
"meaning": {
"fr": "perte"
}
},
{
"level": "b1-3",
"position": 110,
"text": "lower",
"meaning": {
"fr": "inférieur ; (v.) baisser, abaisser"
}
},
{
"level": "b1-3",
"position": 111,
"text": "mad",
"meaning": {
"fr": "fou, furieux"
}
},
{
"level": "b1-3",
"position": 112,
"text": "mainly",
"meaning": {
"fr": "principalement"
}
},
{
"level": "b1-3",
"position": 113,
"text": "major",
"meaning": {
"fr": "majeur, important ; (n.) spécialité"
}
},
{
"level": "b1-3",
"position": 114,
"text": "majority",
"meaning": {
"fr": "majorité"
}
},
{
"level": "b1-3",
"position": 115,
"text": "makeup",
"meaning": {
"fr": "maquillage, composition"
}
},
{
"level": "b1-3",
"position": 116,
"text": "male",
"meaning": {
"fr": "masculin, mâle"
}
},
{
"level": "b1-3",
"position": 117,
"text": "manage",
"meaning": {
"fr": "gérer, réussir à"
}
},
{
"level": "b1-3",
"position": 118,
"text": "management",
"meaning": {
"fr": "gestion, direction"
}
},
{
"level": "b1-3",
"position": 119,
"text": "manufacturer",
"meaning": {
"fr": "fabricant, constructeur"
}
},
{
"level": "b1-3",
"position": 120,
"text": "mark",
"meaning": {
"fr": "marque, note ; (v.) marquer"
}
},
{
"level": "b1-3",
"position": 121,
"text": "marketing",
"meaning": {
"fr": "marketing"
}
},
{
"level": "b1-3",
"position": 122,
"text": "marriage",
"meaning": {
"fr": "mariage"
}
},
{
"level": "b1-3",
"position": 123,
"text": "mask",
"meaning": {
"fr": "masque"
}
},
{
"level": "b1-3",
"position": 124,
"text": "mass",
"meaning": {
"fr": "masse ; (adj.) de masse"
}
},
{
"level": "b1-3",
"position": 125,
"text": "massive",
"meaning": {
"fr": "énorme, massif"
}
},
{
"level": "b1-3",
"position": 126,
"text": "material",
"meaning": {
"fr": "matériau, matière, tissu"
}
},
{
"level": "b1-3",
"position": 127,
"text": "matter",
"meaning": {
"fr": "(v.) importer ; (n.) affaire, matière"
}
},
{
"level": "b1-3",
"position": 128,
"text": "mayor",
"meaning": {
"fr": "maire"
}
},
{
"level": "b1-3",
"position": 129,
"text": "measure",
"meaning": {
"fr": "mesurer ; (n.) mesure"
}
},
{
"level": "b1-3",
"position": 130,
"text": "measurement",
"meaning": {
"fr": "mesure, dimension"
}
},
{
"level": "b1-3",
"position": 131,
"text": "media",
"meaning": {
"fr": "médias"
}
},
{
"level": "b1-3",
"position": 132,
"text": "medical",
"meaning": {
"fr": "médical"
}
},
{
"level": "b1-3",
"position": 133,
"text": "medium",
"meaning": {
"fr": "moyen ; (n.) support, médium"
}
},
{
"level": "b1-3",
"position": 134,
"text": "memory",
"meaning": {
"fr": "mémoire, souvenir"
}
},
{
"level": "b1-3",
"position": 135,
"text": "mental",
"meaning": {
"fr": "mental"
}
},
{
"level": "b1-3",
"position": 136,
"text": "mention",
"meaning": {
"fr": "mentionner ; (n.) mention"
}
},
{
"level": "b1-3",
"position": 137,
"text": "mess",
"meaning": {
"fr": "désordre, pagaille"
}
},
{
"level": "b1-3",
"position": 138,
"text": "meter",
"meaning": {
"fr": "mètre, compteur"
}
},
{
"level": "b1-3",
"position": 139,
"text": "military",
"meaning": {
"fr": "militaire ; (n.) armée"
}
},
{
"level": "b1-3",
"position": 140,
"text": "mind",
"meaning": {
"fr": "esprit ; (v.) faire attention, déranger"
}
},
{
"level": "b1-3",
"position": 141,
"text": "mine",
"meaning": {
"fr": "le mien ; (n.) mine"
}
},
{
"level": "b1-3",
"position": 142,
"text": "minister",
"meaning": {
"fr": "ministre, pasteur"
}
},
{
"level": "b1-3",
"position": 143,
"text": "mission",
"meaning": {
"fr": "mission"
}
},
{
"level": "b1-3",
"position": 144,
"text": "mix",
"meaning": {
"fr": "mélanger ; (n.) mélange"
}
},
{
"level": "b1-3",
"position": 145,
"text": "mixture",
"meaning": {
"fr": "mélange, mixture"
}
},
{
"level": "b1-3",
"position": 146,
"text": "mood",
"meaning": {
"fr": "humeur"
}
},
{
"level": "b1-3",
"position": 147,
"text": "mostly",
"meaning": {
"fr": "surtout, pour la plupart"
}
},
{
"level": "b1-3",
"position": 148,
"text": "motivation",
"meaning": {
"fr": "motivation"
}
},
{
"level": "b1-3",
"position": 149,
"text": "movement",
"meaning": {
"fr": "mouvement, déplacement"
}
},
{
"level": "b1-3",
"position": 150,
"text": "murder",
"meaning": {
"fr": "meurtre ; (v.) assassiner"
}
},
{
"level": "b1-3",
"position": 151,
"text": "muscle",
"meaning": {
"fr": "muscle"
}
},
{
"level": "b1-3",
"position": 152,
"text": "mystery",
"meaning": {
"fr": "mystère"
}
},
{
"level": "b1-3",
"position": 153,
"text": "narrow",
"meaning": {
"fr": "étroit"
}
},
{
"level": "b1-3",
"position": 154,
"text": "nation",
"meaning": {
"fr": "nation"
}
},
{
"level": "b1-3",
"position": 155,
"text": "native",
"meaning": {
"fr": "natal, autochtone, natif"
}
},
{
"level": "b1-3",
"position": 156,
"text": "naturally",
"meaning": {
"fr": "naturellement"
}
},
{
"level": "b1-3",
"position": 157,
"text": "nearby",
"meaning": {
"fr": "à proximité, tout près"
}
},
{
"level": "b1-3",
"position": 158,
"text": "negative",
"meaning": {
"fr": "négatif"
}
},
{
"level": "b1-3",
"position": 159,
"text": "neighborhood",
"meaning": {
"fr": "quartier, voisinage"
}
},
{
"level": "b1-3",
"position": 160,
"text": "neither",
"meaning": {
"fr": "aucun des deux, ni"
}
},
{
"level": "b1-3",
"position": 161,
"text": "network",
"meaning": {
"fr": "réseau"
}
},
{
"level": "b1-3",
"position": 162,
"text": "none",
"meaning": {
"fr": "aucun (de), nul"
}
},
{
"level": "b1-3",
"position": 163,
"text": "northern",
"meaning": {
"fr": "du nord, septentrional"
}
},
{
"level": "b1-3",
"position": 164,
"text": "nowhere",
"meaning": {
"fr": "nulle part"
}
},
{
"level": "b1-3",
"position": 165,
"text": "nuclear",
"meaning": {
"fr": "nucléaire"
}
},
{
"level": "b1-3",
"position": 166,
"text": "nut",
"meaning": {
"fr": "noix, écrou"
}
},
{
"level": "b1-3",
"position": 167,
"text": "object",
"meaning": {
"fr": "objet ; (v.) objecter"
}
},
{
"level": "b1-3",
"position": 168,
"text": "observe",
"meaning": {
"fr": "observer"
}
},
{
"level": "b1-3",
"position": 169,
"text": "obvious",
"meaning": {
"fr": "évident"
}
},
{
"level": "b1-3",
"position": 170,
"text": "obviously",
"meaning": {
"fr": "évidemment"
}
},
{
"level": "b1-3",
"position": 171,
"text": "occasion",
"meaning": {
"fr": "occasion, événement"
}
},
{
"level": "b1-3",
"position": 172,
"text": "occasionally",
"meaning": {
"fr": "de temps en temps, parfois"
}
},
{
"level": "b1-3",
"position": 173,
"text": "occur",
"meaning": {
"fr": "se produire, survenir"
}
},
{
"level": "b1-3",
"position": 174,
"text": "odd",
"meaning": {
"fr": "bizarre, impair"
}
},
{
"level": "b1-3",
"position": 175,
"text": "officer",
"meaning": {
"fr": "officier, agent"
}
},
{
"level": "b1-3",
"position": 176,
"text": "official",
"meaning": {
"fr": "officiel ; (n.) fonctionnaire"
}
},
{
"level": "b1-3",
"position": 177,
"text": "Olympic",
"meaning": {
"fr": "olympique"
}
},
{
"level": "b1-3",
"position": 178,
"text": "onto",
"meaning": {
"fr": "sur (avec mouvement)"
}
},
{
"level": "b1-3",
"position": 179,
"text": "operation",
"meaning": {
"fr": "opération"
}
},
{
"level": "b1-3",
"position": 180,
"text": "opportunity",
"meaning": {
"fr": "occasion, opportunité"
}
},
{
"level": "b1-3",
"position": 181,
"text": "option",
"meaning": {
"fr": "option, choix"
}
},
{
"level": "b1-3",
"position": 182,
"text": "ordinary",
"meaning": {
"fr": "ordinaire, banal"
}
},
{
"level": "b1-3",
"position": 183,
"text": "organic",
"meaning": {
"fr": "bio, biologique"
}
},
{
"level": "b1-3",
"position": 184,
"text": "organization",
"meaning": {
"fr": "organisation, organisme"
}
},
{
"level": "b1-3",
"position": 185,
"text": "organize",
"meaning": {
"fr": "organiser"
}
},
{
"level": "b1-3",
"position": 186,
"text": "origin",
"meaning": {
"fr": "origine"
}
},
{
"level": "b1-3",
"position": 187,
"text": "originally",
"meaning": {
"fr": "à l'origine"
}
},
{
"level": "b1-3",
"position": 188,
"text": "ought",
"meaning": {
"fr": "devrait (ought to)"
}
},
{
"level": "b1-3",
"position": 189,
"text": "oven",
"meaning": {
"fr": "four"
}
},
{
"level": "b1-3",
"position": 190,
"text": "owe",
"meaning": {
"fr": "devoir (de l'argent)"
}
},
{
"level": "b1-4",
"position": 1,
"text": "owner",
"meaning": {
"fr": "propriétaire"
}
},
{
"level": "b1-4",
"position": 2,
"text": "pack",
"meaning": {
"fr": "emballer, faire ses valises ; (n.) paquet"
}
},
{
"level": "b1-4",
"position": 3,
"text": "package",
"meaning": {
"fr": "colis, paquet, forfait"
}
},
{
"level": "b1-4",
"position": 4,
"text": "painful",
"meaning": {
"fr": "douloureux"
}
},
{
"level": "b1-4",
"position": 5,
"text": "Palestinian",
"meaning": {
"fr": "palestinien"
}
},
{
"level": "b1-4",
"position": 6,
"text": "pan",
"meaning": {
"fr": "poêle, casserole"
}
},
{
"level": "b1-4",
"position": 7,
"text": "pant",
"meaning": {
"fr": "pantalon (US) ; (v.) haleter"
}
},
{
"level": "b1-4",
"position": 8,
"text": "participate",
"meaning": {
"fr": "participer"
}
},
{
"level": "b1-4",
"position": 9,
"text": "particular",
"meaning": {
"fr": "particulier, précis"
}
},
{
"level": "b1-4",
"position": 10,
"text": "particularly",
"meaning": {
"fr": "particulièrement"
}
},
{
"level": "b1-4",
"position": 11,
"text": "partly",
"meaning": {
"fr": "en partie, partiellement"
}
},
{
"level": "b1-4",
"position": 12,
"text": "passage",
"meaning": {
"fr": "passage, couloir, extrait"
}
},
{
"level": "b1-4",
"position": 13,
"text": "passion",
"meaning": {
"fr": "passion"
}
},
{
"level": "b1-4",
"position": 14,
"text": "path",
"meaning": {
"fr": "chemin, sentier"
}
},
{
"level": "b1-4",
"position": 15,
"text": "patient",
"meaning": {
"fr": "patient"
}
},
{
"level": "b1-4",
"position": 16,
"text": "pattern",
"meaning": {
"fr": "motif, modèle, schéma"
}
},
{
"level": "b1-4",
"position": 17,
"text": "pause",
"meaning": {
"fr": "pause ; (v.) faire une pause"
}
},
{
"level": "b1-4",
"position": 18,
"text": "payment",
"meaning": {
"fr": "paiement, versement"
}
},
{
"level": "b1-4",
"position": 19,
"text": "peace",
"meaning": {
"fr": "paix"
}
},
{
"level": "b1-4",
"position": 20,
"text": "penalty",
"meaning": {
"fr": "pénalité, sanction, penalty"
}
},
{
"level": "b1-4",
"position": 21,
"text": "per",
"meaning": {
"fr": "par"
}
},
{
"level": "b1-4",
"position": 22,
"text": "percentage",
"meaning": {
"fr": "pourcentage"
}
},
{
"level": "b1-4",
"position": 23,
"text": "perfectly",
"meaning": {
"fr": "parfaitement"
}
},
{
"level": "b1-4",
"position": 24,
"text": "perform",
"meaning": {
"fr": "interpréter, accomplir, jouer"
}
},
{
"level": "b1-4",
"position": 25,
"text": "performance",
"meaning": {
"fr": "performance, représentation"
}
},
{
"level": "b1-4",
"position": 26,
"text": "permanent",
"meaning": {
"fr": "permanent"
}
},
{
"level": "b1-4",
"position": 27,
"text": "permission",
"meaning": {
"fr": "permission, autorisation"
}
},
{
"level": "b1-4",
"position": 28,
"text": "personality",
"meaning": {
"fr": "personnalité"
}
},
{
"level": "b1-4",
"position": 29,
"text": "personally",
"meaning": {
"fr": "personnellement"
}
},
{
"level": "b1-4",
"position": 30,
"text": "persuade",
"meaning": {
"fr": "persuader, convaincre"
}
},
{
"level": "b1-4",
"position": 31,
"text": "physical",
"meaning": {
"fr": "physique"
}
},
{
"level": "b1-4",
"position": 32,
"text": "physically",
"meaning": {
"fr": "physiquement"
}
},
{
"level": "b1-4",
"position": 33,
"text": "pilot",
"meaning": {
"fr": "pilote"
}
},
{
"level": "b1-4",
"position": 34,
"text": "pipe",
"meaning": {
"fr": "tuyau, pipe"
}
},
{
"level": "b1-4",
"position": 35,
"text": "planning",
"meaning": {
"fr": "planification, organisation"
}
},
{
"level": "b1-4",
"position": 36,
"text": "platform",
"meaning": {
"fr": "quai, plateforme"
}
},
{
"level": "b1-4",
"position": 37,
"text": "pleasure",
"meaning": {
"fr": "plaisir"
}
},
{
"level": "b1-4",
"position": 38,
"text": "plenty",
"meaning": {
"fr": "beaucoup, bien assez"
}
},
{
"level": "b1-4",
"position": 39,
"text": "plot",
"meaning": {
"fr": "intrigue, complot, parcelle"
}
},
{
"level": "b1-4",
"position": 40,
"text": "plus",
"meaning": {
"fr": "plus, en plus"
}
},
{
"level": "b1-4",
"position": 41,
"text": "poet",
"meaning": {
"fr": "poète"
}
},
{
"level": "b1-4",
"position": 42,
"text": "poetry",
"meaning": {
"fr": "poésie"
}
},
{
"level": "b1-4",
"position": 43,
"text": "policy",
"meaning": {
"fr": "politique (règles), police d'assurance"
}
},
{
"level": "b1-4",
"position": 44,
"text": "political",
"meaning": {
"fr": "politique (adj.)"
}
},
{
"level": "b1-4",
"position": 45,
"text": "politician",
"meaning": {
"fr": "homme ou femme politique"
}
},
{
"level": "b1-4",
"position": 46,
"text": "politics",
"meaning": {
"fr": "la politique (vie politique)"
}
},
{
"level": "b1-4",
"position": 47,
"text": "pop",
"meaning": {
"fr": "pop ; (v.) éclater"
}
},
{
"level": "b1-4",
"position": 48,
"text": "population",
"meaning": {
"fr": "population"
}
},
{
"level": "b1-4",
"position": 49,
"text": "port",
"meaning": {
"fr": "port"
}
},
{
"level": "b1-4",
"position": 50,
"text": "portrait",
"meaning": {
"fr": "portrait"
}
},
{
"level": "b1-4",
"position": 51,
"text": "position",
"meaning": {
"fr": "position, poste"
}
},
{
"level": "b1-4",
"position": 52,
"text": "possibility",
"meaning": {
"fr": "possibilité"
}
},
{
"level": "b1-4",
"position": 53,
"text": "possibly",
"meaning": {
"fr": "peut-être, éventuellement"
}
},
{
"level": "b1-4",
"position": 54,
"text": "pot",
"meaning": {
"fr": "pot, marmite"
}
},
{
"level": "b1-4",
"position": 55,
"text": "potential",
"meaning": {
"fr": "potentiel"
}
},
{
"level": "b1-4",
"position": 56,
"text": "pour",
"meaning": {
"fr": "verser"
}
},
{
"level": "b1-4",
"position": 57,
"text": "poverty",
"meaning": {
"fr": "pauvreté"
}
},
{
"level": "b1-4",
"position": 58,
"text": "powerful",
"meaning": {
"fr": "puissant"
}
},
{
"level": "b1-4",
"position": 59,
"text": "practical",
"meaning": {
"fr": "pratique, concret"
}
},
{
"level": "b1-4",
"position": 60,
"text": "pray",
"meaning": {
"fr": "prier"
}
},
{
"level": "b1-4",
"position": 61,
"text": "predict",
"meaning": {
"fr": "prédire, prévoir"
}
},
{
"level": "b1-4",
"position": 62,
"text": "pregnant",
"meaning": {
"fr": "enceinte"
}
},
{
"level": "b1-4",
"position": 63,
"text": "preparation",
"meaning": {
"fr": "préparation"
}
},
{
"level": "b1-4",
"position": 64,
"text": "presentation",
"meaning": {
"fr": "présentation, exposé"
}
},
{
"level": "b1-4",
"position": 65,
"text": "press",
"meaning": {
"fr": "(n.) presse ; (v.) appuyer"
}
},
{
"level": "b1-4",
"position": 66,
"text": "pressure",
"meaning": {
"fr": "pression"
}
},
{
"level": "b1-4",
"position": 67,
"text": "pretend",
"meaning": {
"fr": "faire semblant"
}
},
{
"level": "b1-4",
"position": 68,
"text": "prevent",
"meaning": {
"fr": "empêcher, prévenir"
}
},
{
"level": "b1-4",
"position": 69,
"text": "previous",
"meaning": {
"fr": "précédent"
}
},
{
"level": "b1-4",
"position": 70,
"text": "priest",
"meaning": {
"fr": "prêtre"
}
},
{
"level": "b1-4",
"position": 71,
"text": "primary",
"meaning": {
"fr": "primaire, premier"
}
},
{
"level": "b1-4",
"position": 72,
"text": "prison",
"meaning": {
"fr": "prison"
}
},
{
"level": "b1-4",
"position": 73,
"text": "prisoner",
"meaning": {
"fr": "prisonnier"
}
},
{
"level": "b1-4",
"position": 74,
"text": "private",
"meaning": {
"fr": "privé"
}
},
{
"level": "b1-4",
"position": 75,
"text": "process",
"meaning": {
"fr": "(n.) processus ; (v.) traiter"
}
},
{
"level": "b1-4",
"position": 76,
"text": "produce",
"meaning": {
"fr": "produire"
}
},
{
"level": "b1-4",
"position": 77,
"text": "producer",
"meaning": {
"fr": "producteur"
}
},
{
"level": "b1-4",
"position": 78,
"text": "production",
"meaning": {
"fr": "production"
}
},
{
"level": "b1-4",
"position": 79,
"text": "profession",
"meaning": {
"fr": "profession, métier"
}
},
{
"level": "b1-4",
"position": 80,
"text": "professional",
"meaning": {
"fr": "professionnel"
}
},
{
"level": "b1-4",
"position": 81,
"text": "professor",
"meaning": {
"fr": "professeur (d'université)"
}
},
{
"level": "b1-4",
"position": 82,
"text": "profit",
"meaning": {
"fr": "bénéfice, profit"
}
},
{
"level": "b1-4",
"position": 83,
"text": "progress",
"meaning": {
"fr": "progrès"
}
},
{
"level": "b1-4",
"position": 84,
"text": "promise",
"meaning": {
"fr": "(n.) promesse ; (v.) promettre"
}
},
{
"level": "b1-4",
"position": 85,
"text": "promote",
"meaning": {
"fr": "promouvoir"
}
},
{
"level": "b1-4",
"position": 86,
"text": "proper",
"meaning": {
"fr": "convenable, approprié"
}
},
{
"level": "b1-4",
"position": 87,
"text": "properly",
"meaning": {
"fr": "correctement"
}
},
{
"level": "b1-4",
"position": 88,
"text": "property",
"meaning": {
"fr": "propriété, biens"
}
},
{
"level": "b1-4",
"position": 89,
"text": "protection",
"meaning": {
"fr": "protection"
}
},
{
"level": "b1-4",
"position": 90,
"text": "protest",
"meaning": {
"fr": "(n.) manifestation ; (v.) protester"
}
},
{
"level": "b1-4",
"position": 91,
"text": "proud",
"meaning": {
"fr": "fier"
}
},
{
"level": "b1-4",
"position": 92,
"text": "prove",
"meaning": {
"fr": "prouver"
}
},
{
"level": "b1-4",
"position": 93,
"text": "provide",
"meaning": {
"fr": "fournir"
}
},
{
"level": "b1-4",
"position": 94,
"text": "publish",
"meaning": {
"fr": "publier"
}
},
{
"level": "b1-4",
"position": 95,
"text": "punishment",
"meaning": {
"fr": "punition"
}
},
{
"level": "b1-4",
"position": 96,
"text": "purchase",
"meaning": {
"fr": "(n.) achat ; (v.) acheter"
}
},
{
"level": "b1-4",
"position": 97,
"text": "purpose",
"meaning": {
"fr": "but, objectif"
}
},
{
"level": "b1-4",
"position": 98,
"text": "quietly",
"meaning": {
"fr": "doucement, silencieusement"
}
},
{
"level": "b1-4",
"position": 99,
"text": "quit",
"meaning": {
"fr": "quitter, arrêter"
}
},
{
"level": "b1-4",
"position": 100,
"text": "quote",
"meaning": {
"fr": "(v.) citer ; (n.) citation"
}
},
{
"level": "b1-4",
"position": 101,
"text": "rail",
"meaning": {
"fr": "rail, rampe"
}
},
{
"level": "b1-4",
"position": 102,
"text": "raise",
"meaning": {
"fr": "lever, augmenter, élever"
}
},
{
"level": "b1-4",
"position": 103,
"text": "range",
"meaning": {
"fr": "gamme, éventail"
}
},
{
"level": "b1-4",
"position": 104,
"text": "rare",
"meaning": {
"fr": "rare"
}
},
{
"level": "b1-4",
"position": 105,
"text": "rarely",
"meaning": {
"fr": "rarement"
}
},
{
"level": "b1-4",
"position": 106,
"text": "rate",
"meaning": {
"fr": "taux, tarif"
}
},
{
"level": "b1-4",
"position": 107,
"text": "reach",
"meaning": {
"fr": "atteindre"
}
},
{
"level": "b1-4",
"position": 108,
"text": "react",
"meaning": {
"fr": "réagir"
}
},
{
"level": "b1-4",
"position": 109,
"text": "reaction",
"meaning": {
"fr": "réaction"
}
},
{
"level": "b1-4",
"position": 110,
"text": "reality",
"meaning": {
"fr": "réalité"
}
},
{
"level": "b1-4",
"position": 111,
"text": "realize",
"meaning": {
"fr": "se rendre compte, réaliser"
}
},
{
"level": "b1-4",
"position": 112,
"text": "reasonable",
"meaning": {
"fr": "raisonnable"
}
},
{
"level": "b1-4",
"position": 113,
"text": "recent",
"meaning": {
"fr": "récent"
}
},
{
"level": "b1-4",
"position": 114,
"text": "recognize",
"meaning": {
"fr": "reconnaître"
}
},
{
"level": "b1-4",
"position": 115,
"text": "recommend",
"meaning": {
"fr": "recommander"
}
},
{
"level": "b1-4",
"position": 116,
"text": "recording",
"meaning": {
"fr": "enregistrement"
}
},
{
"level": "b1-4",
"position": 117,
"text": "recover",
"meaning": {
"fr": "se rétablir, récupérer"
}
},
{
"level": "b1-4",
"position": 118,
"text": "reduce",
"meaning": {
"fr": "réduire"
}
},
{
"level": "b1-4",
"position": 119,
"text": "refer",
"meaning": {
"fr": "faire référence, renvoyer"
}
},
{
"level": "b1-4",
"position": 120,
"text": "reference",
"meaning": {
"fr": "référence"
}
},
{
"level": "b1-4",
"position": 121,
"text": "reflect",
"meaning": {
"fr": "refléter, réfléchir"
}
},
{
"level": "b1-4",
"position": 122,
"text": "refuse",
"meaning": {
"fr": "refuser"
}
},
{
"level": "b1-4",
"position": 123,
"text": "region",
"meaning": {
"fr": "région"
}
},
{
"level": "b1-4",
"position": 124,
"text": "register",
"meaning": {
"fr": "s'inscrire, enregistrer"
}
},
{
"level": "b1-4",
"position": 125,
"text": "regularly",
"meaning": {
"fr": "régulièrement"
}
},
{
"level": "b1-4",
"position": 126,
"text": "reject",
"meaning": {
"fr": "rejeter"
}
},
{
"level": "b1-4",
"position": 127,
"text": "relate",
"meaning": {
"fr": "relier, se rapporter à"
}
},
{
"level": "b1-4",
"position": 128,
"text": "relation",
"meaning": {
"fr": "relation, rapport"
}
},
{
"level": "b1-4",
"position": 129,
"text": "relationship",
"meaning": {
"fr": "relation, lien"
}
},
{
"level": "b1-4",
"position": 130,
"text": "relative",
"meaning": {
"fr": "(n.) parent ; (adj.) relatif"
}
},
{
"level": "b1-4",
"position": 131,
"text": "relatively",
"meaning": {
"fr": "relativement"
}
},
{
"level": "b1-4",
"position": 132,
"text": "release",
"meaning": {
"fr": "libérer, sortir, publier"
}
},
{
"level": "b1-4",
"position": 133,
"text": "religion",
"meaning": {
"fr": "religion"
}
},
{
"level": "b1-4",
"position": 134,
"text": "religious",
"meaning": {
"fr": "religieux"
}
},
{
"level": "b1-4",
"position": 135,
"text": "rely",
"meaning": {
"fr": "compter sur, dépendre de"
}
},
{
"level": "b1-4",
"position": 136,
"text": "remain",
"meaning": {
"fr": "rester, demeurer"
}
},
{
"level": "b1-4",
"position": 137,
"text": "remaining",
"meaning": {
"fr": "restant"
}
},
{
"level": "b1-4",
"position": 138,
"text": "remarkable",
"meaning": {
"fr": "remarquable"
}
},
{
"level": "b1-4",
"position": 139,
"text": "remind",
"meaning": {
"fr": "rappeler (à qqn)"
}
},
{
"level": "b1-4",
"position": 140,
"text": "remote",
"meaning": {
"fr": "éloigné, isolé"
}
},
{
"level": "b1-4",
"position": 141,
"text": "remove",
"meaning": {
"fr": "enlever, retirer"
}
},
{
"level": "b1-4",
"position": 142,
"text": "replace",
"meaning": {
"fr": "remplacer"
}
},
{
"level": "b1-4",
"position": 143,
"text": "reporter",
"meaning": {
"fr": "journaliste, reporter"
}
},
{
"level": "b1-4",
"position": 144,
"text": "represent",
"meaning": {
"fr": "représenter"
}
},
{
"level": "b1-4",
"position": 145,
"text": "reputation",
"meaning": {
"fr": "réputation"
}
},
{
"level": "b1-4",
"position": 146,
"text": "request",
"meaning": {
"fr": "(n.) demande ; (v.) demander"
}
},
{
"level": "b1-4",
"position": 147,
"text": "require",
"meaning": {
"fr": "exiger, nécessiter"
}
},
{
"level": "b1-4",
"position": 148,
"text": "researcher",
"meaning": {
"fr": "chercheur"
}
},
{
"level": "b1-4",
"position": 149,
"text": "reservation",
"meaning": {
"fr": "réservation"
}
},
{
"level": "b1-4",
"position": 150,
"text": "resource",
"meaning": {
"fr": "ressource"
}
},
{
"level": "b1-4",
"position": 151,
"text": "respect",
"meaning": {
"fr": "(n.) respect ; (v.) respecter"
}
},
{
"level": "b1-4",
"position": 152,
"text": "respond",
"meaning": {
"fr": "réagir, répondre à"
}
},
{
"level": "b1-4",
"position": 153,
"text": "response",
"meaning": {
"fr": "réaction, réponse"
}
},
{
"level": "b1-4",
"position": 154,
"text": "responsibility",
"meaning": {
"fr": "responsabilité"
}
},
{
"level": "b1-4",
"position": 155,
"text": "responsible",
"meaning": {
"fr": "responsable"
}
},
{
"level": "b1-4",
"position": 156,
"text": "retire",
"meaning": {
"fr": "prendre sa retraite"
}
},
{
"level": "b1-4",
"position": 157,
"text": "review",
"meaning": {
"fr": "(n.) critique, bilan ; (v.) réviser"
}
},
{
"level": "b1-4",
"position": 158,
"text": "revolution",
"meaning": {
"fr": "révolution"
}
},
{
"level": "b1-4",
"position": 159,
"text": "rhythm",
"meaning": {
"fr": "rythme"
}
},
{
"level": "b1-4",
"position": 160,
"text": "rid",
"meaning": {
"fr": "se débarrasser de (get rid of)"
}
},
{
"level": "b1-4",
"position": 161,
"text": "rise",
"meaning": {
"fr": "(v.) monter, s'élever ; (n.) hausse"
}
},
{
"level": "b1-4",
"position": 162,
"text": "risk",
"meaning": {
"fr": "risque"
}
},
{
"level": "b1-4",
"position": 163,
"text": "role",
"meaning": {
"fr": "rôle"
}
},
{
"level": "b1-4",
"position": 164,
"text": "roll",
"meaning": {
"fr": "(v.) rouler ; (n.) rouleau"
}
},
{
"level": "b1-4",
"position": 165,
"text": "romantic",
"meaning": {
"fr": "romantique"
}
},
{
"level": "b1-4",
"position": 166,
"text": "roof",
"meaning": {
"fr": "toit"
}
},
{
"level": "b1-4",
"position": 167,
"text": "rope",
"meaning": {
"fr": "corde"
}
},
{
"level": "b1-4",
"position": 168,
"text": "rose",
"meaning": {
"fr": "rose (fleur)"
}
},
{
"level": "b1-4",
"position": 169,
"text": "route",
"meaning": {
"fr": "itinéraire, trajet"
}
},
{
"level": "b1-4",
"position": 170,
"text": "row",
"meaning": {
"fr": "rangée, rang"
}
},
{
"level": "b1-4",
"position": 171,
"text": "rush",
"meaning": {
"fr": "(v.) se précipiter ; (n.) ruée"
}
},
{
"level": "b1-4",
"position": 172,
"text": "safety",
"meaning": {
"fr": "sûreté, sécurité physique"
}
},
{
"level": "b1-4",
"position": 173,
"text": "salary",
"meaning": {
"fr": "salaire"
}
},
{
"level": "b1-4",
"position": 174,
"text": "sales",
"meaning": {
"fr": "ventes, chiffre d'affaires"
}
},
{
"level": "b1-4",
"position": 175,
"text": "sample",
"meaning": {
"fr": "échantillon"
}
},
{
"level": "b1-4",
"position": 176,
"text": "saving",
"meaning": {
"fr": "économie, épargne"
}
},
{
"level": "b1-4",
"position": 177,
"text": "scene",
"meaning": {
"fr": "scène, lieu"
}
},
{
"level": "b1-4",
"position": 178,
"text": "schedule",
"meaning": {
"fr": "emploi du temps, horaire"
}
},
{
"level": "b1-4",
"position": 179,
"text": "scientific",
"meaning": {
"fr": "scientifique (adj.)"
}
},
{
"level": "b1-4",
"position": 180,
"text": "scream",
"meaning": {
"fr": "(v.) hurler ; (n.) cri perçant"
}
},
{
"level": "b1-4",
"position": 181,
"text": "security",
"meaning": {
"fr": "sécurité, protection"
}
},
{
"level": "b1-4",
"position": 182,
"text": "seed",
"meaning": {
"fr": "graine, semence"
}
},
{
"level": "b1-4",
"position": 183,
"text": "select",
"meaning": {
"fr": "sélectionner, choisir"
}
},
{
"level": "b1-4",
"position": 184,
"text": "selection",
"meaning": {
"fr": "sélection, choix"
}
},
{
"level": "b1-4",
"position": 185,
"text": "senior",
"meaning": {
"fr": "supérieur, aîné, senior"
}
},
{
"level": "b1-4",
"position": 186,
"text": "sense",
"meaning": {
"fr": "sens, sentiment"
}
},
{
"level": "b1-4",
"position": 187,
"text": "separate",
"meaning": {
"fr": "(adj.) séparé ; (v.) séparer"
}
},
{
"level": "b1-4",
"position": 188,
"text": "seriously",
"meaning": {
"fr": "sérieusement, gravement"
}
},
{
"level": "b1-4",
"position": 189,
"text": "session",
"meaning": {
"fr": "séance, session"
}
},
{
"level": "b1-4",
"position": 190,
"text": "setting",
"meaning": {
"fr": "cadre, réglage"
}
},
{
"level": "b1-5",
"position": 1,
"text": "settle",
"meaning": {
"fr": "régler, s'installer"
}
},
{
"level": "b1-5",
"position": 2,
"text": "sex",
"meaning": {
"fr": "sexe, rapports sexuels"
}
},
{
"level": "b1-5",
"position": 3,
"text": "shadow",
"meaning": {
"fr": "ombre (portée)"
}
},
{
"level": "b1-5",
"position": 4,
"text": "shake",
"meaning": {
"fr": "secouer, trembler"
}
},
{
"level": "b1-5",
"position": 5,
"text": "shall",
"meaning": {
"fr": "auxiliaire du futur, devoir"
}
},
{
"level": "b1-5",
"position": 6,
"text": "sharp",
"meaning": {
"fr": "tranchant, aigu, net"
}
},
{
"level": "b1-5",
"position": 7,
"text": "sheet",
"meaning": {
"fr": "feuille, drap"
}
},
{
"level": "b1-5",
"position": 8,
"text": "shell",
"meaning": {
"fr": "coquille, coquillage, obus"
}
},
{
"level": "b1-5",
"position": 9,
"text": "shine",
"meaning": {
"fr": "briller"
}
},
{
"level": "b1-5",
"position": 10,
"text": "shock",
"meaning": {
"fr": "choc"
}
},
{
"level": "b1-5",
"position": 11,
"text": "shoot",
"meaning": {
"fr": "tirer (arme), abattre"
}
},
{
"level": "b1-5",
"position": 12,
"text": "shooting",
"meaning": {
"fr": "fusillade, tournage"
}
},
{
"level": "b1-5",
"position": 13,
"text": "shot",
"meaning": {
"fr": "coup de feu, tir"
}
},
{
"level": "b1-5",
"position": 14,
"text": "shut",
"meaning": {
"fr": "fermer"
}
},
{
"level": "b1-5",
"position": 15,
"text": "sight",
"meaning": {
"fr": "vue, spectacle"
}
},
{
"level": "b1-5",
"position": 16,
"text": "signal",
"meaning": {
"fr": "signal"
}
},
{
"level": "b1-5",
"position": 17,
"text": "significant",
"meaning": {
"fr": "important, significatif"
}
},
{
"level": "b1-5",
"position": 18,
"text": "silence",
"meaning": {
"fr": "silence"
}
},
{
"level": "b1-5",
"position": 19,
"text": "silent",
"meaning": {
"fr": "silencieux"
}
},
{
"level": "b1-5",
"position": 20,
"text": "simply",
"meaning": {
"fr": "simplement"
}
},
{
"level": "b1-5",
"position": 21,
"text": "sink",
"meaning": {
"fr": "(n.) évier ; (v.) couler"
}
},
{
"level": "b1-5",
"position": 22,
"text": "sir",
"meaning": {
"fr": "monsieur"
}
},
{
"level": "b1-5",
"position": 23,
"text": "site",
"meaning": {
"fr": "site, emplacement"
}
},
{
"level": "b1-5",
"position": 24,
"text": "slice",
"meaning": {
"fr": "tranche"
}
},
{
"level": "b1-5",
"position": 25,
"text": "slide",
"meaning": {
"fr": "(v.) glisser ; (n.) toboggan, diapositive"
}
},
{
"level": "b1-5",
"position": 26,
"text": "slightly",
"meaning": {
"fr": "légèrement"
}
},
{
"level": "b1-5",
"position": 27,
"text": "smart",
"meaning": {
"fr": "intelligent, malin, élégant"
}
},
{
"level": "b1-5",
"position": 28,
"text": "smooth",
"meaning": {
"fr": "lisse, fluide"
}
},
{
"level": "b1-5",
"position": 29,
"text": "society",
"meaning": {
"fr": "société"
}
},
{
"level": "b1-5",
"position": 30,
"text": "soldier",
"meaning": {
"fr": "soldat"
}
},
{
"level": "b1-5",
"position": 31,
"text": "solid",
"meaning": {
"fr": "solide"
}
},
{
"level": "b1-5",
"position": 32,
"text": "somehow",
"meaning": {
"fr": "d'une manière ou d'une autre"
}
},
{
"level": "b1-5",
"position": 33,
"text": "source",
"meaning": {
"fr": "source"
}
},
{
"level": "b1-5",
"position": 34,
"text": "southern",
"meaning": {
"fr": "du sud, méridional"
}
},
{
"level": "b1-5",
"position": 35,
"text": "speaker",
"meaning": {
"fr": "orateur, haut-parleur"
}
},
{
"level": "b1-5",
"position": 36,
"text": "specialist",
"meaning": {
"fr": "spécialiste"
}
},
{
"level": "b1-5",
"position": 37,
"text": "species",
"meaning": {
"fr": "espèce"
}
},
{
"level": "b1-5",
"position": 38,
"text": "specific",
"meaning": {
"fr": "spécifique, précis"
}
},
{
"level": "b1-5",
"position": 39,
"text": "specifically",
"meaning": {
"fr": "spécifiquement, en particulier"
}
},
{
"level": "b1-5",
"position": 40,
"text": "speech",
"meaning": {
"fr": "discours, parole"
}
},
{
"level": "b1-5",
"position": 41,
"text": "spirit",
"meaning": {
"fr": "esprit, âme, moral"
}
},
{
"level": "b1-5",
"position": 42,
"text": "split",
"meaning": {
"fr": "diviser, partager, se séparer"
}
},
{
"level": "b1-5",
"position": 43,
"text": "spot",
"meaning": {
"fr": "endroit, tache ; (v.) repérer"
}
},
{
"level": "b1-5",
"position": 44,
"text": "spread",
"meaning": {
"fr": "répandre, étaler ; (n.) propagation"
}
},
{
"level": "b1-5",
"position": 45,
"text": "stable",
"meaning": {
"fr": "stable ; (n.) écurie"
}
},
{
"level": "b1-5",
"position": 46,
"text": "staff",
"meaning": {
"fr": "personnel, équipe"
}
},
{
"level": "b1-5",
"position": 47,
"text": "standard",
"meaning": {
"fr": "norme, niveau ; (adj.) standard"
}
},
{
"level": "b1-5",
"position": 48,
"text": "stare",
"meaning": {
"fr": "fixer du regard"
}
},
{
"level": "b1-5",
"position": 49,
"text": "statement",
"meaning": {
"fr": "déclaration, relevé"
}
},
{
"level": "b1-5",
"position": 50,
"text": "statistics",
"meaning": {
"fr": "statistiques"
}
},
{
"level": "b1-5",
"position": 51,
"text": "steady",
"meaning": {
"fr": "régulier, stable, constant"
}
},
{
"level": "b1-5",
"position": 52,
"text": "steel",
"meaning": {
"fr": "acier"
}
},
{
"level": "b1-5",
"position": 53,
"text": "stick",
"meaning": {
"fr": "(n.) bâton ; (v.) coller, planter"
}
},
{
"level": "b1-5",
"position": 54,
"text": "stock",
"meaning": {
"fr": "stock, réserve ; (fin.) action"
}
},
{
"level": "b1-5",
"position": 55,
"text": "storage",
"meaning": {
"fr": "stockage, rangement"
}
},
{
"level": "b1-5",
"position": 56,
"text": "stranger",
"meaning": {
"fr": "inconnu, étranger"
}
},
{
"level": "b1-5",
"position": 57,
"text": "strategy",
"meaning": {
"fr": "stratégie"
}
},
{
"level": "b1-5",
"position": 58,
"text": "stream",
"meaning": {
"fr": "ruisseau, flux ; (v.) diffuser"
}
},
{
"level": "b1-5",
"position": 59,
"text": "strength",
"meaning": {
"fr": "force, point fort"
}
},
{
"level": "b1-5",
"position": 60,
"text": "stretch",
"meaning": {
"fr": "étirer, s'étendre ; (n.) étendue"
}
},
{
"level": "b1-5",
"position": 61,
"text": "strike",
"meaning": {
"fr": "frapper ; (n.) grève"
}
},
{
"level": "b1-5",
"position": 62,
"text": "string",
"meaning": {
"fr": "ficelle, corde, chaîne"
}
},
{
"level": "b1-5",
"position": 63,
"text": "strongly",
"meaning": {
"fr": "fortement, vivement"
}
},
{
"level": "b1-5",
"position": 64,
"text": "structure",
"meaning": {
"fr": "structure ; (v.) structurer"
}
},
{
"level": "b1-5",
"position": 65,
"text": "struggle",
"meaning": {
"fr": "lutter, se battre ; (n.) lutte"
}
},
{
"level": "b1-5",
"position": 66,
"text": "substance",
"meaning": {
"fr": "substance, fond"
}
},
{
"level": "b1-5",
"position": 67,
"text": "successfully",
"meaning": {
"fr": "avec succès"
}
},
{
"level": "b1-5",
"position": 68,
"text": "sudden",
"meaning": {
"fr": "soudain, brusque"
}
},
{
"level": "b1-5",
"position": 69,
"text": "suffer",
"meaning": {
"fr": "souffrir, subir"
}
},
{
"level": "b1-5",
"position": 70,
"text": "supply",
"meaning": {
"fr": "approvisionnement ; (v.) fournir"
}
},
{
"level": "b1-5",
"position": 71,
"text": "supporter",
"meaning": {
"fr": "partisan, supporter"
}
},
{
"level": "b1-5",
"position": 72,
"text": "supposed",
"meaning": {
"fr": "censé, supposé"
}
},
{
"level": "b1-5",
"position": 73,
"text": "surely",
"meaning": {
"fr": "sûrement, certainement"
}
},
{
"level": "b1-5",
"position": 74,
"text": "surface",
"meaning": {
"fr": "surface"
}
},
{
"level": "b1-5",
"position": 75,
"text": "surgery",
"meaning": {
"fr": "chirurgie, opération"
}
},
{
"level": "b1-5",
"position": 76,
"text": "surprisingly",
"meaning": {
"fr": "étonnamment"
}
},
{
"level": "b1-5",
"position": 77,
"text": "surround",
"meaning": {
"fr": "entourer, encercler"
}
},
{
"level": "b1-5",
"position": 78,
"text": "survey",
"meaning": {
"fr": "enquête, sondage"
}
},
{
"level": "b1-5",
"position": 79,
"text": "survive",
"meaning": {
"fr": "survivre"
}
},
{
"level": "b1-5",
"position": 80,
"text": "suspect",
"meaning": {
"fr": "soupçonner ; (n.) suspect"
}
},
{
"level": "b1-5",
"position": 81,
"text": "switch",
"meaning": {
"fr": "changer ; (n.) interrupteur"
}
},
{
"level": "b1-5",
"position": 82,
"text": "symbol",
"meaning": {
"fr": "symbole"
}
},
{
"level": "b1-5",
"position": 83,
"text": "symptom",
"meaning": {
"fr": "symptôme"
}
},
{
"level": "b1-5",
"position": 84,
"text": "tail",
"meaning": {
"fr": "queue"
}
},
{
"level": "b1-5",
"position": 85,
"text": "talent",
"meaning": {
"fr": "talent"
}
},
{
"level": "b1-5",
"position": 86,
"text": "tank",
"meaning": {
"fr": "réservoir, char"
}
},
{
"level": "b1-5",
"position": 87,
"text": "tap",
"meaning": {
"fr": "robinet ; (v.) tapoter"
}
},
{
"level": "b1-5",
"position": 88,
"text": "tape",
"meaning": {
"fr": "ruban adhésif, cassette"
}
},
{
"level": "b1-5",
"position": 89,
"text": "target",
"meaning": {
"fr": "cible, objectif ; (v.) viser"
}
},
{
"level": "b1-5",
"position": 90,
"text": "tax",
"meaning": {
"fr": "impôt, taxe"
}
},
{
"level": "b1-5",
"position": 91,
"text": "tear",
"meaning": {
"fr": "larme ; (v.) déchirer"
}
},
{
"level": "b1-5",
"position": 92,
"text": "technical",
"meaning": {
"fr": "technique (adj.)"
}
},
{
"level": "b1-5",
"position": 93,
"text": "technique",
"meaning": {
"fr": "technique, méthode"
}
},
{
"level": "b1-5",
"position": 94,
"text": "teen",
"meaning": {
"fr": "ado"
}
},
{
"level": "b1-5",
"position": 95,
"text": "temporary",
"meaning": {
"fr": "temporaire, provisoire"
}
},
{
"level": "b1-5",
"position": 96,
"text": "tend",
"meaning": {
"fr": "avoir tendance à"
}
},
{
"level": "b1-5",
"position": 97,
"text": "term",
"meaning": {
"fr": "terme, trimestre, durée"
}
},
{
"level": "b1-5",
"position": 98,
"text": "terms",
"meaning": {
"fr": "conditions, termes"
}
},
{
"level": "b1-5",
"position": 99,
"text": "theme",
"meaning": {
"fr": "thème"
}
},
{
"level": "b1-5",
"position": 100,
"text": "theory",
"meaning": {
"fr": "théorie"
}
},
{
"level": "b1-5",
"position": 101,
"text": "therefore",
"meaning": {
"fr": "donc, par conséquent"
}
},
{
"level": "b1-5",
"position": 102,
"text": "thick",
"meaning": {
"fr": "épais"
}
},
{
"level": "b1-5",
"position": 103,
"text": "thinking",
"meaning": {
"fr": "réflexion, pensée"
}
},
{
"level": "b1-5",
"position": 104,
"text": "though",
"meaning": {
"fr": "bien que, pourtant"
}
},
{
"level": "b1-5",
"position": 105,
"text": "threat",
"meaning": {
"fr": "menace"
}
},
{
"level": "b1-5",
"position": 106,
"text": "threaten",
"meaning": {
"fr": "menacer"
}
},
{
"level": "b1-5",
"position": 107,
"text": "throat",
"meaning": {
"fr": "gorge"
}
},
{
"level": "b1-5",
"position": 108,
"text": "throughout",
"meaning": {
"fr": "partout dans, tout au long de"
}
},
{
"level": "b1-5",
"position": 109,
"text": "tie",
"meaning": {
"fr": "cravate ; (v.) attacher, nouer"
}
},
{
"level": "b1-5",
"position": 110,
"text": "tight",
"meaning": {
"fr": "serré, étroit"
}
},
{
"level": "b1-5",
"position": 111,
"text": "tip",
"meaning": {
"fr": "pourboire, astuce, pointe"
}
},
{
"level": "b1-5",
"position": 112,
"text": "tire",
"meaning": {
"fr": "pneu ; (v.) fatiguer"
}
},
{
"level": "b1-5",
"position": 113,
"text": "toe",
"meaning": {
"fr": "orteil"
}
},
{
"level": "b1-5",
"position": 114,
"text": "tone",
"meaning": {
"fr": "ton"
}
},
{
"level": "b1-5",
"position": 115,
"text": "tongue",
"meaning": {
"fr": "langue (organe)"
}
},
{
"level": "b1-5",
"position": 116,
"text": "tough",
"meaning": {
"fr": "dur, difficile, coriace"
}
},
{
"level": "b1-5",
"position": 117,
"text": "toward",
"meaning": {
"fr": "vers, envers"
}
},
{
"level": "b1-5",
"position": 118,
"text": "towards",
"meaning": {
"fr": "en direction de, vers"
}
},
{
"level": "b1-5",
"position": 119,
"text": "track",
"meaning": {
"fr": "piste, voie ; (v.) suivre"
}
},
{
"level": "b1-5",
"position": 120,
"text": "trade",
"meaning": {
"fr": "commerce ; (v.) échanger"
}
},
{
"level": "b1-5",
"position": 121,
"text": "transfer",
"meaning": {
"fr": "transfert ; (v.) transférer"
}
},
{
"level": "b1-5",
"position": 122,
"text": "transportation",
"meaning": {
"fr": "transport"
}
},
{
"level": "b1-5",
"position": 123,
"text": "treat",
"meaning": {
"fr": "traiter, soigner ; (n.) gâterie"
}
},
{
"level": "b1-5",
"position": 124,
"text": "treatment",
"meaning": {
"fr": "traitement"
}
},
{
"level": "b1-5",
"position": 125,
"text": "trend",
"meaning": {
"fr": "tendance, mode"
}
},
{
"level": "b1-5",
"position": 126,
"text": "trial",
"meaning": {
"fr": "procès, essai"
}
},
{
"level": "b1-5",
"position": 127,
"text": "trick",
"meaning": {
"fr": "tour, astuce ; (v.) duper"
}
},
{
"level": "b1-5",
"position": 128,
"text": "truly",
"meaning": {
"fr": "vraiment, sincèrement"
}
},
{
"level": "b1-5",
"position": 129,
"text": "trust",
"meaning": {
"fr": "confiance ; (v.) faire confiance"
}
},
{
"level": "b1-5",
"position": 130,
"text": "tube",
"meaning": {
"fr": "tube, métro (Londres)"
}
},
{
"level": "b1-5",
"position": 131,
"text": "tunnel",
"meaning": {
"fr": "tunnel"
}
},
{
"level": "b1-5",
"position": 132,
"text": "typically",
"meaning": {
"fr": "généralement, typiquement"
}
},
{
"level": "b1-5",
"position": 133,
"text": "unable",
"meaning": {
"fr": "incapable"
}
},
{
"level": "b1-5",
"position": 134,
"text": "understanding",
"meaning": {
"fr": "compréhension, entente"
}
},
{
"level": "b1-5",
"position": 135,
"text": "uniform",
"meaning": {
"fr": "uniforme"
}
},
{
"level": "b1-5",
"position": 136,
"text": "union",
"meaning": {
"fr": "syndicat, union"
}
},
{
"level": "b1-5",
"position": 137,
"text": "unique",
"meaning": {
"fr": "unique"
}
},
{
"level": "b1-5",
"position": 138,
"text": "universe",
"meaning": {
"fr": "univers"
}
},
{
"level": "b1-5",
"position": 139,
"text": "unknown",
"meaning": {
"fr": "inconnu"
}
},
{
"level": "b1-5",
"position": 140,
"text": "unless",
"meaning": {
"fr": "à moins que, sauf si"
}
},
{
"level": "b1-5",
"position": 141,
"text": "unlike",
"meaning": {
"fr": "contrairement à"
}
},
{
"level": "b1-5",
"position": 142,
"text": "unlikely",
"meaning": {
"fr": "peu probable, improbable"
}
},
{
"level": "b1-5",
"position": 143,
"text": "upon",
"meaning": {
"fr": "sur, dès"
}
},
{
"level": "b1-5",
"position": 144,
"text": "user",
"meaning": {
"fr": "utilisateur"
}
},
{
"level": "b1-5",
"position": 145,
"text": "valuable",
"meaning": {
"fr": "précieux, de valeur"
}
},
{
"level": "b1-5",
"position": 146,
"text": "value",
"meaning": {
"fr": "valeur ; (v.) apprécier"
}
},
{
"level": "b1-5",
"position": 147,
"text": "variety",
"meaning": {
"fr": "variété, diversité"
}
},
{
"level": "b1-5",
"position": 148,
"text": "various",
"meaning": {
"fr": "divers, différents"
}
},
{
"level": "b1-5",
"position": 149,
"text": "vary",
"meaning": {
"fr": "varier"
}
},
{
"level": "b1-5",
"position": 150,
"text": "vehicle",
"meaning": {
"fr": "véhicule"
}
},
{
"level": "b1-5",
"position": 151,
"text": "version",
"meaning": {
"fr": "version"
}
},
{
"level": "b1-5",
"position": 152,
"text": "victim",
"meaning": {
"fr": "victime"
}
},
{
"level": "b1-5",
"position": 153,
"text": "victory",
"meaning": {
"fr": "victoire"
}
},
{
"level": "b1-5",
"position": 154,
"text": "viewer",
"meaning": {
"fr": "téléspectateur, spectateur"
}
},
{
"level": "b1-5",
"position": 155,
"text": "violence",
"meaning": {
"fr": "violence"
}
},
{
"level": "b1-5",
"position": 156,
"text": "violent",
"meaning": {
"fr": "violent"
}
},
{
"level": "b1-5",
"position": 157,
"text": "virus",
"meaning": {
"fr": "virus"
}
},
{
"level": "b1-5",
"position": 158,
"text": "visible",
"meaning": {
"fr": "visible"
}
},
{
"level": "b1-5",
"position": 159,
"text": "volume",
"meaning": {
"fr": "volume"
}
},
{
"level": "b1-5",
"position": 160,
"text": "volunteer",
"meaning": {
"fr": "bénévole ; (v.) se porter volontaire"
}
},
{
"level": "b1-5",
"position": 161,
"text": "vote",
"meaning": {
"fr": "vote ; (v.) voter"
}
},
{
"level": "b1-5",
"position": 162,
"text": "warn",
"meaning": {
"fr": "avertir, prévenir"
}
},
{
"level": "b1-5",
"position": 163,
"text": "warning",
"meaning": {
"fr": "avertissement"
}
},
{
"level": "b1-5",
"position": 164,
"text": "wave",
"meaning": {
"fr": "vague ; (v.) faire signe"
}
},
{
"level": "b1-5",
"position": 165,
"text": "wealth",
"meaning": {
"fr": "richesse, fortune"
}
},
{
"level": "b1-5",
"position": 166,
"text": "wealthy",
"meaning": {
"fr": "riche, aisé"
}
},
{
"level": "b1-5",
"position": 167,
"text": "weapon",
"meaning": {
"fr": "arme"
}
},
{
"level": "b1-5",
"position": 168,
"text": "weekly",
"meaning": {
"fr": "hebdomadaire"
}
},
{
"level": "b1-5",
"position": 169,
"text": "weigh",
"meaning": {
"fr": "peser"
}
},
{
"level": "b1-5",
"position": 170,
"text": "whatever",
"meaning": {
"fr": "quoi que, n'importe quoi"
}
},
{
"level": "b1-5",
"position": 171,
"text": "whenever",
"meaning": {
"fr": "chaque fois que, n'importe quand"
}
},
{
"level": "b1-5",
"position": 172,
"text": "whether",
"meaning": {
"fr": "si (alternative)"
}
},
{
"level": "b1-5",
"position": 173,
"text": "whisper",
"meaning": {
"fr": "chuchoter"
}
},
{
"level": "b1-5",
"position": 174,
"text": "whom",
"meaning": {
"fr": "qui (complément), que"
}
},
{
"level": "b1-5",
"position": 175,
"text": "whose",
"meaning": {
"fr": "dont, à qui"
}
},
{
"level": "b1-5",
"position": 176,
"text": "widely",
"meaning": {
"fr": "largement, couramment"
}
},
{
"level": "b1-5",
"position": 177,
"text": "willing",
"meaning": {
"fr": "disposé, prêt à"
}
},
{
"level": "b1-5",
"position": 178,
"text": "wing",
"meaning": {
"fr": "aile"
}
},
{
"level": "b1-5",
"position": 179,
"text": "wise",
"meaning": {
"fr": "sage, avisé"
}
},
{
"level": "b1-5",
"position": 180,
"text": "within",
"meaning": {
"fr": "à l'intérieur de, d'ici"
}
},
{
"level": "b1-5",
"position": 181,
"text": "witness",
"meaning": {
"fr": "témoin ; (v.) être témoin de"
}
},
{
"level": "b1-5",
"position": 182,
"text": "workshop",
"meaning": {
"fr": "atelier"
}
},
{
"level": "b1-5",
"position": 183,
"text": "worth",
"meaning": {
"fr": "qui vaut, valeur"
}
},
{
"level": "b1-5",
"position": 184,
"text": "wound",
"meaning": {
"fr": "blessure ; (v.) blesser"
}
},
{
"level": "b1-5",
"position": 185,
"text": "wrap",
"meaning": {
"fr": "envelopper, emballer"
}
},
{
"level": "b1-5",
"position": 186,
"text": "yard",
"meaning": {
"fr": "cour, jardin, yard (mesure)"
}
},
{
"level": "b1-5",
"position": 187,
"text": "yell",
"meaning": {
"fr": "hurler, crier"
}
},
{
"level": "b1-5",
"position": 188,
"text": "youth",
"meaning": {
"fr": "jeunesse, jeune"
}
},
{
"level": "b1-5",
"position": 189,
"text": "zone",
"meaning": {
"fr": "zone"
}
},
{
"level": "b2-1",
"position": 1,
"text": "abandon",
"meaning": {
"fr": "abandonner"
}
},
{
"level": "b2-1",
"position": 2,
"text": "abortion",
"meaning": {
"fr": "avortement"
}
},
{
"level": "b2-1",
"position": 3,
"text": "absence",
"meaning": {
"fr": "absence"
}
},
{
"level": "b2-1",
"position": 4,
"text": "absolute",
"meaning": {
"fr": "absolu"
}
},
{
"level": "b2-1",
"position": 5,
"text": "absorb",
"meaning": {
"fr": "absorber"
}
},
{
"level": "b2-1",
"position": 6,
"text": "abuse",
"meaning": {
"fr": "abus, maltraitance"
}
},
{
"level": "b2-1",
"position": 7,
"text": "accompany",
"meaning": {
"fr": "accompagner"
}
},
{
"level": "b2-1",
"position": 8,
"text": "accomplish",
"meaning": {
"fr": "accomplir, réaliser"
}
},
{
"level": "b2-1",
"position": 9,
"text": "accurate",
"meaning": {
"fr": "précis, exact"
}
},
{
"level": "b2-1",
"position": 10,
"text": "accuse",
"meaning": {
"fr": "accuser"
}
},
{
"level": "b2-1",
"position": 11,
"text": "acid",
"meaning": {
"fr": "acide"
}
},
{
"level": "b2-1",
"position": 12,
"text": "acknowledge",
"meaning": {
"fr": "reconnaître, admettre"
}
},
{
"level": "b2-1",
"position": 13,
"text": "acquire",
"meaning": {
"fr": "acquérir"
}
},
{
"level": "b2-1",
"position": 14,
"text": "activist",
"meaning": {
"fr": "militant"
}
},
{
"level": "b2-1",
"position": 15,
"text": "adapt",
"meaning": {
"fr": "adapter, s'adapter"
}
},
{
"level": "b2-1",
"position": 16,
"text": "adequate",
"meaning": {
"fr": "suffisant, adéquat"
}
},
{
"level": "b2-1",
"position": 17,
"text": "adjust",
"meaning": {
"fr": "ajuster, régler"
}
},
{
"level": "b2-1",
"position": 18,
"text": "adjustment",
"meaning": {
"fr": "ajustement, réglage"
}
},
{
"level": "b2-1",
"position": 19,
"text": "administration",
"meaning": {
"fr": "administration, gouvernement"
}
},
{
"level": "b2-1",
"position": 20,
"text": "administrator",
"meaning": {
"fr": "administrateur"
}
},
{
"level": "b2-1",
"position": 21,
"text": "admission",
"meaning": {
"fr": "admission, entrée, aveu"
}
},
{
"level": "b2-1",
"position": 22,
"text": "adolescent",
"meaning": {
"fr": "adolescent"
}
},
{
"level": "b2-1",
"position": 23,
"text": "adopt",
"meaning": {
"fr": "adopter"
}
},
{
"level": "b2-1",
"position": 24,
"text": "adviser",
"meaning": {
"fr": "conseiller, consultant"
}
},
{
"level": "b2-1",
"position": 25,
"text": "advocate",
"meaning": {
"fr": "(n.) défenseur ; (v.) prôner"
}
},
{
"level": "b2-1",
"position": 26,
"text": "affair",
"meaning": {
"fr": "affaire, liaison"
}
},
{
"level": "b2-1",
"position": 27,
"text": "agenda",
"meaning": {
"fr": "ordre du jour, programme"
}
},
{
"level": "b2-1",
"position": 28,
"text": "agricultural",
"meaning": {
"fr": "agricole"
}
},
{
"level": "b2-1",
"position": 29,
"text": "aid",
"meaning": {
"fr": "(n.) aide ; (v.) aider"
}
},
{
"level": "b2-1",
"position": 30,
"text": "aide",
"meaning": {
"fr": "collaborateur, conseiller (politique)"
}
},
{
"level": "b2-1",
"position": 31,
"text": "AIDS",
"meaning": {
"fr": "sida"
}
},
{
"level": "b2-1",
"position": 32,
"text": "alliance",
"meaning": {
"fr": "alliance"
}
},
{
"level": "b2-1",
"position": 33,
"text": "ally",
"meaning": {
"fr": "allié"
}
},
{
"level": "b2-1",
"position": 34,
"text": "alter",
"meaning": {
"fr": "modifier, altérer"
}
},
{
"level": "b2-1",
"position": 35,
"text": "analyst",
"meaning": {
"fr": "analyste"
}
},
{
"level": "b2-1",
"position": 36,
"text": "angle",
"meaning": {
"fr": "angle"
}
},
{
"level": "b2-1",
"position": 37,
"text": "annual",
"meaning": {
"fr": "annuel"
}
},
{
"level": "b2-1",
"position": 38,
"text": "anticipate",
"meaning": {
"fr": "anticiper, prévoir"
}
},
{
"level": "b2-1",
"position": 39,
"text": "anxiety",
"meaning": {
"fr": "anxiété, inquiétude"
}
},
{
"level": "b2-1",
"position": 40,
"text": "apparent",
"meaning": {
"fr": "apparent, évident"
}
},
{
"level": "b2-1",
"position": 41,
"text": "appeal",
"meaning": {
"fr": "(n.) appel, attrait ; (v.) plaire"
}
},
{
"level": "b2-1",
"position": 42,
"text": "appoint",
"meaning": {
"fr": "nommer, désigner"
}
},
{
"level": "b2-1",
"position": 43,
"text": "approval",
"meaning": {
"fr": "approbation, aval"
}
},
{
"level": "b2-1",
"position": 44,
"text": "approve",
"meaning": {
"fr": "approuver"
}
},
{
"level": "b2-1",
"position": 45,
"text": "arise",
"meaning": {
"fr": "survenir, se présenter"
}
},
{
"level": "b2-1",
"position": 46,
"text": "armed",
"meaning": {
"fr": "armé"
}
},
{
"level": "b2-1",
"position": 47,
"text": "aside",
"meaning": {
"fr": "de côté"
}
},
{
"level": "b2-1",
"position": 48,
"text": "assault",
"meaning": {
"fr": "agression, assaut"
}
},
{
"level": "b2-1",
"position": 49,
"text": "assert",
"meaning": {
"fr": "affirmer, revendiquer"
}
},
{
"level": "b2-1",
"position": 50,
"text": "assess",
"meaning": {
"fr": "évaluer"
}
},
{
"level": "b2-1",
"position": 51,
"text": "assessment",
"meaning": {
"fr": "évaluation"
}
},
{
"level": "b2-1",
"position": 52,
"text": "asset",
"meaning": {
"fr": "atout, bien, actif"
}
},
{
"level": "b2-1",
"position": 53,
"text": "assign",
"meaning": {
"fr": "attribuer, affecter"
}
},
{
"level": "b2-1",
"position": 54,
"text": "assignment",
"meaning": {
"fr": "mission, devoir, tâche"
}
},
{
"level": "b2-1",
"position": 55,
"text": "assist",
"meaning": {
"fr": "aider, assister"
}
},
{
"level": "b2-1",
"position": 56,
"text": "assistance",
"meaning": {
"fr": "aide, assistance"
}
},
{
"level": "b2-1",
"position": 57,
"text": "associate",
"meaning": {
"fr": "(v.) associer ; (n.) associé"
}
},
{
"level": "b2-1",
"position": 58,
"text": "association",
"meaning": {
"fr": "association"
}
},
{
"level": "b2-1",
"position": 59,
"text": "assumption",
"meaning": {
"fr": "hypothèse, supposition"
}
},
{
"level": "b2-1",
"position": 60,
"text": "assure",
"meaning": {
"fr": "assurer, garantir"
}
},
{
"level": "b2-1",
"position": 61,
"text": "athletic",
"meaning": {
"fr": "athlétique, sportif (adj.)"
}
},
{
"level": "b2-1",
"position": 62,
"text": "attorney",
"meaning": {
"fr": "avocat (US)"
}
},
{
"level": "b2-1",
"position": 63,
"text": "attribute",
"meaning": {
"fr": "(v.) attribuer ; (n.) attribut"
}
},
{
"level": "b2-1",
"position": 64,
"text": "auto",
"meaning": {
"fr": "auto, automobile"
}
},
{
"level": "b2-1",
"position": 65,
"text": "awareness",
"meaning": {
"fr": "conscience, sensibilisation"
}
},
{
"level": "b2-1",
"position": 66,
"text": "barely",
"meaning": {
"fr": "à peine"
}
},
{
"level": "b2-1",
"position": 67,
"text": "barrel",
"meaning": {
"fr": "tonneau, baril"
}
},
{
"level": "b2-1",
"position": 68,
"text": "barrier",
"meaning": {
"fr": "barrière, obstacle"
}
},
{
"level": "b2-1",
"position": 69,
"text": "beneath",
"meaning": {
"fr": "sous, au-dessous de"
}
},
{
"level": "b2-1",
"position": 70,
"text": "bind",
"meaning": {
"fr": "lier, relier"
}
},
{
"level": "b2-1",
"position": 71,
"text": "biological",
"meaning": {
"fr": "biologique"
}
},
{
"level": "b2-1",
"position": 72,
"text": "blade",
"meaning": {
"fr": "lame"
}
},
{
"level": "b2-1",
"position": 73,
"text": "bombing",
"meaning": {
"fr": "bombardement, attentat"
}
},
{
"level": "b2-1",
"position": 74,
"text": "bond",
"meaning": {
"fr": "lien, obligation (finance)"
}
},
{
"level": "b2-1",
"position": 75,
"text": "boom",
"meaning": {
"fr": "essor, boom, détonation"
}
},
{
"level": "b2-1",
"position": 76,
"text": "boundary",
"meaning": {
"fr": "limite"
}
},
{
"level": "b2-1",
"position": 77,
"text": "breast",
"meaning": {
"fr": "sein"
}
},
{
"level": "b2-1",
"position": 78,
"text": "broad",
"meaning": {
"fr": "large, vaste"
}
},
{
"level": "b2-1",
"position": 79,
"text": "buck",
"meaning": {
"fr": "dollar (fam.), mâle (cerf)"
}
},
{
"level": "b2-1",
"position": 80,
"text": "bullet",
"meaning": {
"fr": "balle (d'arme)"
}
},
{
"level": "b2-1",
"position": 81,
"text": "bunch",
"meaning": {
"fr": "bouquet, tas, groupe"
}
},
{
"level": "b2-1",
"position": 82,
"text": "burden",
"meaning": {
"fr": "fardeau, charge pesante"
}
},
{
"level": "b2-1",
"position": 83,
"text": "cabinet",
"meaning": {
"fr": "armoire, cabinet (gouv.)"
}
},
{
"level": "b2-1",
"position": 84,
"text": "capability",
"meaning": {
"fr": "aptitude, capacité technique"
}
},
{
"level": "b2-1",
"position": 85,
"text": "capacity",
"meaning": {
"fr": "contenance, capacité (volume)"
}
},
{
"level": "b2-1",
"position": 86,
"text": "carrier",
"meaning": {
"fr": "transporteur, porteur"
}
},
{
"level": "b2-1",
"position": 87,
"text": "CEO",
"meaning": {
"fr": "PDG, directeur général"
}
},
{
"level": "b2-1",
"position": 88,
"text": "chairman",
"meaning": {
"fr": "président (d'un conseil)"
}
},
{
"level": "b2-1",
"position": 89,
"text": "chamber",
"meaning": {
"fr": "chambre, salle"
}
},
{
"level": "b2-1",
"position": 90,
"text": "characterize",
"meaning": {
"fr": "caractériser"
}
},
{
"level": "b2-1",
"position": 91,
"text": "cholesterol",
"meaning": {
"fr": "cholestérol"
}
},
{
"level": "b2-1",
"position": 92,
"text": "circumstance",
"meaning": {
"fr": "circonstance"
}
},
{
"level": "b2-1",
"position": 93,
"text": "cite",
"meaning": {
"fr": "citer"
}
},
{
"level": "b2-1",
"position": 94,
"text": "civilian",
"meaning": {
"fr": "civil (non militaire)"
}
},
{
"level": "b2-1",
"position": 95,
"text": "clinical",
"meaning": {
"fr": "clinique (adj.), médical"
}
},
{
"level": "b2-1",
"position": 96,
"text": "cluster",
"meaning": {
"fr": "groupe, grappe"
}
},
{
"level": "b2-1",
"position": 97,
"text": "coalition",
"meaning": {
"fr": "coalition"
}
},
{
"level": "b2-1",
"position": 98,
"text": "cognitive",
"meaning": {
"fr": "cognitif"
}
},
{
"level": "b2-1",
"position": 99,
"text": "collapse",
"meaning": {
"fr": "(v.) s'effondrer ; (n.) effondrement"
}
},
{
"level": "b2-1",
"position": 100,
"text": "collective",
"meaning": {
"fr": "collectif"
}
},
{
"level": "b2-1",
"position": 101,
"text": "colonial",
"meaning": {
"fr": "colonial"
}
},
{
"level": "b2-1",
"position": 102,
"text": "command",
"meaning": {
"fr": "ordre, commandement ; (v.) ordonner"
}
},
{
"level": "b2-1",
"position": 103,
"text": "commander",
"meaning": {
"fr": "commandant"
}
},
{
"level": "b2-1",
"position": 104,
"text": "commission",
"meaning": {
"fr": "commission ; (v.) commander (une œuvre)"
}
},
{
"level": "b2-1",
"position": 105,
"text": "commit",
"meaning": {
"fr": "commettre, s'engager"
}
},
{
"level": "b2-1",
"position": 106,
"text": "commitment",
"meaning": {
"fr": "engagement"
}
},
{
"level": "b2-1",
"position": 107,
"text": "component",
"meaning": {
"fr": "composant, élément constitutif"
}
},
{
"level": "b2-1",
"position": 108,
"text": "compose",
"meaning": {
"fr": "composer"
}
},
{
"level": "b2-1",
"position": 109,
"text": "composition",
"meaning": {
"fr": "composition"
}
},
{
"level": "b2-1",
"position": 110,
"text": "comprehensive",
"meaning": {
"fr": "complet, exhaustif, détaillé"
}
},
{
"level": "b2-1",
"position": 111,
"text": "conclude",
"meaning": {
"fr": "conclure, terminer"
}
},
{
"level": "b2-1",
"position": 112,
"text": "concrete",
"meaning": {
"fr": "béton ; (adj.) concret"
}
},
{
"level": "b2-1",
"position": 113,
"text": "conduct",
"meaning": {
"fr": "mener, diriger ; (n.) conduite"
}
},
{
"level": "b2-1",
"position": 114,
"text": "confront",
"meaning": {
"fr": "affronter, faire face à"
}
},
{
"level": "b2-1",
"position": 115,
"text": "Congress",
"meaning": {
"fr": "Congrès (américain)"
}
},
{
"level": "b2-1",
"position": 116,
"text": "congressional",
"meaning": {
"fr": "du Congrès"
}
},
{
"level": "b2-1",
"position": 117,
"text": "consciousness",
"meaning": {
"fr": "conscience"
}
},
{
"level": "b2-1",
"position": 118,
"text": "consensus",
"meaning": {
"fr": "consensus"
}
},
{
"level": "b2-1",
"position": 119,
"text": "considerable",
"meaning": {
"fr": "considérable"
}
},
{
"level": "b2-1",
"position": 120,
"text": "consideration",
"meaning": {
"fr": "réflexion, considération"
}
},
{
"level": "b2-1",
"position": 121,
"text": "consistent",
"meaning": {
"fr": "cohérent, régulier"
}
},
{
"level": "b2-1",
"position": 122,
"text": "constitute",
"meaning": {
"fr": "constituer"
}
},
{
"level": "b2-1",
"position": 123,
"text": "constitutional",
"meaning": {
"fr": "constitutionnel"
}
},
{
"level": "b2-1",
"position": 124,
"text": "construct",
"meaning": {
"fr": "construire, bâtir"
}
},
{
"level": "b2-1",
"position": 125,
"text": "consultant",
"meaning": {
"fr": "consultant, conseiller"
}
},
{
"level": "b2-1",
"position": 126,
"text": "consume",
"meaning": {
"fr": "consommer"
}
},
{
"level": "b2-1",
"position": 127,
"text": "consumption",
"meaning": {
"fr": "consommation"
}
},
{
"level": "b2-1",
"position": 128,
"text": "contemporary",
"meaning": {
"fr": "contemporain"
}
},
{
"level": "b2-1",
"position": 129,
"text": "continued",
"meaning": {
"fr": "continu, prolongé"
}
},
{
"level": "b2-1",
"position": 130,
"text": "contribute",
"meaning": {
"fr": "contribuer, participer"
}
},
{
"level": "b2-1",
"position": 131,
"text": "contribution",
"meaning": {
"fr": "contribution, apport"
}
},
{
"level": "b2-1",
"position": 132,
"text": "controversial",
"meaning": {
"fr": "controversé"
}
},
{
"level": "b2-1",
"position": 133,
"text": "controversy",
"meaning": {
"fr": "controverse, polémique"
}
},
{
"level": "b2-1",
"position": 134,
"text": "convention",
"meaning": {
"fr": "convention, congrès, usage"
}
},
{
"level": "b2-1",
"position": 135,
"text": "conventional",
"meaning": {
"fr": "conventionnel, classique"
}
},
{
"level": "b2-1",
"position": 136,
"text": "convert",
"meaning": {
"fr": "convertir, transformer"
}
},
{
"level": "b2-1",
"position": 137,
"text": "conviction",
"meaning": {
"fr": "condamnation, conviction"
}
},
{
"level": "b2-1",
"position": 138,
"text": "cooperation",
"meaning": {
"fr": "coopération"
}
},
{
"level": "b2-1",
"position": 139,
"text": "cop",
"meaning": {
"fr": "flic, policier"
}
},
{
"level": "b2-1",
"position": 140,
"text": "cope",
"meaning": {
"fr": "faire face, se débrouiller"
}
},
{
"level": "b2-1",
"position": 141,
"text": "core",
"meaning": {
"fr": "noyau, cœur ; (adj.) central"
}
},
{
"level": "b2-1",
"position": 142,
"text": "corporate",
"meaning": {
"fr": "d'entreprise, corporatif"
}
},
{
"level": "b2-1",
"position": 143,
"text": "corporation",
"meaning": {
"fr": "grande entreprise, société"
}
},
{
"level": "b2-1",
"position": 144,
"text": "correspondent",
"meaning": {
"fr": "correspondant, envoyé spécial"
}
},
{
"level": "b2-1",
"position": 145,
"text": "counselor",
"meaning": {
"fr": "conseiller, psychologue"
}
},
{
"level": "b2-1",
"position": 146,
"text": "counter",
"meaning": {
"fr": "comptoir ; (v.) contrer"
}
},
{
"level": "b2-1",
"position": 147,
"text": "county",
"meaning": {
"fr": "comté"
}
},
{
"level": "b2-1",
"position": 148,
"text": "coverage",
"meaning": {
"fr": "couverture (médiatique)"
}
},
{
"level": "b2-1",
"position": 149,
"text": "crack",
"meaning": {
"fr": "fissure ; (v.) fissurer, craquer"
}
},
{
"level": "b2-1",
"position": 150,
"text": "craft",
"meaning": {
"fr": "artisanat, métier"
}
},
{
"level": "b2-1",
"position": 151,
"text": "criteria",
"meaning": {
"fr": "critères"
}
},
{
"level": "b2-1",
"position": 152,
"text": "crop",
"meaning": {
"fr": "culture, récolte"
}
},
{
"level": "b2-1",
"position": 153,
"text": "crucial",
"meaning": {
"fr": "crucial, décisif"
}
},
{
"level": "b2-1",
"position": 154,
"text": "curriculum",
"meaning": {
"fr": "programme scolaire"
}
},
{
"level": "b2-1",
"position": 155,
"text": "custom",
"meaning": {
"fr": "coutume ; (adj.) sur mesure"
}
},
{
"level": "b2-1",
"position": 156,
"text": "dare",
"meaning": {
"fr": "oser"
}
},
{
"level": "b2-1",
"position": 157,
"text": "dealer",
"meaning": {
"fr": "marchand, revendeur, dealer"
}
},
{
"level": "b2-1",
"position": 158,
"text": "deck",
"meaning": {
"fr": "pont (de navire), terrasse"
}
},
{
"level": "b2-1",
"position": 159,
"text": "declare",
"meaning": {
"fr": "déclarer"
}
},
{
"level": "b2-1",
"position": 160,
"text": "decline",
"meaning": {
"fr": "déclin, baisse ; (v.) décliner"
}
},
{
"level": "b2-1",
"position": 161,
"text": "defendant",
"meaning": {
"fr": "accusé, prévenu"
}
},
{
"level": "b2-1",
"position": 162,
"text": "defensive",
"meaning": {
"fr": "défensif, sur la défensive"
}
},
{
"level": "b2-1",
"position": 163,
"text": "deficit",
"meaning": {
"fr": "déficit"
}
},
{
"level": "b2-1",
"position": 164,
"text": "Democrat",
"meaning": {
"fr": "démocrate (parti américain)"
}
},
{
"level": "b2-1",
"position": 165,
"text": "depict",
"meaning": {
"fr": "représenter, dépeindre"
}
},
{
"level": "b2-1",
"position": 166,
"text": "depression",
"meaning": {
"fr": "dépression"
}
},
{
"level": "b2-1",
"position": 167,
"text": "deputy",
"meaning": {
"fr": "adjoint, suppléant"
}
},
{
"level": "b2-1",
"position": 168,
"text": "derive",
"meaning": {
"fr": "dériver, tirer (de)"
}
},
{
"level": "b2-1",
"position": 169,
"text": "desire",
"meaning": {
"fr": "désir, envie ; (v.) désirer"
}
},
{
"level": "b2-1",
"position": 170,
"text": "detect",
"meaning": {
"fr": "détecter"
}
},
{
"level": "b2-1",
"position": 171,
"text": "developing",
"meaning": {
"fr": "en développement"
}
},
{
"level": "b2-1",
"position": 172,
"text": "devote",
"meaning": {
"fr": "consacrer"
}
},
{
"level": "b2-1",
"position": 173,
"text": "differ",
"meaning": {
"fr": "différer"
}
},
{
"level": "b2-1",
"position": 174,
"text": "dimension",
"meaning": {
"fr": "dimension"
}
},
{
"level": "b2-1",
"position": 175,
"text": "dirt",
"meaning": {
"fr": "saleté, terre"
}
},
{
"level": "b2-1",
"position": 176,
"text": "disability",
"meaning": {
"fr": "handicap, invalidité"
}
},
{
"level": "b2-1",
"position": 177,
"text": "discipline",
"meaning": {
"fr": "discipline"
}
},
{
"level": "b2-2",
"position": 1,
"text": "discourse",
"meaning": {
"fr": "discours (analyse), propos"
}
},
{
"level": "b2-2",
"position": 2,
"text": "discrimination",
"meaning": {
"fr": "discrimination"
}
},
{
"level": "b2-2",
"position": 3,
"text": "dismiss",
"meaning": {
"fr": "congédier, rejeter"
}
},
{
"level": "b2-2",
"position": 4,
"text": "disorder",
"meaning": {
"fr": "trouble, désordre"
}
},
{
"level": "b2-2",
"position": 5,
"text": "dispute",
"meaning": {
"fr": "conflit, différend, litige"
}
},
{
"level": "b2-2",
"position": 6,
"text": "distant",
"meaning": {
"fr": "lointain, éloigné"
}
},
{
"level": "b2-2",
"position": 7,
"text": "distinct",
"meaning": {
"fr": "distinct, net"
}
},
{
"level": "b2-2",
"position": 8,
"text": "distinction",
"meaning": {
"fr": "distinction"
}
},
{
"level": "b2-2",
"position": 9,
"text": "distinguish",
"meaning": {
"fr": "distinguer"
}
},
{
"level": "b2-2",
"position": 10,
"text": "distribute",
"meaning": {
"fr": "distribuer, répartir"
}
},
{
"level": "b2-2",
"position": 11,
"text": "distribution",
"meaning": {
"fr": "distribution, répartition"
}
},
{
"level": "b2-2",
"position": 12,
"text": "diverse",
"meaning": {
"fr": "divers, varié"
}
},
{
"level": "b2-2",
"position": 13,
"text": "diversity",
"meaning": {
"fr": "diversité"
}
},
{
"level": "b2-2",
"position": 14,
"text": "DNA",
"meaning": {
"fr": "ADN"
}
},
{
"level": "b2-2",
"position": 15,
"text": "domestic",
"meaning": {
"fr": "domestique, intérieur, national"
}
},
{
"level": "b2-2",
"position": 16,
"text": "dominant",
"meaning": {
"fr": "dominant"
}
},
{
"level": "b2-2",
"position": 17,
"text": "dominate",
"meaning": {
"fr": "dominer"
}
},
{
"level": "b2-2",
"position": 18,
"text": "draft",
"meaning": {
"fr": "brouillon, ébauche ; (v.) rédiger"
}
},
{
"level": "b2-2",
"position": 19,
"text": "drag",
"meaning": {
"fr": "traîner, faire glisser"
}
},
{
"level": "b2-2",
"position": 20,
"text": "eager",
"meaning": {
"fr": "désireux, impatient"
}
},
{
"level": "b2-2",
"position": 21,
"text": "earnings",
"meaning": {
"fr": "revenus, gains"
}
},
{
"level": "b2-2",
"position": 22,
"text": "ease",
"meaning": {
"fr": "facilité, aisance ; (v.) apaiser"
}
},
{
"level": "b2-2",
"position": 23,
"text": "economics",
"meaning": {
"fr": "sciences économiques"
}
},
{
"level": "b2-2",
"position": 24,
"text": "economist",
"meaning": {
"fr": "économiste"
}
},
{
"level": "b2-2",
"position": 25,
"text": "edition",
"meaning": {
"fr": "édition"
}
},
{
"level": "b2-2",
"position": 26,
"text": "educator",
"meaning": {
"fr": "éducateur, enseignant"
}
},
{
"level": "b2-2",
"position": 27,
"text": "efficiency",
"meaning": {
"fr": "efficacité, rendement"
}
},
{
"level": "b2-2",
"position": 28,
"text": "elementary",
"meaning": {
"fr": "élémentaire, primaire"
}
},
{
"level": "b2-2",
"position": 29,
"text": "eliminate",
"meaning": {
"fr": "éliminer"
}
},
{
"level": "b2-2",
"position": 30,
"text": "elite",
"meaning": {
"fr": "élite"
}
},
{
"level": "b2-2",
"position": 31,
"text": "embrace",
"meaning": {
"fr": "étreindre, adopter (une idée)"
}
},
{
"level": "b2-2",
"position": 32,
"text": "emerge",
"meaning": {
"fr": "émerger, apparaître"
}
},
{
"level": "b2-2",
"position": 33,
"text": "emission",
"meaning": {
"fr": "émission (de gaz)"
}
},
{
"level": "b2-2",
"position": 34,
"text": "emphasis",
"meaning": {
"fr": "accent, importance"
}
},
{
"level": "b2-2",
"position": 35,
"text": "emphasize",
"meaning": {
"fr": "souligner, insister sur"
}
},
{
"level": "b2-2",
"position": 36,
"text": "enable",
"meaning": {
"fr": "permettre"
}
},
{
"level": "b2-2",
"position": 37,
"text": "encounter",
"meaning": {
"fr": "rencontre ; (v.) rencontrer"
}
},
{
"level": "b2-2",
"position": 38,
"text": "enforcement",
"meaning": {
"fr": "application (de la loi)"
}
},
{
"level": "b2-2",
"position": 39,
"text": "engage",
"meaning": {
"fr": "engager, impliquer"
}
},
{
"level": "b2-2",
"position": 40,
"text": "enhance",
"meaning": {
"fr": "améliorer, renforcer"
}
},
{
"level": "b2-2",
"position": 41,
"text": "ensure",
"meaning": {
"fr": "assurer, garantir"
}
},
{
"level": "b2-2",
"position": 42,
"text": "enterprise",
"meaning": {
"fr": "entreprise, initiative"
}
},
{
"level": "b2-2",
"position": 43,
"text": "era",
"meaning": {
"fr": "ère, époque"
}
},
{
"level": "b2-2",
"position": 44,
"text": "essentially",
"meaning": {
"fr": "essentiellement, au fond"
}
},
{
"level": "b2-2",
"position": 45,
"text": "establishment",
"meaning": {
"fr": "établissement"
}
},
{
"level": "b2-2",
"position": 46,
"text": "estate",
"meaning": {
"fr": "domaine, propriété, biens"
}
},
{
"level": "b2-2",
"position": 47,
"text": "ethics",
"meaning": {
"fr": "éthique, morale"
}
},
{
"level": "b2-2",
"position": 48,
"text": "ethnic",
"meaning": {
"fr": "ethnique"
}
},
{
"level": "b2-2",
"position": 49,
"text": "evaluate",
"meaning": {
"fr": "évaluer"
}
},
{
"level": "b2-2",
"position": 50,
"text": "evaluation",
"meaning": {
"fr": "évaluation"
}
},
{
"level": "b2-2",
"position": 51,
"text": "evolution",
"meaning": {
"fr": "évolution"
}
},
{
"level": "b2-2",
"position": 52,
"text": "evolve",
"meaning": {
"fr": "évoluer"
}
},
{
"level": "b2-2",
"position": 53,
"text": "exceed",
"meaning": {
"fr": "dépasser, excéder"
}
},
{
"level": "b2-2",
"position": 54,
"text": "executive",
"meaning": {
"fr": "cadre, dirigeant ; (adj.) exécutif"
}
},
{
"level": "b2-2",
"position": 55,
"text": "exhibit",
"meaning": {
"fr": "exposer ; (n.) pièce exposée"
}
},
{
"level": "b2-2",
"position": 56,
"text": "existence",
"meaning": {
"fr": "existence"
}
},
{
"level": "b2-2",
"position": 57,
"text": "existing",
"meaning": {
"fr": "existant, actuel"
}
},
{
"level": "b2-2",
"position": 58,
"text": "expand",
"meaning": {
"fr": "développer, étendre, agrandir"
}
},
{
"level": "b2-2",
"position": 59,
"text": "expansion",
"meaning": {
"fr": "expansion, développement"
}
},
{
"level": "b2-2",
"position": 60,
"text": "expectation",
"meaning": {
"fr": "attente, espérance"
}
},
{
"level": "b2-2",
"position": 61,
"text": "expense",
"meaning": {
"fr": "dépense, frais"
}
},
{
"level": "b2-2",
"position": 62,
"text": "expose",
"meaning": {
"fr": "exposer, dévoiler"
}
},
{
"level": "b2-2",
"position": 63,
"text": "exposure",
"meaning": {
"fr": "exposition (à), révélation"
}
},
{
"level": "b2-2",
"position": 64,
"text": "extend",
"meaning": {
"fr": "prolonger, étendre"
}
},
{
"level": "b2-2",
"position": 65,
"text": "extension",
"meaning": {
"fr": "extension, prolongation"
}
},
{
"level": "b2-2",
"position": 66,
"text": "extensive",
"meaning": {
"fr": "vaste, étendu, important"
}
},
{
"level": "b2-2",
"position": 67,
"text": "extent",
"meaning": {
"fr": "étendue, mesure, ampleur"
}
},
{
"level": "b2-2",
"position": 68,
"text": "external",
"meaning": {
"fr": "externe, extérieur"
}
},
{
"level": "b2-2",
"position": 69,
"text": "fabric",
"meaning": {
"fr": "tissu, étoffe"
}
},
{
"level": "b2-2",
"position": 70,
"text": "faculty",
"meaning": {
"fr": "faculté, corps enseignant"
}
},
{
"level": "b2-2",
"position": 71,
"text": "fade",
"meaning": {
"fr": "s'estomper, pâlir"
}
},
{
"level": "b2-2",
"position": 72,
"text": "fate",
"meaning": {
"fr": "destin, sort"
}
},
{
"level": "b2-2",
"position": 73,
"text": "federal",
"meaning": {
"fr": "fédéral"
}
},
{
"level": "b2-2",
"position": 74,
"text": "fellow",
"meaning": {
"fr": "camarade, compagnon"
}
},
{
"level": "b2-2",
"position": 75,
"text": "fiber",
"meaning": {
"fr": "fibre"
}
},
{
"level": "b2-2",
"position": 76,
"text": "fighter",
"meaning": {
"fr": "combattant, avion de chasse"
}
},
{
"level": "b2-2",
"position": 77,
"text": "finding",
"meaning": {
"fr": "conclusion, découverte"
}
},
{
"level": "b2-2",
"position": 78,
"text": "flame",
"meaning": {
"fr": "flamme"
}
},
{
"level": "b2-2",
"position": 79,
"text": "flee",
"meaning": {
"fr": "fuir, s'enfuir"
}
},
{
"level": "b2-2",
"position": 80,
"text": "flesh",
"meaning": {
"fr": "chair"
}
},
{
"level": "b2-2",
"position": 81,
"text": "formation",
"meaning": {
"fr": "formation (structure)"
}
},
{
"level": "b2-2",
"position": 82,
"text": "formula",
"meaning": {
"fr": "formule"
}
},
{
"level": "b2-2",
"position": 83,
"text": "forth",
"meaning": {
"fr": "en avant, dehors"
}
},
{
"level": "b2-2",
"position": 84,
"text": "foundation",
"meaning": {
"fr": "fondation, fondement"
}
},
{
"level": "b2-2",
"position": 85,
"text": "founder",
"meaning": {
"fr": "fondateur"
}
},
{
"level": "b2-2",
"position": 86,
"text": "framework",
"meaning": {
"fr": "cadre (structure)"
}
},
{
"level": "b2-2",
"position": 87,
"text": "frequency",
"meaning": {
"fr": "fréquence"
}
},
{
"level": "b2-2",
"position": 88,
"text": "frustration",
"meaning": {
"fr": "frustration"
}
},
{
"level": "b2-2",
"position": 89,
"text": "fundamental",
"meaning": {
"fr": "fondamental"
}
},
{
"level": "b2-2",
"position": 90,
"text": "funding",
"meaning": {
"fr": "financement"
}
},
{
"level": "b2-2",
"position": 91,
"text": "furthermore",
"meaning": {
"fr": "de plus, en outre"
}
},
{
"level": "b2-2",
"position": 92,
"text": "galaxy",
"meaning": {
"fr": "galaxie"
}
},
{
"level": "b2-2",
"position": 93,
"text": "gaze",
"meaning": {
"fr": "(v.) fixer du regard ; regard fixe"
}
},
{
"level": "b2-2",
"position": 94,
"text": "gene",
"meaning": {
"fr": "gène"
}
},
{
"level": "b2-2",
"position": 95,
"text": "generate",
"meaning": {
"fr": "générer, produire"
}
},
{
"level": "b2-2",
"position": 96,
"text": "genetic",
"meaning": {
"fr": "génétique"
}
},
{
"level": "b2-2",
"position": 97,
"text": "gesture",
"meaning": {
"fr": "geste"
}
},
{
"level": "b2-2",
"position": 98,
"text": "gifted",
"meaning": {
"fr": "doué"
}
},
{
"level": "b2-2",
"position": 99,
"text": "glance",
"meaning": {
"fr": "coup d'œil ; jeter un coup d'œil"
}
},
{
"level": "b2-2",
"position": 100,
"text": "governor",
"meaning": {
"fr": "gouverneur"
}
},
{
"level": "b2-2",
"position": 101,
"text": "grab",
"meaning": {
"fr": "saisir, attraper"
}
},
{
"level": "b2-2",
"position": 102,
"text": "grain",
"meaning": {
"fr": "grain, céréales"
}
},
{
"level": "b2-2",
"position": 103,
"text": "grand",
"meaning": {
"fr": "grandiose, magnifique"
}
},
{
"level": "b2-2",
"position": 104,
"text": "grant",
"meaning": {
"fr": "subvention ; accorder"
}
},
{
"level": "b2-2",
"position": 105,
"text": "grave",
"meaning": {
"fr": "tombe ; (adj.) grave"
}
},
{
"level": "b2-2",
"position": 106,
"text": "guideline",
"meaning": {
"fr": "directive, ligne directrice"
}
},
{
"level": "b2-2",
"position": 107,
"text": "habitat",
"meaning": {
"fr": "habitat"
}
},
{
"level": "b2-2",
"position": 108,
"text": "handful",
"meaning": {
"fr": "poignée (petite quantité)"
}
},
{
"level": "b2-2",
"position": 109,
"text": "headquarters",
"meaning": {
"fr": "siège, quartier général"
}
},
{
"level": "b2-2",
"position": 110,
"text": "hearing",
"meaning": {
"fr": "audition, ouïe"
}
},
{
"level": "b2-2",
"position": 111,
"text": "heel",
"meaning": {
"fr": "talon"
}
},
{
"level": "b2-2",
"position": 112,
"text": "heritage",
"meaning": {
"fr": "patrimoine, héritage"
}
},
{
"level": "b2-2",
"position": 113,
"text": "hip",
"meaning": {
"fr": "hanche"
}
},
{
"level": "b2-2",
"position": 114,
"text": "historian",
"meaning": {
"fr": "historien"
}
},
{
"level": "b2-2",
"position": 115,
"text": "holy",
"meaning": {
"fr": "saint, sacré"
}
},
{
"level": "b2-2",
"position": 116,
"text": "horizon",
"meaning": {
"fr": "horizon"
}
},
{
"level": "b2-2",
"position": 117,
"text": "household",
"meaning": {
"fr": "ménage (famille)"
}
},
{
"level": "b2-2",
"position": 118,
"text": "housing",
"meaning": {
"fr": "logement"
}
},
{
"level": "b2-2",
"position": 119,
"text": "hunter",
"meaning": {
"fr": "chasseur"
}
},
{
"level": "b2-2",
"position": 120,
"text": "hypothesis",
"meaning": {
"fr": "hypothèse"
}
},
{
"level": "b2-2",
"position": 121,
"text": "identification",
"meaning": {
"fr": "identification, pièce d'identité"
}
},
{
"level": "b2-2",
"position": 122,
"text": "immigration",
"meaning": {
"fr": "immigration"
}
},
{
"level": "b2-2",
"position": 123,
"text": "implement",
"meaning": {
"fr": "mettre en œuvre"
}
},
{
"level": "b2-2",
"position": 124,
"text": "implication",
"meaning": {
"fr": "conséquence, implication"
}
},
{
"level": "b2-2",
"position": 125,
"text": "imply",
"meaning": {
"fr": "sous-entendre, impliquer"
}
},
{
"level": "b2-2",
"position": 126,
"text": "impose",
"meaning": {
"fr": "imposer"
}
},
{
"level": "b2-2",
"position": 127,
"text": "incentive",
"meaning": {
"fr": "incitation, motivation"
}
},
{
"level": "b2-2",
"position": 128,
"text": "incident",
"meaning": {
"fr": "incident"
}
},
{
"level": "b2-2",
"position": 129,
"text": "incorporate",
"meaning": {
"fr": "incorporer, intégrer"
}
},
{
"level": "b2-2",
"position": 130,
"text": "increasing",
"meaning": {
"fr": "croissant, grandissant"
}
},
{
"level": "b2-2",
"position": 131,
"text": "increasingly",
"meaning": {
"fr": "de plus en plus"
}
},
{
"level": "b2-2",
"position": 132,
"text": "index",
"meaning": {
"fr": "index, indice"
}
},
{
"level": "b2-2",
"position": 133,
"text": "indication",
"meaning": {
"fr": "indication, signe"
}
},
{
"level": "b2-2",
"position": 134,
"text": "infant",
"meaning": {
"fr": "nourrisson, bébé"
}
},
{
"level": "b2-2",
"position": 135,
"text": "inflation",
"meaning": {
"fr": "inflation"
}
},
{
"level": "b2-2",
"position": 136,
"text": "initiative",
"meaning": {
"fr": "initiative"
}
},
{
"level": "b2-2",
"position": 137,
"text": "inner",
"meaning": {
"fr": "intérieur, intime"
}
},
{
"level": "b2-2",
"position": 138,
"text": "inquiry",
"meaning": {
"fr": "demande de renseignements, enquête"
}
},
{
"level": "b2-2",
"position": 139,
"text": "insight",
"meaning": {
"fr": "aperçu, perspicacité"
}
},
{
"level": "b2-2",
"position": 140,
"text": "instance",
"meaning": {
"fr": "exemple, cas"
}
},
{
"level": "b2-2",
"position": 141,
"text": "institution",
"meaning": {
"fr": "institution"
}
},
{
"level": "b2-2",
"position": 142,
"text": "institutional",
"meaning": {
"fr": "institutionnel"
}
},
{
"level": "b2-2",
"position": 143,
"text": "intellectual",
"meaning": {
"fr": "intellectuel"
}
},
{
"level": "b2-2",
"position": 144,
"text": "intense",
"meaning": {
"fr": "intense"
}
},
{
"level": "b2-2",
"position": 145,
"text": "intensity",
"meaning": {
"fr": "intensité"
}
},
{
"level": "b2-2",
"position": 146,
"text": "interaction",
"meaning": {
"fr": "interaction"
}
},
{
"level": "b2-2",
"position": 147,
"text": "internal",
"meaning": {
"fr": "interne"
}
},
{
"level": "b2-2",
"position": 148,
"text": "interpret",
"meaning": {
"fr": "interpréter"
}
},
{
"level": "b2-2",
"position": 149,
"text": "interpretation",
"meaning": {
"fr": "interprétation"
}
},
{
"level": "b2-2",
"position": 150,
"text": "intervention",
"meaning": {
"fr": "intervention"
}
},
{
"level": "b2-2",
"position": 151,
"text": "invasion",
"meaning": {
"fr": "invasion"
}
},
{
"level": "b2-2",
"position": 152,
"text": "investigator",
"meaning": {
"fr": "enquêteur"
}
},
{
"level": "b2-2",
"position": 153,
"text": "investor",
"meaning": {
"fr": "investisseur"
}
},
{
"level": "b2-2",
"position": 154,
"text": "involvement",
"meaning": {
"fr": "participation, engagement"
}
},
{
"level": "b2-2",
"position": 155,
"text": "Iraqi",
"meaning": {
"fr": "irakien"
}
},
{
"level": "b2-2",
"position": 156,
"text": "Islamic",
"meaning": {
"fr": "islamique"
}
},
{
"level": "b2-2",
"position": 157,
"text": "Israeli",
"meaning": {
"fr": "israélien"
}
},
{
"level": "b2-2",
"position": 158,
"text": "Jew",
"meaning": {
"fr": "Juif, Juive"
}
},
{
"level": "b2-2",
"position": 159,
"text": "Jewish",
"meaning": {
"fr": "juif"
}
},
{
"level": "b2-2",
"position": 160,
"text": "joint",
"meaning": {
"fr": "commun ; articulation"
}
},
{
"level": "b2-2",
"position": 161,
"text": "journal",
"meaning": {
"fr": "revue, journal (spécialisé)"
}
},
{
"level": "b2-2",
"position": 162,
"text": "judgment",
"meaning": {
"fr": "jugement"
}
},
{
"level": "b2-2",
"position": 163,
"text": "jury",
"meaning": {
"fr": "jury"
}
},
{
"level": "b2-2",
"position": 164,
"text": "justice",
"meaning": {
"fr": "justice"
}
},
{
"level": "b2-2",
"position": 165,
"text": "justify",
"meaning": {
"fr": "justifier"
}
},
{
"level": "b2-2",
"position": 166,
"text": "killing",
"meaning": {
"fr": "meurtre, tuerie"
}
},
{
"level": "b2-2",
"position": 167,
"text": "labor",
"meaning": {
"fr": "travail, main-d'œuvre"
}
},
{
"level": "b2-2",
"position": 168,
"text": "lap",
"meaning": {
"fr": "genoux (assis), tour de piste"
}
},
{
"level": "b2-2",
"position": 169,
"text": "largely",
"meaning": {
"fr": "en grande partie"
}
},
{
"level": "b2-2",
"position": 170,
"text": "latter",
"meaning": {
"fr": "ce dernier, second"
}
},
{
"level": "b2-2",
"position": 171,
"text": "lawsuit",
"meaning": {
"fr": "procès, poursuite"
}
},
{
"level": "b2-2",
"position": 172,
"text": "leadership",
"meaning": {
"fr": "direction, leadership"
}
},
{
"level": "b2-2",
"position": 173,
"text": "leading",
"meaning": {
"fr": "principal, de premier plan"
}
},
{
"level": "b2-2",
"position": 174,
"text": "lean",
"meaning": {
"fr": "se pencher, s'appuyer ; (adj.) maigre"
}
},
{
"level": "b2-2",
"position": 175,
"text": "legacy",
"meaning": {
"fr": "héritage, legs"
}
},
{
"level": "b2-2",
"position": 176,
"text": "legislation",
"meaning": {
"fr": "législation"
}
},
{
"level": "b2-2",
"position": 177,
"text": "legitimate",
"meaning": {
"fr": "légitime"
}
},
{
"level": "b2-3",
"position": 1,
"text": "liberal",
"meaning": {
"fr": "libéral, progressiste"
}
},
{
"level": "b2-3",
"position": 2,
"text": "lifetime",
"meaning": {
"fr": "vie entière, durée de vie"
}
},
{
"level": "b2-3",
"position": 3,
"text": "limitation",
"meaning": {
"fr": "limitation, restriction"
}
},
{
"level": "b2-3",
"position": 4,
"text": "literally",
"meaning": {
"fr": "littéralement"
}
},
{
"level": "b2-3",
"position": 5,
"text": "literary",
"meaning": {
"fr": "littéraire"
}
},
{
"level": "b2-3",
"position": 6,
"text": "long-term",
"meaning": {
"fr": "à long terme"
}
},
{
"level": "b2-3",
"position": 7,
"text": "loose",
"meaning": {
"fr": "lâche, desserré, ample"
}
},
{
"level": "b2-3",
"position": 8,
"text": "lover",
"meaning": {
"fr": "amant, amoureux, passionné"
}
},
{
"level": "b2-3",
"position": 9,
"text": "lung",
"meaning": {
"fr": "poumon"
}
},
{
"level": "b2-3",
"position": 10,
"text": "maintain",
"meaning": {
"fr": "maintenir, entretenir"
}
},
{
"level": "b2-3",
"position": 11,
"text": "maintenance",
"meaning": {
"fr": "entretien, maintenance"
}
},
{
"level": "b2-3",
"position": 12,
"text": "maker",
"meaning": {
"fr": "fabricant, créateur"
}
},
{
"level": "b2-3",
"position": 13,
"text": "manner",
"meaning": {
"fr": "manière, façon"
}
},
{
"level": "b2-3",
"position": 14,
"text": "manufacturing",
"meaning": {
"fr": "fabrication, industrie"
}
},
{
"level": "b2-3",
"position": 15,
"text": "margin",
"meaning": {
"fr": "marge"
}
},
{
"level": "b2-3",
"position": 16,
"text": "master",
"meaning": {
"fr": "maître ; (v.) maîtriser"
}
},
{
"level": "b2-3",
"position": 17,
"text": "meanwhile",
"meaning": {
"fr": "pendant ce temps, entre-temps"
}
},
{
"level": "b2-3",
"position": 18,
"text": "mechanism",
"meaning": {
"fr": "mécanisme"
}
},
{
"level": "b2-3",
"position": 19,
"text": "medication",
"meaning": {
"fr": "médicament, traitement"
}
},
{
"level": "b2-3",
"position": 20,
"text": "membership",
"meaning": {
"fr": "adhésion, abonnement"
}
},
{
"level": "b2-3",
"position": 21,
"text": "mere",
"meaning": {
"fr": "simple, pur"
}
},
{
"level": "b2-3",
"position": 22,
"text": "merely",
"meaning": {
"fr": "simplement, seulement"
}
},
{
"level": "b2-3",
"position": 23,
"text": "minor",
"meaning": {
"fr": "mineur, secondaire"
}
},
{
"level": "b2-3",
"position": 24,
"text": "minority",
"meaning": {
"fr": "minorité"
}
},
{
"level": "b2-3",
"position": 25,
"text": "miracle",
"meaning": {
"fr": "miracle"
}
},
{
"level": "b2-3",
"position": 26,
"text": "missile",
"meaning": {
"fr": "missile"
}
},
{
"level": "b2-3",
"position": 27,
"text": "mode",
"meaning": {
"fr": "mode (manière)"
}
},
{
"level": "b2-3",
"position": 28,
"text": "moderate",
"meaning": {
"fr": "modéré"
}
},
{
"level": "b2-3",
"position": 29,
"text": "modest",
"meaning": {
"fr": "modeste"
}
},
{
"level": "b2-3",
"position": 30,
"text": "monitor",
"meaning": {
"fr": "écran, moniteur ; (v.) surveiller"
}
},
{
"level": "b2-3",
"position": 31,
"text": "moral",
"meaning": {
"fr": "moral ; (n.) morale"
}
},
{
"level": "b2-3",
"position": 32,
"text": "moreover",
"meaning": {
"fr": "de plus, en outre"
}
},
{
"level": "b2-3",
"position": 33,
"text": "mortgage",
"meaning": {
"fr": "prêt immobilier, hypothèque"
}
},
{
"level": "b2-3",
"position": 34,
"text": "motion",
"meaning": {
"fr": "mouvement, motion"
}
},
{
"level": "b2-3",
"position": 35,
"text": "motor",
"meaning": {
"fr": "moteur"
}
},
{
"level": "b2-3",
"position": 36,
"text": "mount",
"meaning": {
"fr": "monter, gravir ; (n.) mont"
}
},
{
"level": "b2-3",
"position": 37,
"text": "multiple",
"meaning": {
"fr": "multiple"
}
},
{
"level": "b2-3",
"position": 38,
"text": "Muslim",
"meaning": {
"fr": "musulman"
}
},
{
"level": "b2-3",
"position": 39,
"text": "mutual",
"meaning": {
"fr": "mutuel, réciproque"
}
},
{
"level": "b2-3",
"position": 40,
"text": "myth",
"meaning": {
"fr": "mythe"
}
},
{
"level": "b2-3",
"position": 41,
"text": "naked",
"meaning": {
"fr": "nu"
}
},
{
"level": "b2-3",
"position": 42,
"text": "narrative",
"meaning": {
"fr": "récit, narration"
}
},
{
"level": "b2-3",
"position": 43,
"text": "necessarily",
"meaning": {
"fr": "nécessairement"
}
},
{
"level": "b2-3",
"position": 44,
"text": "negotiate",
"meaning": {
"fr": "négocier"
}
},
{
"level": "b2-3",
"position": 45,
"text": "negotiation",
"meaning": {
"fr": "négociation"
}
},
{
"level": "b2-3",
"position": 46,
"text": "nerve",
"meaning": {
"fr": "nerf, culot"
}
},
{
"level": "b2-3",
"position": 47,
"text": "net",
"meaning": {
"fr": "filet ; (adj.) net"
}
},
{
"level": "b2-3",
"position": 48,
"text": "nevertheless",
"meaning": {
"fr": "néanmoins, pourtant"
}
},
{
"level": "b2-3",
"position": 49,
"text": "newly",
"meaning": {
"fr": "récemment, nouvellement"
}
},
{
"level": "b2-3",
"position": 50,
"text": "nod",
"meaning": {
"fr": "hocher la tête"
}
},
{
"level": "b2-3",
"position": 51,
"text": "nomination",
"meaning": {
"fr": "nomination, candidature"
}
},
{
"level": "b2-3",
"position": 52,
"text": "nonetheless",
"meaning": {
"fr": "néanmoins, malgré tout"
}
},
{
"level": "b2-3",
"position": 53,
"text": "nor",
"meaning": {
"fr": "ni"
}
},
{
"level": "b2-3",
"position": 54,
"text": "notion",
"meaning": {
"fr": "notion, idée"
}
},
{
"level": "b2-3",
"position": 55,
"text": "numerous",
"meaning": {
"fr": "nombreux"
}
},
{
"level": "b2-3",
"position": 56,
"text": "objective",
"meaning": {
"fr": "objectif"
}
},
{
"level": "b2-3",
"position": 57,
"text": "obligation",
"meaning": {
"fr": "obligation"
}
},
{
"level": "b2-3",
"position": 58,
"text": "observation",
"meaning": {
"fr": "observation"
}
},
{
"level": "b2-3",
"position": 59,
"text": "observer",
"meaning": {
"fr": "observateur"
}
},
{
"level": "b2-3",
"position": 60,
"text": "obtain",
"meaning": {
"fr": "obtenir"
}
},
{
"level": "b2-3",
"position": 61,
"text": "occupation",
"meaning": {
"fr": "profession, occupation"
}
},
{
"level": "b2-3",
"position": 62,
"text": "occupy",
"meaning": {
"fr": "occuper"
}
},
{
"level": "b2-3",
"position": 63,
"text": "odds",
"meaning": {
"fr": "chances, probabilités"
}
},
{
"level": "b2-3",
"position": 64,
"text": "offense",
"meaning": {
"fr": "infraction, délit, offense"
}
},
{
"level": "b2-3",
"position": 65,
"text": "offensive",
"meaning": {
"fr": "offensant, choquant"
}
},
{
"level": "b2-3",
"position": 66,
"text": "ongoing",
"meaning": {
"fr": "en cours, continu"
}
},
{
"level": "b2-3",
"position": 67,
"text": "opening",
"meaning": {
"fr": "ouverture, inauguration"
}
},
{
"level": "b2-3",
"position": 68,
"text": "operate",
"meaning": {
"fr": "fonctionner, opérer"
}
},
{
"level": "b2-3",
"position": 69,
"text": "operating",
"meaning": {
"fr": "d'exploitation, en fonctionnement"
}
},
{
"level": "b2-3",
"position": 70,
"text": "operator",
"meaning": {
"fr": "opérateur"
}
},
{
"level": "b2-3",
"position": 71,
"text": "opponent",
"meaning": {
"fr": "adversaire"
}
},
{
"level": "b2-3",
"position": 72,
"text": "oppose",
"meaning": {
"fr": "s'opposer à"
}
},
{
"level": "b2-3",
"position": 73,
"text": "opposition",
"meaning": {
"fr": "opposition"
}
},
{
"level": "b2-3",
"position": 74,
"text": "orientation",
"meaning": {
"fr": "orientation"
}
},
{
"level": "b2-3",
"position": 75,
"text": "otherwise",
"meaning": {
"fr": "sinon, autrement"
}
},
{
"level": "b2-3",
"position": 76,
"text": "outcome",
"meaning": {
"fr": "résultat, issue"
}
},
{
"level": "b2-3",
"position": 77,
"text": "overall",
"meaning": {
"fr": "global, dans l'ensemble"
}
},
{
"level": "b2-3",
"position": 78,
"text": "overcome",
"meaning": {
"fr": "surmonter, vaincre"
}
},
{
"level": "b2-3",
"position": 79,
"text": "overlook",
"meaning": {
"fr": "négliger, donner sur"
}
},
{
"level": "b2-3",
"position": 80,
"text": "pace",
"meaning": {
"fr": "rythme, allure"
}
},
{
"level": "b2-3",
"position": 81,
"text": "palm",
"meaning": {
"fr": "paume, palmier"
}
},
{
"level": "b2-3",
"position": 82,
"text": "panel",
"meaning": {
"fr": "panneau, jury, comité"
}
},
{
"level": "b2-3",
"position": 83,
"text": "participant",
"meaning": {
"fr": "participant"
}
},
{
"level": "b2-3",
"position": 84,
"text": "participation",
"meaning": {
"fr": "participation"
}
},
{
"level": "b2-3",
"position": 85,
"text": "partnership",
"meaning": {
"fr": "partenariat"
}
},
{
"level": "b2-3",
"position": 86,
"text": "patch",
"meaning": {
"fr": "parcelle, pièce, tache"
}
},
{
"level": "b2-3",
"position": 87,
"text": "peak",
"meaning": {
"fr": "sommet, pic"
}
},
{
"level": "b2-3",
"position": 88,
"text": "peer",
"meaning": {
"fr": "pair, homologue"
}
},
{
"level": "b2-3",
"position": 89,
"text": "perceive",
"meaning": {
"fr": "percevoir"
}
},
{
"level": "b2-3",
"position": 90,
"text": "perception",
"meaning": {
"fr": "perception"
}
},
{
"level": "b2-3",
"position": 91,
"text": "permit",
"meaning": {
"fr": "permettre ; (n.) permis"
}
},
{
"level": "b2-3",
"position": 92,
"text": "personnel",
"meaning": {
"fr": "personnel (employés)"
}
},
{
"level": "b2-3",
"position": 93,
"text": "perspective",
"meaning": {
"fr": "perspective, point de vue"
}
},
{
"level": "b2-3",
"position": 94,
"text": "phase",
"meaning": {
"fr": "phase, étape"
}
},
{
"level": "b2-3",
"position": 95,
"text": "phenomenon",
"meaning": {
"fr": "phénomène"
}
},
{
"level": "b2-3",
"position": 96,
"text": "philosophy",
"meaning": {
"fr": "philosophie"
}
},
{
"level": "b2-3",
"position": 97,
"text": "physician",
"meaning": {
"fr": "médecin"
}
},
{
"level": "b2-3",
"position": 98,
"text": "pile",
"meaning": {
"fr": "tas, pile ; (v.) empiler"
}
},
{
"level": "b2-3",
"position": 99,
"text": "pine",
"meaning": {
"fr": "pin"
}
},
{
"level": "b2-3",
"position": 100,
"text": "pitch",
"meaning": {
"fr": "terrain, ton ; (v.) lancer"
}
},
{
"level": "b2-3",
"position": 101,
"text": "pole",
"meaning": {
"fr": "poteau, pôle"
}
},
{
"level": "b2-3",
"position": 102,
"text": "politically",
"meaning": {
"fr": "politiquement"
}
},
{
"level": "b2-3",
"position": 103,
"text": "poll",
"meaning": {
"fr": "sondage, scrutin"
}
},
{
"level": "b2-3",
"position": 104,
"text": "porch",
"meaning": {
"fr": "porche, véranda"
}
},
{
"level": "b2-3",
"position": 105,
"text": "portion",
"meaning": {
"fr": "portion, part"
}
},
{
"level": "b2-3",
"position": 106,
"text": "portray",
"meaning": {
"fr": "représenter, dépeindre"
}
},
{
"level": "b2-3",
"position": 107,
"text": "pose",
"meaning": {
"fr": "poser (problème) ; (n.) pose"
}
},
{
"level": "b2-3",
"position": 108,
"text": "possess",
"meaning": {
"fr": "posséder"
}
},
{
"level": "b2-3",
"position": 109,
"text": "potentially",
"meaning": {
"fr": "potentiellement"
}
},
{
"level": "b2-3",
"position": 110,
"text": "powder",
"meaning": {
"fr": "poudre"
}
},
{
"level": "b2-3",
"position": 111,
"text": "prayer",
"meaning": {
"fr": "prière"
}
},
{
"level": "b2-3",
"position": 112,
"text": "precisely",
"meaning": {
"fr": "précisément"
}
},
{
"level": "b2-3",
"position": 113,
"text": "preference",
"meaning": {
"fr": "préférence"
}
},
{
"level": "b2-3",
"position": 114,
"text": "pregnancy",
"meaning": {
"fr": "grossesse"
}
},
{
"level": "b2-3",
"position": 115,
"text": "prescription",
"meaning": {
"fr": "ordonnance"
}
},
{
"level": "b2-3",
"position": 116,
"text": "presence",
"meaning": {
"fr": "présence"
}
},
{
"level": "b2-3",
"position": 117,
"text": "preserve",
"meaning": {
"fr": "préserver, protéger"
}
},
{
"level": "b2-3",
"position": 118,
"text": "presidential",
"meaning": {
"fr": "présidentiel"
}
},
{
"level": "b2-3",
"position": 119,
"text": "previously",
"meaning": {
"fr": "auparavant, précédemment"
}
},
{
"level": "b2-3",
"position": 120,
"text": "pride",
"meaning": {
"fr": "fierté"
}
},
{
"level": "b2-3",
"position": 121,
"text": "primarily",
"meaning": {
"fr": "principalement"
}
},
{
"level": "b2-3",
"position": 122,
"text": "prime",
"meaning": {
"fr": "principal, de premier ordre"
}
},
{
"level": "b2-3",
"position": 123,
"text": "principal",
"meaning": {
"fr": "(adj.) principal ; (n.) directeur"
}
},
{
"level": "b2-3",
"position": 124,
"text": "principle",
"meaning": {
"fr": "principe"
}
},
{
"level": "b2-3",
"position": 125,
"text": "prior",
"meaning": {
"fr": "antérieur, préalable"
}
},
{
"level": "b2-3",
"position": 126,
"text": "priority",
"meaning": {
"fr": "priorité"
}
},
{
"level": "b2-3",
"position": 127,
"text": "privacy",
"meaning": {
"fr": "vie privée, intimité"
}
},
{
"level": "b2-3",
"position": 128,
"text": "procedure",
"meaning": {
"fr": "procédure"
}
},
{
"level": "b2-3",
"position": 129,
"text": "proceed",
"meaning": {
"fr": "procéder, poursuivre"
}
},
{
"level": "b2-3",
"position": 130,
"text": "profile",
"meaning": {
"fr": "profil"
}
},
{
"level": "b2-3",
"position": 131,
"text": "prominent",
"meaning": {
"fr": "éminent, important"
}
},
{
"level": "b2-3",
"position": 132,
"text": "prompt",
"meaning": {
"fr": "(v.) inciter ; (adj.) prompt"
}
},
{
"level": "b2-3",
"position": 133,
"text": "proof",
"meaning": {
"fr": "preuve"
}
},
{
"level": "b2-3",
"position": 134,
"text": "proportion",
"meaning": {
"fr": "proportion, part"
}
},
{
"level": "b2-3",
"position": 135,
"text": "proposal",
"meaning": {
"fr": "proposition"
}
},
{
"level": "b2-3",
"position": 136,
"text": "propose",
"meaning": {
"fr": "proposer"
}
},
{
"level": "b2-3",
"position": 137,
"text": "proposed",
"meaning": {
"fr": "proposé, envisagé"
}
},
{
"level": "b2-3",
"position": 138,
"text": "prosecutor",
"meaning": {
"fr": "procureur"
}
},
{
"level": "b2-3",
"position": 139,
"text": "prospect",
"meaning": {
"fr": "perspective"
}
},
{
"level": "b2-3",
"position": 140,
"text": "protein",
"meaning": {
"fr": "protéine"
}
},
{
"level": "b2-3",
"position": 141,
"text": "provider",
"meaning": {
"fr": "fournisseur, prestataire"
}
},
{
"level": "b2-3",
"position": 142,
"text": "province",
"meaning": {
"fr": "province"
}
},
{
"level": "b2-3",
"position": 143,
"text": "provision",
"meaning": {
"fr": "fourniture, disposition"
}
},
{
"level": "b2-3",
"position": 144,
"text": "psychological",
"meaning": {
"fr": "psychologique"
}
},
{
"level": "b2-3",
"position": 145,
"text": "psychologist",
"meaning": {
"fr": "psychologue"
}
},
{
"level": "b2-3",
"position": 146,
"text": "psychology",
"meaning": {
"fr": "psychologie"
}
},
{
"level": "b2-3",
"position": 147,
"text": "publication",
"meaning": {
"fr": "publication"
}
},
{
"level": "b2-3",
"position": 148,
"text": "publicly",
"meaning": {
"fr": "publiquement"
}
},
{
"level": "b2-3",
"position": 149,
"text": "publisher",
"meaning": {
"fr": "éditeur"
}
},
{
"level": "b2-3",
"position": 150,
"text": "pure",
"meaning": {
"fr": "pur"
}
},
{
"level": "b2-3",
"position": 151,
"text": "pursue",
"meaning": {
"fr": "poursuivre"
}
},
{
"level": "b2-3",
"position": 152,
"text": "qualify",
"meaning": {
"fr": "se qualifier, être admissible"
}
},
{
"level": "b2-3",
"position": 153,
"text": "quarterback",
"meaning": {
"fr": "quarterback (football américain)"
}
},
{
"level": "b2-3",
"position": 154,
"text": "racial",
"meaning": {
"fr": "racial"
}
},
{
"level": "b2-3",
"position": 155,
"text": "radical",
"meaning": {
"fr": "radical"
}
},
{
"level": "b2-3",
"position": 156,
"text": "rank",
"meaning": {
"fr": "rang, grade"
}
},
{
"level": "b2-3",
"position": 157,
"text": "rapid",
"meaning": {
"fr": "rapide, accéléré"
}
},
{
"level": "b2-3",
"position": 158,
"text": "rapidly",
"meaning": {
"fr": "rapidement, à grande vitesse"
}
},
{
"level": "b2-3",
"position": 159,
"text": "rating",
"meaning": {
"fr": "note, classement"
}
},
{
"level": "b2-3",
"position": 160,
"text": "ratio",
"meaning": {
"fr": "ratio, rapport"
}
},
{
"level": "b2-3",
"position": 161,
"text": "raw",
"meaning": {
"fr": "cru, brut"
}
},
{
"level": "b2-3",
"position": 162,
"text": "recall",
"meaning": {
"fr": "se remémorer, rappel"
}
},
{
"level": "b2-3",
"position": 163,
"text": "recognition",
"meaning": {
"fr": "reconnaissance"
}
},
{
"level": "b2-3",
"position": 164,
"text": "recommendation",
"meaning": {
"fr": "recommandation"
}
},
{
"level": "b2-3",
"position": 165,
"text": "recovery",
"meaning": {
"fr": "guérison, reprise"
}
},
{
"level": "b2-3",
"position": 166,
"text": "recruit",
"meaning": {
"fr": "recruter"
}
},
{
"level": "b2-3",
"position": 167,
"text": "reduction",
"meaning": {
"fr": "réduction, baisse"
}
},
{
"level": "b2-3",
"position": 168,
"text": "reflection",
"meaning": {
"fr": "reflet, réflexion"
}
},
{
"level": "b2-3",
"position": 169,
"text": "reform",
"meaning": {
"fr": "réforme"
}
},
{
"level": "b2-3",
"position": 170,
"text": "refugee",
"meaning": {
"fr": "réfugié"
}
},
{
"level": "b2-3",
"position": 171,
"text": "regard",
"meaning": {
"fr": "(v.) considérer ; (n.) égard"
}
},
{
"level": "b2-3",
"position": 172,
"text": "regarding",
"meaning": {
"fr": "concernant, au sujet de"
}
},
{
"level": "b2-3",
"position": 173,
"text": "regardless",
"meaning": {
"fr": "quoi qu'il en soit, malgré"
}
},
{
"level": "b2-3",
"position": 174,
"text": "regime",
"meaning": {
"fr": "régime (politique)"
}
},
{
"level": "b2-3",
"position": 175,
"text": "regional",
"meaning": {
"fr": "régional"
}
},
{
"level": "b2-3",
"position": 176,
"text": "regulate",
"meaning": {
"fr": "réglementer, réguler"
}
},
{
"level": "b2-3",
"position": 177,
"text": "regulation",
"meaning": {
"fr": "réglementation"
}
},
{
"level": "b2-4",
"position": 1,
"text": "reinforce",
"meaning": {
"fr": "renforcer, consolider"
}
},
{
"level": "b2-4",
"position": 2,
"text": "relevant",
"meaning": {
"fr": "pertinent"
}
},
{
"level": "b2-4",
"position": 3,
"text": "relief",
"meaning": {
"fr": "soulagement, secours"
}
},
{
"level": "b2-4",
"position": 4,
"text": "repeatedly",
"meaning": {
"fr": "à plusieurs reprises"
}
},
{
"level": "b2-4",
"position": 5,
"text": "representation",
"meaning": {
"fr": "représentation"
}
},
{
"level": "b2-4",
"position": 6,
"text": "representative",
"meaning": {
"fr": "représentant"
}
},
{
"level": "b2-4",
"position": 7,
"text": "Republican",
"meaning": {
"fr": "républicain"
}
},
{
"level": "b2-4",
"position": 8,
"text": "requirement",
"meaning": {
"fr": "exigence, condition"
}
},
{
"level": "b2-4",
"position": 9,
"text": "resemble",
"meaning": {
"fr": "ressembler à"
}
},
{
"level": "b2-4",
"position": 10,
"text": "resident",
"meaning": {
"fr": "résident, habitant"
}
},
{
"level": "b2-4",
"position": 11,
"text": "resist",
"meaning": {
"fr": "résister"
}
},
{
"level": "b2-4",
"position": 12,
"text": "resistance",
"meaning": {
"fr": "résistance"
}
},
{
"level": "b2-4",
"position": 13,
"text": "resolution",
"meaning": {
"fr": "résolution"
}
},
{
"level": "b2-4",
"position": 14,
"text": "resolve",
"meaning": {
"fr": "résoudre, régler"
}
},
{
"level": "b2-4",
"position": 15,
"text": "resort",
"meaning": {
"fr": "station (de vacances), recours"
}
},
{
"level": "b2-4",
"position": 16,
"text": "respondent",
"meaning": {
"fr": "personne interrogée"
}
},
{
"level": "b2-4",
"position": 17,
"text": "restore",
"meaning": {
"fr": "restaurer, rétablir"
}
},
{
"level": "b2-4",
"position": 18,
"text": "restriction",
"meaning": {
"fr": "restriction, limitation"
}
},
{
"level": "b2-4",
"position": 19,
"text": "retain",
"meaning": {
"fr": "conserver, retenir"
}
},
{
"level": "b2-4",
"position": 20,
"text": "retirement",
"meaning": {
"fr": "retraite"
}
},
{
"level": "b2-4",
"position": 21,
"text": "reveal",
"meaning": {
"fr": "révéler"
}
},
{
"level": "b2-4",
"position": 22,
"text": "revenue",
"meaning": {
"fr": "recettes, revenus"
}
},
{
"level": "b2-4",
"position": 23,
"text": "rifle",
"meaning": {
"fr": "fusil"
}
},
{
"level": "b2-4",
"position": 24,
"text": "root",
"meaning": {
"fr": "racine"
}
},
{
"level": "b2-4",
"position": 25,
"text": "rough",
"meaning": {
"fr": "rugueux, approximatif"
}
},
{
"level": "b2-4",
"position": 26,
"text": "roughly",
"meaning": {
"fr": "environ, approximativement"
}
},
{
"level": "b2-4",
"position": 27,
"text": "rub",
"meaning": {
"fr": "frotter"
}
},
{
"level": "b2-4",
"position": 28,
"text": "rural",
"meaning": {
"fr": "rural"
}
},
{
"level": "b2-4",
"position": 29,
"text": "sacred",
"meaning": {
"fr": "sacré"
}
},
{
"level": "b2-4",
"position": 30,
"text": "sake",
"meaning": {
"fr": "égard (pour l'amour de)"
}
},
{
"level": "b2-4",
"position": 31,
"text": "sanction",
"meaning": {
"fr": "sanction"
}
},
{
"level": "b2-4",
"position": 32,
"text": "satellite",
"meaning": {
"fr": "satellite"
}
},
{
"level": "b2-4",
"position": 33,
"text": "satisfaction",
"meaning": {
"fr": "satisfaction"
}
},
{
"level": "b2-4",
"position": 34,
"text": "satisfy",
"meaning": {
"fr": "satisfaire"
}
},
{
"level": "b2-4",
"position": 35,
"text": "scale",
"meaning": {
"fr": "échelle, ampleur"
}
},
{
"level": "b2-4",
"position": 36,
"text": "scandal",
"meaning": {
"fr": "scandale"
}
},
{
"level": "b2-4",
"position": 37,
"text": "scenario",
"meaning": {
"fr": "scénario"
}
},
{
"level": "b2-4",
"position": 38,
"text": "scheme",
"meaning": {
"fr": "plan, dispositif, combine"
}
},
{
"level": "b2-4",
"position": 39,
"text": "scholar",
"meaning": {
"fr": "érudit, savant"
}
},
{
"level": "b2-4",
"position": 40,
"text": "scholarship",
"meaning": {
"fr": "bourse d'études"
}
},
{
"level": "b2-4",
"position": 41,
"text": "scope",
"meaning": {
"fr": "portée, champ d'application"
}
},
{
"level": "b2-4",
"position": 42,
"text": "script",
"meaning": {
"fr": "script, texte"
}
},
{
"level": "b2-4",
"position": 43,
"text": "sector",
"meaning": {
"fr": "secteur"
}
},
{
"level": "b2-4",
"position": 44,
"text": "secure",
"meaning": {
"fr": "(adj.) sécurisé ; (v.) obtenir"
}
},
{
"level": "b2-4",
"position": 45,
"text": "seek",
"meaning": {
"fr": "chercher à, rechercher"
}
},
{
"level": "b2-4",
"position": 46,
"text": "segment",
"meaning": {
"fr": "segment, tranche"
}
},
{
"level": "b2-4",
"position": 47,
"text": "seize",
"meaning": {
"fr": "saisir, s'emparer de"
}
},
{
"level": "b2-4",
"position": 48,
"text": "self",
"meaning": {
"fr": "soi, moi"
}
},
{
"level": "b2-4",
"position": 49,
"text": "Senate",
"meaning": {
"fr": "Sénat"
}
},
{
"level": "b2-4",
"position": 50,
"text": "senator",
"meaning": {
"fr": "sénateur"
}
},
{
"level": "b2-4",
"position": 51,
"text": "sensitive",
"meaning": {
"fr": "sensible"
}
},
{
"level": "b2-4",
"position": 52,
"text": "sequence",
"meaning": {
"fr": "séquence, suite"
}
},
{
"level": "b2-4",
"position": 53,
"text": "settlement",
"meaning": {
"fr": "accord, règlement, colonie"
}
},
{
"level": "b2-4",
"position": 54,
"text": "severe",
"meaning": {
"fr": "grave, sévère"
}
},
{
"level": "b2-4",
"position": 55,
"text": "sexual",
"meaning": {
"fr": "sexuel"
}
},
{
"level": "b2-4",
"position": 56,
"text": "shade",
"meaning": {
"fr": "ombre (abri), nuance"
}
},
{
"level": "b2-4",
"position": 57,
"text": "shelter",
"meaning": {
"fr": "abri, refuge"
}
},
{
"level": "b2-4",
"position": 58,
"text": "shift",
"meaning": {
"fr": "(n.) changement, poste ; (v.) déplacer"
}
},
{
"level": "b2-4",
"position": 59,
"text": "shit",
"meaning": {
"fr": "merde"
}
},
{
"level": "b2-4",
"position": 60,
"text": "shore",
"meaning": {
"fr": "rivage, côte"
}
},
{
"level": "b2-4",
"position": 61,
"text": "shortly",
"meaning": {
"fr": "sous peu, peu après"
}
},
{
"level": "b2-4",
"position": 62,
"text": "shrug",
"meaning": {
"fr": "hausser les épaules"
}
},
{
"level": "b2-4",
"position": 63,
"text": "sigh",
"meaning": {
"fr": "(v.) soupirer ; (n.) soupir"
}
},
{
"level": "b2-4",
"position": 64,
"text": "significance",
"meaning": {
"fr": "importance, signification"
}
},
{
"level": "b2-4",
"position": 65,
"text": "significantly",
"meaning": {
"fr": "considérablement, sensiblement"
}
},
{
"level": "b2-4",
"position": 66,
"text": "similarly",
"meaning": {
"fr": "de même, pareillement"
}
},
{
"level": "b2-4",
"position": 67,
"text": "sin",
"meaning": {
"fr": "péché"
}
},
{
"level": "b2-4",
"position": 68,
"text": "slave",
"meaning": {
"fr": "esclave"
}
},
{
"level": "b2-4",
"position": 69,
"text": "slight",
"meaning": {
"fr": "léger"
}
},
{
"level": "b2-4",
"position": 70,
"text": "slip",
"meaning": {
"fr": "glisser, déraper"
}
},
{
"level": "b2-4",
"position": 71,
"text": "snap",
"meaning": {
"fr": "casser net, claquer"
}
},
{
"level": "b2-4",
"position": 72,
"text": "so-called",
"meaning": {
"fr": "soi-disant, prétendu"
}
},
{
"level": "b2-4",
"position": 73,
"text": "soil",
"meaning": {
"fr": "sol, terre"
}
},
{
"level": "b2-4",
"position": 74,
"text": "solar",
"meaning": {
"fr": "solaire"
}
},
{
"level": "b2-4",
"position": 75,
"text": "somewhat",
"meaning": {
"fr": "quelque peu"
}
},
{
"level": "b2-4",
"position": 76,
"text": "sophisticated",
"meaning": {
"fr": "sophistiqué, raffiné"
}
},
{
"level": "b2-4",
"position": 77,
"text": "soul",
"meaning": {
"fr": "âme"
}
},
{
"level": "b2-4",
"position": 78,
"text": "Soviet",
"meaning": {
"fr": "soviétique"
}
},
{
"level": "b2-4",
"position": 79,
"text": "spending",
"meaning": {
"fr": "dépenses"
}
},
{
"level": "b2-4",
"position": 80,
"text": "spin",
"meaning": {
"fr": "tourner, faire tourner ; (n.) rotation"
}
},
{
"level": "b2-4",
"position": 81,
"text": "spiritual",
"meaning": {
"fr": "spirituel"
}
},
{
"level": "b2-4",
"position": 82,
"text": "spokesman",
"meaning": {
"fr": "porte-parole"
}
},
{
"level": "b2-4",
"position": 83,
"text": "squeeze",
"meaning": {
"fr": "presser, serrer"
}
},
{
"level": "b2-4",
"position": 84,
"text": "stability",
"meaning": {
"fr": "stabilité"
}
},
{
"level": "b2-4",
"position": 85,
"text": "stake",
"meaning": {
"fr": "enjeu, participation, pieu"
}
},
{
"level": "b2-4",
"position": 86,
"text": "standing",
"meaning": {
"fr": "debout ; (n.) réputation, rang"
}
},
{
"level": "b2-4",
"position": 87,
"text": "status",
"meaning": {
"fr": "statut, situation"
}
},
{
"level": "b2-4",
"position": 88,
"text": "stir",
"meaning": {
"fr": "remuer, mélanger"
}
},
{
"level": "b2-4",
"position": 89,
"text": "strategic",
"meaning": {
"fr": "stratégique"
}
},
{
"level": "b2-4",
"position": 90,
"text": "strengthen",
"meaning": {
"fr": "renforcer"
}
},
{
"level": "b2-4",
"position": 91,
"text": "strip",
"meaning": {
"fr": "bande ; (v.) dépouiller, déshabiller"
}
},
{
"level": "b2-4",
"position": 92,
"text": "stroke",
"meaning": {
"fr": "AVC, coup, caresse"
}
},
{
"level": "b2-4",
"position": 93,
"text": "submit",
"meaning": {
"fr": "soumettre, présenter"
}
},
{
"level": "b2-4",
"position": 94,
"text": "subsequent",
"meaning": {
"fr": "ultérieur, suivant"
}
},
{
"level": "b2-4",
"position": 95,
"text": "substantial",
"meaning": {
"fr": "considérable, important"
}
},
{
"level": "b2-4",
"position": 96,
"text": "sue",
"meaning": {
"fr": "poursuivre en justice"
}
},
{
"level": "b2-4",
"position": 97,
"text": "sufficient",
"meaning": {
"fr": "suffisant"
}
},
{
"level": "b2-4",
"position": 98,
"text": "suicide",
"meaning": {
"fr": "suicide"
}
},
{
"level": "b2-4",
"position": 99,
"text": "summit",
"meaning": {
"fr": "sommet (réunion, montagne)"
}
},
{
"level": "b2-4",
"position": 100,
"text": "Supreme",
"meaning": {
"fr": "suprême"
}
},
{
"level": "b2-4",
"position": 101,
"text": "survival",
"meaning": {
"fr": "survie"
}
},
{
"level": "b2-4",
"position": 102,
"text": "survivor",
"meaning": {
"fr": "survivant"
}
},
{
"level": "b2-4",
"position": 103,
"text": "sustain",
"meaning": {
"fr": "soutenir, maintenir, subir"
}
},
{
"level": "b2-4",
"position": 104,
"text": "swear",
"meaning": {
"fr": "jurer"
}
},
{
"level": "b2-4",
"position": 105,
"text": "sweep",
"meaning": {
"fr": "balayer"
}
},
{
"level": "b2-4",
"position": 106,
"text": "swing",
"meaning": {
"fr": "balancer ; (n.) balançoire"
}
},
{
"level": "b2-4",
"position": 107,
"text": "tablespoon",
"meaning": {
"fr": "cuillère à soupe"
}
},
{
"level": "b2-4",
"position": 108,
"text": "tactic",
"meaning": {
"fr": "tactique"
}
},
{
"level": "b2-4",
"position": 109,
"text": "tale",
"meaning": {
"fr": "conte, récit"
}
},
{
"level": "b2-4",
"position": 110,
"text": "taxpayer",
"meaning": {
"fr": "contribuable"
}
},
{
"level": "b2-4",
"position": 111,
"text": "teaspoon",
"meaning": {
"fr": "cuillère à café"
}
},
{
"level": "b2-4",
"position": 112,
"text": "telescope",
"meaning": {
"fr": "télescope"
}
},
{
"level": "b2-4",
"position": 113,
"text": "tendency",
"meaning": {
"fr": "tendance"
}
},
{
"level": "b2-4",
"position": 114,
"text": "tension",
"meaning": {
"fr": "tension"
}
},
{
"level": "b2-4",
"position": 115,
"text": "territory",
"meaning": {
"fr": "territoire"
}
},
{
"level": "b2-4",
"position": 116,
"text": "terror",
"meaning": {
"fr": "terreur"
}
},
{
"level": "b2-4",
"position": 117,
"text": "terrorism",
"meaning": {
"fr": "terrorisme"
}
},
{
"level": "b2-4",
"position": 118,
"text": "terrorist",
"meaning": {
"fr": "terroriste"
}
},
{
"level": "b2-4",
"position": 119,
"text": "testify",
"meaning": {
"fr": "témoigner"
}
},
{
"level": "b2-4",
"position": 120,
"text": "testimony",
"meaning": {
"fr": "témoignage"
}
},
{
"level": "b2-4",
"position": 121,
"text": "testing",
"meaning": {
"fr": "essais, tests"
}
},
{
"level": "b2-4",
"position": 122,
"text": "therapy",
"meaning": {
"fr": "thérapie"
}
},
{
"level": "b2-4",
"position": 123,
"text": "thus",
"meaning": {
"fr": "ainsi, de ce fait"
}
},
{
"level": "b2-4",
"position": 124,
"text": "tissue",
"meaning": {
"fr": "mouchoir en papier, tissu"
}
},
{
"level": "b2-4",
"position": 125,
"text": "tobacco",
"meaning": {
"fr": "tabac"
}
},
{
"level": "b2-4",
"position": 126,
"text": "toss",
"meaning": {
"fr": "lancer, jeter en l'air"
}
},
{
"level": "b2-4",
"position": 127,
"text": "tournament",
"meaning": {
"fr": "tournoi"
}
},
{
"level": "b2-4",
"position": 128,
"text": "trace",
"meaning": {
"fr": "trace ; (v.) retracer, suivre"
}
},
{
"level": "b2-4",
"position": 129,
"text": "tragedy",
"meaning": {
"fr": "tragédie"
}
},
{
"level": "b2-4",
"position": 130,
"text": "trail",
"meaning": {
"fr": "sentier, piste, trace"
}
},
{
"level": "b2-4",
"position": 131,
"text": "transform",
"meaning": {
"fr": "transformer"
}
},
{
"level": "b2-4",
"position": 132,
"text": "transformation",
"meaning": {
"fr": "transformation"
}
},
{
"level": "b2-4",
"position": 133,
"text": "transition",
"meaning": {
"fr": "transition"
}
},
{
"level": "b2-4",
"position": 134,
"text": "treaty",
"meaning": {
"fr": "traité"
}
},
{
"level": "b2-4",
"position": 135,
"text": "tremendous",
"meaning": {
"fr": "énorme, formidable"
}
},
{
"level": "b2-4",
"position": 136,
"text": "tribe",
"meaning": {
"fr": "tribu"
}
},
{
"level": "b2-4",
"position": 137,
"text": "troop",
"meaning": {
"fr": "troupe"
}
},
{
"level": "b2-4",
"position": 138,
"text": "ultimate",
"meaning": {
"fr": "ultime, final"
}
},
{
"level": "b2-4",
"position": 139,
"text": "ultimately",
"meaning": {
"fr": "finalement, en fin de compte"
}
},
{
"level": "b2-4",
"position": 140,
"text": "undergo",
"meaning": {
"fr": "subir"
}
},
{
"level": "b2-4",
"position": 141,
"text": "universal",
"meaning": {
"fr": "universel"
}
},
{
"level": "b2-4",
"position": 142,
"text": "upper",
"meaning": {
"fr": "supérieur"
}
},
{
"level": "b2-4",
"position": 143,
"text": "urban",
"meaning": {
"fr": "urbain"
}
},
{
"level": "b2-4",
"position": 144,
"text": "urge",
"meaning": {
"fr": "exhorter, pousser ; (n.) envie"
}
},
{
"level": "b2-4",
"position": 145,
"text": "utility",
"meaning": {
"fr": "service public, utilité"
}
},
{
"level": "b2-4",
"position": 146,
"text": "variable",
"meaning": {
"fr": "variable"
}
},
{
"level": "b2-4",
"position": 147,
"text": "variation",
"meaning": {
"fr": "variation"
}
},
{
"level": "b2-4",
"position": 148,
"text": "vast",
"meaning": {
"fr": "vaste, immense"
}
},
{
"level": "b2-4",
"position": 149,
"text": "venture",
"meaning": {
"fr": "entreprise, projet risqué"
}
},
{
"level": "b2-4",
"position": 150,
"text": "versus",
"meaning": {
"fr": "contre, face à"
}
},
{
"level": "b2-4",
"position": 151,
"text": "vessel",
"meaning": {
"fr": "navire, vaisseau, récipient"
}
},
{
"level": "b2-4",
"position": 152,
"text": "veteran",
"meaning": {
"fr": "ancien combattant, vétéran"
}
},
{
"level": "b2-4",
"position": 153,
"text": "via",
"meaning": {
"fr": "via, par"
}
},
{
"level": "b2-4",
"position": 154,
"text": "violate",
"meaning": {
"fr": "violer, enfreindre"
}
},
{
"level": "b2-4",
"position": 155,
"text": "violation",
"meaning": {
"fr": "violation, infraction"
}
},
{
"level": "b2-4",
"position": 156,
"text": "virtually",
"meaning": {
"fr": "pratiquement, quasiment"
}
},
{
"level": "b2-4",
"position": 157,
"text": "virtue",
"meaning": {
"fr": "vertu"
}
},
{
"level": "b2-4",
"position": 158,
"text": "vision",
"meaning": {
"fr": "vision, vue"
}
},
{
"level": "b2-4",
"position": 159,
"text": "visual",
"meaning": {
"fr": "visuel"
}
},
{
"level": "b2-4",
"position": 160,
"text": "vital",
"meaning": {
"fr": "vital, essentiel"
}
},
{
"level": "b2-4",
"position": 161,
"text": "voter",
"meaning": {
"fr": "électeur"
}
},
{
"level": "b2-4",
"position": 162,
"text": "vs",
"meaning": {
"fr": "contre (abrév.)"
}
},
{
"level": "b2-4",
"position": 163,
"text": "vulnerable",
"meaning": {
"fr": "vulnérable"
}
},
{
"level": "b2-4",
"position": 164,
"text": "wage",
"meaning": {
"fr": "salaire"
}
},
{
"level": "b2-4",
"position": 165,
"text": "wander",
"meaning": {
"fr": "errer, se promener"
}
},
{
"level": "b2-4",
"position": 166,
"text": "welfare",
"meaning": {
"fr": "bien-être, aide sociale"
}
},
{
"level": "b2-4",
"position": 167,
"text": "whereas",
"meaning": {
"fr": "tandis que, alors que"
}
},
{
"level": "b2-4",
"position": 168,
"text": "widespread",
"meaning": {
"fr": "répandu, généralisé"
}
},
{
"level": "b2-4",
"position": 169,
"text": "wipe",
"meaning": {
"fr": "essuyer"
}
},
{
"level": "b2-4",
"position": 170,
"text": "wire",
"meaning": {
"fr": "fil (métallique), câble"
}
},
{
"level": "b2-4",
"position": 171,
"text": "wisdom",
"meaning": {
"fr": "sagesse"
}
},
{
"level": "b2-4",
"position": 172,
"text": "withdraw",
"meaning": {
"fr": "retirer, se retirer"
}
},
{
"level": "b2-4",
"position": 173,
"text": "works",
"meaning": {
"fr": "usine, œuvres, travaux"
}
},
{
"level": "b2-4",
"position": 174,
"text": "yield",
"meaning": {
"fr": "rendement ; (v.) produire, céder"
}
}
]$en3000$)
       AS w(level text, position integer, text text, meaning jsonb)
  JOIN langues.levels l ON l.language_code = 'en' AND l.slug = w.level;
