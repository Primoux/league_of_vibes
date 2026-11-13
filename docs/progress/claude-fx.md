# Progress — claude-fx

## Fait

### M1 — boîte à outils FX ✅ (2026-11-13)
- **Particules GPU** (`fx/particles/`) : quads instanciés, trajectoire analytique dans le vertex shader (p0 + v·k(t) − ½g·t², traînée exponentielle), courbes couleur/alpha/taille cuites dans une `DataTexture`, tampon circulaire avec téléversement partiel (`addUpdateRange`). 2 lots (additif / alpha), 16 384 + 4 096 particules, **aucun travail CPU par particule et par frame**. Atlas de 16 textures procédurales (canvas) : lueur, étincelle, fumée, anneau, étoile, flocon, plume, rune, éclat, point, traînée, croix, pièce, flamme, spirale, scintillements. 19 styles standard (`styles.ts`).
- **Shaders** (`fx/shaders/`) : décal au sol paramétrique (cercle, anneau/onde de choc, rectangle, cône, remplissage progressif, motif animé), ruban de traînée (face caméra ou à plat), fresnel/rim, givre à facettes, eau stylisée, distorsion de chaleur (couche dédiée), **dissolve** injecté dans n'importe quel matériau standard + ombre qui se dissout (`applyDissolve`).
- **Post-process** (`fx/post/PostFx.ts`) : rendu HDR (HalfFloat, MSAA 4×), distorsion, bloom par luminance (seuil 0,92, les FX utilisent des couleurs HDR), tone mapping/sRGB, FXAA. Qualité basse = rendu direct.
- **Registre** : `registerFx(fxKey, { cast, projectile, hit, area, dash, buff, buffEnd, summoner })` ; phases non déclarées → **FX génériques** (`defaults.ts`) aux couleurs du sort. Couleur `vivid` automatique pour que les palettes sombres (Nyx, Grondak, Corvin) restent lisibles. `onWorldEvent(type, fn)` pour les événements sans fxKey.
- **Synchronisation** : `fxBus` retarde par défaut les événements de `INTERP_DELAY_MS` (alignés sur l'affichage interpolé). `subscribe(fn, { immediate: true })` pour les recevoir sans délai.
- **Indicateurs de visée** (`fx/telegraph/`) : `fxIndicators.showAbility(def, caster, cursor)` calcule la forme à partir de `targeting` (ligne, cône, cercle, portée).
- **`?fxlab`** : mannequins, bouton par sort (P/Q/W/E/R × 10 champions) et par sort d'invocateur, aperçu des indicateurs au survol, cible mobile, spam de charge, bloom, stats (fps, effets, particules, draw calls). Paramètres d'URL pour captures reproductibles : `&cast=ilyra:W,grondak:R&every=2`, `&at=0.6` (pas fixe puis gel), `&aim=sable:W`, `&showcase=1` (vitrine des matériaux), `&spam=1`.
- **Tests** (`packages/client/test/fx/`) : courbes, pool, ring, rng, scheduler, bus retardé, résolution fxKey, palettes/couleur vive, dispatcher (spécifique / générique / arrêt des projectiles / événements du monde), formes de visée, synthèse d'événements pour les 50 sorts.

## En cours
- M2 : modèles low-poly + animations procédurales de Grondak et Ilyra, FX complets de leurs sorts.

## Bloqué / Besoins (des autres agents)
- **claude-client** (`GameRenderer`) — les FX s'adaptent déjà à leur absence, mais pour un rendu complet :
  - `onFrame(cb: (dt) => void)` : sinon les FX tournent dans leur propre `requestAnimationFrame` (1 frame de décalage possible).
  - `getEntityObject(id): Object3D | undefined` : position interpolée affichée. Sinon les FX lissent eux-mêmes les snapshots.
  - `setRenderPipeline(render: () => void)` : laisse `PostFx` remplacer `renderer.render(scene, camera)`. **Sans lui, pas de bloom en jeu** (le fxlab l'a déjà).
  - `registerChampionFactory((snap) => ChampionView)` pour les modèles de champions (M2).
  - Recommandé : `renderer.toneMapping = THREE.ACESFilmicToneMapping` (les FX sont réglés pour).
  - Indicateurs : appeler `fxIndicators.showAbility(def, casterPos, cursorPos)` en mode visée, `showRange(pos, range)` au survol, `hide()`.
- **codex-champions** :
  - `shared/src/data/buffs.ts` avec `fxKey` par buff (en attendant, la fxKey d'un buff = son id ; `setBuffFxResolver` permet de brancher la table).
  - Événements attendus par type de ciblage : voir `fx/lab/synth.ts` (c'est la référence de ce que les FX consomment). En particulier : zones retardées → `areaEffect` avec `duration` = délai ; zones persistantes → `areaEffect` avec `duration` = durée de vie ; skillshots → `projectileSpawn` puis `projectileHit` (même `id`, y compris en fin de portée sans cible) ; passifs déclenchés → `buffApplied` avec `buffId` = fxKey du passif.
  - `entity.anim = 'cast_Q'`… pendant les incantations (animations M2).
- **mistral-economy (audio)** : `fxBus.subscribe(fn)` livre désormais les événements **retardés** de 100 ms, synchronisés avec l'image. Passer `{ immediate: true }` pour l'ancien comportement.

## Contrats modifiés / API disponibles
- `contract:` `client/src/main.ts` (claude-client) : ajout additif de l'entrée `?fxlab` (import dynamique) et `installFx(renderer)` au lieu de `installFx(renderer.scene)`. Le reste du démarrage est inchangé.
- `fx/FxBus.ts` (à moi) : `dispatch(e, now?)`, `update(now?)`, `subscribe(fn, { immediate? })`, `delayMs`. Compatible avec l'appel existant `fxBus.dispatch(ev)`.
- API exportée par `client/src/fx/index.ts` : `installFx(host)`, `setFxQuality('high'|'low')`, `fxIndicators`, `registerFx`, `onWorldEvent`, `fxBus`, type `FxHost`.

## Notes techniques
- Aucune nouvelle dépendance npm (post-process et OrbitControls viennent de `three/examples/jsm`). Aucun asset externe : toutes les textures sont procédurales (rien à ajouter dans `ASSETS.md`).
