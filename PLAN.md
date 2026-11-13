# PLAN.md — League of Vibes

> Document de design complet du jeu. Commun à toutes les IA. Lis-le en entier avant de coder.
> Ta tâche précise est dans `YOUR_PLAN.md` (sur ta branche). Les règles de collaboration sont dans `AGENTS.md`.

## 1. Vision

**League of Vibes** est un MOBA 4v4 jouable dans le navigateur, avec **les mêmes mécaniques que League of Legends** :
phase de pick, 3 voies, jungle, sbires, tours, inhibiteurs, nexus, drakes élémentaires + âme + Drake Ancien,
larves, Colosse du Rift (≈ héraut), Léviathan (≈ baron), sorts d'invocateur, objets, brouillard de guerre.

- **8 joueurs humains en ligne** (pas de bots). Un mode dev (`?dev=1` côté client, `DEV_LOBBY=1` côté serveur) permet de lancer une partie avec moins de joueurs pour tester.
- **10 champions originaux**. Chaque champion n'est pris qu'une fois par partie.
- **Rendu 3D Three.js** en vue isométrique, low-poly stylisé, **FX soignés** (c'est un critère de qualité majeur).
- **Propriété intellectuelle** : on reprend les *règles*, jamais les noms, champions, visuels, sons ou la carte de Riot. Aucun nom « League of Legends », « Baron Nashor », « Rift Herald », « Voidgrubs », « Summoner's Rift » dans le jeu. Assets procéduraux ou CC0 uniquement.

## 2. Architecture

```
packages/
  shared/   types, protocole, schémas, données (champions, objets, sorts, carte, constantes) — source de vérité
  server/   Node + ws : serveur AUTORITAIRE (toute la logique de jeu tourne ici)
  client/   Vite + Three.js (rendu, FX) + Preact (UI en overlay DOM)
```

- **Serveur autoritaire** : le client n'envoie que des intentions (`ClientMsg` dans `shared/src/protocol.ts`). Le serveur simule à **30 ticks/s** et envoie des **snapshots à 20 Hz** (`Snapshot`) contenant les entités visibles par l'équipe du joueur, l'état privé du joueur (`SelfState`) et les **événements** du tick (`GameEvent`).
- **Client** : interpole les entités avec ~100 ms de retard, prédit localement le déplacement du joueur, joue les FX/sons à partir des `GameEvent`.
- **Unités** : valeurs « LoL-like » (portée AA mêlée 125–200, distance 525–650, vitesse 330–350, tours portée 750…). Carte 12 000 × 12 000. Rendu Three.js : 1 unité de jeu = 0,01 unité 3D ; coordonnée jeu `(x, y)` → monde `(x*0.01, h, -y*0.01)` (voir `toWorld` dans `client/src/render/GameRenderer.ts`).
- **Lancement** : `npm install`, `npm run dev` (serveur :3000 + Vite :5173 avec proxy `/ws`). Production : `npm run build && npm start` (le serveur sert `client/dist`).

### 2.1 Simulation serveur
- `World` (`server/src/world.ts`) : map d'entités, `EventBus`, file de commandes joueurs `world.commands` (lisible par tous les systèmes pendant le tick, vidée à la fin).
- `Entity` : champs communs (pos, hp, mana, shield, stats, team, kind, subtype, anim, moveTarget, path, attackTarget, champ). Les données propres à un domaine vont dans `entity.data.<domaine>` (ex. `data.minion`, `data.buffs`, `data.tower`).
- **Systèmes** exécutés dans cet ordre (`server/src/systems/index.ts`) :
  1. `input` — ordres de déplacement/attaque (claude-server)
  2. `buffs` — CC, buffs, recalcul des stats (codex-champions)
  3. `summoners` — sorts d'invocateur (codex-champions)
  4. `abilities` — sorts des champions, projectiles, zones, dashes (codex-champions)
  5. `movement` — pathfinding, collisions, dashes (claude-server)
  6. `combat` — auto-attaques, dégâts, morts, respawn, kills/assists (claude-server)
  7. `minions` — vagues et IA des sbires (codex-objectives)
  8. `structures` — tours, inhibiteurs, nexus, victoire (codex-objectives)
  9. `jungle` — camps (codex-objectives)
  10. `objectives` — drakes, larves, colosse, Léviathan (codex-objectives)
  11. `leveling` — XP et niveaux (codex-champions)
  12. `economy` — or, boutique, rappel, balises (mistral-economy)
  13. `vision` — visibilité par équipe (claude-server)
- **API de dégâts unique** (à fournir par claude-server dans `server/src/combat/damage.ts`) : `applyDamage(world, { source, target, amount, type, ability? })` qui applique résistances/pénétration, boucliers, émet `damage`, gère la mort (`death` avec killer/assists/bounty). **Tout le monde passe par cette fonction**. Idem `applyHeal`, `applyShield`.
- **API de buffs** (codex-champions, `server/src/combat/buffs.ts`) : `addBuff(world, target, buffId, { duration, source, stacks, params })`, `hasCC(entity, type)`, `canAct/canMove/canCast(entity)`. Tout le monde l'utilise pour les CC/buffs (ex. buff du Léviathan, buffs rouge/bleu).
- Les mocks sont autorisés tant qu'une API n'existe pas : crée-la avec la signature ci-dessus dans le fichier prévu et implémentation minimale, signale-le dans ton `docs/progress`.

### 2.2 Client
- `client/src/state.ts` : store global (lobby, draft, game, dernier snapshot). L'UI s'abonne via `useStore()`.
- `client/src/net/` : WebSocket, buffer de snapshots, interpolation, prédiction.
- `client/src/render/` : scène, caméra, carte, entités (structures, sbires, monstres). `render/champions/` : modèles et animations des champions (claude-fx).
- `client/src/fx/` : `fxBus` (reçoit chaque `GameEvent`), particules, shaders, effets par `fxKey`.
- `client/src/ui/` : tous les écrans et le HUD (Preact).
- `client/src/audio/` : sons et musique.
- `client/src/input/` : contrôles.

## 3. Flux d'une partie

1. **Accueil** : nom d'invocateur, créer une salle ou rejoindre par code (5 lettres).
2. **Lobby** : 8 places (2 × 4), changer d'équipe, choisir un rôle préféré (top/jungle/mid/bot), prêt. L'hôte lance quand 8 joueurs sont prêts (ou ≥1 en mode dev).
3. **Draft** (pick alterné, sans bans) : ordre **B1 – R1 R2 – B2 B3 – R3 R4 – B4** (1-2-2-2-1), 30 s par tour, picks simultanés dans un même tour. Survol visible par les alliés. Champion unique dans la partie. À expiration : pick aléatoire parmi les disponibles. Puis **15 s de finalisation** : choix des 2 sorts d'invocateur (défaut Flash + Ignite, Smite conseillé au jungler).
4. **Chargement** (5 s) : écran avec les 8 cartes de champions.
5. **Partie** : jusqu'à destruction d'un nexus. Reconnexion possible (même token de session).
6. **Fin** : Victoire/Défaite, tableau KDA, CS, or, dégâts, objets.

## 4. Carte « Faille des Vibes »

Données dans `shared/src/map.ts` (positions provisoires à affiner par claude-client).
- Origine en bas-gauche ; **base bleue** bas-gauche, **base rouge** haut-droite. Symétrie par rotation 180° (`mirror`).
- **3 voies** : top (bord gauche puis haut), mid (diagonale), bot (bord bas puis droit). **Rivière** sur la diagonale x+y=12000.
- **Fosse du dragon** dans la rivière côté bot, **fosse du Léviathan** côté top (les larves et le Colosse y apparaissent aussi).
- Par équipe : 11 tours (3 par voie + 2 de nexus), 3 inhibiteurs, 1 nexus, 1 fontaine (soins rapides, tir mortel sur les ennemis, boutique).
- **Jungle** par côté : buff bleu, grenouille, loups (côté top) ; buff rouge, raptors, krugs (côté bot). 2 crabes de rivière.
- Murs, brousses (cachent de la vision ennemie). Navigation : grille 50 u dérivée des murs.

### Rôles 4v4
top (solo), jungle, mid (solo), bot (solo, tireur ou support). Pas de rôle support dédié, mais Solenne est jouable bot/mid.

## 5. Champions

Stats de base, scalings et paramètres complets dans `shared/src/data/champions/*.ts` (fichier = source de vérité, ce tableau est un résumé).

| Champion | Classe | Rôles | Passif | Q | W | E | R |
|---|---|---|---|---|---|---|---|
| **Grondak**, le Rempart de Granit | Tank | top/jgl | Bouclier 10 % PV max après 8 s hors combat | Ligne + slow | Réduction dégâts + taunt AoE | Charge ciblée + stun | Zone retardée + knockup |
| **Tharox**, la Charge Sauvage | Tank | jgl/top | Élan : AA renforcée (%PV + slow) | Dash + knockback | Résistances + aura dégâts | AoE slow | Charge très longue + stun |
| **Kaelen**, la Lame de l'Aube | Combattant | top | 3e AA bonus + soin | Triple fente (3e knockup) | Parade (bloque + stun) | Dash + bouclier | Exécution (%PV manquants) |
| **Vrenna**, la Lance des Steppes | Combattant | jgl/top | Saignement cumulable | Ligne + attraction | Vitesse d'attaque + soin | Saut AoE + slow | Bond ciblé + knockup |
| **Ilyra**, la Sentinelle du Givre | Mage | mid | 3 cumuls de givre = stun | Skillshot slow | Nova retardée | Bouclier allié | Blizzard persistant |
| **Zephyr**, l'Arcaniste Instable | Mage | mid | Tous les 4 sorts : sort amplifié | Orbe aller-retour | Blink + explosion | Chaîne → root | 3 rayons longue portée |
| **Sable**, la Tireuse des Dunes | Tireur | bot | Crits renforcés, vitesse sur hit | AA explosive | Volée en cône | Roulade + crit garanti | Balle globale |
| **Corvin**, l'Archer des Corbeaux | Tireur | bot | 3 marques = %PV max | Flèche perçante | Pièges (root) | Saut arrière + vitesse | Zone de corbeaux + anti-soin |
| **Nyx**, l'Ombre Sans Nom | Assassin | mid/jgl | +20 % dégâts sur cible isolée | Lame ciblée (reset sur kill) | Invisibilité | Téléport derrière une unité | Marque qui explose |
| **Solenne**, la Prêtresse de l'Aube | Support | bot/mid | Soins amplifiés selon PV manquants | Rayon root | Soin allié + soi | Bouclier + AS | Soin + bouclier de zone |

Règles communes : niveaux 1–18, 1 point de compétence par niveau, R disponible aux niveaux 6/11/16, QWE max rang 5, un sort ne peut pas dépasser `ceil(level/2)` rangs. Ressource : mana (régén), énergie (Nyx, 200, +50/5 s… à équilibrer), aucune (Kaelen). Respawn : `6 + niveau * 2.5` secondes environ (scalé avec le temps de jeu, à équilibrer). XP : partagée entre champions alliés dans un rayon de 1400.

**FX** : chaque sort a une `fxKey` (`<champion>_<slot>[_variante]`, ex. `ilyra_r`). Le serveur émet `abilityCast`, `projectileSpawn/Hit`, `dash`, `areaEffect`, `buffApplied` avec ces clés ; le client joue l'effet correspondant.

## 6. Sorts d'invocateur
Flash (400 u), Ignite, Heal, Barrier, Exhaust, Teleport, Smite (requis pour l'or/xp complets en jungle, 600 dégâts bruts aux monstres), Ghost, Cleanse. Données : `shared/src/data/summoners.ts`.

## 7. Monde : sbires, structures, jungle, objectifs

Tous les timers sont dans `shared/src/constants.ts` (`TIMERS`), en valeurs LoL **non scalées**, multipliées par `TIME_SCALE = 0.75` via `scaled()`.

### Sbires
- Première vague à 1:05, puis toutes les 30 s, sur les 3 voies. Composition : 3 mêlée + 3 casters, + 1 canon une vague sur 3 (une sur 2 après 15:00). Inhibiteur ennemi détruit → super-sbire sur cette voie.
- Suivent `lanePathFor(lane, team)`. **Aggro LoL** : cible la plus proche ; si un champion ennemi attaque un champion allié à proximité, les sbires le ciblent (priorité : champion qui attaque un champion allié > sbire qui attaque un allié > champion qui attaque un allié > plus proche).
- Or/XP au last hit (or uniquement si le champion porte le coup final ; XP partagée si à proximité). Stats et or qui augmentent avec le temps.

### Structures
- **Tours** : portée 750, attaquent la cible prioritaire (sbires d'abord, sauf si un champion ennemi agresse un champion allié dans sa portée), dégâts croissants sur la même cible champion (+40 %/tir jusqu'à +120 %). Les sbires prennent des dégâts proportionnels à leur PV max. **Protection anti-backdoor** : une tour gagne +200 armure/RM quand aucun sbire ennemi n'est à moins de 1000 u. **Fortification** : les tours externes subissent −50 % de dégâts avant 3:00.
- **Plaques** : tours externes, 5 plaques jusqu'à 14:00 (non scalé : `towerPlatesUntil`), chaque plaque donne de l'or aux champions proches ; chaque plaque détruite donne de la résistance à la tour pour 30 s si plusieurs champions sont proches.
- **Ordre** : externe → interne → inhibiteur → inhibiteur (structure) → tours de nexus → nexus. Une structure n'est attaquable que si la précédente de la voie est détruite.
- **Inhibiteurs** : réapparaissent après 5 min (scalé). **Nexus** détruit = victoire (`world.winner`).
- Or global de tour et or local, annonces.

### Jungle
- Camps (`CAMPS`), premier spawn 1:30. Leash : si tirés au-delà de `leashRadius` ou sans cible pendant 5 s, reset (retour + régénération).
- **Buff bleu** (2 min, régén de mana + haste), **buff rouge** (2 min, AA brûlent + ralentissent). Transférés au tueur si le porteur meurt.
- Crabe de rivière : spawn 3:30, donne vision de la rivière (balise) et bonus de vitesse.
- Smite : tous les camps ; jungler avec Smite reçoit l'or/XP complets et de quoi soigner.

### Objectifs épiques
- **Drakes** : premier à 5:00, réapparition 5 min. Types : infernal (+AD/AP %), océan (régén PV), montagne (armure/RM), nuage (vitesse hors combat), hextech (haste + vitesse d'attaque), chemtech (ténacité + soins). Les 2 premiers drakes sont aléatoires ; le 3e détermine le **type de la Faille** (les suivants sont de ce type). **Âme** à 4 drakes pour une équipe (bonus puissant selon le type). Puis **Drake Ancien** (6 min) : buff 2:30 qui inflige des brûlures et **exécute** les champions sous 20 % PV.
- **Larves** (×3) : à 8:00 dans la fosse du Léviathan. Chaque larve donne un cumul : bonus de dégâts aux structures. À 3 cumuls : des larvelettes apparaissent en attaquant les tours.
- **Colosse du Rift** : 15:00 (même fosse), disparaît à 19:45. Donne un objet « Œil du colosse » : l'utiliser invoque le Colosse qui charge la tour la plus proche (énormes dégâts + plaques).
- **Léviathan** : 20:00, réapparition 6 min. Buff d'équipe 3 min aux champions vivants : +AD/AP, rappel renforcé (4 s), sbires proches renforcés (dégâts, résistances, portée).
- Les objectifs se reset (leash) hors de leur fosse. L'or et l'XP vont à l'équipe.

## 8. Économie et objets
- Or de départ 500, or passif 20,4 / 10 s dès 1:05.
- Last hit sbires (mêlée ~21, caster ~14, canon ~60–90), kills 300 (primes : séries +, morts consécutives −, premier sang +100), assists 50 % répartis, plaques 125, tours, objectifs.
- **Boutique** uniquement dans la fontaine (rayon `FOUNTAIN_RADIUS`) ou en mort. Achat, vente (70 %), annulation (undo de la dernière action tant qu'on n'est pas sorti). Composition des objets (coût réduit des composants possédés). 6 emplacements + trinket.
- **~30 objets** (données `shared/src/data/items.ts`) :
  - Départ : Lame du novice, Anneau du novice, Bouclier du novice, Machette du chasseur (jungle), Potion (×5).
  - Bottes : Bottes, Bottes de berserker (AS), Bottes de sorcier (magic pen), Plaques acier (armure), Bottes de mercure (RM + ténacité), Bottes de lucidité (haste).
  - Basiques/épiques : Épée longue, Tome amplificateur, Cristal de rubis, Dague, Cape d'agilité (crit), Armure en tissu, Cape de négation, Pierre de saphir (mana), Marteau de guerre, Baguette explosive.
  - Légendaires (~12) : 2 tireurs (crit/AS), 2 combattants (AD/PV), 1 assassin (létalité), 3 mages (AP, pénétration, sablier = stase active), 2 tanks (armure, RM), 1 support (bouclier/soins), 1 anti-soin.
- Rappel (B) : 8 s de canalisation, interrompu par dégâts/déplacement.
- Balises : trinket (2 charges, 90 s, invisibles), révèlent une zone de 900 u.

## 9. Vision et brouillard de guerre
- Chaque unité alliée révèle un rayon (champions 1350, sbires 1100, tours 1350, balises 900). Les murs ne bloquent pas la vision (simplification), **les brousses** cachent les unités qui y sont sauf si un ennemi est dans la même brousse ou une balise la couvre.
- Le serveur **ne transmet pas** les entités ennemies non visibles (anti-triche). Structures toujours visibles (dernier état connu).
- Le client assombrit les zones non visibles (texture de brouillard calculée à partir des positions alliées).

## 10. UI et contrôles
- **Contrôles** : clic droit = déplacer/attaquer ; A + clic = attaque-déplacement ; Q W E R = sorts (cast normal : clic gauche pour confirmer, quick-cast optionnel dans les paramètres) ; Ctrl+Q/W/E/R = monter un sort ; D/F = invocateur ; B = rappel ; P = boutique ; Tab = tableau des scores ; Espace = centrer la caméra ; Y = verrouiller la caméra ; molette = zoom limité ; bords d'écran = déplacer la caméra ; Alt+clic = ping.
- **HUD** : portrait + niveau + XP, PV/mana, 4 sorts avec cooldowns et rangs, 2 sorts d'invocateur, 6 objets + trinket, or, stats détaillées (C), minimap cliquable en bas à droite (icônes champions, structures, timers), kill feed, annonces centrales, score d'équipe et chrono en haut, barres de vie au-dessus des unités (3D, claude-client).
- Indicateurs de portée et de visée des sorts (claude-fx en relation avec claude-client).

## 11. Qualité visuelle (FX)
- Style : low-poly coloré, éclairage chaud, contours légers, bloom modéré. Chaque champion a une palette (`palette` dans ses données) utilisée par son modèle et ses FX.
- Chaque sort : télégraphie (indicateur au sol pour les zones retardées), projectile/trail, impact, éventuel effet persistant. Morts : dissolve. Level-up : colonne lumineuse. Rappel : canal lumineux. Flash : éclair + traînée. Drakes : souffle élémentaire coloré. Destruction de tour/inhibiteur/nexus : explosion + débris.
- Performance : 60 fps sur un laptop récent avec 8 champions, 60 sbires et les FX ; particules GPU instanciées, pool d'objets, pas d'allocation par frame.

## 12. Audio
Sons générés (zzfx) ou CC0 : auto-attaques, impacts par type, sorts (par `fxKey`, avec fallback par type), morts, level-up, or, achat, annonces vocales synthétiques ou jingles (premier sang, multi-kills, objectifs, tour détruite, victoire/défaite), musique d'ambiance douce. Volume par catégorie dans les paramètres.

## 13. Jalons communs
- **M1 — tranche isolée** : chaque domaine fonctionne seul (mocks autorisés), tests unitaires verts.
- **M2 — tranche verticale** : 2 joueurs, voie mid : sbires, tours, 2 champions avec leurs sorts, auto-attaques, morts/respawn, or et achat, HUD de base, FX de base.
- **M3 — contenu complet** : draft 8 joueurs, 10 champions, carte complète, jungle, tous les objectifs, vision, tous les objets.
- **M4 — polish** : FX et audio complets, équilibrage, perf (60 fps, < 30 Ko/s par client), reconnexion, écran de fin, correction de bugs.

## 14. Définition de « fini »
Huit personnes ouvrent l'URL, créent une salle, draftent, jouent une partie complète de ~25–30 min avec toutes les mécaniques ci-dessus, sans désynchronisation ni crash, avec des FX lisibles et beaux, et un nexus finit par exploser.
