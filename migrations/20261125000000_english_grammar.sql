-- Grammar topics of the languages section (`docs/languages.md`): a lesson
-- that explains a point (`on` or `in`?) and fill-in-the-blank sentences to
-- practise it, played as a game by the front end. In `langues`, whose
-- default privileges already give `pulsars_api` its rows' rights.
--
-- And English, the second language taught, which starts with its grammar.

-- One topic of a language. `choices` are the words a blank can take, the
-- same for every sentence of the topic (`{on,in}`). `lesson` is keyed by
-- interface language like the other texts, each value a list of sections:
-- `[{"title": …, "text": …, "examples": [{"text": …, "note": …}]}]`.
CREATE TABLE langues.topics (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    language_code text NOT NULL REFERENCES langues.languages (code) ON UPDATE CASCADE,
    slug text NOT NULL CHECK (slug ~ '^[a-z0-9]+(-[a-z0-9]+)*$'),
    position smallint NOT NULL,
    title jsonb NOT NULL CHECK (title ? 'fr'),
    description jsonb NOT NULL CHECK (description ? 'fr'),
    choices text[] NOT NULL CHECK (cardinality(choices) BETWEEN 2 AND 4),
    lesson jsonb NOT NULL CHECK (jsonb_typeof(lesson -> 'fr') = 'array'),
    UNIQUE (language_code, slug)
);

-- The sentences of a topic. `sentence` has exactly one blank, `___`;
-- `answer` is one of the topic's choices; `phrase` is the group the answer
-- belongs to (`on holiday`), shown with the correction; `explanation` says
-- why, keyed by interface language.
CREATE TABLE langues.topic_exercises (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    topic_id bigint NOT NULL REFERENCES langues.topics (id) ON DELETE CASCADE,
    position integer NOT NULL,
    sentence text NOT NULL CHECK (sentence ~ '^[^_]*___[^_]*$'),
    answer text NOT NULL,
    phrase text NOT NULL,
    explanation jsonb NOT NULL CHECK (explanation ? 'fr'),
    UNIQUE (topic_id, position)
);

INSERT INTO langues.languages (code, name, native_name, script, reading_system, countries, position, published)
VALUES ('en', '{"fr": "Anglais", "en": "English"}', 'English', 'Latn', NULL,
        '{GB,US,IE,CA,AU,NZ}', 2, true)
ON CONFLICT (code) DO NOTHING;

INSERT INTO langues.topics (language_code, slug, position, title, description, choices, lesson)
VALUES ('en', 'on-in', 1,
 '{"fr": "On ou in ?", "en": "On or in?"}',
 '{"fr": "Sur ou dans : le lieu, le temps, les transports et les expressions toutes faites.",
   "en": "On a surface or inside: places, time, transport and set phrases."}',
 '{on,in}',
 $lesson${
 "fr": [
  {
   "title": "In : dedans",
   "text": "In veut dire « dans » : la chose est à l’intérieur d’un espace qui l’entoure, fermé ou délimité. Une pièce, une boîte, un bâtiment, un jardin, une ville, un pays.",
   "examples": [
    {
     "text": "The cat is sleeping in the box.",
     "note": "le chat est à l’intérieur"
    },
    {
     "text": "She’s in the kitchen.",
     "note": "une pièce fermée"
    },
    {
     "text": "The children are in the garden.",
     "note": "un espace délimité"
    },
    {
     "text": "He lives in London.",
     "note": "une ville, un pays : in London, in Spain"
    },
    {
     "text": "We stayed in a hotel.",
     "note": "un bâtiment"
    }
   ]
  },
  {
   "title": "On : sur une surface",
   "text": "On veut dire « sur » : la chose touche une surface, posée dessus ou collée contre, sans être dedans. Une table, une étagère, un mur, une porte, le sol, une page, une liste.",
   "examples": [
    {
     "text": "The keys are on the table.",
     "note": "posées sur la table"
    },
    {
     "text": "The picture is on the wall.",
     "note": "accroché contre le mur"
    },
    {
     "text": "Read the text on page 12.",
     "note": "une page est une surface"
    },
    {
     "text": "Your name is on the list.",
     "note": "écrit sur la liste"
    },
    {
     "text": "She’s on the cover of the magazine.",
     "note": "la couverture est une surface"
    }
   ]
  },
  {
   "title": "Le temps : on pour un jour, in pour une période",
   "text": "On devant un jour : un jour de la semaine, une date, un anniversaire, une fête d’un jour. In devant une période plus longue : un mois, une saison, une année, une décennie, un siècle, et les parties de la journée. Quand un jour est précisé, c’est le jour qui décide : on Friday morning.",
   "examples": [
    {
     "text": "See you on Monday.",
     "note": "un jour de la semaine"
    },
    {
     "text": "The party is on June 12th.",
     "note": "une date précise"
    },
    {
     "text": "He came on Christmas Eve.",
     "note": "une fête d’un jour"
    },
    {
     "text": "We met in June.",
     "note": "un mois"
    },
    {
     "text": "We go swimming in summer.",
     "note": "une saison"
    },
    {
     "text": "I was born in 2001.",
     "note": "une année ; in the 1990s, in the 21st century"
    },
    {
     "text": "She works in the morning.",
     "note": "une partie de la journée"
    },
    {
     "text": "I’ll call you on Friday morning.",
     "note": "jour + moment : le jour l’emporte"
    }
   ]
  },
  {
   "title": "In + une durée : dans (au futur)",
   "text": "In suivi d’une durée dit dans combien de temps quelque chose arrivera. Et le passé, le futur : in the past, in the future.",
   "examples": [
    {
     "text": "I’ll be back in ten minutes.",
     "note": "dans dix minutes"
    },
    {
     "text": "We’ll finish it in a few days.",
     "note": "dans quelques jours"
    },
    {
     "text": "People were poorer in the past.",
     "note": "dans le passé"
    }
   ]
  },
  {
   "title": "Les transports",
   "text": "On pour les transports où l’on peut se lever et marcher : bus, train, avion, bateau. In pour une voiture ou un taxi, petits véhicules où l’on reste assis.",
   "examples": [
    {
     "text": "Get on the bus!",
     "note": "on the bus, on the train, on a plane"
    },
    {
     "text": "Get in the car!",
     "note": "in a car, in a taxi"
    }
   ]
  },
  {
   "title": "Le corps, l’eau, les étages, les coins",
   "text": "Ce qui est sur la peau prend on, ce qui est à l’intérieur du corps prend in. Dans l’eau : in ; à sa surface ou au bord : on. Les étages prennent on. Le coin d’une pièce se voit de l’intérieur (in), celui d’une rue de l’extérieur (on).",
   "examples": [
    {
     "text": "There’s a ring on her finger.",
     "note": "porté sur le corps"
    },
    {
     "text": "I have a pain in my back.",
     "note": "à l’intérieur du corps"
    },
    {
     "text": "We swam in the lake.",
     "note": "dans l’eau"
    },
    {
     "text": "There’s a boat on the lake.",
     "note": "à la surface de l’eau"
    },
    {
     "text": "Paris is on the Seine.",
     "note": "au bord d’un fleuve"
    },
    {
     "text": "Their house is on the island.",
     "note": "sur une île ; on the coast : sur la côte"
    },
    {
     "text": "They live on the fifth floor.",
     "note": "un étage"
    },
    {
     "text": "The lamp is in the corner of the room.",
     "note": "coin intérieur"
    },
    {
     "text": "There’s a café on the corner of the street.",
     "note": "coin de rue"
    }
   ]
  },
  {
   "title": "Langue, couleur, outil : in",
   "text": "On parle, on écrit, on s’habille « dans » quelque chose : une langue, une couleur, un vêtement, un stylo.",
   "examples": [
    {
     "text": "Say it in English.",
     "note": "une langue"
    },
    {
     "text": "She’s dressed in red.",
     "note": "habillé en"
    },
    {
     "text": "Write in pen, not pencil.",
     "note": "au stylo, au crayon"
    }
   ]
  },
  {
   "title": "Après un verbe, un adjectif ou un nom",
   "text": "Certains mots appellent toujours la même préposition : on les apprend avec elle.",
   "examples": [
    {
     "text": "It depends on the weather.",
     "note": "depend on"
    },
    {
     "text": "Congratulations on your new job!",
     "note": "congratulations on"
    },
    {
     "text": "I’m working on a new project.",
     "note": "work on"
    },
    {
     "text": "Turn on the light.",
     "note": "turn on : allumer (turn off : éteindre) ; Is the TV on? : allumée"
    },
    {
     "text": "You can rely on me.",
     "note": "rely on, insist on"
    },
    {
     "text": "He believes in ghosts.",
     "note": "believe in, succeed in"
    },
    {
     "text": "I’m interested in music.",
     "note": "interested in"
    },
    {
     "text": "She took part in the race.",
     "note": "take part in"
    },
    {
     "text": "There’s an increase in prices.",
     "note": "an increase in, a fall in"
    }
   ]
  },
  {
   "title": "Les expressions toutes faites",
   "text": "Elles ne se raisonnent pas : on les retient telles quelles.",
   "examples": [
    {
     "text": "He’s on holiday.",
     "note": "en vacances"
    },
    {
     "text": "He’s on the phone.",
     "note": "au téléphone ; on TV, on the radio, on the internet"
    },
    {
     "text": "I saw it on YouTube.",
     "note": "un site, une appli, un réseau : on Instagram, on Amazon"
    },
    {
     "text": "on a trip, on strike, on call, on my way",
     "note": "en voyage, en grève, d’astreinte, en chemin"
    },
    {
     "text": "The train arrived on time.",
     "note": "on time : à l’heure ; in time : à temps"
    },
    {
     "text": "The shop is on the left.",
     "note": "on the left, on the right"
    },
    {
     "text": "on fire, on a diet, on sale, on duty, on purpose",
     "note": "en feu, au régime, en promo, de garde, exprès"
    },
    {
     "text": "She’s still in bed.",
     "note": "au lit ; in hospital : à l’hôpital"
    },
    {
     "text": "I like walking in the rain.",
     "note": "la météo nous entoure : in the rain, in the sun"
    },
    {
     "text": "in my opinion, in a hurry, in love, in trouble",
     "note": "à mon avis, pressé, amoureux, dans le pétrin"
    },
    {
     "text": "in a meeting, in charge of",
     "note": "en réunion, responsable de"
    },
    {
     "text": "in a good mood, in danger, in stock, in cash, in debt",
     "note": "de bonne humeur, en danger, en stock, en espèces, endetté"
    },
    {
     "text": "in class, in control of, in my view, in the beginning",
     "note": "en cours, maître de, à mon avis, au début"
    }
   ]
  },
  {
   "title": "Les pièges",
   "text": "Le même genre de chose peut changer de préposition selon qu’on est posé dessus ou entouré.",
   "examples": [
    {
     "text": "on a chair, in an armchair",
     "note": "sur une chaise, mais au creux du fauteuil"
    },
    {
     "text": "on page 12, in the book",
     "note": "la page est une surface, le livre contient le texte"
    },
    {
     "text": "on the bus, in the car",
     "note": "on marche dans un bus, on reste assis en voiture"
    },
    {
     "text": "Who’s the man in the photo?",
     "note": "on est dans une photo, un tableau, un film"
    }
   ]
  }
 ],
 "en": [
  {
   "title": "In: inside",
   "text": "In means the thing is inside a space that surrounds it, closed or with limits: a room, a box, a building, a garden, a town, a country.",
   "examples": [
    {
     "text": "The cat is sleeping in the box.",
     "note": "the cat is inside"
    },
    {
     "text": "She’s in the kitchen.",
     "note": "an enclosed room"
    },
    {
     "text": "The children are in the garden.",
     "note": "an area with limits"
    },
    {
     "text": "He lives in London.",
     "note": "a city, a country: in London, in Spain"
    },
    {
     "text": "We stayed in a hotel.",
     "note": "a building"
    }
   ]
  },
  {
   "title": "On: on a surface",
   "text": "On means the thing touches a surface, resting on it or stuck against it, without being inside: a table, a shelf, a wall, a door, the floor, a page, a list.",
   "examples": [
    {
     "text": "The keys are on the table.",
     "note": "resting on the table"
    },
    {
     "text": "The picture is on the wall.",
     "note": "hanging against the wall"
    },
    {
     "text": "Read the text on page 12.",
     "note": "a page is a surface"
    },
    {
     "text": "Your name is on the list.",
     "note": "written on the list"
    },
    {
     "text": "She’s on the cover of the magazine.",
     "note": "a cover is a surface"
    }
   ]
  },
  {
   "title": "Time: on for a day, in for a period",
   "text": "On before a day: a day of the week, a date, a birthday, a one-day holiday. In before a longer period: a month, a season, a year, a decade, a century, and the parts of the day. When a day is named, the day decides: on Friday morning.",
   "examples": [
    {
     "text": "See you on Monday.",
     "note": "a day of the week"
    },
    {
     "text": "The party is on June 12th.",
     "note": "an exact date"
    },
    {
     "text": "He came on Christmas Eve.",
     "note": "a one-day holiday"
    },
    {
     "text": "We met in June.",
     "note": "a month"
    },
    {
     "text": "We go swimming in summer.",
     "note": "a season"
    },
    {
     "text": "I was born in 2001.",
     "note": "a year; in the 1990s, in the 21st century"
    },
    {
     "text": "She works in the morning.",
     "note": "a part of the day"
    },
    {
     "text": "I’ll call you on Friday morning.",
     "note": "day + part of the day: the day wins"
    }
   ]
  },
  {
   "title": "In + a length of time: from now",
   "text": "In followed by a length of time says how long until something happens. And the past and the future: in the past, in the future.",
   "examples": [
    {
     "text": "I’ll be back in ten minutes.",
     "note": "ten minutes from now"
    },
    {
     "text": "We’ll finish it in a few days.",
     "note": "a few days from now"
    },
    {
     "text": "People were poorer in the past.",
     "note": "in the past"
    }
   ]
  },
  {
   "title": "Transport",
   "text": "On for transport you can stand up and walk in: bus, train, plane, ship. In for a car or a taxi, small vehicles where you stay seated.",
   "examples": [
    {
     "text": "Get on the bus!",
     "note": "on the bus, on the train, on a plane"
    },
    {
     "text": "Get in the car!",
     "note": "in a car, in a taxi"
    }
   ]
  },
  {
   "title": "The body, water, floors, corners",
   "text": "What is on the skin takes on, what is inside the body takes in. In the water: in; on its surface or beside it: on. Floors take on. The corner of a room is seen from inside (in), the corner of a street from outside (on).",
   "examples": [
    {
     "text": "There’s a ring on her finger.",
     "note": "worn on the body"
    },
    {
     "text": "I have a pain in my back.",
     "note": "inside the body"
    },
    {
     "text": "We swam in the lake.",
     "note": "in the water"
    },
    {
     "text": "There’s a boat on the lake.",
     "note": "on the surface of the water"
    },
    {
     "text": "Paris is on the Seine.",
     "note": "beside a river"
    },
    {
     "text": "Their house is on the island.",
     "note": "on an island; on the coast"
    },
    {
     "text": "They live on the fifth floor.",
     "note": "a floor"
    },
    {
     "text": "The lamp is in the corner of the room.",
     "note": "inside corner"
    },
    {
     "text": "There’s a café on the corner of the street.",
     "note": "street corner"
    }
   ]
  },
  {
   "title": "Language, colour, tool: in",
   "text": "You speak, write and dress “in” something: a language, a colour, clothes, a pen.",
   "examples": [
    {
     "text": "Say it in English.",
     "note": "a language"
    },
    {
     "text": "She’s dressed in red.",
     "note": "wearing"
    },
    {
     "text": "Write in pen, not pencil.",
     "note": "in pen, in pencil"
    }
   ]
  },
  {
   "title": "After a verb, an adjective or a noun",
   "text": "Some words always call for the same preposition: learn them together.",
   "examples": [
    {
     "text": "It depends on the weather.",
     "note": "depend on"
    },
    {
     "text": "Congratulations on your new job!",
     "note": "congratulations on"
    },
    {
     "text": "I’m working on a new project.",
     "note": "work on"
    },
    {
     "text": "Turn on the light.",
     "note": "turn on (the opposite: turn off); Is the TV on?: switched on"
    },
    {
     "text": "You can rely on me.",
     "note": "rely on, insist on"
    },
    {
     "text": "He believes in ghosts.",
     "note": "believe in, succeed in"
    },
    {
     "text": "I’m interested in music.",
     "note": "interested in"
    },
    {
     "text": "She took part in the race.",
     "note": "take part in"
    },
    {
     "text": "There’s an increase in prices.",
     "note": "an increase in, a fall in"
    }
   ]
  },
  {
   "title": "Set phrases",
   "text": "These cannot be reasoned out: learn them as they are.",
   "examples": [
    {
     "text": "He’s on holiday.",
     "note": "on holiday"
    },
    {
     "text": "He’s on the phone.",
     "note": "on the phone; on TV, on the radio, on the internet"
    },
    {
     "text": "I saw it on YouTube.",
     "note": "a website, an app, a network: on Instagram, on Amazon"
    },
    {
     "text": "on a trip, on strike, on call, on my way",
     "note": "set phrases with on"
    },
    {
     "text": "The train arrived on time.",
     "note": "on time: punctual; in time: early enough"
    },
    {
     "text": "The shop is on the left.",
     "note": "on the left, on the right"
    },
    {
     "text": "on fire, on a diet, on sale, on duty, on purpose",
     "note": "burning, dieting, discounted, working, deliberately"
    },
    {
     "text": "She’s still in bed.",
     "note": "in bed; in hospital"
    },
    {
     "text": "I like walking in the rain.",
     "note": "weather surrounds you: in the rain, in the sun"
    },
    {
     "text": "in my opinion, in a hurry, in love, in trouble",
     "note": "set phrases with in"
    },
    {
     "text": "in a meeting, in charge of",
     "note": "set phrases with in"
    },
    {
     "text": "in a good mood, in danger, in stock, in cash, in debt",
     "note": "set phrases with in"
    },
    {
     "text": "in class, in control of, in my view, in the beginning",
     "note": "set phrases with in"
    }
   ]
  },
  {
   "title": "Traps",
   "text": "The same kind of thing can change preposition depending on whether you rest on it or are surrounded by it.",
   "examples": [
    {
     "text": "on a chair, in an armchair",
     "note": "on top of a chair, sunk into an armchair"
    },
    {
     "text": "on page 12, in the book",
     "note": "a page is a surface, a book holds the text"
    },
    {
     "text": "on the bus, in the car",
     "note": "you walk around a bus, you stay seated in a car"
    },
    {
     "text": "Who’s the man in the photo?",
     "note": "you are in a photo, a painting, a film"
    }
   ]
  }
 ]
}$lesson$::jsonb)
ON CONFLICT (language_code, slug) DO NOTHING;

INSERT INTO langues.topic_exercises (topic_id, position, sentence, answer, phrase, explanation)
SELECT t.id, e.position, e.sentence, e.answer, e.phrase, jsonb_build_object('fr', e.fr, 'en', e.en)
  FROM langues.topics t
 CROSS JOIN (VALUES
    (1, 'I was born ___ 2001.', 'in', 'in 2001',
     'On dit « in » pour une année, car c’est une longue période. → in 2001',
     'A year is a long period, so “in”. → in 2001'),
    (2, 'See you ___ Monday.', 'on', 'on Monday',
     'On dit « on » pour un jour de la semaine. → on Monday',
     'A day of the week takes “on”. → on Monday'),
    (3, 'The keys are ___ the table.', 'on', 'on the table',
     'Les clés sont posées SUR la table (une surface), donc « on ».',
     'The keys rest ON the table (a surface), so “on”.'),
    (4, 'She’s ___ the kitchen.', 'in', 'in the kitchen',
     'Elle est DANS la cuisine (une pièce fermée), donc « in ».',
     'She is INSIDE the kitchen (an enclosed room), so “in”.'),
    (5, 'We met ___ June.', 'in', 'in June',
     'Un mois est une longue période, donc « in ». → in June',
     'A month is a long period, so “in”. → in June'),
    (6, 'The party is ___ June 12th.', 'on', 'on June 12th',
     'Une date précise (jour + mois) prend toujours « on ». → on June 12th',
     'An exact date (day + month) always takes “on”. → on June 12th'),
    (7, 'He lives ___ London.', 'in', 'in London',
     'On est DANS une ville, donc « in ». → in London',
     'You are INSIDE a city, so “in”. → in London'),
    (8, 'There’s a fly ___ the window.', 'on', 'on the window',
     'La mouche est posée SUR la vitre (une surface), donc « on ».',
     'The fly sits ON the glass (a surface), so “on”.'),
    (9, 'I’ll be back ___ ten minutes.', 'in', 'in ten minutes',
     '« in + durée » veut dire « dans 10 minutes » (un délai), donc « in ».',
     '“in + a length of time” means that long from now, so “in”.'),
    (10, 'The cat is sleeping ___ the box.', 'in', 'in the box',
     'Le chat est À L’INTÉRIEUR de la boîte, donc « in ».',
     'The cat is INSIDE the box, so “in”.'),
    (11, 'He’s ___ the phone.', 'on', 'on the phone',
     'Expression figée : on dit toujours « on the phone » (au téléphone).',
     'Set phrase: always “on the phone”.'),
    (12, 'I watched it ___ TV.', 'on', 'on TV',
     'Expression figée : on dit toujours « on TV » (à la télé).',
     'Set phrase: always “on TV”.'),
    (13, 'I found it ___ the internet.', 'on', 'on the internet',
     'Expression figée : on dit toujours « on the internet ».',
     'Set phrase: always “on the internet”.'),
    (14, 'Read the text ___ page 12.', 'on', 'on page 12',
     'Une page est une surface, le texte est écrit DESSUS → on page 12.',
     'A page is a surface, the text is written ON it → on page 12.'),
    (15, 'The answer is ___ the book.', 'in', 'in the book',
     'La réponse est DANS le livre (on l’ouvre pour la trouver), donc « in ».',
     'The answer is INSIDE the book (you open it to find it), so “in”.'),
    (16, 'We go swimming ___ summer.', 'in', 'in summer',
     'Une saison est une longue période, donc « in ». → in summer',
     'A season is a long period, so “in”. → in summer'),
    (17, 'She works ___ the morning.', 'in', 'in the morning',
     'Une partie de la journée (matin, après-midi, soir) prend « in ». → in the morning',
     'A part of the day (morning, afternoon, evening) takes “in”. → in the morning'),
    (18, 'I’ll call you ___ Friday morning.', 'on', 'on Friday morning',
     'Quand un jour est précisé, c’est le jour qui décide → on Friday morning (pas « in »).',
     'When a day is named, the day decides → on Friday morning (not “in”).'),
    (19, 'They live ___ the fifth floor.', 'on', 'on the fifth floor',
     'Pour les étages, on dit toujours « on ». → on the fifth floor',
     'Floors of a building always take “on”. → on the fifth floor'),
    (20, 'The children are ___ the garden.', 'in', 'in the garden',
     'Le jardin est un espace délimité (avec des limites), on est DEDANS → in the garden.',
     'A garden is an area with limits, you are INSIDE it → in the garden.'),
    (21, 'He’s sitting ___ the floor.', 'on', 'on the floor',
     'Il est assis SUR le sol (une surface), donc « on ».',
     'He sits ON the floor (a surface), so “on”.'),
    (22, 'She’s still ___ bed.', 'in', 'in bed',
     'Être au lit = être sous les draps, DANS le lit → in bed.',
     'In bed = under the sheets, INSIDE the bed → in bed.'),
    (23, 'The picture is ___ the wall.', 'on', 'on the wall',
     'Le tableau est accroché SUR le mur (surface), donc « on ».',
     'The picture hangs ON the wall (a surface), so “on”.'),
    (24, 'There’s milk ___ the fridge.', 'in', 'in the fridge',
     'Le lait est À L’INTÉRIEUR du frigo, donc « in ».',
     'The milk is INSIDE the fridge, so “in”.'),
    (25, 'He’s ___ holiday in Spain.', 'on', 'on holiday',
     'Expression figée : « on holiday » = en vacances.',
     'Set phrase: “on holiday”.'),
    (26, 'She’s ___ Spain right now.', 'in', 'in Spain',
     'On est DANS un pays, donc « in ». → in Spain',
     'You are INSIDE a country, so “in”. → in Spain'),
    (27, 'Get ___ the bus!', 'on', 'on the bus',
     'Bus, train, avion, bateau : on peut se lever et marcher dedans → « on ».',
     'Bus, train, plane, ship: you can stand up and walk around → “on”.'),
    (28, 'Get ___ the car!', 'in', 'in the car',
     'Voiture et taxi : petits véhicules où on reste assis → « in ».',
     'Car and taxi: small vehicles where you stay seated → “in”.'),
    (29, 'I’m ___ the train.', 'on', 'on the train',
     'Le train est un transport public où on peut marcher → « on ».',
     'A train is public transport you can walk around in → “on”.'),
    (30, 'He’s ___ a taxi.', 'in', 'in a taxi',
     'Un taxi est un petit véhicule (comme une voiture) → « in ».',
     'A taxi is a small vehicle (like a car) → “in”.'),
    (31, 'I like walking ___ the rain.', 'in', 'in the rain',
     'Pour la météo, on est « dans » le temps qu’il fait → in the rain / in the sun.',
     'Weather surrounds you → in the rain / in the sun.'),
    (32, '___ my opinion, it’s wrong.', 'in', 'in my opinion',
     'Expression figée : « in my opinion » = à mon avis.',
     'Set phrase: “in my opinion”.'),
    (33, 'Who’s the man ___ the photo?', 'in', 'in the photo',
     'Une personne sur une photo est « dans » l’image → in the photo / in the picture.',
     'A person in a photo is inside the image → in the photo / in the picture.'),
    (34, 'She’s dressed ___ red.', 'in', 'dressed in red',
     'Pour dire « habillé en » une couleur ou un vêtement → dressed in red.',
     'Wearing a colour or clothes → dressed in red.'),
    (35, 'Say it ___ English.', 'in', 'in English',
     'Pour une langue, on dit toujours « in ». → in English',
     'A language always takes “in”. → in English'),
    (36, 'It depends ___ the weather.', 'on', 'depends on',
     'Le verbe « depend » est toujours suivi de « on ». → depend on',
     'The verb “depend” is always followed by “on”. → depend on'),
    (37, 'Congratulations ___ your new job!', 'on', 'congratulations on',
     '« Congratulations » est toujours suivi de « on ». → congratulations on',
     '“Congratulations” is always followed by “on”.'),
    (38, 'I’m interested ___ music.', 'in', 'interested in',
     '« Interested » est toujours suivi de « in ». → interested in',
     '“Interested” is always followed by “in”.'),
    (39, 'She took part ___ the race.', 'in', 'take part in',
     '« Take part » est toujours suivi de « in ». → take part in',
     '“Take part” is always followed by “in”.'),
    (40, 'There’s an increase ___ prices.', 'in', 'an increase in',
     '« An increase » (une hausse) est suivi de « in ». → an increase in prices',
     '“An increase” is followed by “in”. → an increase in prices'),
    (41, 'The train arrived ___ time.', 'on', 'on time',
     '« On time » = à l’heure pile (ne pas confondre avec « in time » = à temps).',
     '“On time” = punctual (not to be confused with “in time” = early enough).'),
    (42, 'Your name is ___ the list.', 'on', 'on the list',
     'Un nom écrit SUR une liste → on the list.',
     'A name written ON a list → on the list.'),
    (43, 'There’s a mistake ___ the first line.', 'in', 'in the first line',
     'Une erreur se trouve DANS un texte, une ligne, un document → in the first line.',
     'A mistake is INSIDE a text, a line, a document → in the first line.'),
    (44, 'The shop is ___ the left.', 'on', 'on the left',
     'Pour les directions, on dit « on the left / on the right ».',
     'Directions: “on the left / on the right”.'),
    (45, 'The lamp is ___ the corner of the room.', 'in', 'in the corner of the room',
     'Le coin d’une pièce, vu de l’intérieur : on est DEDANS → in the corner of the room.',
     'The corner of a room, seen from inside: you are IN it → in the corner of the room.'),
    (46, 'There’s a café ___ the corner of the street.', 'on', 'on the corner of the street',
     'Le coin d’une rue, vu de l’extérieur → on (ou at) the corner of the street.',
     'The corner of a street, seen from outside → on (or at) the corner of the street.'),
    (47, 'My office is ___ the city centre.', 'in', 'in the city centre',
     'Le centre-ville est une zone, on est DEDANS → in the city centre.',
     'The city centre is an area, you are INSIDE it → in the city centre.'),
    (48, 'There’s a ring ___ her finger.', 'on', 'on her finger',
     'Une bague est portée SUR le doigt (surface du corps), donc « on ».',
     'A ring is worn ON the finger (the body’s surface), so “on”.'),
    (49, 'I have a pain ___ my back.', 'in', 'a pain in my back',
     'Une douleur est à l’intérieur du corps → a pain in my back.',
     'Pain is inside the body → a pain in my back.'),
    (50, 'He has a scar ___ his face.', 'on', 'on his face',
     'Une cicatrice est SUR la peau, à la surface → on his face.',
     'A scar is ON the skin, at the surface → on his face.'),
    (51, 'It’s ___ the top shelf.', 'on', 'on the top shelf',
     'L’objet est posé SUR l’étagère, donc « on ».',
     'The thing rests ON the shelf, so “on”.'),
    (52, 'There are no clouds ___ the sky.', 'in', 'in the sky',
     'Le ciel est vu comme un grand espace → in the sky.',
     'The sky is seen as a vast space → in the sky.'),
    (53, 'We stayed ___ a hotel.', 'in', 'in a hotel',
     'On loge DANS un hôtel (un bâtiment), donc « in ».',
     'You stay INSIDE a hotel (a building), so “in”.'),
    (54, 'The house is ___ fire!', 'on', 'on fire',
     'Expression figée : « on fire » = en feu.',
     'Set phrase: “on fire”.'),
    (55, 'Put it ___ your pocket.', 'in', 'in your pocket',
     'On met quelque chose DANS sa poche, donc « in ».',
     'You put something INSIDE your pocket, so “in”.'),
    (56, 'Sorry, I’m ___ a hurry.', 'in', 'in a hurry',
     'Expression figée : « in a hurry » = pressé.',
     'Set phrase: “in a hurry”.'),
    (57, 'She’s ___ love with him.', 'in', 'in love',
     'Expression figée : « in love » = amoureux.',
     'Set phrase: “in love”.'),
    (58, 'He’s ___ a diet.', 'on', 'on a diet',
     'Expression figée : « on a diet » = au régime.',
     'Set phrase: “on a diet”.'),
    (59, 'These shoes are ___ sale.', 'on', 'on sale',
     'Expression figée : « on sale » = en promo / en vente.',
     'Set phrase: “on sale”.'),
    (60, 'The book is ___ the shelf.', 'on', 'on the shelf',
     'Le livre est posé SUR l’étagère, donc « on ».',
     'The book rests ON the shelf, so “on”.'),
    (61, 'Write ___ pen, not pencil.', 'in', 'in pen',
     'Pour l’outil d’écriture, on dit « in pen / in pencil » (au stylo / au crayon).',
     'For what you write with: “in pen / in pencil”.'),
    (62, 'I’m ___ trouble.', 'in', 'in trouble',
     'Expression figée : « in trouble » = dans le pétrin.',
     'Set phrase: “in trouble”.'),
    (63, 'Turn ___ the light, please.', 'on', 'turn on',
     '« Turn on » = allumer (le contraire est « turn off »).',
     '“Turn on” = switch on (the opposite is “turn off”).'),
    (64, 'He came ___ Christmas Eve.', 'on', 'on Christmas Eve',
     'Christmas Eve (le 24 décembre) est un jour précis, donc « on ».',
     'Christmas Eve (24 December) is a single day, so “on”.'),
    (65, 'Leaves fall ___ autumn.', 'in', 'in autumn',
     'Une saison est une longue période, donc « in ». → in autumn',
     'A season is a long period, so “in”. → in autumn'),
    (66, 'That song was popular ___ the 1990s.', 'in', 'in the 1990s',
     'Une décennie est une longue période, donc « in ». → in the 1990s',
     'A decade is a long period, so “in”. → in the 1990s'),
    (67, 'We live ___ the 21st century.', 'in', 'in the 21st century',
     'Un siècle est une longue période, donc « in ». → in the 21st century',
     'A century is a long period, so “in”. → in the 21st century'),
    (68, 'The shops are closed ___ New Year’s Day.', 'on', 'on New Year’s Day',
     'New Year’s Day est un jour précis, donc « on ».',
     'New Year’s Day is a single day, so “on”.'),
    (69, 'I got a cake ___ my birthday.', 'on', 'on my birthday',
     'Un anniversaire est un jour, donc « on ». → on my birthday',
     'A birthday is a day, so “on”. → on my birthday'),
    (70, 'The museum is open ___ weekdays.', 'on', 'on weekdays',
     'Les jours de semaine sont des jours, donc « on ». → on weekdays',
     'Weekdays are days, so “on”. → on weekdays'),
    (71, 'I study ___ the evening.', 'in', 'in the evening',
     'Une partie de la journée prend « in ». → in the evening',
     'A part of the day takes “in”. → in the evening'),
    (72, 'Let’s go out ___ Tuesday evening.', 'on', 'on Tuesday evening',
     'Jour + moment de la journée : c’est le jour qui décide → on Tuesday evening.',
     'Day + part of the day: the day decides → on Tuesday evening.'),
    (73, 'We had a picnic ___ a sunny day.', 'on', 'on a sunny day',
     'Même décrit (« un jour ensoleillé »), ça reste un jour → on a sunny day.',
     'Even described (“a sunny day”), it is still a day → on a sunny day.'),
    (74, 'People were poorer ___ the past.', 'in', 'in the past',
     'Expression : « in the past » = dans le passé.',
     'Set phrase: “in the past”.'),
    (75, 'Robots will do more jobs ___ the future.', 'in', 'in the future',
     'Expression : « in the future » = dans le futur.',
     'Set phrase: “in the future”.'),
    (76, 'My grandfather is ___ hospital.', 'in', 'in hospital',
     'On est DANS l’hôpital (bâtiment) → in hospital.',
     'You are INSIDE the hospital (a building) → in hospital.'),
    (77, 'I heard the news ___ the radio.', 'on', 'on the radio',
     'Comme « on TV », on dit « on the radio ».',
     'Like “on TV”: “on the radio”.'),
    (78, 'I read it ___ a newspaper.', 'in', 'in a newspaper',
     'Un article est DANS un journal (comme dans un livre ou un magazine) → in a newspaper.',
     'An article is INSIDE a newspaper (like a book or a magazine) → in a newspaper.'),
    (79, 'There’s a poster ___ the door.', 'on', 'on the door',
     'L’affiche est collée SUR la porte (surface), donc « on ».',
     'The poster is stuck ON the door (a surface), so “on”.'),
    (80, 'The key is still ___ the lock.', 'in', 'in the lock',
     'La clé est insérée DANS la serrure, donc « in ».',
     'The key is pushed INTO the lock, so “in”.'),
    (81, 'We spent the day ___ the beach.', 'on', 'on the beach',
     'La plage est une surface sur laquelle on se pose → on the beach.',
     'A beach is a surface you settle on → on the beach.'),
    (82, 'We swam ___ the lake.', 'in', 'in the lake',
     'On nage DANS l’eau du lac, donc « in ».',
     'You swim INSIDE the water of the lake, so “in”.'),
    (83, 'There’s a boat ___ the lake.', 'on', 'on the lake',
     'Le bateau flotte SUR la surface de l’eau, donc « on ».',
     'The boat floats ON the surface of the water, so “on”.'),
    (84, 'Paris is ___ the Seine.', 'on', 'on the Seine',
     'Une ville située au bord d’un fleuve → on the Seine.',
     'A city beside a river → on the Seine.'),
    (85, 'Fish live ___ water.', 'in', 'in water',
     'Les poissons sont DANS l’eau, donc « in ».',
     'Fish are INSIDE the water, so “in”.'),
    (86, 'We sat ___ the second row.', 'in', 'in the second row',
     'On est assis DANS une rangée → in the second row.',
     'You sit IN a row → in the second row.'),
    (87, 'He’s relaxing ___ an armchair.', 'in', 'in an armchair',
     'Dans un fauteuil, on est entouré par les accoudoirs, on est « dedans » → in an armchair.',
     'In an armchair, the arms surround you, you are “inside” → in an armchair.'),
    (88, 'She’s sitting ___ a chair.', 'on', 'on a chair',
     'Sur une chaise simple, on est assis DESSUS → on a chair.',
     'On a plain chair, you sit ON top → on a chair.'),
    (89, 'I can’t talk, I’m ___ a meeting.', 'in', 'in a meeting',
     'Expression : « in a meeting » = en réunion.',
     'Set phrase: “in a meeting”.'),
    (90, 'The nurse is ___ duty tonight.', 'on', 'on duty',
     'Expression figée : « on duty » = de service / de garde.',
     'Set phrase: “on duty”.'),
    (91, 'I’m working ___ a new project.', 'on', 'working on',
     '« Work on something » = travailler sur quelque chose.',
     '“Work on something”.'),
    (92, 'She’s ___ charge of the team.', 'in', 'in charge of',
     'Expression : « in charge of » = responsable de.',
     'Set phrase: “in charge of”.'),
    (93, 'He did it ___ purpose.', 'on', 'on purpose',
     'Expression figée : « on purpose » = exprès.',
     'Set phrase: “on purpose”.'),
    (94, 'We’ll finish it ___ a few days.', 'in', 'in a few days',
     '« in + durée » = dans quelques jours (un délai), donc « in ».',
     '“in + a length of time” = that long from now, so “in”.'),
    (95, 'The film is ___ French with subtitles.', 'in', 'in French',
     'Pour une langue, on dit toujours « in ». → in French',
     'A language always takes “in”. → in French'),
    (96, 'She’s ___ the cover of the magazine.', 'on', 'on the cover',
     'La couverture est une surface, elle apparaît DESSUS → on the cover.',
     'The cover is a surface, she appears ON it → on the cover.'),
    (97, 'There’s a stain ___ your shirt.', 'on', 'on your shirt',
     'La tache est SUR le tissu (surface), donc « on ».',
     'The stain is ON the fabric (a surface), so “on”.'),
    (98, 'I left my phone ___ the car.', 'in', 'in the car',
     'Le téléphone est DANS la voiture (petit véhicule), donc « in ».',
     'The phone is INSIDE the car (a small vehicle), so “in”.'),
    (99, 'The shop is closed ___ Sundays.', 'on', 'on Sundays',
     'Les dimanches sont des jours, donc « on ». → on Sundays',
     'Sundays are days, so “on”. → on Sundays'),
    (100, 'She lives ___ a small village.', 'in', 'in a small village',
     'On habite DANS un village (comme dans une ville), donc « in ».',
     'You live INSIDE a village (like a town), so “in”.'),
    (101, 'I’ll see you ___ Wednesday.', 'on', 'on Wednesday',
     'Un jour de la semaine prend toujours « on ». → on Wednesday',
     'A day of the week always takes “on”. → on Wednesday'),
    (102, 'She was born ___ 1995.', 'in', 'in 1995',
     'Une année est une longue période, donc « in ». → in 1995',
     'A year is a long period, so “in”. → in 1995'),
    (103, 'There’s a book ___ the desk.', 'on', 'on the desk',
     'Le livre est posé SUR le bureau (une surface), donc « on ».',
     'The book rests ON the desk (a surface), so “on”.'),
    (104, 'He’s waiting ___ the car.', 'in', 'in the car',
     'Il est DANS la voiture (petit véhicule), donc « in ».',
     'He is INSIDE the car (a small vehicle), so “in”.'),
    (105, 'The concert is ___ August.', 'in', 'in August',
     'Un mois est une longue période, donc « in ». → in August',
     'A month is a long period, so “in”. → in August'),
    (106, 'My exam is ___ May 3rd.', 'on', 'on May 3rd',
     'Une date précise prend toujours « on ». → on May 3rd',
     'An exact date always takes “on”. → on May 3rd'),
    (107, 'They live ___ Berlin.', 'in', 'in Berlin',
     'On est DANS une ville, donc « in ». → in Berlin',
     'You are INSIDE a city, so “in”. → in Berlin'),
    (108, 'There’s a sticker ___ my laptop.', 'on', 'on my laptop',
     'L’autocollant est collé SUR l’ordinateur (surface), donc « on ».',
     'The sticker is stuck ON the laptop (a surface), so “on”.'),
    (109, 'The film starts ___ five minutes.', 'in', 'in five minutes',
     '« in + durée » = dans 5 minutes (un délai), donc « in ».',
     '“in + a length of time” = five minutes from now, so “in”.'),
    (110, 'The pens are ___ the drawer.', 'in', 'in the drawer',
     'Les stylos sont À L’INTÉRIEUR du tiroir, donc « in ».',
     'The pens are INSIDE the drawer, so “in”.'),
    (111, 'She’s ___ Instagram all day.', 'on', 'on Instagram',
     'Réseaux sociaux et sites : on dit « on Instagram », comme « on the internet ».',
     'Social networks and websites: “on Instagram”, like “on the internet”.'),
    (112, 'What’s ___ TV tonight?', 'on', 'on TV',
     'Expression figée : « on TV » = à la télé.',
     'Set phrase: “on TV”.'),
    (113, 'I bought it ___ Amazon.', 'on', 'on Amazon',
     'Un site web ou une appli prend « on ». → on Amazon',
     'A website or an app takes “on”. → on Amazon'),
    (114, 'Look at the picture ___ page 5.', 'on', 'on page 5',
     'Une page est une surface, l’image est DESSUS → on page 5.',
     'A page is a surface, the picture is ON it → on page 5.'),
    (115, 'There’s a nice story ___ this magazine.', 'in', 'in this magazine',
     'Un article est DANS un magazine (on l’ouvre), donc « in ».',
     'A story is INSIDE a magazine (you open it), so “in”.'),
    (116, 'It snows a lot ___ winter.', 'in', 'in winter',
     'Une saison est une longue période, donc « in ». → in winter',
     'A season is a long period, so “in”. → in winter'),
    (117, 'I never drink coffee ___ the afternoon.', 'in', 'in the afternoon',
     'Une partie de la journée prend « in ». → in the afternoon',
     'A part of the day takes “in”. → in the afternoon'),
    (118, 'We have English ___ Thursday afternoon.', 'on', 'on Thursday afternoon',
     'Jour + moment : c’est le jour qui décide → on Thursday afternoon.',
     'Day + part of the day: the day decides → on Thursday afternoon.'),
    (119, 'Our flat is ___ the ground floor.', 'on', 'on the ground floor',
     'Pour les étages, on dit toujours « on ». → on the ground floor',
     'Floors always take “on”. → on the ground floor'),
    (120, 'We played football ___ the park.', 'in', 'in the park',
     'Le parc est un espace délimité, on est DEDANS → in the park.',
     'A park is an area with limits, you are INSIDE it → in the park.'),
    (121, 'Don’t sit ___ the grass.', 'on', 'on the grass',
     'On s’assoit SUR l’herbe (une surface), donc « on ».',
     'You sit ON the grass (a surface), so “on”.'),
    (122, 'Put your coat ___ the hook.', 'on', 'on the hook',
     'Le manteau est accroché SUR le crochet, donc « on ».',
     'The coat hangs ON the hook, so “on”.'),
    (123, 'There’s a calendar ___ the wall.', 'on', 'on the wall',
     'Accroché SUR le mur (surface), donc « on ».',
     'Hanging ON the wall (a surface), so “on”.'),
    (124, 'The ice cream is ___ the freezer.', 'in', 'in the freezer',
     'La glace est À L’INTÉRIEUR du congélateur, donc « in ».',
     'The ice cream is INSIDE the freezer, so “in”.'),
    (125, 'They’re ___ a business trip.', 'on', 'on a business trip',
     'Expression figée : « on a trip / on a business trip » (comme on holiday).',
     'Set phrase: “on a trip / on a business trip” (like on holiday).'),
    (126, 'He works ___ Italy.', 'in', 'in Italy',
     'On est DANS un pays, donc « in ». → in Italy',
     'You are INSIDE a country, so “in”. → in Italy'),
    (127, 'She got ___ the plane.', 'on', 'on the plane',
     'Avion, bus, train, bateau : on peut marcher dedans → « on ».',
     'Plane, bus, train, ship: you can walk around → “on”.'),
    (128, 'Jump ___ my car, I’ll drive you.', 'in', 'in my car',
     'Voiture = petit véhicule où on reste assis → « in ».',
     'A car is a small vehicle where you stay seated → “in”.'),
    (129, 'We met ___ the ship.', 'on', 'on the ship',
     'Un bateau est un grand véhicule où on marche → « on ».',
     'A ship is a big vehicle you walk around → “on”.'),
    (130, 'The kids are ___ the back seat.', 'in', 'in the back seat',
     'On est assis DANS le siège arrière (on s’y enfonce) → in the back seat.',
     'You sit IN the back seat (you sink into it) → in the back seat.'),
    (131, 'Don’t stay ___ the sun too long.', 'in', 'in the sun',
     'Pour la météo, on est « dans » le soleil, la pluie → in the sun.',
     'Weather surrounds you: the sun, the rain → in the sun.'),
    (132, '___ my view, it’s a bad idea.', 'in', 'in my view',
     'Expression figée : « in my view » = à mon avis (comme in my opinion).',
     'Set phrase: “in my view” (like in my opinion).'),
    (133, 'Who’s the girl ___ this painting?', 'in', 'in this painting',
     'Une personne représentée est DANS l’image → in the painting.',
     'A person shown is INSIDE the image → in the painting.'),
    (134, 'He was dressed ___ a black suit.', 'in', 'dressed in',
     'Pour dire « habillé en » → dressed in a black suit.',
     'Wearing something → dressed in a black suit.'),
    (135, 'The letter was written ___ German.', 'in', 'in German',
     'Pour une langue, on dit toujours « in ». → in German',
     'A language always takes “in”. → in German'),
    (136, 'You can rely ___ me.', 'on', 'rely on',
     'Le verbe « rely » (compter sur) est toujours suivi de « on ».',
     'The verb “rely” is always followed by “on”.'),
    (137, 'She insisted ___ paying.', 'on', 'insist on',
     'Le verbe « insist » est toujours suivi de « on ». → insist on',
     'The verb “insist” is always followed by “on”.'),
    (138, 'He believes ___ ghosts.', 'in', 'believe in',
     '« Believe in » = croire en l’existence de quelque chose.',
     '“Believe in” = believe that something exists.'),
    (139, 'She succeeded ___ passing the exam.', 'in', 'succeed in',
     '« Succeed » est toujours suivi de « in ». → succeed in doing',
     '“Succeed” is always followed by “in”. → succeed in doing'),
    (140, 'There’s been a fall ___ unemployment.', 'in', 'a fall in',
     'Une baisse/hausse DE quelque chose → a fall in / a rise in.',
     'A fall or a rise OF something → a fall in / a rise in.'),
    (141, 'The bus was ___ time today.', 'on', 'on time',
     '« On time » = à l’heure pile.',
     '“On time” = punctual.'),
    (142, 'Sign your name ___ the form.', 'on', 'on the form',
     'Le nom est écrit SUR le formulaire (une feuille = surface), donc « on ».',
     'The name is written ON the form (a sheet is a surface), so “on”.'),
    (143, 'There are three mistakes ___ this sentence.', 'in', 'in this sentence',
     'Une erreur se trouve DANS une phrase, un texte → in this sentence.',
     'A mistake is INSIDE a sentence, a text → in this sentence.'),
    (144, 'The bank is ___ your right.', 'on', 'on your right',
     'Pour les directions : « on the right / on the left ».',
     'Directions: “on the right / on the left”.'),
    (145, 'I sat ___ the corner of the room.', 'in', 'in the corner',
     'Coin intérieur d’une pièce : on est DEDANS → in the corner.',
     'The inside corner of a room: you are IN it → in the corner.'),
    (146, 'They live ___ the coast.', 'on', 'on the coast',
     'Au bord de la mer, sur la ligne de la côte → on the coast.',
     'By the sea, along the line of the coast → on the coast.'),
    (147, 'We live ___ the suburbs.', 'in', 'in the suburbs',
     'La banlieue est une zone, on est DEDANS → in the suburbs.',
     'The suburbs are an area, you are INSIDE it → in the suburbs.'),
    (148, 'She has a tattoo ___ her arm.', 'on', 'on her arm',
     'Un tatouage est SUR la peau (surface du corps), donc « on ».',
     'A tattoo is ON the skin (the body’s surface), so “on”.'),
    (149, 'I’ve got something ___ my eye.', 'in', 'in my eye',
     'Quelque chose est entré DANS l’œil, donc « in ».',
     'Something got INTO the eye, so “in”.'),
    (150, 'There’s dirt ___ your face.', 'on', 'on your face',
     'La saleté est SUR le visage (surface), donc « on ».',
     'The dirt is ON the face (a surface), so “on”.'),
    (151, 'The cups are ___ the cupboard.', 'in', 'in the cupboard',
     'Les tasses sont À L’INTÉRIEUR du placard (fermé), donc « in ».',
     'The cups are INSIDE the cupboard (closed), so “in”.'),
    (152, 'Birds fly ___ the air.', 'in', 'in the air',
     'L’air est vu comme un espace, on est DEDANS → in the air.',
     'The air is seen as a space, you are INSIDE it → in the air.'),
    (153, 'We had dinner ___ a restaurant.', 'in', 'in a restaurant',
     'On mange DANS un bâtiment, donc « in ».',
     'You eat INSIDE a building, so “in”.'),
    (154, 'Is the TV ___?', 'on', 'the TV is on',
     '« On » = allumé (le contraire est « off »).',
     '“On” = switched on (the opposite is “off”).'),
    (155, 'She keeps her money ___ her wallet.', 'in', 'in her wallet',
     'L’argent est DANS le portefeuille, donc « in ».',
     'The money is INSIDE the wallet, so “in”.'),
    (156, 'I’m ___ a good mood today.', 'in', 'in a good mood',
     'Expression figée : « in a good/bad mood » = de bonne/mauvaise humeur.',
     'Set phrase: “in a good/bad mood”.'),
    (157, 'They’re ___ danger.', 'in', 'in danger',
     'Expression figée : « in danger » = en danger.',
     'Set phrase: “in danger”.'),
    (158, 'The workers are ___ strike.', 'on', 'on strike',
     'Expression figée : « on strike » = en grève.',
     'Set phrase: “on strike”.'),
    (159, 'Is this model ___ stock?', 'in', 'in stock',
     'Expression figée : « in stock » = en stock, disponible.',
     'Set phrase: “in stock” = available.'),
    (160, 'Your glasses are ___ your head!', 'on', 'on your head',
     'Les lunettes sont posées SUR la tête, donc « on ».',
     'The glasses rest ON the head, so “on”.'),
    (161, 'Can I pay ___ cash?', 'in', 'in cash',
     'Expression figée : « in cash » = en espèces.',
     'Set phrase: “in cash”.'),
    (162, 'He’s ___ debt.', 'in', 'in debt',
     'Expression figée : « in debt » = endetté.',
     'Set phrase: “in debt”.'),
    (163, 'Switch ___ your phone.', 'on', 'switch on',
     '« Switch on » = allumer (le contraire est « switch off »).',
     '“Switch on” (the opposite is “switch off”).'),
    (164, 'We go out ___ Saturday nights.', 'on', 'on Saturday nights',
     'Jour + moment : c’est le jour qui décide → on Saturday nights.',
     'Day + part of the day: the day decides → on Saturday nights.'),
    (165, 'The trees are green ___ spring.', 'in', 'in spring',
     'Une saison est une longue période, donc « in ». → in spring',
     'A season is a long period, so “in”. → in spring'),
    (166, 'The house was built ___ the 1800s.', 'in', 'in the 1800s',
     'Une longue période (siècle, décennie) prend « in ».',
     'A long period (century, decade) takes “in”.'),
    (167, 'Women got the vote ___ the 20th century.', 'in', 'in the 20th century',
     'Un siècle est une longue période, donc « in ».',
     'A century is a long period, so “in”.'),
    (168, 'We exchange gifts ___ Christmas Day.', 'on', 'on Christmas Day',
     'Christmas Day (le 25) est un jour précis, donc « on ».',
     'Christmas Day (the 25th) is a single day, so “on”.'),
    (169, 'We got married ___ a Saturday.', 'on', 'on a Saturday',
     'Un jour de la semaine prend « on ». → on a Saturday',
     'A day of the week takes “on”. → on a Saturday'),
    (170, 'I go to the gym ___ Mondays.', 'on', 'on Mondays',
     'Les jours (même au pluriel = tous les lundis) prennent « on ».',
     'Days (even plural = every Monday) take “on”.'),
    (171, 'It’s cold ___ the early morning.', 'in', 'in the early morning',
     'Une partie de la journée prend « in ». → in the early morning',
     'A part of the day takes “in”. → in the early morning'),
    (172, 'We met ___ the morning of June 5th.', 'on', 'on the morning of June 5th',
     'Piège : le matin D’UN JOUR PRÉCIS → « on » (le jour l’emporte).',
     'Trap: the morning OF A GIVEN DAY → “on” (the day wins).'),
    (173, 'It happened ___ a cold winter night.', 'on', 'on a cold winter night',
     'Piège : même avec « winter », c’est UNE nuit précise → « on ».',
     'Trap: even with “winter”, it is ONE given night → “on”.'),
    (174, '___ the beginning, it was hard.', 'in', 'in the beginning',
     'Expression : « in the beginning » = au début.',
     'Set phrase: “in the beginning”.'),
    (175, 'He’ll be famous ___ a few years.', 'in', 'in a few years',
     '« in + durée » = dans quelques années (un délai), donc « in ».',
     '“in + a length of time” = a few years from now, so “in”.'),
    (176, 'He’s ___ the office.', 'in', 'in the office',
     'On est DANS le bureau (bâtiment/pièce), donc « in ».',
     'You are INSIDE the office (a building or a room), so “in”.'),
    (177, 'I saw the ad ___ YouTube.', 'on', 'on YouTube',
     'Un site ou une plateforme prend « on ». → on YouTube',
     'A website or a platform takes “on”. → on YouTube'),
    (178, 'I found the word ___ the dictionary.', 'in', 'in the dictionary',
     'Le mot est DANS le dictionnaire (un livre), donc « in ».',
     'The word is INSIDE the dictionary (a book), so “in”.'),
    (179, 'There’s a note ___ the fridge.', 'on', 'on the fridge',
     'Piège : la note est collée SUR la porte du frigo (surface), donc « on ».',
     'Trap: the note is stuck ON the fridge door (a surface), so “on”.'),
    (180, 'There’s water ___ the bottle.', 'in', 'in the bottle',
     'L’eau est À L’INTÉRIEUR de la bouteille, donc « in ».',
     'The water is INSIDE the bottle, so “in”.'),
    (181, 'Their house is ___ the island.', 'on', 'on the island',
     'On est SUR une île (une surface de terre), donc « on ».',
     'You are ON an island (a piece of land), so “on”.'),
    (182, 'The kids are playing ___ the sea.', 'in', 'in the sea',
     'Ils sont DANS l’eau, donc « in ».',
     'They are INSIDE the water, so “in”.'),
    (183, 'There are ships ___ the ocean.', 'on', 'on the ocean',
     'Les bateaux flottent SUR la surface de l’eau, donc « on ».',
     'Ships float ON the surface of the water, so “on”.'),
    (184, 'London is ___ the Thames.', 'on', 'on the Thames',
     'Une ville au bord d’un fleuve → on the Thames.',
     'A city beside a river → on the Thames.'),
    (185, 'Sugar dissolves ___ water.', 'in', 'in water',
     'Le sucre se dissout DANS l’eau, donc « in ».',
     'Sugar dissolves INSIDE the water, so “in”.'),
    (186, 'She’s ___ the front row.', 'in', 'in the front row',
     'On est assis DANS une rangée → in the front row.',
     'You sit IN a row → in the front row.'),
    (187, 'He’s lying ___ the sofa.', 'on', 'on the sofa',
     'Allongé SUR le canapé, donc « on ».',
     'Lying ON the sofa, so “on”.'),
    (188, 'He’s sitting ___ the bench.', 'on', 'on the bench',
     'Assis SUR un banc (surface plate), donc « on ».',
     'Sitting ON a bench (a flat surface), so “on”.'),
    (189, 'She’s ___ class right now.', 'in', 'in class',
     'Expression : « in class » = en cours.',
     'Set phrase: “in class”.'),
    (190, 'The doctor is ___ call this weekend.', 'on', 'on call',
     'Expression figée : « on call » = de garde / d’astreinte.',
     'Set phrase: “on call”.'),
    (191, 'She’s working ___ her thesis.', 'on', 'working on',
     '« Work on something » = travailler sur quelque chose.',
     '“Work on something”.'),
    (192, 'He’s ___ control of the situation.', 'in', 'in control of',
     'Expression figée : « in control of » = maître de.',
     'Set phrase: “in control of”.'),
    (193, 'I’m ___ my way home.', 'on', 'on my way',
     'Expression figée : « on my way » = en chemin.',
     'Set phrase: “on my way”.'),
    (194, 'I’ll be there ___ an hour.', 'in', 'in an hour',
     '« in + durée » = dans une heure (un délai), donc « in ».',
     '“in + a length of time” = an hour from now, so “in”.'),
    (195, 'Write your name ___ capital letters.', 'in', 'in capital letters',
     'Expression : « in capital letters » = en majuscules.',
     'Set phrase: “in capital letters”.'),
    (196, 'Her photo was ___ the front page.', 'on', 'on the front page',
     'Une page = surface, la photo est DESSUS → on the front page.',
     'A page is a surface, the photo is ON it → on the front page.'),
    (197, 'There’s a hole ___ my sock.', 'in', 'in my sock',
     'Un trou est DANS le tissu (il le traverse), donc « in ».',
     'A hole is IN the fabric (it goes through it), so “in”.'),
    (198, 'There’s a fly ___ my soup!', 'in', 'in my soup',
     'La mouche est DANS la soupe (dans le liquide), donc « in ».',
     'The fly is IN the soup (inside the liquid), so “in”.'),
    (199, 'Let’s meet ___ Valentine’s Day.', 'on', 'on Valentine’s Day',
     'Un jour précis (le 14 février) prend « on ».',
     'A single day (14 February) takes “on”.'),
    (200, 'He grew up ___ the countryside.', 'in', 'in the countryside',
     'La campagne est une zone, on est DEDANS → in the countryside.',
     'The countryside is an area, you are INSIDE it → in the countryside.')
 ) AS e (position, sentence, answer, phrase, fr, en)
 WHERE t.language_code = 'en' AND t.slug = 'on-in'
ON CONFLICT (topic_id, position) DO NOTHING;
