# Progress — codex-champions

## Fait
- Lecture des plans, synchronisation initiale avec main ; installation, typecheck et 4 tests initiaux verts.

## En cours
- M1 : moteur de sorts, buffs/CC, progression et contrats d'intégration.

## Bloqué / Besoins (des autres agents)
- Le combat et le déplacement sont encore des stubs sur main. API de dégâts de secours dans `combat/damage.ts`, autorisée par le plan ; à réconcilier par claude-server avec son moteur.
- Le client, le rendu, le combat automatique et la vision restent la responsabilité de leurs propriétaires.

## Contrats modifiés / API disponibles
- Ajouts optionnels uniquement dans `shared/types.ts` : `fxKey` sur abilityCast, summonerCast, buffApplied, buffRemoved, levelUp ; `stacks` sur buffApplied. Aucun champ renommé/supprimé.

## Jalon M2 / M3 local (2026-11-13)
- Moteur opérationnel : validations, approche des cibles hors portée, projectiles avec collisions balayées et tri des impacts, ciblés/perçants, zones retardées/persistantes, cônes, dashes/blinks, liens, recasts et incantations interruptibles.
- Buffs/CC, boucliers indépendants à expiration, invisibilité, révélation, anti-soin ; stats recalculées avec statAtLevel, objets et registerStatSource.
- XP 1–18, points et rangs, paliers de R, XP locale partagée ; ressources mana/énergie/aucune.
- Les dix kits, leurs passifs et les neuf sorts d'invocateur sont implémentés.
- 207 tests verts, dont un fichier par champion, 40 vérifications coût/CD et 90 duels (45 paires × niveaux 6/11 avec équipements par classe). Les duels injectent les AA par applyDamage car combat était un stub au départ : ce n'est pas une certification d'équilibrage PvP.
- Build client validé au jalon M2 (avertissement existant de chunk >500 Ko).
- Publication tentée par l'intégration GitHub : refus 403 « Resource not accessible by integration ». Aucun push distant ni merge distant réalisé à ce stade.

## Intégration à préserver
- `combat/damage.ts` est un secours, à remplacer/réconcilier par claude-server ; tout le gameplay utilise applyDamage/applyHeal/applyShield.
- `abilities/combatHooks.ts` : registerDamageModifier(world, fn), beforeDamage(world, context) pour critiques renforcés, isolement et coups renforcés. Exécuter avant la mitigation.
- `damage` sans `ability` = impact confirmé d'une AA (déclenche les passifs) ; `autoAttack` = départ de l'attaque, brise seulement l'invisibilité. Ne pas émettre un deuxième damage pour un même impact.
- `data.dash` : from/to/speed/duration/remaining/fxKey/unstoppable. Avancement de secours dans abilities ; le système movement peut exposer `handlesDashes: true` pour en reprendre l'avancement. Les callbacks de collision/fin restent gérés par abilities.
- `data.invisible`, `data.revealed`, `data.ghosted` à lire par vision/movement. Les contrôles doivent passer par combat/buffs.ts.
- `getBuffs(entity)` expose les buffId/remaining/stacks pour les snapshots. `xpToNextLevel(level)` expose l'XP requise du niveau courant.
- `registerStatSource((world, entity) => ({ flat, percent, tenacity }))` retourne un désabonnement ; les stats brutes des objets sont déjà additionnées depuis ITEMS[champ.items], le hook ajoute les passifs.
- XP de mort : data.xpReward prioritaire, sinon data.minion.xp / data.jungle.xp / data.monster.xp ; data.xpGlobal pour un objectif global.
- Aucun ajout de dépendance ni asset.
