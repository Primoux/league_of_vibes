# Progress — claude-fx

## Fait

### M1 — boîte à outils FX ✅ (2026-11-13)
- **Particules GPU** (`fx/particles/`) : quads instanciés, trajectoire analytique dans le vertex shader (p0 + v·k(t) − ½g·t², traînée exponentielle), courbes couleur/alpha/taille cuites dans une `DataTexture`, tampon circulaire avec téléversement partiel (`addUpdateRange`). 2 lots (additif / alpha), 16 384 + 4 096 particules, **aucun travail CPU par particule et par frame**. Atlas de 16 textures procédurales (canvas) : lueur, étincelle, fumée, anneau, étoile, flocon, plume, rune, éclat, point, traînée, croix, pièce, flamme, spirale, scintillements. 19 styles standard (`styles.ts`).
- **Shaders** (`fx/shaders/`) : décal au sol paramétrique (cercle, anneau/onde de choc, rectangle, cône, remplissage progressif, motif animé), ruban de traînée (face caméra ou à plat), fresnel/rim, givre à facettes, eau stylisée, distorsion de chaleur (couche dédiée), **dissolve** injecté dans n'importe quel matériau standard + ombre qui se dissout (`applyDissolve`).
- **Post-process** (`fx/post/PostFx.ts`) : rendu HDR (HalfFloat, MSAA 4×), distorsion, bloom par luminance (seuil 0,92, les FX utilisent des couleurs HDR), tone mapping/sRGB, FXAA. Qualité basse = rendu direct.
- **Registre** : `registerFx(fxKey, { cast, projectile, hit, area, dash, buff, buffEnd, summoner })` ; phases non déclarées → **FX génériques** (`defaults.ts`) aux couleurs du sort. Couleur `vivid` automatique pour que les palettes sombres (Nyx, Grondak, Corvin) restent lisibles. `onWorldEvent(type, fn)` pour les événements sans fxKey.
- **Synchronisation** : `net/client.ts` (claude-client) publie déjà les événements quand le rendu interpolé atteint leur snapshot ; `fxBus.delayMs` vaut donc 0. File de retard optionnelle conservée (`delayMs`, `subscribe(fn, { immediate: true })`).
- **Indicateurs de visée** (`fx/telegraph/`) : `fxIndicators.showAbility(def, caster, cursor)` calcule la forme à partir de `targeting` (ligne, cône, cercle, portée).
- **`?fxlab`** : mannequins, bouton par sort (P/Q/W/E/R × 10 champions) et par sort d'invocateur, aperçu des indicateurs au survol, cible mobile, spam de charge, bloom, stats (fps, effets, particules, draw calls). Paramètres d'URL pour captures reproductibles : `&cast=ilyra:W,grondak:R&every=2`, `&at=0.6` (pas fixe puis gel), `&aim=sable:W`, `&showcase=1` (vitrine des matériaux), `&spam=1`.
- **Tests** (`packages/client/test/fx/`) : courbes, pool, ring, rng, scheduler, bus retardé, résolution fxKey, palettes/couleur vive, dispatcher (spécifique / générique / arrêt des projectiles / événements du monde), formes de visée, synthèse d'événements pour les 50 sorts.

### M2 — champions Grondak et Ilyra ✅ (2026-11-13)
- **Modèles procéduraux** (`render/champions/`) : squelette de 13 os (`rig.ts`), pièces low-poly fusionnées en **un seul `SkinnedMesh` à skinning rigide** (1 os par sommet, poids 1) + contour par coque inversée skinnée + parties lumineuses HDR : **3 draw calls par champion**. Couleurs de la `palette`, ceinture/liseré à la couleur d'équipe.
- **Animations procédurales** (`anim/pose.ts`, `anim/clips.ts`, logique pure testée) : idle, run (cadence selon la vitesse affichée), attackReady (armement tenu) + strike (déclenché par l'événement `autoAttack`, donc calé sur l'impact serveur), cast / cast_<slot>, cast_R, dash, death, recall, stunned. Fondu enchaîné entre clips, repli `cast_Q → cast → idle`.
- **Mort** : chute puis dissolve (modèle **et ombre**), invisible jusqu'à la réapparition.
- **Grondak** (golem de granit, fissures de magma) et **Ilyra** (mage de givre, bâton, éclats en orbite) : modèles et clips dédiés. Les 8 autres champions ont un **modèle générique animé** par classe (arme selon la classe) en attendant M3.
- **FX dédiés** (`fx/champions/`) :
  - Grondak — P coque ambrée + éclats en orbite (jusqu'à `buffRemoved`) ; Q fissure au sol + pics de roche jaillissants ; W ondes de provocation + armure de pierre ; E ruée dans la poussière + impact étourdissant (étoiles) ; R sol qui tremble puis éruption de pics et débris.
  - Ilyra — P prison de glace puis éclatement ; Q éclat de glace tournoyant + sillage de neige ; W runes de givre puis couronne de pics de glace ; E flux de givre + bouclier à éclats ; R blizzard tourbillonnant, éclats qui tombent, brume.
- **Auto-attaques** (`fx/world/autoAttacks.ts`) : projectiles à tête chercheuse (vitesse `autoAttack.speed`), impacts de mêlée, crits renforcés ; variantes Grondak/Ilyra ; tirs de tours et de sbires aux couleurs d'équipe.
- Nouvelles primitives : `MeshBurstSystem` (pics/débris instanciés : roche, glace), auras annulables par clé (`rt.setAura/endAura`, boucliers), étoiles d'étourdissement, brume.
- fxlab : vrais modèles (le lanceur prend le modèle du champion joué, 3 cibles rouges), bouton AA, mort/réapparition, rappel ; paramètres `&champ=`, `&close=1|2`, `&cast=…:AA|DIE|RECALL`.

### M3 — en cours
- **fxlab piloté par la vraie simulation serveur** (`fx/lab/labSim.ts`) : World + systèmes de combat de claude-server et codex-champions importés en lecture seule. Le lab reçoit exactement les événements du jeu (journal affiché dans le panneau) ; champions niveau 18 + un allié pour les sorts de soutien. `synth.ts` (simulation maison) supprimé.
- FX alignés sur les événements réels : fxKey portée par `abilityCast` / `buffApplied` / `buffRemoved` / `summonerCast` ; zones retardées `<clé>_telegraph` puis `<clé>` ; buffs de sort distingués par `buffId`.
- **Statuts génériques** (`fx/world/status.ts`), par `buffId`, colorés par le sort : stun, root, slow (`*_slow`), knockup (+ parabole du modèle), taunt, fear, silence, blind, suppress, grievous, invisible (+ modèle translucide), revealed.
- **Événements du monde** (`fx/world/events.ts`) : chiffres flottants GPU (dégâts physiques/magiques/bruts, crits, dégâts subis en rouge, soins, or — seulement ce qui concerne le joueur local), level-up, âme à la mort, réapparition, canal de rappel (réussi / annulé), destruction de tour / plaque / inhibiteur / nexus, objectifs pris/apparus (couleur d'élément), auras de buffs (bleu, rouge, Léviathan, Ancien), brûlures.
- **9 sorts d'invocateur** (`fx/summoners.ts`).

## En cours
- M3 : modèles et FX dédiés des 8 autres champions.

## Bloqué / Besoins (des autres agents)
- **claude-client** (`GameRenderer`) — `onFrame`, `getEntityObject`, `registerChampionFactory` et ACES sont déjà là et utilisés automatiquement par `installFx(renderer)`. Il manque :
  - `setRenderPipeline(render: () => void)` : laisse `PostFx` remplacer `this.renderer.render(this.scene, this.camera)` dans `frame()`. **Sans lui, pas de bloom ni de FXAA en jeu** (le fxlab les a déjà). Implémentation suggérée : un champ `private renderFn = () => this.renderer.render(this.scene, this.camera)` remplaçable, et `PostFx.setSize` appelé sur resize (déjà géré côté FX via `resize`).
  - Indicateurs : appeler `fxIndicators.showAbility(def, casterPos, cursorPos)` en mode visée, `showRange(pos, range)` au survol, `hide()` (export de `client/src/fx/index.ts`).
- **claude-client** — optionnel : une API `shake(intensité, durée)` sur la caméra (impacts de R, explosions de structures).
- **codex-champions** :
  - `shared/src/data/buffs.ts` avec `fxKey` par buff (en attendant, la fxKey d'un buff = son id ; `setBuffFxResolver` permet de brancher la table).
  - Événements attendus par type de ciblage : voir `fx/lab/synth.ts` (c'est la référence de ce que les FX consomment). En particulier : zones retardées → `areaEffect` avec `duration` = délai ; zones persistantes → `areaEffect` avec `duration` = durée de vie ; skillshots → `projectileSpawn` puis `projectileHit` (même `id`, y compris en fin de portée sans cible) ; passifs déclenchés → `buffApplied` avec `buffId` = fxKey du passif.
  - Les incantations sont animées à partir de l'événement `abilityCast` (pas besoin de `entity.anim = 'cast_Q'`, mais c'est géré aussi).
  - Boucliers à durée variable (passif de Grondak) : émettre `buffRemoved` quand le bouclier casse, les FX s'arrêtent dessus.
  - Étourdissements : les FX lisent `snap.buffs` (ids `stun`, `knockup`, `suppress`, `taunt`, `fear`) pour l'animation d'étourdissement.

- **codex-objectives** : les buffs posés via `world-ai/runtime.ts` n'ont pas de `fxKey` ; les FX les reconnaissent par `buffId` : `blue_buff`/`blue`, `red_buff`/`red`, `leviathan`/`leviathan_buff`, `elder`/`elder_buff`, `red_burn`, `elder_burn`, `elder_execute`. Merci de garder ces ids (ou de me prévenir).

## Contrats modifiés / API disponibles
- `contract:` `client/src/main.ts` (claude-client) : ajout additif de l'entrée `?fxlab` (import dynamique) et `installFx(renderer)` au lieu de `installFx(renderer.scene)`. Le reste du démarrage est inchangé.
- `fx/FxBus.ts` (à moi) : `dispatch(e, now?)`, `update(now?)`, `subscribe(fn, { immediate? })`, `delayMs` (0 par défaut). Compatible avec l'existant : `fxBus.dispatch(ev)` / `fxBus.subscribe(fn)` se comportent comme avant.
- API exportée par `client/src/fx/index.ts` : `installFx(host)`, `setFxQuality('high'|'low')`, `fxIndicators`, `registerFx`, `onWorldEvent`, `fxBus`, type `FxHost`.

## Notes techniques
- Aucune nouvelle dépendance npm (post-process et OrbitControls viennent de `three/examples/jsm`). Aucun asset externe : toutes les textures sont procédurales (rien à ajouter dans `ASSETS.md`).
