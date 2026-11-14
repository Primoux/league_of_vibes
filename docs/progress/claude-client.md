# Progress — claude-client

## Fait
### M1 (moteur client)
- **Buffer de snapshots + interpolation** (`net/snapshots.ts`) : vue fusionnée, snapshots complets ou delta, historique ring buffer par entité, interpolation à `INTERP_DELAY_MS`, extrapolation bornée (120 ms), pas d'interpolation sur téléportation/respawn.
- **Horloge** (`net/clock.ts`) : offset serveur estimé depuis `snapshot.serverTime` (max glissant + lissage).
- **Prédiction locale** (`net/prediction.ts`) : ligne droite vers le clic, réconciliation douce (recalage > 250 u), désactivée si mur sur la trajectoire, pendant un dash (événement `dash`), ou si le serveur reste immobile.
- **Caméra** (`render/CameraController.ts`) : iso 56°, verrouillée/libre (Y), bords d'écran, flèches, molette bornée, Espace maintenu, `store.cameraFocus` (clic minimap), limites de carte.
- **Input complet** (`input/controls.ts`) : clic droit attaque/déplacement (+ maintien = suivi du curseur), marqueurs de clic, A+clic, QWER normal/quick-cast (Maj inverse), Ctrl+QWER, D/F, 4 (balise), B, S, Y, P, Tab (maintenu), C, Échap, Alt+clic ping, curseurs contextuels.
- **Picking** (`render/picking.ts`) : rayon / cylindres verticaux par type d'entité.
- **Reconnexion auto** (backoff, `hello` + token) et reprise de session au rechargement.
- Démo **`?maplab`** : mini-serveur local (latence `?lag=`, `?jitter=`), champion contrôlable, sbires, monstres.
- Tests : `packages/client/test/{net,render}.test.ts`.

### M2
- **Carte** (`shared/src/map.ts`) : jungle redessinée — quadrant ouest défini à la main, quadrant sud = reflet sur la diagonale (jungle équilibrée), équipe rouge = rotation 180°. Couloirs 350–600 u, entrées de camps, brousses de voie/rivière/mid. Tests `shared/test/map.test.ts` : convexité CCW, symétrie, voies dégagées, structures/camps dégagés, **flood fill** (tout accessible, aucune poche isolée).
  - ⚠️ positions des camps légèrement modifiées (ids inchangés) ; ajout de `RIVER_HALF_WIDTH`, `LANE_HALF_WIDTH`, `polygonArea`.
- **Rendu carte** (`render/MapView.ts`, `render/map/*`) : terrain avec relief (rivière creusée, `terrainHeight(x,y)` exporté), texture peinte (herbe, voies, dallage des bases, fosses), rivière en shader animé, murs = socle rocheux + forêt instanciée, bordure de forêt, rochers, brousses = herbes hautes au vent (transparentes quand le joueur local est dedans), fontaines.
- **Modèles low-poly animés** (`render/models/*`) : tours/inhibiteurs/nexus par équipe, 4 sbires, loups, raptors, krugs, gromp, golems de buff, crabe, 6 drakes + Ancien (élément lu depuis `objectiveSpawn`), larves, Colosse, Léviathan. Rig procédural (idle/run/attack/cast/death, effondrement des structures).
- **Barres de vie** (`render/HealthBars.ts`) : 1 draw call instancié, PV/bouclier/mana, graduations 100 PV, couleurs soi/allié/ennemi/neutre, étiquette niveau + nom des champions.
### M3
- **Brouillard de guerre** (`render/FogOfWar.ts`) : visibilité calculée depuis les alliés (rayons PLAN §9), fondu à la perte de vision, appliqué au décor par injection de shader (`applyFog(material)`). `renderer.fog.canvas` réutilisable par la minimap.
- **Indicateurs** (`render/Indicators.ts`) : cercle AA (survol de son champion / attaque-déplacement), indicateurs de sort par défaut (portée, skillshot, zone, cône, dash).
- Démos : `?maplab&zoo` (tous les modèles), `?maplab&cx=..&cy=..` (position caméra), `?maplab&aim=Q` (indicateur).

### M4
- **MeshBatcher** (`render/MeshBatcher.ts`) : les vues procédurales (sbires, monstres, structures, balises) gardent leur hiérarchie animée, mais leurs pièces sont dessinées en InstancedMesh partagés par (géométrie, matériau). Les champions de claude-fx ne sont pas touchés.
- **Végétation en tuiles** de 2000 u (frustum culling), arbres hors carte sans ombre portée ; terrain 200² segments.
- **Qualité basse/haute à chaud** (`store.settings.quality` → `GameRenderer.applyQuality`) : pixel ratio, ombres, décor de fond ; `setFxQuality` (claude-fx) suit via `main.ts`.
- Mesures, scénario `?maplab&bench` (8 champions + 60 sbires) avec `?stats` :
  | | draw calls | triangles |
  |---|---|---|
  | avant M4 | 1356 | 594k |
  | haute | 413 | 249k |
  | basse | 212 | 98k |
- Plus d'allocation par frame dans les étiquettes ; `applySnapshot` ne tourne qu'une fois par snapshot.
- Retour de revue PR #1 : élément du drake appliqué avant la création de la vue (et vue reconstruite si périmée).
- **`?autoplay`** (dev) : connexion + salle dev + prêt + lancement automatiques. Avec `SKIP_DRAFT=1` côté serveur : partie solo sans clic. Validé de bout en bout avec le vrai serveur.
- `?stats` : overlay fps / draw calls / triangles (`?stats=log` : aussi dans la console).

### Demandes des autres agents
- **mistral-economy** : `audio.setListener(() => store.state.cameraCenter)` câblé dans `main.ts` (objet mis à jour en place chaque frame, aucune allocation).
- **claude-fx** : `renderer.shake(intensité, durée)` / `shakeCamera(intensité, durée)` (export de `render/GameRenderer.ts`) — intensité = déplacement max en unités de jeu (≈ 15 léger, 40 fort, borné à 80), décroissance quadratique, un tremblement plus faible ne remplace pas un plus fort en cours.
- **mistral-ui** : `store.state.cameraView` (silencieux, chaque frame) = emprise réelle de la vue au sol en unités de jeu : `quad` (trapèze bas-gauche, bas-droite, haut-droite, haut-gauche) + `minX/minY/maxX/maxY/w/h`. Le cadre de la minimap peut remplacer son approximation (`Minimap.tsx`, ton fichier).

- **mistral-ui** : touches 1 2 3 5 6 7 → `useItem` (slots 0 à 5, `targetPos` = curseur), 4 → balise, par position physique (`e.code`).

### Diagnostic réseau (`packages/client/tools/netcheck.ts`)
- Fait tourner le vrai pipeline client (SnapshotBuffer, ServerClock, MovePredictor) à 60 Hz contre le vrai serveur, avec N joueurs et latence/gigue simulées. Mesure marge du tampon, extrapolation, saccades, erreur et recalages de prédiction, cadence/débit des snapshots.
  `SKIP_DRAFT=1 npm run dev:server` puis `npx tsx packages/client/tools/netcheck.ts --players 8 --seconds 40 --lag 60 --jitter 30`.
- **Bug trouvé et corrigé** : l'horloge ajoutait RTT/2 à l'offset, le rendu visait le temps serveur réel et rognait le délai d'interpolation → 44,5 % de frames extrapolées (marge p50 4 ms). Après : **0,1 %**, marge p50 62 ms, 0 saccade, erreur de prédiction p95 34 u (8 joueurs, 60 ms ± 15).
- Débit mesuré : ~38 Ko/s par client (JSON décompressé) — au-dessus de la cible PLAN de 30 Ko/s (domaine claude-server ; la compression permessage-deflate le réduit déjà sur le fil).
- `fix(server)` séparé : `useItem` manquait dans `server/src/net/validate.ts` (retour de revue PR #15), ajout additif + test `server/test/validate-useitem.test.ts`.

## En cours
- Mesures netcheck sur parties longues (sbires, combats) et plus de latence. Pistes : LOD des modèles si besoin (non nécessaire après le batching), mesures fps sur machine réelle.

### Intégrations
- `GameRenderer.setRenderPipeline(fn)` pour PostFx (claude-fx) ; indicateurs de claude-fx branchés via `registerIndicatorRenderer(fxIndicatorRenderer(fxIndicators))` dans `main.ts`.
- RTT : `netPing` toutes les 2 s → `store.rtt` (lissé), utilisé par l'horloge et la prédiction.

## Bloqué / Besoins (des autres agents)
- **claude-server** : ⚠️ j'ai rendu 4 tests (`server/test/nav.test.ts`, `combat.test.ts`) indépendants de la géométrie de `map.ts` (commit `test(server)` séparé) : ils codaient en dur l'ancien layout provisoire et cassaient avec la nouvelle jungle. À relire / reprendre si tu préfères une autre approche.
- **claude-fx** : tes décals d'indicateurs sont posés à y = 0 ; le terrain a un relief léger (rivière creusée jusqu'à −32 u). `terrainHeight(x, y)` (`render/map/geometry.ts`) donne la hauteur du sol si besoin.
- **mistral-ui** : minimap → `store.set({ cameraFocus: { pos, seq: n + 1 } })` pour déplacer la caméra, `store.state.cameraCenter` pour le cadre, `getRenderer()?.fog.canvas` pour le brouillard.

## Contrats modifiés / API disponibles
### `store` (`client/src/state.ts`)
- Existants : `connected, playerId, name, lobby, draft, game, snapshot, result, error`.
- `snapshot.entities` = **vue complète fusionnée** (deltas appliqués). `snapshot.events` = événements du dernier snapshot.
- Ajouts : `loadingEndsAt`, `selfEntity`, `hover` (id survolé), `cursorGame` (silencieux), `targeting` (ciblage en cours : `{kind:'ability',slot}` / `summoner` / `attackMove` / `ward`), `cameraLocked`, `cameraCenter` (silencieux, pour le cadre minimap), `cameraFocus` ({pos, seq} : incrémenter `seq` pour déplacer la caméra), `shopOpen`, `scoreboardOpen`, `statsOpen`, `settingsOpen`, `settings` ({quickCast, quality, edgePan, cameraSpeed} → `store.setSettings(patch)`, persisté), `rtt`.
- `store.setQuiet(key, value)` : mise à jour sans notification (champs haute fréquence).
### `GameRenderer` (`render/GameRenderer.ts`)
- `scene`, `camera`, `renderer`, `toWorld(p,h)`, `toGame(v)`, `getEntityObject(id)`, `pickEntity(x,y,filter?)`, `pickGround(x,y)`, `onFrame(cb(dt)) → unsubscribe`, `interpolatedEntity(id)` (aussi exporté en fonction module), `getRenderer()` (instance active).
- `renderer.aim` (`AimState`, mis à jour chaque frame) et **`registerIndicatorRenderer({ update(aim, dt) })`** pour claude-fx : remplace les indicateurs par défaut. `AimState` = { active, kind, slot, targeting, abilityId, fxKey, origin, cursor, clamped (borné à la portée), range, radius, width, hover, attackRange }.
- `renderer.fog.canvas` (visibilité, blanc = visible, 192², y du jeu vers le haut) ; `applyFog(material)` pour brouiller un matériau de décor.
- `terrainHeight(x, y)` (`render/map/geometry.ts`) : hauteur du sol en unités de jeu (poser un FX au sol : `y = terrainHeight * RENDER_SCALE`).
- `renderer.shake(intensity, duration)` / `shakeCamera(...)` ; `store.state.cameraView` (emprise de la vue au sol).
- `registerChampionFactory((snap) => ChampionView | null)` (`render/EntityView.ts`, réexporté) ; `ChampionView = { object3d, update(snap, dt), playAnim(name), dispose() }`. Le renderer place `object3d` au sol (y = 0) et règle `rotation.y = facing` (le modèle regarde +x local). Sans fabrique : capsule colorée.
### `net` (`net/client.ts`)
- `fxBus` reçoit les `GameEvent` **synchronisés avec le rendu interpolé** (~100 ms après réception). L'UI peut lire `store.snapshot.events` immédiatement.
- `snapshots` (SnapshotBuffer), `clock`, `renderTime()`, `net.inject(msg)` / `net.setLoopback(fn)` pour démos hors ligne.

### Jouabilité navigateur (branches `feat/client-playability` + `feat/ui-playability`)
- **Cast au relâchement** : en cast normal, maintenir QWER/DF affiche l'indicateur, relâcher lance (clic gauche fonctionne toujours, clic droit/Échap annulent).
- **Mode jeu** (`input/gameMode.ts`) : plein écran de la page (la barre d'onglets ne réapparaît plus au bord de l'écran) + Keyboard Lock (Chromium : Ctrl+W/Ctrl+Q capturés). F11 intercepté. Auto au premier clic en partie (`settings.autoFullscreen`).
- **Gardes navigateur** en partie : F1/F3/F5/F6/F7/F10, Ctrl+…, Alt+←/→, Alt relâché, boutons Précédent/Suivant, clic molette, Ctrl+molette, menu contextuel, sélection, glisser ; `beforeunload` (`settings.confirmLeave`) ; verrou d'historique (popstate).
- **Alt+QWER** (en plus de Ctrl) pour monter un sort ; lettres via `keyLetter` (disposition clavier, Option macOS).
- **Clic molette maintenu** = glisser la caméra ; **² / `** maintenu = ne cibler que les champions ; option **attaque-déplacement au clic gauche**.
- Contrats : `Settings.{autoFullscreen, confirmLeave, leftClickAttackMove}`, `ClientState.{fullscreen, keyboardLocked}`, `DEFAULT_GAME_SETTINGS` exporté (ajouts uniquement).
