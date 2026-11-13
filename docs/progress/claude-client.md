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

## En cours
- M2 : carte 3D détaillée, modèles low-poly, barres de vie.

## Bloqué / Besoins (des autres agents)
- **claude-server** : pas de message client de ping (le `ping` existant est le ping de carte). Pour afficher un RTT, il faudrait un `{ t: 'netPing'; clientTime }` → `pong`. En attendant `store.rtt` reste null et la prédiction suppose 80 ms.
- **claude-server** : merci de confirmer la sémantique delta : `removed` défini ⇒ snapshot delta (entités absentes conservées), sinon complet.

## Contrats modifiés / API disponibles
### `store` (`client/src/state.ts`)
- Existants : `connected, playerId, name, lobby, draft, game, snapshot, result, error`.
- `snapshot.entities` = **vue complète fusionnée** (deltas appliqués). `snapshot.events` = événements du dernier snapshot.
- Ajouts : `loadingEndsAt`, `selfEntity`, `hover` (id survolé), `cursorGame` (silencieux), `targeting` (ciblage en cours : `{kind:'ability',slot}` / `summoner` / `attackMove` / `ward`), `cameraLocked`, `cameraCenter` (silencieux, pour le cadre minimap), `cameraFocus` ({pos, seq} : incrémenter `seq` pour déplacer la caméra), `shopOpen`, `scoreboardOpen`, `statsOpen`, `settingsOpen`, `settings` ({quickCast, quality, edgePan, cameraSpeed} → `store.setSettings(patch)`, persisté), `rtt`.
- `store.setQuiet(key, value)` : mise à jour sans notification (champs haute fréquence).
### `GameRenderer` (`render/GameRenderer.ts`)
- `scene`, `camera`, `renderer`, `toWorld(p,h)`, `toGame(v)`, `getEntityObject(id)`, `pickEntity(x,y,filter?)`, `pickGround(x,y)`, `onFrame(cb(dt)) → unsubscribe`, `interpolatedEntity(id)` (aussi exporté en fonction module), `getRenderer()` (instance active).
- `registerChampionFactory((snap) => ChampionView | null)` (`render/EntityView.ts`, réexporté) ; `ChampionView = { object3d, update(snap, dt), playAnim(name), dispose() }`. Le renderer place `object3d` au sol (y = 0) et règle `rotation.y = facing` (le modèle regarde +x local). Sans fabrique : capsule colorée.
### `net` (`net/client.ts`)
- `fxBus` reçoit les `GameEvent` **synchronisés avec le rendu interpolé** (~100 ms après réception). L'UI peut lire `store.snapshot.events` immédiatement.
- `snapshots` (SnapshotBuffer), `clock`, `renderTime()`, `net.inject(msg)` / `net.setLoopback(fn)` pour démos hors ligne.
