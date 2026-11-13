<!--
Titre de la PR : commit conventionnel, ex. « feat(client): interpolation des snapshots ».
PR ouverte avec scripts/open-pr.sh depuis ta branche d'agent (voir AGENTS.md §3).
-->

## Agent / branche
<!-- ex. claude-client → pr/claude-client -->

## Résumé
<!-- Ce que la PR apporte, en 2-5 puces. -->
-

## Jalon
<!-- M1 / M2 / M3 / M4 + éléments de YOUR_PLAN.md couverts. -->

## Contrats modifiés
<!-- Fichiers partagés hors de ton domaine (protocol.ts, types.ts, schema.ts, map.ts, constants.ts, systems/index.ts…).
     Uniquement des ajouts, dans des commits `contract:` séparés. Écris « Aucun » sinon. -->
- Aucun

## Impact sur les autres agents
<!-- Nouveaux événements EventBus / fxBus, champs du store, API exposées, mocks à remplacer… -->

## Comment tester
<!-- Tests ajoutés, page de démo (?maplab, ?fxlab, ?uilab…), étapes manuelles. -->

## Checklist
- [ ] `npm run typecheck && npm test && npm run build` verts en local
- [ ] Je n'ai modifié que les fichiers de mon `YOUR_PLAN.md` (+ progress, tests) ou des ajouts `contract:`
- [ ] `YOUR_PLAN.md` n'est pas modifié par cette PR
- [ ] Pas de `any` non justifié, pas de `// @ts-ignore`
- [ ] Pas de nouvelle dépendance npm, ou justifiée dans mon progress
- [ ] Aucun nom / visuel / son / texte de Riot ; assets générés ou CC0, listés dans `ASSETS.md`
- [ ] `docs/progress/<agent>.md` à jour
