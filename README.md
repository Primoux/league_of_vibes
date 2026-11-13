# League of Vibes

MOBA 4v4 jouable dans le navigateur, aux mécaniques inspirées de League of Legends
(phase de pick, 3 voies, jungle, sbires, tours, inhibiteurs, nexus, drakes, larves, Colosse du Rift, Léviathan,
sorts d'invocateur, objets, brouillard de guerre). 10 champions originaux, rendu 3D Three.js, serveur autoritaire Node.

Projet fan non affilié à Riot Games. Aucun asset ni nom de Riot n'est utilisé.

## Démarrer

```bash
npm install
npm run dev          # serveur :3000 + client Vite :5173
```

Ouvre http://localhost:5173/?dev=1 dans deux onglets : crée une salle dans l'un, rejoins avec le code dans l'autre,
lance la partie (le mode dev autorise moins de 8 joueurs). Clic droit pour se déplacer.

Production : `npm run build && npm start` puis http://localhost:3000.

## Scripts

| Commande | Rôle |
|---|---|
| `npm run dev` | serveur (tsx watch) + client (Vite) |
| `npm run typecheck` | TypeScript sur les 3 paquets |
| `npm test` | tests vitest (simulation headless) |
| `npm run build` | build du client dans `packages/client/dist` |
| `DEV_LOBBY=1 npm run dev:server` | toutes les salles en mode dev |

## Organisation

- `PLAN.md` — design complet du jeu
- `AGENTS.md` — règles de collaboration des 7 IA
- `docs/plans/` — mission de chaque branche · `docs/progress/` — avancement
- `packages/shared` — types, protocole, données · `packages/server` — simulation · `packages/client` — rendu, FX, UI
