# Progress — mistral-ui

## Fait
- **M1** : Accueil (nom, créer/rejoindre, mode dev, état de connexion), Lobby (2 × 4 places, rejoindre une équipe en cliquant une place libre, rôle préféré, prêt, lancer, code de salle copiable, hôte, joueurs hors ligne), **Draft** (grille des 10 champions avec filtre par rôle, tour en cours + piste des 5 tours, anneau de chrono, survol des alliés visible, champions pris grisés/verrouillés, double-clic = verrouiller, fiche complète du champion : classe, rôles, difficulté, lore, barres de profil, kit P/Q/W/E/R avec valeurs par rang, choix des 2 sorts d'invocateur avec permutation automatique), **Chargement** (8 cartes, progression, astuces).
- **M2** : HUD bas (portrait + anneau d'XP + niveau + points dispo, PV avec bouclier et graduations, mana/énergie, régénération, 4 sorts avec rangs, recharge circulaire + secondes, grisé sans mana, bouton + → `levelAbility`, passif, D/F avec recharge, 6 objets + balise, or cliquable → boutique, buffs/debuffs, barre de rappel), infobulles détaillées (dégâts, ratios, valeur calculée selon les stats, coût, recharge avec accélération), score haut (kills, tours, drakes colorés, larves, chrono), KDA/CS perso, **kill feed**, **annonces** (premier sang, double/triple/quadra, série, « ennemi légendaire », arrêt de série, équipe anéantie, exécution, tours, inhibiteurs, drakes par élément, Léviathan, Colosse + clés `announcement` du serveur), cadres des alliés, **minimap** canvas (terrain, voies, rivière, murs, brousses depuis `map.ts`, structures, champions, sbires, monstres, balises, pings, cadre caméra, timers d'objectifs ; clic/glisser = caméra, clic droit = déplacement, Alt+clic = ping).
- **M3** : Boutique (P ou clic sur l'or : onglets dont « Conseillés » selon la classe, recherche sans accents, arbre de recette, « utilisé dans », prix réduit par les composants possédés, achat double-clic/clic droit, vente, annuler, désactivée hors fontaine sauf mort), tableau des scores (Tab maintenu), stats détaillées (C), écran de mort (désaturation du canvas + compte à rebours), écran de fin (Victoire/Défaite, tableaux, barres de dégâts, MVP, retour au salon), paramètres (Échap ou roue dentée : volumes, lancement rapide par touche, bords d'écran, caméra, timers, taille de l'interface, animations réduites, qualité graphique, raccourcis) persistés en localStorage.
- **M4 (partiel)** : animations CSS partout, HUD en `em` (720p → 4K + curseur de taille), sélecteurs fins (`useSelect`, `useMini`, `useWorld`) pour éviter de re-rendre tout l'arbre à 20 Hz, minimap redessinée seulement quand un snapshot arrive.
- Icônes **100 % générées en SVG** : portraits procéduraux par champion (palette + emblème de classe + motif), sorts, objets (déduits de `icon`/stats/catégorie, marchent pour les futurs objets), sorts d'invocateur, rôles.
- `?uilab` : faux serveur complet (lobby, draft qui avance, partie simulée avec sbires, sorts qui partent en recharge, achat/vente/annuler, rappel) + panneau d'événements. Paramètres d'URL pour les démos et captures : `?uilab=home|lobby|draft|draft3|final|loading|game|end|endlose`, `&panel=shop|scores|stats|settings`, `&ev=multi,tower,drake,...`, `&hidelab`, `&still` (coupe les animations).
- Tests : `packages/client/test/ui-logic.test.ts` (formatage, prix de recette récursif, verdicts d'achat, filtre, rangs de sorts, tri du tableau, kill feed et annonces).

## En cours / à faire
- Polish après les premiers tests en vrai avec le serveur.
- Chat d'équipe (le protocole a `chat`) et roue de pings.

## Intégration avec claude-client (fait)
- Panneaux pilotés par `store.shopOpen / scoreboardOpen / statsOpen / settingsOpen` (l'input gère P, Tab, C, Échap ; l'UI n'écoute plus le clavier).
- Minimap → `store.cameraFocus` (+ `cameraLocked: false`) ; cadre lu dans `store.cameraCenter` (taille de vue approximative).
- Chargement → `store.loadingEndsAt` ; comptes à rebours (draft, chargement) via `clock.serverNow()`.
- Réglages de jeu (lancement rapide, qualité, bords d'écran, vitesse caméra) → `store.setSettings` ; réglages UI (volumes, taille de l'interface, timers, animations réduites) dans `settings` (`ui/settings.ts`, clé localStorage `lov.ui`).
- `store.snapshot.entities` = vue complète : le cache UI (`ui/game/world.ts`) est reconstruit à chaque snapshot + mémoire des champions pour le tableau des scores (brouillard).
- `?uilab` passe par `net.setLoopback` / `net.inject` : les faux snapshots traversent le vrai pipeline (fusion, horloge, fxBus) et s'affichent dans le moteur 3D.

## Session 2 — après les PR #6 à #9
- Fin de partie : « Retour au salon » envoie `{ t: 'returnToLobby' }` (demande claude-server).
- Minimap : brouillard de guerre via `getRenderer()?.fog.canvas` (demande claude-client).
- Objets actifs : clic sur un objet actif du HUD → `{ t: 'useItem', slot, targetPos }` (contrat de mistral-economy) ; clic sur la balise → mode ciblage `ward` ; charges et recharge de la balise (`self.wardCharges`).
- Boutique : objets `shop: false` masqués (Œil du colosse) ; icônes générées pour les 35 objets (couronne, hache, encensoir, voile…).
- Buffs : noms et buff/debuff lus dans le registre partagé `BUFFS`, une seule table locale (`buffName`, `isDebuff` dans `logic/format.ts`) — remarque de Fable.
- Noms originaux : « Golems de pierre » (au lieu de Krugs), « Gardien d’azur » / « Bête écarlate » pour les buffs de camp — remarque de Fable.
- Polices : piles de secours déjà en place (`'Cinzel', 'Times New Roman', serif` et `'Inter', system-ui, …`) avec `display=swap` — remarque de Fable.
- Draft : grille adaptative (11 champions).

## Bloqué / Besoins (des autres agents)
- **claude-client** : les touches **1, 2, 3, 5, 6, 7** devraient envoyer `{ t: 'useItem', slot }` (slots 0 à 5) ; aujourd'hui seul le clic dans le HUD le fait.
- **claude-client** : exposer la taille réelle de la vue caméra (ex. `store.cameraView = { w, h }` en unités de jeu, mis à jour en silence) pour un cadre de minimap exact. Optionnel.
- **mistral-economy** : exporte `setVolume(channel, v)` depuis `audio/AudioManager.ts` (canaux `master`, `sfx`, `voice`, `music`, `ui`, valeurs 0..1) : l'UI l'appelle automatiquement au démarrage et à chaque changement. Pour les objets : un champ `icon` parlant (`sword`, `tome`, `boots`, `hourglass`, `shield`…) suffit pour l'icône générée. Inventaire attendu : `champ.items` de longueur 7, index 6 = trinket.
- **Annonces** : l'UI dérive elle-même premier sang, multi-kills, séries, arrêt de série, équipe anéantie, tours, inhibiteurs et objectifs à partir de `death`/`structureDestroyed`/`objectiveTaken`. **Ne pas** émettre d'`announcement` pour ceux-là (doublons). Clés `announcement` reconnues : `welcome`, `minionsSoon`, `minionsSpawn`, `dragonSpawn`, `elderSpawn`, `grubsSpawn`, `heraldSpawn`, `leviathanSpawn`, `inhibitorRespawn`, `dragonSoul`, ou toute clé avec `params.text`.

## Contrats modifiés / API disponibles
- `contract:` `packages/shared/src/index.ts` : ajout de `export * from './data/buffs.ts'` (le registre `BUFFS` n'était pas exporté par `@lov/shared`). Ajout pur, aucun renommage.
- API exposées par l'UI : `settings` (réglages UI + volumes), `ui` (kill feed, annonce en cours), `focusCamera(pos)`.

## Environnement
- La machine de dev avait Node 12 : Node 22 installé dans `~/.local/node22` (`export PATH=~/.local/node22/bin:$PATH`). Le lockfile ne contient pas le binding Linux de rolldown : `npm install --no-save @rolldown/binding-linux-x64-gnu@1.2.19` (sans modifier le lockfile).
