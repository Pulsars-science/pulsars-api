-- Spanish (`es`) for the reference data: the subjects' names, and Mandarin
-- with its HSK levels and first words. Additive: a key that already has a
-- Spanish value keeps it.

INSERT INTO contenu.matieres (slug, langue, nom) VALUES
    ('mathematiques', 'es', 'Matemáticas'),
    ('physique',      'es', 'Física'),
    ('chimie',        'es', 'Química'),
    ('biologie',      'es', 'Biología'),
    ('informatique',  'es', 'Informática'),
    ('cryptographie', 'es', 'Criptografía')
ON CONFLICT DO NOTHING;

UPDATE langues.languages
   SET name = '{"es": "Chino mandarín"}'::jsonb || name
 WHERE code = 'zh' AND NOT name ? 'es';

UPDATE langues.levels l
   SET title = jsonb_build_object('es', v.title) || l.title,
       description = jsonb_build_object('es', v.description) || l.description
  FROM (VALUES
    ('hsk-1', 'HSK 1', 'Las primeras palabras: presentarse, saludar, contar, la familia y las comidas.'),
    ('hsk-2', 'HSK 2', 'La vida cotidiana: el tiempo, los desplazamientos, las compras.')
  ) AS v (slug, title, description)
 WHERE l.language_code = 'zh' AND l.slug = v.slug AND NOT l.title ? 'es';

UPDATE langues.words w
   SET meaning = jsonb_build_object('es', v.meaning) || w.meaning
  FROM langues.levels l,
       (VALUES
    ('我',   'yo, me'),
    ('你',   'tú, te'),
    ('他',   'él'),
    ('她',   'ella'),
    ('好',   'bueno, bien'),
    ('是',   'ser'),
    ('不',   'no'),
    ('的',   'partícula posesiva'),
    ('谢谢', 'gracias'),
    ('再见', 'adiós'),
    ('一',   'uno'),
    ('二',   'dos'),
    ('三',   'tres'),
    ('八',   'ocho'),
    ('多',   'mucho, numeroso'),
    ('大',   'grande'),
    ('人',   'persona, ser humano'),
    ('爸爸', 'papá'),
    ('妈妈', 'mamá'),
    ('儿子', 'hijo'),
    ('朋友', 'amigo'),
    ('老师', 'profesor'),
    ('学生', 'alumno, estudiante'),
    ('中国', 'China'),
    ('北京', 'Pekín'),
    ('爱',   'amar'),
    ('吃',   'comer'),
    ('喝',   'beber'),
    ('读',   'leer'),
    ('水',   'agua'),
    ('茶',   'té'),
    ('菜',   'plato, verdura'),
    ('杯子', 'vaso, taza'),
    ('饭店', 'restaurante, hotel'),
    ('东西', 'cosa, objeto'),
    ('电脑', 'ordenador, computadora'),
    ('飞机', 'avión'),
    ('点',   'hora (en el reloj)')
  ) AS v (text, meaning)
 WHERE l.id = w.level_id AND l.language_code = 'zh' AND l.slug = 'hsk-1'
   AND w.text = v.text AND NOT w.meaning ? 'es';
