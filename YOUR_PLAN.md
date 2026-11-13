# YOUR_PLAN — codex-objectives (Codex 2)

Branche : `codex-objectives`

## Mission
Tu implémentes **le monde PvE et les structures côté serveur** : sbires, tours, inhibiteurs, nexus et victoire, camps de jungle, drakes (6 éléments, âme, Drake Ancien), larves, Colosse du Rift, Léviathan. Règles : PLAN §7 (ce sont celles de League of Legends, noms changés).

## Tu possèdes
- `packages/server/src/systems/{minions,structures,jungle,objectives}.ts`
- `packages/server/src/world-ai/**` (à créer : IA partagée sbires/tours/monstres, ciblage)
- `packages/shared/src/constants.ts` → **bloc `TIMERS` uniquement** (+ `TIME_SCALE`)
- `packages/shared/src/data/monsters.ts`, `data/minions.ts`, `data/structures.ts` (à créer : stats et récompenses)
- Tests `packages/server/test/{minions,structures,jungle,objectives}*.test.ts`

## Interdit
Client, systèmes des autres, `map.ts` (claude-client : demande-lui une position si besoin, ou propose un commit `contract:`).

## Dépendances
- `applyDamage` (claude-server, `combat/damage.ts`) pour TOUS les dégâts ; mort via l'événement `death`.
- `addBuff` (codex-champions, `combat/buffs.ts`) pour les buffs rouge/bleu, drakes, âme, Ancien, Léviathan, larves. Mocke si absent.
- Or/XP : émets `death` avec `killer` ; mistral-economy donne l'or, codex-champions l'XP. Pour les récompenses d'équipe (tours, objectifs), émets `objectiveTaken` / `structureDestroyed` avec les infos nécessaires ; documente les montants dans `data/*.ts`.
- Auto-attaques des sbires/tours/monstres : tu peux réutiliser le combat de claude-server (`attackTarget` sur l'entité) une fois disponible ; d'ici là, implémente une attaque simple dans `world-ai` derrière une interface remplaçable.

## Jalons
### M1 — voie
1. **Structures** au `init` : tours (`TOWERS`), inhibiteurs, nexus avec stats (PV LoL-like : externe 5000, interne 3600, inhib 3300, nexus 2700 ; nexus 5500), `subtype` = tier.
2. **Sbires** : vagues (`TIMERS`), composition, spawn près du nexus, suivi de `lanePathFor`, ciblage LoL (PLAN §7), attaque mêlée/à distance (projectile → événement `autoAttack`), mort → `death`. Scaling avec le temps.
3. **Tours** : ciblage (sbires → champion qui agresse un champion allié), dégâts croissants sur champion, fortification avant 3:00, anti-backdoor, plaques jusqu'à 14:00 (`structureDestroyed` avec `plate: true`), ordre de destruction des structures (invulnérables tant que la précédente est debout).
4. **Inhibiteurs** : respawn, super-sbires. **Nexus** : destruction → `world.winner`.

### M2
5. Tests headless : une voie sans champions doit rester équilibrée (les vagues se rencontrent au milieu), une tour tue une vague de sbires seule en un temps cohérent, une partie sans joueurs ne se termine pas.

### M3 — jungle et épiques
6. **Camps** (`CAMPS`) : spawn, petits monstres, aggro sur l'attaquant, leash/reset (retour + régénération), respawn. Buff bleu / rouge (via `addBuff`), transfert à la mort du porteur. Crabe (vision + vitesse).
7. **Drakes** : spawn, éléments (2 aléatoires puis type de la Faille au 3e), buffs cumulables par équipe, **âme** à 4, **Drake Ancien** ensuite (brûlure + exécution < 20 %). Attaque AoE (souffle → `areaEffect` avec fxKey `dragon_<element>`).
8. **Larves** (×3, cumuls de dégâts aux structures), **Colosse du Rift** (objet « Œil du colosse » donné au tueur — coordonne avec mistral-economy pour l'objet ; l'invocation crée une entité alliée qui charge la tour la plus proche), **Léviathan** (attaques AoE, buff d'équipe : stats, rappel renforcé — expose `data.leviathanBuff` lu par economy pour le rappel —, sbires renforcés).
9. `objectiveSpawn` / `objectiveTaken` / `announcement` pour l'UI, et `objectiveTimers` (expose une fonction `getObjectiveTimers(world, team)` que claude-server mettra dans le snapshot).

### M4
10. Équilibrage des timers/PV/dégâts pour des parties de 25–30 min. Tests de non-régression.

## Tests
Ciblage des tours (priorité, aggro), plaques, ordre des structures, respawn inhibiteur, leash, âme au 4e drake, Ancien exécute, buff Léviathan appliqué aux vivants seulement, victoire au nexus.
