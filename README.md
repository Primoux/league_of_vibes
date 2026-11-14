# League of Vibes

MOBA 4v4 jouable dans le navigateur, aux mécaniques inspirées de League of Legends
(phase de pick, 3 voies, jungle, sbires, tours, inhibiteurs, nexus, drakes, larves, Colosse du Rift, Léviathan,
sorts d'invocateur, objets, brouillard de guerre). 10 champions originaux et une caricature jouable de Mélenchon,
rendu 3D Three.js, serveur autoritaire Node. Avec le 67 Master et le Touilleur, le roster compte 13 champions.

Projet fan non affilié à Riot Games. Aucun asset ni nom de Riot n'est utilisé.

## Démarrer

```bash
npm install
npm run dev          # serveur :3000 + client Vite :5173
```

Ouvre http://localhost:5173/?dev=1 dans deux onglets : crée une salle dans l'un, rejoins avec le code dans l'autre,
lance la partie (le mode dev autorise moins de 8 joueurs). Clic droit pour se déplacer.

Production : `npm run build && npm start` puis http://localhost:3000.

## Portraits et 67 Master

Chaque champion a son illustration de sélection : les treize portraits sont également repris au chargement,
dans le HUD et le tableau des scores. Images locales WebP de 512 × 512, environ 700 Ko pour le roster complet ;
le portrait SVG de secours reste disponible si une image ne charge pas. Prompts et provenance dans
`packages/client/src/assets/champions/manifest.json`.

Le **67 Master**, mage original en hoodie violet et cyan, alterne continuellement ses deux paumes,
y compris pendant la course et les sorts. Sept impacts de sorts espacés déclenchent son passif.

| Touche | Sort | Effet |
|---|---|---|
| Q | Six. | Projectile sur la première cible, dégâts et ralentissement |
| W | Seven. | 7 géant retardé, dégâts de zone et immobilisation |
| E | Les mains en l’air | Téléportation et bouclier temporaire |
| R | SIX… SEVEN ! | Deux ondes : ralentissement, puis projection ; incantation initiale interruptible |

Démo sans serveur : `?uilab=draft` pour les portraits et `?fxlab&champ=master67&close=1`
pour le modèle et ses sorts (Q/W/E/R dans le panneau). Le labo FX utilise la vraie simulation du serveur.

## Modèles 3D inspirés des portraits

Les treize champions reprennent désormais les silhouettes, coiffures, costumes et armes de leurs portraits :
visages sculptés, étoffes plissées, capuches ouvertes, métal distinct du tissu, cristaux et ornements.
Les animations existantes restent utilisées, y compris les gestes du 67 Master.

Pour les comparer : `npm run dev:client`, puis `http://localhost:5173/?modellab`.
Choix du champion, portrait de référence, rotation/zoom, vue de jeu, course, attaque, Q/W/E/R et mort/retour.
Un lien comme `?modellab&champ=master67` sélectionne directement un champion.
Ce labo montre les animations ; `?fxlab` reste le banc de test des sorts avec simulation.

[Aperçu comparatif des douze modèles](docs/previews/champion-models.webp) — rendu CPU de la géométrie réelle,
sans les contours GPU ni le bloom du jeu. [Détails de vérification](docs/champion-models.md).

## Le Touilleur, le Virtuose du Moulinet

Mage de mêlée en manteau turquoise, moustache cuivrée et gant ivoire. Son index levé décrit un moulinet
continu en repos, course, attaques, sorts et rappel. Le doigt a un poste à temps plein.

| Touche | Sort | Effet |
|---|---|---|
| P | Ça monte à la tête | Trois impacts de sorts espacés sur des champions donnent dégâts bonus et vitesse |
| Q | Coup de doigt | Projectile sur le premier ennemi, dégâts et ralentissement |
| W | Tourbillon de poche | Zone retardée de dégâts et ralentissement |
| E | Pas de côté | Téléportation courte et bouclier |
| R | La grande touille | Passe derrière un champion, dégâts et étourdissement jusqu'à 2 s pendant une canalisation burlesque |

L'ultime respecte les murs, les limites de carte et les collisions : il ne touche pas si aucune arrivée
derrière la cible n'est possible. Ténacité et Purification fonctionnent ; contrôle dur/silence du lanceur,
ordre d'arrêt/déplacement, mort, stase ou rupture de proximité terminent le canal et son stun.

Portrait, modèle et moulinet : `?modellab&champ=touilleur`. Sorts réellement simulés :
`?fxlab&champ=touilleur&close=1`. [Aperçu du portrait et du modèle](docs/previews/touilleur.webp).

## Mélenchon, le Tribun indigné

Une caricature satirique de mage, disponible dans le draft. Le micro n'a pas de bouton d'arrêt ; ses sorts ont bien des cooldowns.

| Touche | Sort | Effet |
|---|---|---|
| P | Indignation permanente | Trois impacts de sorts espacés sur des champions donnent bouclier et vitesse |
| Q | Question au gouvernement | Micro projectile, première cible touchée et ralentie |
| W | Motion de censure | Pluie d'amendements retardée, dégâts de zone et silence |
| E | Hologramme | Téléportation et bouclier ; le double resté au départ explose après 1,2 s |
| R | La République, c’est moi ! | Déclaration interruptible de 0,6 s, onde tricolore, provocation et bouclier |

Pour examiner le modèle sans serveur, lance `npm run dev:client` et ouvre `http://localhost:5173/?melenchonlab`.
Boutons Q/W/E/R ou touches correspondantes ; vues Portrait/Silhouette, rotation à la souris et marche.
Ce labo est une vitrine visuelle ; les règles réelles sont exécutées et testées côté serveur.
Le champion et ses effets sont aussi disponibles dans `?fxlab`.

## Scripts

| Commande | Rôle |
|---|---|
| `npm run dev` | serveur (tsx watch) + client (Vite) |
| `npm run typecheck` | TypeScript sur les 3 paquets |
| `npm test` | tests vitest (simulation headless) |
| `npm run build` | build du client dans `packages/client/dist` |
| `DEV_LOBBY=1 npm run dev:server` | toutes les salles en mode dev (draft accéléré : 10 s / tour, 3 s de finalisation, 2 s de chargement) |
| `LOV_PERF=1 npm run dev:server` | log toutes les 10 s : durée des ticks, coût et régularité des snapshots, débit par client, retard de la boucle Node |
| `SKIP_DRAFT=1 npm run dev:server` | pas de draft : la partie démarre directement (champions attribués automatiquement) |
| `npx tsx scripts/bots.ts --url ws://localhost:3000/ws --bots 8 --seconds 30` | test de charge : 8 bots WebSocket (CI uniquement) |

## Déploiement

Le serveur sert le client construit et le WebSocket sur le même port (`PORT`, défaut 3000). `GET /health` renvoie
`{ ok, uptime, rooms, players }`. Une partie tient en mémoire dans un seul processus : **une seule instance**
(pas de répartition de charge sans affinité de session).

**Docker**
```bash
docker build -t league-of-vibes .
docker run -p 3000:3000 league-of-vibes
```

**Fly.io** : `fly launch` (le Dockerfile est détecté), `internal_port = 3000`, health check HTTP sur `/health`,
`min_machines_running = 1` et `auto_stop_machines = false` (sinon la machine s'arrête en pleine partie).

**Render** : nouveau *Web Service* depuis le dépôt, runtime Docker (ou Node : build `npm ci && npm run build`,
start `npm start`), health check `/health`. Les WebSockets sont pris en charge sans configuration.

Derrière un reverse proxy (nginx, Caddy), transmettre l'upgrade WebSocket sur `/ws`.

## Organisation

- `PLAN.md` — design complet du jeu
- `AGENTS.md` — règles de collaboration des 7 IA
- `docs/plans/` — mission de chaque branche · `docs/progress/` — avancement
- `packages/shared` — types, protocole, données · `packages/server` — simulation · `packages/client` — rendu, FX, UI
