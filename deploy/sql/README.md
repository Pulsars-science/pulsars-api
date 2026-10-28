# La base d'un serveur neuf

Le serveur part de zéro (PLAN-REFONTE.md). On ne reprend de l'ancienne base
que le contenu : cours, examens, ressources, actualités, PDF et traductions.

Une commande par ligne, en `ubuntu` sur le serveur. Les mots de passe sont
dans `~/pulsars-secrets` (`MDP_MIGRATE`, `MDP_API`, `MDP_ADMIN`).

## 1. Base, rôles et schémas

```
sudo -u postgres createdb pulsars
sudo -u postgres psql pulsars -v ON_ERROR_STOP=1 -v mdp_migrate="$(cat ~/pulsars-secrets/MDP_MIGRATE)" -v mdp_api="$(cat ~/pulsars-secrets/MDP_API)" -f /opt/pulsars-api/current/deploy/sql/roles.sql
```

`roles.sql` crée `pulsars_owner` (sans connexion, propriétaire de tout),
`pulsars_migrate` (les migrations) et `pulsars_api` (le service, qui lit et
écrit les lignes sans jamais changer la structure), puis les schémas `site`,
`contenu`, `game` et `langues`. Le relancer ne change rien.

## 2. Les tables

`deploy-api` lance `pulsars-api migrate` avant chaque mise en service, avec
`DATABASE_MIGRATION_URL` (`/etc/pulsars-api/migrate.env`). La première
migration crée tout le schéma, la seconde les données de référence (rôles,
thèmes du forum, licences, matières, mandarin).

## 3. Le compte d'administration

```
sudo -u pulsars-api sh -c 'set -a; . /etc/pulsars-api/pulsars-api.env; /opt/pulsars-api/current/pulsars-api create-admin jikuji ADRESSE' < ~/pulsars-secrets/MDP_ADMIN
```

Le mot de passe est lu sur l'entrée standard (12 caractères au moins) :
jamais sur la ligne de commande.

## 4. Le contenu de l'ancienne base

La copie faite avant la remise à zéro (`pg_dump -Fc`) se restaure dans une
base à part, qu'on supprime ensuite :

```
sudo -u postgres createdb pulsars_ancien
sudo -u postgres pg_restore -d pulsars_ancien --no-owner --no-acl ~/pulsars-avant-reset.dump
sudo -u postgres psql pulsars_ancien -c "CREATE ROLE pulsars_lecture LOGIN PASSWORD 'lecture-temporaire'; GRANT USAGE ON SCHEMA site TO pulsars_lecture; GRANT SELECT ON ALL TABLES IN SCHEMA site TO pulsars_lecture;"
sudo -u pulsars-api sh -c 'set -a; . /etc/pulsars-api/pulsars-api.env; /opt/pulsars-api/current/pulsars-api import-content postgres://pulsars_lecture:lecture-temporaire@127.0.0.1/pulsars_ancien jikuji'
sudo -u postgres dropdb pulsars_ancien
sudo -u postgres psql -c "DROP ROLE pulsars_lecture"
```

`import-content` copie les tables dans l'ordre (cours, chapitres, leçons,
exercices, examens, parties, ressources, actualités, fichiers, traductions,
téléchargements) en une seule transaction, et donne tous les contenus au
compte nommé. Il refuse une base qui a déjà des cours. Essayé sur la copie
du 18/10 : 272 cours, 1 084 leçons, 1 120 exercices, 32 examens, 27 PDF et
6 260 traductions, en 4 secondes.
