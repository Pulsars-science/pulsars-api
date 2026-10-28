# Scripts SQL ponctuels

Lancés **une fois**, à la main, en superutilisateur, sur la base de
production. Chaque script a son retour arrière. Les deux sont idempotents et
s'exécutent dans une transaction. L'archive de release les contient (`sql/`).

| Script | Retour | Effet |
|---|---|---|
| `phase-1.sql` | `phase-1-retour.sql` | rôles `pulsars_owner` / `pulsars_migrate`, schéma `contenu`, plus aucun droit de structure pour `pulsars_site` et `pulsars_api` (`docs/contract/database.md` §2.1) |

## Phase 1 en production, dans l'ordre

Chaque étape laisse le site en marche. Une commande par ligne.

### 1. L'API qui ne migre plus au démarrage

Déployer la release qui contient ce dossier, comme d'habitude (`deploy-api`).
Sans `/etc/pulsars-api/migrate.env`, elle migre encore avec `DATABASE_URL` :
`pulsars_api` est encore propriétaire de `game`.

### 2. Le site regarde aussi dans `contenu`

Laravel impose son propre `search_path` à chaque connexion. Sans ce
changement, il ne trouverait plus les cours une fois déplacés. Le schéma
n'existe pas encore, et PostgreSQL l'ignore d'ici là : ce réglage peut donc
passer avant le script.

```
ssh tmh
cd /var/www/pulsars
sudo -u www-data sed -i 's/^DB_SEARCH_PATH=.*/DB_SEARCH_PATH="site,contenu"/' .env
grep ^DB_SEARCH_PATH .env
sudo -u www-data php artisan config:cache
sudo systemctl reload pulsars-octane
sudo systemctl restart pulsars-queue
curl -s -o /dev/null -w '%{http_code}\n' https://pulsars.science/bibliotheque
```

Le `grep` doit afficher `DB_SEARCH_PATH="site,contenu"`, et le `curl` 200.

### 3. Sauvegarde, mot de passe, script

```
sudo -u postgres pg_dump -Fc pulsars > ~/pulsars-avant-phase-1.dump
ls -lh ~/pulsars-avant-phase-1.dump
openssl rand -hex 24 > ~/pulsars-secrets/MDP_MIGRATE
chmod 600 ~/pulsars-secrets/MDP_MIGRATE
sudo -u postgres psql pulsars -v ON_ERROR_STOP=1 -v mdp_migrate="$(cat ~/pulsars-secrets/MDP_MIGRATE)" -f /opt/pulsars-api/current/sql/phase-1.sql
```

Le script ne doit afficher aucune `ERROR`. Les `NOTICE` (« already exists,
skipping ») ne sont normaux qu'à la deuxième fois.

### 4. Connexion du compte de migration

Dans `pg_hba.conf`, à côté des lignes de `pulsars_site` et `pulsars_api` :

```
sudo -u postgres psql -Atc 'show hba_file'
sudo nano /etc/postgresql/17/main/pg_hba.conf
```

Ligne à ajouter (même méthode que les deux autres) :

```
host  pulsars  pulsars_migrate  127.0.0.1/32  scram-sha-256
```

```
sudo systemctl reload postgresql
echo "DATABASE_MIGRATION_URL=postgres://pulsars_migrate:$(cat ~/pulsars-secrets/MDP_MIGRATE)@127.0.0.1:5432/pulsars" | sudo tee /etc/pulsars-api/migrate.env > /dev/null
sudo chmod 600 /etc/pulsars-api/migrate.env
```

### 5. Vérifications

```
sudo /opt/pulsars-api/current/deploy-api --no-system
sudo -u postgres psql pulsars -Atc "select schemaname, tableowner, count(*) from pg_tables where schemaname in ('site','contenu','game') group by 1, 2 order by 1"
curl -s https://api.pulsars.science/v1/courses | head -c 200; echo
curl -s -o /dev/null -w '%{http_code}\n' https://pulsars.science/bibliotheque
```

- `deploy-api` doit écrire « migrations: nothing to apply » : c'est la
  preuve que `pulsars_migrate` se connecte.
- Toutes les tables doivent appartenir à `pulsars_owner`.
- L'API et le site doivent répondre normalement.

## Retour arrière de la phase 1

Ordre inverse : le script, puis l'API, puis le site.

```
sudo rm /etc/pulsars-api/migrate.env
sudo -u postgres psql pulsars -v ON_ERROR_STOP=1 -f /opt/pulsars-api/current/sql/phase-1-retour.sql
sudo systemctl restart pulsars-api
cd /var/www/pulsars && sudo -u www-data sed -i 's/^DB_SEARCH_PATH=.*/DB_SEARCH_PATH=site/' .env && sudo -u www-data php artisan config:cache && sudo systemctl reload pulsars-octane
```

- Après le retour, `pulsars_api` est de nouveau propriétaire de `game` :
  la release actuelle continue de fonctionner, rien d'autre n'est à
  redéployer.
- La ligne `pulsars_migrate` de `pg_hba.conf` peut rester : le rôle
  n'existe plus.
- En dernier recours : `pg_restore` de `~/pulsars-avant-phase-1.dump`.
