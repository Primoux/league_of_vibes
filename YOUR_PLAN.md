# YOUR_PLAN — claude-client (Claude 2)

Branche : `claude-client`

## Mission
Tu construis le **moteur client 3D** : scène Three.js, caméra isométrique, carte, rendu des entités non-champions, input, réseau client avec interpolation et prédiction. Les autres agents client (claude-fx, mistral-ui, mistral-economy audio) se branchent sur ce que tu fournis.

## Tu possèdes
- `packages/client/src/render/**` **sauf** `render/champions/**` (claude-fx)
- `packages/client/src/net/**`, `packages/client/src/input/**`, `packages/client/src/state.ts`, `packages/client/src/main.ts`
- `packages/client/index.html`, `vite.config.ts`
- `packages/shared/src/map.ts` (géométrie de la carte : murs, brousses, positions)

## Interdit
`server/**`, `client/src/{ui,fx,audio}/**`, `render/champions/**`.

## Contrats que tu fournis (documente-les dans ton progress)
- `store` (`state.ts`) : état global + champ `hover` (entité survolée), `selfEntity`, `cursorGame` (position jeu du curseur).
- `GameRenderer` : `scene`, `camera`, `toWorld/toGame`, `getEntityObject(id)`, `pickEntity(x,y)`, `pickGround(x,y)`, hook `onFrame(cb(dt))`.
- **Interface champion** pour claude-fx : `registerChampionFactory((snap) => ChampionView)` où `ChampionView = { object3d, update(snap, dt), playAnim(name), dispose() }`. Tant que claude-fx n'a rien enregistré, tu affiches une capsule colorée.
- Interpolation : `interpolatedEntity(id)` → position/facing interpolés pour FX et UI (barres de vie, minimap).

## Jalons
### M1
1. **Buffer de snapshots + interpolation** (100 ms, `INTERP_DELAY_MS`), synchronisation d'horloge (ping/pong). Application des snapshots delta (`removed`).
2. **Prédiction locale** du déplacement du joueur (ligne droite vers le point cliqué, réconciliation douce avec le serveur).
3. **Caméra** : vue iso (angle ~56°), verrouillée/libre (Y), bords d'écran, molette (zoom limité), Espace (recentrer), clic minimap (via store), limites de carte.
4. **Input complet** (PLAN §10) : clic droit sur ennemi = `attack`, sur sol = `move` (+ marqueur de clic), A+clic = `attackMove`, QWER avec mode normal (indicateur puis clic gauche) et quick-cast, Ctrl+QWER, D/F, B, S, Alt+clic ping, P/Tab/C transmis à l'UI via store. Curseur contextuel (attaque sur ennemi).
5. **Picking d'entités** (raycast sur hitbox simplifiées).

### M2
6. **Carte 3D** : terrain avec relief léger, voies (texture/couleur), rivière animée (shader eau simple), murs/falaises extrudés stylisés, brousses (herbes hautes instanciées, transparence quand le joueur est dedans), bases et fontaines, décor (arbres/rochers instanciés hors chemins). Éclairage chaud + ombres. Respecter `WALLS`/`BRUSHES` de `map.ts` (affine-les pour une jungle jouable : chemins, entrées de camps, fosses fermées sur 3 côtés ; garde la symétrie `mirror` ; tests shared verts).
7. **Modèles low-poly** des tours (par équipe), inhibiteurs, nexus, sbires (mêlée/caster/canon/super, couleur d'équipe), monstres de jungle, drakes (6 variantes de couleur), larves, Colosse, Léviathan. Animations simples (marche, attaque, mort) procédurales.
8. **Barres de vie 3D** au-dessus des unités (sprites/instancing) : PV, bouclier, mana pour les champions, niveau, nom ; couleurs allié/ennemi.

### M3
9. **Brouillard de guerre** : texture de visibilité calculée côté client à partir des entités alliées (rayons PLAN §9), appliquée au terrain (shader), entités ennemies non visibles masquées, structures en « dernier état connu ».
10. **Indicateurs de portée** (cercle AA au survol, indicateurs de sort fournis par claude-fx via un hook : tu fournis la position/visée, eux le visuel).

### M4
11. Perf : instancing pour sbires/décor, LOD, frustum culling, 60 fps avec 8 champions + 60 sbires. Option qualité (basse/haute) dans le store.
12. Reconnexion auto côté client (relance `hello` avec le token).

## Tests
Tests vitest pour la logique pure (interpolation, buffer, conversion coordonnées, réconciliation). Démo `?maplab` : carte seule avec caméra libre.
