# Progress — claude-server

## Fait
- **M1.1 API de dégâts** (`packages/server/src/combat/damage.ts`) — livrée sur `main`.
- **M1.2 Pathfinding** (`packages/server/src/nav/grid.ts`) : grille 50 u depuis `WALLS` (+35 u de marge), A* octile (buffers réutilisés), lissage par ligne de vue, destination dans un mur → point accessible le plus proche. Budget : 24 A* / tick, raccourci en ligne droite quand la vue est dégagée.
- **M1.3 Collisions** : séparation douce entre champions/sbires/monstres, structures infranchissables, `data.ghost` ignore les unités.
- **M1.4 Dashes** via `entity.data.dash` (voir ci-dessous).
- **M1.5 Combat** : auto-attaques (portée bord à bord, windup, projectiles, annulation au déplacement pendant le windup), poursuite, attaque-déplacement, mort/respawn à la fontaine, K/D/A, `damageToChampions`.
- **M1.7 Fontaine** : soin rapide des alliés (12 %/s PV + mana), laser 1800 dégâts bruts/s sur les champions ennemis (rayon 1200).
- **Vision** (en avance sur M3) : rayons par type, brousses, balises, invisibilité, révélations, révélation 1 s quand on attaque depuis une brousse.
- Tests : `packages/server/test/{damage,nav,combat,vision}.test.ts`.

## En cours
- Draft (M1.6), fin de partie, reconnexion, snapshots filtrés par vision + delta.

## Bloqué / Besoins (des autres agents)
- Rien pour l'instant.
- **Environnement** : le projet exige Node ≥ 22 (`engines`). Avec Node 21, `vitest` échoue (binding natif rolldown manquant). Utiliser Node 24 comme la CI.

## Contrats modifiés
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

### Vision — `systems/vision.ts`
- `isVisibleTo(e, team)` ; `data.sightRadius` (surcharge), `data.invisible`, `data.revealedUntil` (temps de jeu).
- Rayons : champion 1350, sbire 1100, tour 1350, inhibiteur/nexus 1000, balise (`kind: 'ward'`) 900. Neutres : aucun.
- `data.brush` = index de brousse courant (-1 sinon).
