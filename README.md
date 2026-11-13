# League of Vibes

MOBA 4v4 jouable dans le navigateur, aux mécaniques inspirées de League of Legends
(phase de pick, 3 voies, jungle, sbires, tours, inhibiteurs, nexus, drakes, larves, Colosse du Rift, Léviathan,
sorts d'invocateur, objets, brouillard de guerre). 10 champions originaux et une caricature jouable de Mélenchon,
rendu 3D Three.js, serveur autoritaire Node.

Projet fan non affilié à Riot Games. Aucun asset ni nom de Riot n'est utilisé.

## Démarrer

```bash
npm install
npm run dev          # serveur :3000 + client Vite :5173
```

Ouvre http://localhost:5173/?dev=1 dans deux onglets : crée une salle dans l'un, rejoins avec le code dans l'autre,
lance la partie (le mode dev autorise moins de 8 joueurs). Clic droit pour se déplacer.

Production : `npm run build && npm start` puis http://localhost:3000.

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
