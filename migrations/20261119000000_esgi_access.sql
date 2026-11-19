-- A role any admin can give from the roles page, so someone without a
-- @myskolae.fr address can still read the ESGI space. Not a permission
-- role: it grants reading only, imports stay with admin_esgi.
INSERT INTO site.roles (slug, nom, categorie, couleur, niveau, description, ordre)
VALUES (
    'esgi',
    'Accès ESGI',
    'acces',
    '#4f7df0',
    0,
    'Lecture de l''espace ESGI sans adresse @myskolae.fr.',
    4
)
ON CONFLICT (slug) DO NOTHING;
