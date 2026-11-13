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
