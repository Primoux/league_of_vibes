# YOUR_PLAN — mistral-ui (Mistral 1)

Branche : `mistral-ui`

## Mission
Tu construis **toute l'interface utilisateur** en Preact (overlay DOM au-dessus du canvas Three.js) : menus, lobby, **phase de pick**, chargement, **HUD** complet, minimap, boutique, tableau des scores, annonces, écran de fin, paramètres. Interface en français, belle, lisible, cohérente avec un MOBA (cadre doré sombre, mais **design original**, pas une copie de l'interface Riot).

## Tu possèdes
- `packages/client/src/ui/**` (composants, styles, icônes SVG générées, polices Google Fonts autorisées)
- Démo `?uilab` (UI alimentée par un faux état, sans serveur)

## Interdit
`server/**`, `shared/**` (sauf ajout `contract:` justifié), `client/src/{render,net,input,fx,audio}/**`, `state.ts` (claude-client : demande-lui des champs si besoin).

## Contrats consommés
- `useStore()` (`ui/useStore.ts`) → `ClientState` (`client/src/state.ts`) : `lobby`, `draft`, `game`, `snapshot` (entités, `self`, `events`, `score`, `objectiveTimers`), `result`, `error`.
- `net.send(ClientMsg)` (`client/src/net/client.ts`) pour toutes les actions (setTeam, setRole, setReady, startGame, draftHover, draftLock, draftSummoners, buy, sell, undoShop, levelAbility…).
- Données : `CHAMPION_LIST`, `SUMMONERS`, `ITEMS`/`ITEM_LIST` depuis `@lov/shared`.
- Événements (`snapshot.events`) pour kill feed et annonces : `death`, `structureDestroyed`, `objectiveTaken`, `announcement`, `levelUp`, `itemBought`.

## Jalons
### M1 — avec `?uilab` (faux état)
1. **Accueil** (nom, créer/rejoindre, mode dev), **Lobby** (2 colonnes d'équipe, 4 places chacune, rôles, prêt, lancer, code de salle copiable).
2. **Draft** : grille des 10 champions (portraits générés : initiale stylisée + palette du champion, ou rendu SVG), indication du tour en cours et du timer, survol visible des alliés, champions pris grisés, verrouillage, choix des 2 sorts d'invocateur, panneau des deux équipes, infos du champion (classe, rôles, kit avec descriptions des sorts).
3. **Chargement** : 8 cartes de champions.

### M2 — HUD
4. **HUD bas** : portrait + niveau + barre XP, PV/mana (avec bouclier), 4 sorts (icône, rang en points, cooldown en overlay circulaire + secondes, grisé si pas de mana, bouton + quand point disponible → `levelAbility`), D/F avec cooldown, 6 objets + trinket, or. Infobulles des sorts/objets avec valeurs.
5. **Haut** : score kills bleu/rouge, tours, drakes pris, chrono. **Kill feed** à droite. **Annonces** centrales (premier sang, double/triple/quadra kill, tour détruite, drakes, Léviathan, inhibiteur, victoire/défaite, « ennemi légendaire »…).
6. **Minimap** (bas droite) en canvas 2D : carte simplifiée (voies, rivière, murs depuis `map.ts`), structures, icônes de champions (initiale + couleur), sbires en points, cadre de caméra ; clic = déplacer la caméra (via store, coordonne avec claude-client), clic droit = déplacement. Timers d'objectifs.

### M3
7. **Boutique** (P ou clic sur l'or) : catégories, arbre de recette, recherche, achat/vente/annuler, prix réduits selon composants possédés, désactivée hors fontaine (`self.canShop`).
8. **Tableau des scores** (Tab) : 8 joueurs, KDA, CS, objets, sorts d'invocateur, niveau.
9. **Écran de mort** (désaturation + compte à rebours de respawn), **stats détaillées** (C), **écran de fin** (victoire/défaite, tableau complet, bouton retour lobby).
10. **Paramètres** : volume (canaux exposés par mistral-economy), quick-cast, qualité graphique (champ dans le store), raccourcis. Persistés en localStorage (try/catch).

### M4
11. Polish : animations CSS, responsive (1280×720 → 4K), accessibilité (contraste, tailles), perf (pas de re-render à 20 Hz de tout l'arbre : sélecteurs fins, memo).

## Tests
Tests vitest des fonctions pures (formatage, calcul de prix de recette, tri du tableau, kill feed). `?uilab` couvre tous les écrans.
