<!--
Titre : commit conventionnel, où le scope est l'AREA de .github/areas.json.
  feat(fx-core): halo de soin persistant
  fix(ui-hud): barre de mana tronquée en 1280×720
  contract: ajoute PlayerStats.visionScore au protocole   ← PR de contrat, seule, rien d'autre dedans
La CI refuse une PR qui sort de son area. Un mainteneur relit et merge : ne merge pas toi-même.
-->

Closes #<numéro de l'issue>

## Résumé
-

## Contrat partagé
<!-- `protocol.ts`, `types.ts`, `schema.ts`, `map.ts`, `constants.ts`, `systems/index.ts`.
     Uniquement des ajouts, et uniquement dans une PR « contract: » dédiée. « Aucun » sinon. -->
Aucun

## Impact pour les autres
<!-- Nouveaux événements EventBus / fxBus, champs du store, API exposées, mock à remplacer. -->

## Comment tester
<!-- Tests ajoutés + étapes manuelles. -->

## Checklist
- [ ] `npm run typecheck && npm test && npm run build` verts en local
- [ ] Une seule area, une seule issue
- [ ] `main` mergé dans ma branche (PR sans conflit)
- [ ] Pas de `any` non justifié, pas de `// @ts-ignore`, pas de test affaibli ou supprimé
- [ ] Pas de nouvelle dépendance npm, ou justifiée ci-dessus
- [ ] Aucun nom / visuel / son / texte de Riot ; assets générés ou CC0, listés dans `ASSETS.md`
- [ ] Textes du jeu en français
