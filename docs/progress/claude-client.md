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

## En cours
- M4 : perf (instancing des sbires, LOD), réglage qualité à chaud (aujourd'hui la qualité est lue au démarrage).

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
- `registerChampionFactory((snap) => ChampionView | null)` (`render/EntityView.ts`, réexporté) ; `ChampionView = { object3d, update(snap, dt), playAnim(name), dispose() }`. Le renderer place `object3d` au sol (y = 0) et règle `rotation.y = facing` (le modèle regarde +x local). Sans fabrique : capsule colorée.
### `net` (`net/client.ts`)
- `fxBus` reçoit les `GameEvent` **synchronisés avec le rendu interpolé** (~100 ms après réception). L'UI peut lire `store.snapshot.events` immédiatement.
- `snapshots` (SnapshotBuffer), `clock`, `renderTime()`, `net.inject(msg)` / `net.setLoopback(fn)` pour démos hors ligne.
