# YOUR_PLAN — claude-fx (Claude 3)

Branche : `claude-fx`

## Mission
Tu es responsable de **tout ce qui fait « waouh »** : les FX de qualité (particules, shaders, post-process) pour les 40 sorts + passifs, les sorts d'invocateur, les événements de jeu, et les **modèles + animations des 10 champions**. Le PLAN insiste : « on veut des FX propres ».

## Tu possèdes
- `packages/client/src/fx/**`
- `packages/client/src/render/champions/**`
- Démo `?fxlab` (page de test des FX sans serveur)

## Interdit
`server/**`, `client/src/{ui,net,input,audio}/**`, reste de `render/**` (claude-client). Tu consommes leurs API.

## Contrats consommés
- `fxBus` : chaque `GameEvent` (`abilityCast`, `projectileSpawn`, `projectileHit`, `dash`, `areaEffect`, `buffApplied/Removed`, `damage`, `death`, `levelUp`, `recallStart/End`, `summonerCast`, `structureDestroyed`, `objectiveTaken`…). Les `fxKey` sont dans `shared/src/data/champions/*.ts` (`<champion>_<slot>`), `summoners.ts` (`sum_*`).
- `GameRenderer` de claude-client : `scene`, `camera`, `onFrame`, `toWorld`, `getEntityObject(id)`, `registerChampionFactory(...)` (si pas encore prêt : ajoute-le dans ton dossier un adaptateur et demande-le dans ton progress).

## Jalons
### M1 — boîte à outils FX (avec `?fxlab`)
1. **Système de particules GPU** instancié (pool, émetteurs : burst, continu, trail, ring ; courbes de taille/couleur/alpha ; additive/alpha blending ; textures procédurales générées dans un canvas : glow, étincelle, fumée, anneau).
2. **Shaders** : dissolve (mort), fresnel/rim glow, shockwave au sol, ruban de traînée (trail), décal au sol (indicateurs, zones), distorsion légère (chaleur), eau/givre stylisés.
3. **Post-process** : bloom sélectif modéré, outline léger, FXAA. Option qualité basse sans post-process.
4. **Registre** : `registerFx(fxKey, handler)` + fallback générique par type d'événement (projectile coloré selon la palette du champion, impact, zone) pour que tout sort ait *au moins* un FX propre dès le début.
5. Télégraphies : indicateurs de visée des sorts (ligne, cône, cercle, portée) exposés à claude-client.

### M2 — champions (2 premiers complets : Grondak et Ilyra)
6. **Modèles low-poly procéduraux** (géométries Three.js composées ou glTF CC0 listé dans `ASSETS.md`), silhouette lisible de loin, palette `palette` du champion, accent de couleur d'équipe discret.
7. **Animations** : idle, run, attack, cast_Q/W/E/R (ou cast générique), death, recall — procédurales (squelette simple ou transformations de parties) ou clips glTF.
8. FX complets de leurs sorts + passifs.

### M3 — les 10 champions + monde
9. Les 8 autres champions (modèle, anims, tous les FX de sorts). Chaque FX : anticipation, projectile/effet, impact, persistance si besoin.
10. Sorts d'invocateur : flash (éclair + traînée), ignite (flammes sur la cible), heal, barrier (dôme), exhaust, teleport (canal + colonne), smite (éclair divin), ghost, cleanse.
11. Événements : impacts d'auto-attaques (mêlée/distance par champion), chiffres de dégâts flottants (physique orange / magique bleu / brut blanc, crits plus gros), level-up, rappel, morts, respawn, buffs (stun étoiles, slow, root, bouclier), buffs bleu/rouge, buff Léviathan, Drake Ancien, souffle des drakes (couleur élémentaire), explosion de tour/inhibiteur/nexus, gain d'or (pièces).

### M4 — polish
12. Revue de chaque sort en jeu, cohérence visuelle, lisibilité en teamfight (FX ennemis légèrement teintés rouge), perf (budget particules, pas d'allocation par frame, 60 fps).

## Tests
Tests vitest pour la logique pure (courbes, pool, registre). `?fxlab` : boutons pour déclencher chaque FX de chaque champion sur des mannequins.
