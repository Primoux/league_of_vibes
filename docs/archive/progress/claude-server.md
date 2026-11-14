# Progress — claude-server

## Fait
- **M1.1 API de dégâts** (`packages/server/src/combat/damage.ts`) — livrée sur `main`.
- **M1.2 Pathfinding** (`packages/server/src/nav/grid.ts`) : grille 50 u depuis `WALLS` (+35 u de marge), A* octile (buffers réutilisés), lissage par ligne de vue, destination dans un mur → point accessible le plus proche. Budget : 24 A* / tick, raccourci en ligne droite quand la vue est dégagée.
- **M1.3 Collisions** : séparation douce entre champions/sbires/monstres, structures infranchissables, `data.ghost` ignore les unités.
- **M1.4 Dashes** via `entity.data.dash` (voir ci-dessous).
- **M1.5 Combat** : auto-attaques (portée bord à bord, windup, projectiles, annulation au déplacement pendant le windup), poursuite, attaque-déplacement, mort/respawn à la fontaine, K/D/A, `damageToChampions`.
- **M1.7 Fontaine** : soin rapide des alliés (12 %/s PV + mana), laser 1800 dégâts bruts/s sur les champions ennemis (rayon 1200).
- **Vision** (en avance sur M3) : rayons par type, brousses, balises, invisibilité, révélations, révélation 1 s quand on attaque depuis une brousse.
- **M1.6 Draft** (`room.ts`) : ordre 1-2-2-2-1, 30 s/tour, survol visible des alliés seulement, champion unique, auto-pick (survol sinon aléatoire), finalisation 15 s (sorts d'invocateur ; défaut Flash+Ignite, Flash+Smite si rôle jungle), chargement 5 s, puis partie.
- **M2 Fin de partie** : `world.winner` → dernier snapshot + `gameOver` (stats) → phase `end` → `returnToLobby`.
- **M2 Reconnexion** : `reconnectToken` secret (≠ playerId public), reprise de la salle à n'importe quelle phase ; en partie : `gameStart` renvoyé + snapshot complet. Salle vide en partie conservée 3 min.
- **M3 Snapshots** : filtrés par vision, delta (`removed`), structures toujours envoyées, événements filtrés par équipe, score (kills/tours/drakes/larves). `permessage-deflate` activé.
- **M3 Validation** des `ClientMsg` (forme, nombres finis, bornes) + rate-limit 40 msg/s (rafale 80) par connexion. `netPing` → `pong { clientTime }`.
- **M4** : `/health` JSON, Dockerfile, doc Fly/Render dans le README, CI (typecheck + test + build + 8 bots), `scripts/bots.ts`.
- **Perf** (test `perf.test.ts`) : tick ≈ 0,1 ms pour 8 champions + 60 sbires en combat ; pire cas ≈ 130 Ko/s de JSON brut, ≈ 12 Ko/s compressé. Bots réels : 20 snapshots/s, 0 erreur.
- Tests : `packages/server/test/{damage,nav,combat,vision,room,snapshot,net,perf}.test.ts`.

- **M2 Intégration** (`test/room.integration.test.ts`) : 2 joueurs pilotés uniquement par `ClientMsg` à travers `Room` + `createSystems()` réels : draft → partie, levelAbility, poursuite + AA, sort de Kaelen, CC de `buffs` qui bloque déplacement/attaque, mort, kill crédité dans les snapshots, respawn, `gameOver`.
- **Revue lead (13/11)** : PR ouvertes `pr/mistral-economy` et `pr/feat/champion-melenchon` vérifiées fusionnées avec `main` dans un worktree propre → typecheck + tests (396 / 393) + build verts. Melenchon : conflit trivial sur `ASSETS.md` uniquement. codex-champions utilise correctement `hooks.canMove/canAttack/isTargetable` et délègue ses dashes à `movement`.
- **Workflow** : intégration par PR (`pr/claude-server` → `main`), plus de push direct sur `main`.

- **Lag réseau (14/11)** — mesuré avec `packages/client/tools/netcheck.ts` (vrai pipeline client), 8 joueurs, 75 s :
  - Causes : snapshots irréguliers dès le serveur (30 Hz / 20 Hz = 1,5 tick par snapshot + `setInterval` → intervalles 33/67 ms, jusqu'à 83 ms) et débit qui explose avec les sbires (entité entière renvoyée à chaque mouvement : ~110 Ko/s JSON/client après 75 s).
  - Correctifs : boucle à échéances fixes (sans dérive) ; snapshot à **chaque tick (30 Hz)** horodaté à l'instant prévu du tick ; **patches par champ** (`Snapshot.patches`, protocole v2) ; rafraîchissement complet étalé (chaque entité renvoyée en entier toutes les 2 s → auto-réparation) ; arrondis du `SelfState`.
  - Résultat (latence 40 ± 10 ms) : intervalle p95 78 → **47 ms**, max 100 → **56 ms** ; marge tampon p5 28 → **55 ms** ; débit 80 → **44 Ko/s** JSON. Gigue ±30 ms : 0 % d'extrapolation (15 Hz testé : 7,4 %).
  - `LOV_PERF=1` : métriques de la boucle serveur dans les logs.

## En cours
- Étendre le test d'intégration M2 aux sbires/tours (codex-objectives, pas encore en PR) et à l'or/achat (mistral-economy, PR ouverte).

## Bloqué / Besoins (des autres agents)
- **mistral-ui** : le serveur fait maintenant un vrai draft (`draft` puis `loading` puis `gameStart`). L'UI bootstrap n'a pas d'écran de draft : en attendant, l'auto-pick fait avancer la partie (salle dev : 10 s/tour + 3 s + 2 s), ou lancer le serveur avec `SKIP_DRAFT=1`. Messages : `draftHover`, `draftLock`, `draftSummoners`. Les survols/sorts ennemis arrivent masqués (`hoverChampionId: null`).
- **mistral-ui** : écran de fin → bouton « Retour au lobby » = `{ t: 'returnToLobby' }`.
- **claude-client** : `netPing { clientTime }` → `pong { serverTime, clientTime }` disponible pour le RTT.
- **claude-client** : j'ai ajouté l'application de `snap.patches` dans `net/snapshots.ts` (correction d'intégration, à relire). Snapshots maintenant à 30 Hz. Les « saccades » restantes de netcheck (40–55 u, Vrenna/Corvin/sbires mêlée) existaient avant et semblent venir de dashes / poussées de collision côté simulation, pas du réseau : je creuse côté movement.
- **codex-champions** : enregistre tes CC via `world.hooks.canMove/canAttack` (et `isTargetable` pour stase/untargetable) ; dashes via `data.dash` (movement émet l'événement `dash`).
- **codex-objectives** : tours/sbires/monstres peuvent utiliser `attackTarget` + `data.attackConfig` ; montée en dégâts des tours via `hooks.beforeAttackHit` (filtrer `ctx.attacker.kind === 'tower'`) ; `getObjectiveTimers` → `hooks.objectiveTimers`. Mettre `data.keepCorpse = true` si tu gères toi-même la disparition d'un monstre.
- **mistral-economy** : primes via `hooks.bounty`, `data.goldEarned` pour l'écran de fin, `hooks.selfState` pour `canShop`/`recalling`.
- **Environnement** : le projet exige Node ≥ 22 (`engines`). Avec Node 21, `vitest` échoue (binding natif rolldown manquant). Utiliser Node 24 comme la CI.

## Contrats modifiés
- `contract:` **protocole v2** : `Snapshot.patches?: EntityPatch[]` (champs modifiés seulement + `u` = champs supprimés), `applyEntityPatch(prev, patch)` dans `protocol.ts`. `SNAPSHOT_RATE` 20 → 30 (doit diviser `TICK_RATE`).
- Intégration client (claude-client, correction signalée) : `SnapshotBuffer.push` applique `snap.patches` (6 lignes dans `net/snapshots.ts`).
- `contract:` `ClientMsg` + `{ t: 'netPing'; clientTime }` (→ `pong { serverTime, clientTime }`) et `{ t: 'returnToLobby' }` (phase `end` → lobby).
- Sémantique delta confirmée et documentée dans `Snapshot.removed` : `removed` défini ⇒ delta (claude-client : ✅ c'est bien ta règle).
- `contract:` `GameEvent` + `{ type: 'respawn'; target; pos }` (émis quand un champion réapparaît à la fontaine).
- `contract:` `autoAttack` + champs optionnels `speed?` (vitesse du projectile, AA à distance) et `crit?`.

## API disponibles

### Dégâts — `packages/server/src/combat/damage.ts`
```ts
applyDamage(world, { source: Entity | EntityId | undefined; target: Entity; amount: number; type: DamageType;
                     ability?: string; isAutoAttack?: boolean; crit?: boolean; silent?: boolean }): number
// Ordre : hooks.beforeDamage → % pen puis pen fixe (stats.armorPen/magicPen du crédité + ctx.flatPen/percentPen)
// → damageMultiplier → ctx.multiplier → boucliers → PV → lifesteal (AA seulement) → hooks.afterDamage → mort.
// Retourne les dégâts infligés (bouclier absorbé inclus). Émet `damage` (sauf silent).
applyHeal(world, { source, target, amount, silent? }): number        // plafonné aux PV max, hooks.beforeHeal, émet `heal`
applyShield(world, { source, target, amount, duration?, id? }): number // entity.shield = somme ; id = remplace le précédent
removeShield(target, id?)
kill(world, target, killer?)            // mort directe (exécutions) : même logique que la mort par dégâts
previewDamage(source, target, amount, type): number   // sans effet de bord
timeSinceCombat(world, e): number       // s depuis le dernier dégât subi/infligé (passif Grondak, rappel, nuage...)
markAssist(world, helper, victim)       // donne un crédit d'assist (CC, soin d'un allié...)
creditOf(world, source)                 // suit data.owner (EntityId) : invocations, zones, projectiles
combatData(e) : { lastDamagedAt, lastDealtAt, championDamagers, shields, diedAt? }   // e.data.combat
CRIT_MULTIPLIER = 1.75 ; ASSIST_WINDOW = 10 ; defaultRespawnTime(level, gameTime)
```
- **Crédit** : si une entité a `data.owner = <EntityId>`, les dégâts/kills sont crédités au propriétaire.
- **Kill par une tour/un sbire** : le dernier champion ennemi ayant touché la victime < 10 s récupère le kill.
- Mort → `entity.dead = true`, `anim = 'death'`, ordres effacés, `death { target, killer?, assists?, bounty? }`.
  Champions : `champ.respawnIn`, respawn automatique à la fontaine (événement `respawn`).
  Sbires/monstres/balises morts : retirés du monde 2 s après la mort, sauf `data.keepCorpse = true`.

### Hooks — `world.hooks` (s'enregistrer dans `init(world)` de ton système)
```ts
beforeDamage(ctx: DamageContext, world)       // ctx.amount, ctx.multiplier, ctx.flatPen, ctx.percentPen, ctx.type modifiables
afterDamage(ctx, dealt, world)
beforeHeal(ctx: HealContext, world)           // ctx.amount (anti-soin, amplification)
preventDeath(target, killer, world) => bool   // true + target.hp > 0 → mort annulée
bounty(victim, killer, assists, world) => number | undefined     // mistral-economy
respawnTime(victim, world) => number | undefined
canMove(e, world) => bool                     // codex-champions : root/stun/knockup/canalisation
canAttack(e, world) => bool                   // stun, désarmement
isTargetable(target, attacker, world) => bool // stase, untargetable
beforeAttackHit(ctx: AttackContext, world)    // ctx.amount (AD), ctx.crit, ctx.critMultiplier, ctx.bonus, ctx.damageType
afterAttackHit(ctx, dealt, world)             // on-hit
```

### Déplacement — `systems/movement.ts`
- `entity.moveTarget = {x, y}` → chemin calculé automatiquement (`entity.path`).
- **Dash** : `entity.data.dash = { to: Vec2; speed: number; ignoreWalls?; targetId?; stopDistance?; fxKey?; onArrive?(world, e) }`.
  Prioritaire sur le déplacement et les CC de mouvement. Movement **émet l'événement `dash`** au premier tick
  (ne l'émets pas toi-même), s'arrête au mur (sauf `ignoreWalls`), supprime `data.dash` puis appelle `onArrive`.
  `targetId` : dash qui suit une cible et s'arrête à `stopDistance` bord à bord.
- `entity.data.ghost = true` → ignore les collisions entre unités.
- `navGrid()` (`nav/grid.ts`) : `walkable(x,y)`, `nearestWalkable(p)`, `lineWalkable(a,b)`, `lastWalkableOnSegment(a,b)`, `findPath(a,b)`. Utile pour Flash / blinks (`nearestWalkable`).

### Combat — `systems/combat.ts`
- `entity.attackTarget = id` → poursuite + auto-attaques (marche aussi pour sbires, tours, monstres).
- `entity.data.attackConfig = { windup?: number /* fraction */; projectileSpeed?: number /* 0 = mêlée */ }`.
- `cancelAttack(e)`, `resetAttackTimer(e)` (reset d'AA), `hit(world, attacker, target, crit)` (impact d'AA avec hooks),
  `orderAttackMove(e, pos)`, `edgeDistance(a, b)`, `isValidAttackTarget(world, attacker, t)`.
- Événement `autoAttack { source, target, ranged, speed?, crit? }` émis au lancement (mêlée : à l'impact).

### Snapshot / réseau — `net/snapshot.ts`, `room.ts`
- Hooks `world.hooks.selfState(e, s, world)` (complète `SelfState` : `xpToNext` par leveling, `canShop`/`recalling` par economy),
  `world.hooks.entitySnapshot(e, s, world)` (ex. `s.buffs` par codex-champions ; doit être déterministe pour le delta),
  `world.hooks.objectiveTimers(team, world) => Record<string, number>` (codex-objectives).
- `SelfState.canShop` par défaut : mort ou à moins de `FOUNTAIN_RADIUS` de sa fontaine.
- Stats de fin : `gold` = `entity.data.goldEarned` (or total gagné, à tenir par mistral-economy) sinon or courant.
- Événements privés à l'équipe : `goldGain`, `ping`, `chat` (teamOnly). `itemBought/Sold`, `recall*`, `respawn` : équipe + ennemis qui voient l'unité.
- `ping` et `chat` sont gérés par `input` (émis en événements ; ping limité à 1 / 0,4 s par joueur).

### Vision — `systems/vision.ts`
- `isVisibleTo(e, team)` ; `data.sightRadius` (surcharge), `data.invisible`, `data.revealedUntil` (temps de jeu).
- Rayons : champion 1350, sbire 1100, tour 1350, inhibiteur/nexus 1000, balise (`kind: 'ward'`) 900. Neutres : aucun.
- `data.brush` = index de brousse courant (-1 sinon).
