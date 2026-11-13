# Progress — codex-champions

Date : 2026-11-13. Branche de travail : `codex-champions`.

## Fait
- **M1** : moteur de sorts serveur autoritaire. Validation des rangs, ressources, cooldowns, CC, visibilité, cible et portée ; approche automatique des cibles hors portée, annulable par un nouvel ordre. Incantations interruptibles, animations `cast_Q/W/E/R`, haste, recasts et charges.
- Primitives : projectiles avec collisions balayées triées par distance (première cible / perçants / ciblés / retour), zones instantanées, retardées avec télégraphie et persistantes, cônes, dashes, blinks respectant les murs, liens interruptibles par la distance.
- Buffs : stun/root/slow/silence/knockup/taunt/suppress/blind/grounded ; ténacité sauf knockup/suppress ; ralentissement le plus fort, y compris plusieurs sorts du même lanceur. Boucliers temporaires indépendants, invisibilité, révélation, anti-soin et hooks de stase.
- Recalcul des stats par statAtLevel + objets + buffs + hooks ; supports des unités non championnes sans accumulation des modificateurs. Mana, énergie de Nyx et absence de ressource de Kaelen ; régénération, XP partagée à 1400 u, niveaux 1–18, points de compétence, rangs QWE et paliers R 6/11/16.
- **M2** : Grondak et Ilyra, quatre sorts et passifs testés ; neuf invocateurs : Flash, Ignite, Heal, Barrier, Exhaust, Teleport, Smite, Ghost, Cleanse. Flash ne laisse pas le rayon du champion dans un mur ; Teleport s'interrompt sur dégâts, CC, ordre ou disparition de la cible.
- **M3** : les dix kits complets : Grondak, Tharox, Kaelen, Vrenna, Ilyra, Zephyr, Sable, Corvin, Nyx, Solenne. Attaques renforcées, marques, saignement, riposte, trois fentes/rayons, orbe aller-retour, pièges limités, invisibilité, marque de mort et soins/boucliers amplifiés.
- **M4 — garde-fous** : 90 duels (45 paires × niveaux 6 et 11, équipements par classe), déterministes, sur les vrais systèmes combat/movement/vision. Contrôle des ressources, valeurs finies, dégâts aberrants et répétabilité. 40 tests de coûts/CD des sorts ; descriptions FR alignées sur les paramètres. Cela ne remplace pas l'équilibrage en partie humaine.
- Dernière validation complète : **320 tests / 31 fichiers**, `npm run typecheck`, `npm test`, `npm run build` verts sur Node 24.22.1. Avertissement de chunk client >500 Ko préexistant, hors de ce domaine.
- Synchronisation avec main jusqu'à `f9b6e369bd6b1f6623ed76d0a2b8979e7d8cb8eb`. Conflits résolus en conservant **strictement l'API damage.ts de claude-server**, et les deux ensembles d'événements partagés. Aucun changement des systèmes des autres domaines.
- Aucune dépendance ni asset ajouté.

## En cours
- Validation réseau 8 clients WebSocket et préparation de la livraison / PR.

## Bloqué / Besoins
- **Publication bloquée par l'accès GitHub** : l'intégration lit le dépôt et indique push=true pour le compte, mais les écritures Git Trees et Contents retournent toutes deux HTTP 403 « Resource not accessible by integration ». Aucun push ni PR distante créé. Il faut autoriser l'intégration à écrire dans `Primoux/league_of_vibes`.
- Les nouvelles règles d'AGENTS.md sont prises en compte : publication via `pr/codex-champions`, YOUR_PLAN de main conservé, revue/merge par Fable uniquement.
- Le réglage compétitif, les visuels et l'ergonomie nécessitent les parties humaines et les branches client/FX/UI ; le moteur expose les événements et états requis.

## Contrats modifiés
- Ajouts **optionnels** dans `shared/types.ts`, commit `contract:` : `fxKey` sur abilityCast, summonerCast, buffApplied, buffRemoved, levelUp ; `stacks` sur buffApplied. Événements respawn et extensions autoAttack de claude-server préservés. Aucun champ supprimé/renommé.
- `SummonerDef.params?` (schema possédé) contient les valeurs de gameplay des invocateurs.
- L'API de dégâts de secours livrée au premier jalon a été **retirée lors de la synchronisation** : le fichier final `combat/damage.ts` est identique à celui de main. Tous les sorts passent par cette API et ses hooks.

## API disponibles / intégration

### Buffs — `server/src/combat/buffs.ts`
- `addBuff(world, target, id, { duration, source?, stacks?, params?, definition? })`, `removeBuff`, `getBuff`, `getBuffs`, `hasBuff`, `hasCC`, `canAct/canMove/canAttack/canCast`, `getTenacity`, `getStatMods`, `cleanse`.
- `params.statMods` / `statPercentMods` : bonus plats / relatifs ; `amount` : ralentissement ; `tenacity`, `damageReduction`, `damageDealtReduction`, `grievous`, `invisible`, `ghosted`, `revealed` : effets.
- `registerStatSource((world, entity) => ({ flat, percent, tenacity }))` retourne un désabonnement. Les stats brutes d'ITEMS[champ.items] sont déjà additionnées ; le hook ajoute les passifs, sans compter deux fois ces stats.
- `setBaseStats(world, entity, stats)` : mise à jour de la base des sbires/monstres dont les stats progressent alors qu'un buff est actif.
- Enregistrement automatique sur `world.hooks.canMove/canAttack/isTargetable/beforeAttackHit/beforeDamage/beforeHeal/entitySnapshot`.
- Snapshot : buffs `{id, remaining, stacks?}` via entitySnapshot ; `data.invisible`, `data.ghost`, `data.revealedUntil` compatibles avec movement/vision de claude-server.

### Combat, boucliers et passifs
- `abilities/effects.ts` re-exporte applyDamage/applyHeal et adapte applyShield avec `{ source, target, amount, duration?, buffId?, fxKey? }`. Le bouclier réel reste exclusivement dans `combatData(target).shields`, via l'API de claude-server ; les buffs n'en stockent qu'une vue/identité.
- `abilities/combatHooks.ts` : registerDamageModifier(world, fn), branchement direct sur hooks.beforeDamage.
- Passifs : **hooks.afterDamage**, précision non arrondie et `ctx.isAutoAttack` pour les impacts confirmés. L'événement `autoAttack` de départ brise l'invisibilité, sans compter deux fois les impacts. La marque de Nyx stocke seulement les PV effectivement retirés, pas les boucliers ni l'overkill.
- Dashes : primitives.startDash produit le contrat `DashData` de movement (targetId, stopDistance, ignoreWalls, onArrive). Le mouvement avance une seule fois et émet l'événement ; abilities assure collisions/CC/fin. Secours local seulement si movementSystem absent du World.

### Progression et FX
- `grantXP`, `levelAbility`, `maxAbilityRank`, `xpToNextLevel`, `xpForLevel`, `cooldownWithHaste` dans systems/leveling.ts.
- `world.hooks.selfState` renseigne xpToNext. XP de mort : `data.xpReward`, sinon `data.minion.xp` / `data.jungle.xp` / `data.monster.xp` ; `data.xpGlobal` pour les objectifs globaux.
- abilityCast, projectileSpawn/Hit, dash, areaEffect, buffApplied/Removed, summonerCast, levelUp avec fxKey. Télégraphies `<sort>_telegraph`, impacts/variantes conservent les clés des données.

## Vérification reproductible
```bash
npm ci
npm run typecheck
npm test
npm run build
# CI réseau : démarrer le serveur, puis scripts/bots.ts --bots 8 --seconds 15.
# L'environnement local n'autorise pas le pipe IPC du CLI tsx : utiliser node --import tsx.
```
