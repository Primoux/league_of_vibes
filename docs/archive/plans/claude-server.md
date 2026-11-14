# YOUR_PLAN — claude-server (Claude 1, lead intégration)

Branche : `claude-server`

## Mission
Tu construis le **cœur du serveur autoritaire** et tu es le **lead d'intégration** : tu garantis que `main` reste vert et jouable, tu arbitres les contrats de `shared/`, tu merges régulièrement et tu débloques les autres.

## Tu possèdes
- `packages/server/src/index.ts`, `room.ts`, `world.ts`, `net/**`
- `packages/server/src/systems/{input,movement,combat,vision}.ts`, `systems/index.ts`
- `packages/server/src/combat/damage.ts` (à créer), `packages/server/src/nav/**` (à créer)
- `packages/shared/src/protocol.ts`, `types.ts`, `constants.ts` (hors bloc `TIMERS`), `math.ts`
- `.github/workflows/**`, `scripts/**`, `README.md`, tests `packages/server/test/{world,room,net,nav,combat,vision}*.test.ts`

## Interdit
`client/**` (sauf correction d'intégration triviale signalée), systèmes des autres agents, données de champions/objets.

## Jalons
### M1 — fondations (priorité absolue, les autres en dépendent)
1. **API de dégâts** `server/src/combat/damage.ts` : `applyDamage`, `applyHeal`, `applyShield` (résistances via `damageMultiplier`, pénétration, boucliers consommés d'abord, crits, lifesteal sur AA, émission `damage`/`heal`/`shield`, mort → `death` avec killer, assists (dégâts reçus de champions dans les 10 dernières secondes), bounty fourni par economy via hook). **Livre-la tôt** sur `main` (même minimale) et préviens dans ton progress.
2. **Pathfinding** : grille 50 u depuis `WALLS` (+ marge du rayon), A* avec heuristique octile, lissage (string pulling / line-of-sight), clic dans un mur → point accessible le plus proche. Cache/budget par tick. Tests.
3. **Collisions** légères entre unités (séparation douce, champions ne traversent pas les structures), ghosting quand `data.ghost`.
4. **Dashes** : `entity.data.dash = { to, speed, onArrive? }` consommé par movement (ignore le pathfinding, s'arrête aux murs sauf `ignoreWalls`). Documenté pour codex-champions.
5. **Combat** : auto-attaques (portée bord à bord, windup `attackWindup`, projectile pour distance avec `projectileSpeed`, annulation au déplacement après windup), poursuite de la cible, attaque-déplacement (`attackMove`), émission `autoAttack`. Mort : anim `death`, timer de respawn, respawn à la fontaine, K/D/A, `damageToChampions`.
6. **Draft** dans `room.ts` : ordre 1-2-2-2-1, timers 30 s, hover, lock, auto-pick, finalisation 15 s (sorts d'invocateur), puis `loading` 5 s, puis `game`. Messages `draft` et `loading`.
7. **Fontaine** : soins rapides des alliés, laser mortel sur les ennemis.

### M2 — tranche verticale
- Tests d'intégration : 2 joueurs sur la voie mid avec sbires/tours (codex-objectives) et sorts (codex-champions).
- **Fin de partie** : `world.winner` → `gameOver` avec stats, phase `end`, retour lobby possible.
- Reconnexion : token de session (UUID par joueur dans la salle), reprise de l'entité, renvoi `gameStart` + snapshot complet.

### M3 — vision et réseau
- **Vision** (`systems/vision.ts`) : rayons par type (PLAN §9), brousses (`isInBrush`), balises (entités `ward`), révélations (`data.revealedUntil`), invisibilité (`data.invisible`). Résultat : `entity.data.visibleTo: Set<Team>` ou équivalent.
- **Snapshots** : filtrés par vision de l'équipe, **delta** (n'envoyer que ce qui a changé + `removed`), structures toujours envoyées, événements filtrés (pas d'événements d'unités invisibles). Objectif < 30 Ko/s par client.
- Rate-limit et validation des `ClientMsg` (anti-spam, champs valides).

### M4 — prod
- Déploiement simple (Dockerfile ou instructions Render/Fly dans README), `/health`.
- Profilage du tick (< 5 ms pour 8 champions + 60 sbires), tests de charge (script de 8 bots WebSocket *de test* qui envoient des commandes aléatoires — uniquement pour la CI, jamais en jeu).
- Revue des merges, correction des régressions d'intégration.

## Rôle de lead
- Merge ta branche sur `main` souvent. Vérifie après chaque merge des autres que `main` est vert ; sinon corrige ou ouvre un point dans ton progress.
- Tiens à jour la section « API disponibles » de `docs/progress/claude-server.md` (signatures exactes) : c'est ce que les autres consultent.
- Ajoute la CI : typecheck + test + build sur push/PR.

## Tests attendus
Pathfinding (contournement de mur, point inaccessible), dégâts (armure, pénétration, bouclier, mort), auto-attaque (cadence, portée, windup), draft (ordre, timeout, unicité), vision (brousse, balise), snapshot (filtrage ennemi invisible).
