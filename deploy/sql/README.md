# Scripts SQL ponctuels

Lancés **une fois**, à la main, en superutilisateur, sur la base de
production. Chaque script a son retour arrière. Les deux sont idempotents et
s'exécutent dans une transaction. L'archive de release les contient (`sql/`).

| Script | Retour | Effet |
|---|---|---|
| `phase-1.sql` | `phase-1-retour.sql` | rôles `pulsars_owner` / `pulsars_migrate`, schéma `contenu`, plus aucun droit de structure pour `pulsars_site` et `pulsars_api` (`docs/contract/database.md` §2.1) |

## Phase 1 en production, dans l'ordre (0.2.0 → 0.4.0)

Chaque étape laisse le site en marche. Une commande par ligne.

La 0.3.0 n'a jamais été installée : la 0.4.0 la contient, et passe
directement après la 0.2.0. Elle lit `contenu` : elle ne peut s'installer
qu'**après** le script.
Pendant ce temps, la 0.2.0 continue de servir : elle lit les vues `api_read`,
qui suivent les tables déplacées. Ne pas la redémarrer entre les étapes 3
et 5 : au démarrage, elle essaierait de migrer avec un rôle qui n'en a plus
le droit (elle sert quand même, en l'écrivant au journal).

### 1. Récupérer la release 0.4.0

Sans encore l'installer : on a besoin de ses scripts SQL. Sur ton Mac
(connecté à `gh`) :

```
gh release download v0.4.0 -R Pulsars-science/pulsars-api -p "pulsars-api-v0.4.0-x86_64-unknown-linux-gnu.tar.gz*"
scp pulsars-api-v0.4.0-x86_64-unknown-linux-gnu.tar.gz* tmh:/tmp/
```

Puis sur le serveur :

```
ssh tmh
cd /tmp
sha256sum -c pulsars-api-v0.4.0-x86_64-unknown-linux-gnu.tar.gz.sha256
tar xzf pulsars-api-v0.4.0-x86_64-unknown-linux-gnu.tar.gz
ls /tmp/pulsars-api-v0.4.0-x86_64-unknown-linux-gnu/sql
```

### 2. Le site regarde aussi dans `contenu`

Laravel impose son propre `search_path` à chaque connexion. Sans ce
changement, il ne trouverait plus les cours une fois déplacés. Le schéma
n'existe pas encore, et PostgreSQL l'ignore d'ici là : ce réglage peut donc
passer avant le script.

```
cd /var/www/pulsars
sudo -u www-data sed -i 's/^DB_SEARCH_PATH=.*/DB_SEARCH_PATH="site,contenu"/' .env
grep ^DB_SEARCH_PATH .env
sudo -u www-data php artisan config:cache
sudo systemctl reload pulsars-octane
sudo systemctl restart pulsars-queue
curl -s -o /dev/null -w '%{http_code}\n' https://pulsars.science/bibliotheque
cd /tmp
```

Le `grep` doit afficher `DB_SEARCH_PATH="site,contenu"`, et le `curl` 200.

### 3. Sauvegarde, mot de passe, script

```
sudo -u postgres pg_dump -Fc pulsars > ~/pulsars-avant-phase-1.dump
ls -lh ~/pulsars-avant-phase-1.dump
openssl rand -hex 24 > ~/pulsars-secrets/MDP_MIGRATE
chmod 600 ~/pulsars-secrets/MDP_MIGRATE
sudo -u postgres psql pulsars -v ON_ERROR_STOP=1 -v mdp_migrate="$(cat ~/pulsars-secrets/MDP_MIGRATE)" -f /tmp/pulsars-api-v0.4.0-x86_64-unknown-linux-gnu/sql/phase-1.sql
curl -s -o /dev/null -w '%{http_code}\n' https://pulsars.science/bibliotheque
curl -s -o /dev/null -w '%{http_code}\n' https://api.pulsars.science/v1/courses
```

Le script ne doit afficher aucune `ERROR`. Les `NOTICE` (« already exists,
skipping ») ne sont normaux qu'à la deuxième fois. Les deux `curl` : 200.

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

### 4 bis. Clé de service

Le site lira le contenu par l'API (étape 7), depuis la machine même, pour
tous ses visiteurs. Avec cette clé, ses appels ne sont pas limités.

```
openssl rand -hex 32 > ~/pulsars-secrets/SERVICE_KEY
chmod 600 ~/pulsars-secrets/SERVICE_KEY
echo "SERVICE_KEY=$(cat ~/pulsars-secrets/SERVICE_KEY)" | sudo tee -a /etc/pulsars-api/pulsars-api.env > /dev/null
sudo grep -c ^SERVICE_KEY= /etc/pulsars-api/pulsars-api.env
```

Le `grep` doit afficher `1`. La 0.2.0 ignore cette ligne ; la 0.4.0 la lit
au démarrage de l'étape 5.

### 5. Installer la 0.4.0

```
/tmp/pulsars-api-v0.4.0-x86_64-unknown-linux-gnu/deploy-api
```

- « Migrations » applique `20261028000000_contenu_reference` en tant que
  `pulsars_migrate` (« migrations: applied … »). Si le script de l'étape 3
  n'a pas tourné, elle échoue (« schema contenu does not exist ») et le
  déploiement s'arrête avant la bascule : rien ne change.
- « Content check » lance `pulsars-api check-content` : pour chaque
  ressource, la 0.4.0 doit lire exactement les lignes que montre la vue
  `api_read` correspondante. Une seule ligne de différence arrête le
  déploiement avant la bascule.

### 6. Vérifications

```
sudo -u postgres psql pulsars -Atc "select schemaname, tableowner, count(*) from pg_tables where schemaname in ('site','contenu','game') group by 1, 2 order by 1"
curl -s https://api.pulsars.science/v1/courses | head -c 200; echo
curl -s -o /dev/null -w '%{http_code}\n' https://api.pulsars.science/v1/news
curl -s -o /dev/null -w '%{http_code}\n' https://pulsars.science/bibliotheque
sudo systemd-run --quiet --wait --pipe --collect --property=EnvironmentFile=/etc/pulsars-api/pulsars-api.env --property=User=pulsars-api /opt/pulsars-api/current/pulsars-api check-content
```

- Toutes les tables doivent appartenir à `pulsars_owner`.
- L'API et le site doivent répondre normalement.
- La dernière commande refait la comparaison de l'étape 5 à la main.

### 7. Le site lit le contenu par l'API

Une fois le site déployé avec pulsars#29 (`deploy-pulsars`), et la 0.4.0
vérifiée :

```
cd /var/www/pulsars
echo "PULSARS_API_SERVICE_KEY=$(cat ~/pulsars-secrets/SERVICE_KEY)" | sudo -u www-data tee -a .env > /dev/null
echo "PULSARS_API_URL=http://127.0.0.1:8080" | sudo -u www-data tee -a .env > /dev/null
echo "PULSARS_CONTENU_VIA_API=true" | sudo -u www-data tee -a .env > /dev/null
grep -c "^PULSARS_" .env
sudo -u www-data php artisan config:cache
sudo systemctl reload pulsars-octane
curl -s -o /dev/null -w '%{http_code}\n' https://pulsars.science/bibliotheque
curl -s -o /dev/null -w '%{http_code}\n' https://pulsars.science/actualite
sudo grep -c "lecture en base" storage/logs/laravel.log
```

- Le premier `grep` : `3`. Les deux `curl` : 200.
- Le dernier `grep` compte les pages qui ont dû relire la base faute de
  réponse de l'API. Il doit rester à `0` (ou au chiffre d'avant) quand on
  ouvre quelques pages de la bibliothèque.
- Pour revenir en arrière : remettre `PULSARS_CONTENU_VIA_API=false`, puis
  `config:cache` et `reload`. Voir `docs/CONTENU-VIA-API.md` du site.

## Retour arrière de la phase 1

D'abord le site, s'il lit par l'API : `PULSARS_CONTENU_VIA_API=false`,
`config:cache`, `reload` (il relirait sinon la base, mais en journalisant
chaque page). Ensuite, ordre inverse : l'API d'abord (la 0.4.0 ne sait pas lire sans `contenu`),
puis le script, puis le site.

```
sudo /opt/pulsars-api/current/deploy-api --rollback
sudo rm /etc/pulsars-api/migrate.env
sudo -u postgres psql pulsars -v ON_ERROR_STOP=1 -f /opt/pulsars-api/releases/v0.4.0/sql/phase-1-retour.sql
sudo systemctl restart pulsars-api
cd /var/www/pulsars
sudo -u www-data sed -i 's/^DB_SEARCH_PATH=.*/DB_SEARCH_PATH=site/' .env
sudo -u www-data php artisan config:cache
sudo systemctl reload pulsars-octane
```

- `deploy-api --rollback` remet la 0.2.0, qui lit les vues, toujours en
  place.
- Après le script, `pulsars_api` est de nouveau propriétaire de `game` : la
  0.2.0 migre au démarrage comme avant. Les tables de référence de la 0.4.0
  et leur ligne de migration sont retirées, pour qu'un nouvel aller les
  recrée.
- La ligne `pulsars_migrate` de `pg_hba.conf` peut rester : le rôle
  n'existe plus.
- En dernier recours : `pg_restore` de `~/pulsars-avant-phase-1.dump`.
