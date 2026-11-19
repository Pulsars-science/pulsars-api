-- Mandarin Chinese in simplified characters (`zh`) for the reference data:
-- the subjects' names, and the name and levels of Mandarin itself. The words'
-- meanings get no Chinese: explaining 我 by 我 helps nobody, they fall back
-- to French. Additive: an existing Chinese value is kept.

INSERT INTO contenu.matieres (slug, langue, nom) VALUES
    ('mathematiques', 'zh', '数学'),
    ('physique',      'zh', '物理'),
    ('chimie',        'zh', '化学'),
    ('biologie',      'zh', '生物'),
    ('informatique',  'zh', '计算机科学'),
    ('cryptographie', 'zh', '密码学')
ON CONFLICT DO NOTHING;

UPDATE langues.languages
   SET name = '{"zh": "普通话"}'::jsonb || name
 WHERE code = 'zh' AND NOT name ? 'zh';

UPDATE langues.levels l
   SET title = jsonb_build_object('zh', v.title) || l.title,
       description = jsonb_build_object('zh', v.description) || l.description
  FROM (VALUES
    ('hsk-1', 'HSK 一级', '最初的词语：自我介绍、打招呼、数数、家庭和饮食。'),
    ('hsk-2', 'HSK 二级', '日常生活：时间、出行、购物。')
  ) AS v (slug, title, description)
 WHERE l.language_code = 'zh' AND l.slug = v.slug AND NOT l.title ? 'zh';
