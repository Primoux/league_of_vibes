# Progress — codex-champions

Date : 2026-11-14. Branche de travail actuelle : `feat/champion-67-portraits` (extension demandée par l'utilisateur).

## Extension : portraits de sélection et 67 Master

- Base initiale : `main` à `c945df9`, puis synchronisation avec `bca314e` (animations et correctifs FX récents préservés) ; les précédentes PR de champions sont fusionnées. Installation, typage et 483 tests verts avant les ajouts.
- Douze portraits originaux générés avec l'outil de génération d'images intégré, palettes et descriptions tirées des données/modèles existants ; Mélenchon conserve sa ressemblance caricaturale et le 67 Master est un personnage adulte fictif. Prompts et provenance dans `client/src/assets/champions/manifest.json`.
- WebP 512 × 512, ~650 Ko pour les douze fichiers ; import Vite avec URL hashée compatible sous-chemin, réutilisation dans le cadre SVG commun (draft, chargement, HUD, feed et scores). Portrait procédural existant conservé en dessous et réaffiché en cas d'erreur réseau/image inconnue. Aucun service distant sollicité par le jeu pour les illustrations.
- Nouveau mage jouable `master67` : Q projectile/slow, W zone retardée/root, E blink/bouclier, R deux ondes retardées (slow puis knockup), passif au septième impact de sort confirmé sur champion, avec verrouillage anti-multi-proc de 0,67 s et expiration des cumuls. Mort avant la seconde onde = seconde onde annulée.
- Modèle procédural dédié dans le pipeline skinné commun : hoodie violet, cheveux bouclés, sourire, doigts et deux paumes ouvertes qui alternent continuellement en idle, course, attaques, Q/W/E/R et rappel. Mort et CC utilisent les comportements communs. Trois meshes, ~1 100 triangles de géométrie opaque/émissive ; tests de direction des paumes et d'alternance.
- FX dédiés par événements réels : 6 projectile, 7 qui tombe, trace de téléportation et deux ondes numérotées. Aucun changement de protocole, aucune dépendance npm ajoutée.
- Branchement hors domaine demandé explicitement par l'utilisateur, isolé dans un commit `contract:` : une illustration optionnelle dans le cadre SVG de `ui/icons/Icons.tsx`, une entrée de modèle et l'import des FX. Pas de remplacement de l'interface ni des autres modèles.
- Validation : **525 tests / 53 fichiers**, typage et build verts. Les garde-fous couvrent désormais 132 duels déterministes (66 paires × deux niveaux) et les coûts/CD des 48 sorts. Tests des dimensions/format/poids/unicité des douze images, du repli, des gestes, des délais, des cibles, des cumuls et de la mort entre deux ondes. Portraits et géométrie skinnée inspectés localement ; aucun contrôle WebGL interactif revendiqué.
- Test réseau réel à huit clients, picks distincts et 67 Master confirmé dans gameStart : 15 s de commandes, **20 snapshots/s sur chacun des huit clients, aucune erreur**, sortie 0. JSON décompressé : 44,1–44,5 Ko/s/client ; ce chiffre ne certifie pas le débit compressé. Script temporaire dérivé de scripts/bots.ts, non livré.
- Publication : l'intégration GitHub refuse les écritures (403). La connexion GitHub CLI a été rétablie par le flux officiel après préparation des commits et de la description de PR ; publication via scripts/open-pr.sh, revue et fusion laissées à Fable.

## Historique : Mélenchon

## Extension : Mélenchon

- Mise à jour de la PR #7 : conflit dans ASSETS.md résolu en conservant les polices de l'interface et les deux déclarations d'assets procéduraux de Mélenchon. Aucun changement de gameplay ni de contrat ; YOUR_PLAN.md de main conservé.

- La PR #2 des dix kits est fusionnée. Cette nouvelle PR ne contient que le champion satirique et son intégration ; synchronisation avec `main` jusqu'à `b689267` (PR #3, #4 et #6 incluses).
- Mage mana complet : Indignation permanente (trois impacts espacés, bouclier et vitesse), Q projectile/slow, W zone retardée/silence, E blink/bouclier/hologramme explosif au départ, R « La République, c’est moi ! » interruptible, dégâts/provocation/bouclier. Valeurs et descriptions dans shared.
- Modèle original sans texture importée : cheveux gris en arrière, lunettes rectangulaires, nez marqué, costume marine, cravate rouge, dossier et index accusateur. Gestes Q/R, marche, respiration, mort/réapparition et aura du buff R ; hologramme translucide. Budget testé <14 000 triangles, géométries partagées dans chaque modèle, ressources libérées à dispose.
- FX : micro projectile, amendements, clone bleu au point de départ, télégraphies et onde tricolore avec pancarte de l'ultime. La pancarte n'apparaît qu'à l'impact confirmé par le serveur, pas au début d'une incantation interrompue.
- Démo autonome `?melenchonlab`, portrait/silhouette, Q/W/E/R, marche et contrôles caméra ; disponible également dans le labo FX commun et le draft.
- L'utilisateur a explicitement demandé un modèle ressemblant, ce qui autorise cette extension aux fichiers client/FX en plus du domaine serveur initial. Intégration additive isolée dans des commits `contract:` : main (démo), DefaultViews (secours), fabrique de modèles (interface AnimatedChampionView conservant les opérations existantes), index FX et type de vue dans fxlab. Aucun nouveau message réseau ni champ de protocole, aucune dépendance npm.
- Validation : **393 tests / 37 fichiers**, typecheck et build verts. Tests spécifiques de gameplay, ciblage, interruptions, expiration du passif et ressources du modèle ; 110 duels déterministes (55 paires × deux niveaux), 44 sorts vérifiés pour coûts/CD. Le navigateur distant ne peut pas accéder au serveur local et refuse les URL fichier : pas de validation WebGL interactive revendiquée. Silhouette/portrait inspectés avec un rendu CPU local de la géométrie, puis affinés.
- Test réseau réel : draft déterministe de huit champions distincts, Mélenchon confirmé dans le gameStart, puis 15 s de commandes simultanées ; **20 snapshots/s sur chacun des huit clients, aucune erreur**, sortie 0. Script de smoke temporaire basé sur scripts/bots.ts, sans modification du bot de charge du dépôt. Débit JSON brut 36,7–37,4 Ko/s ; ne certifie pas le débit compressé.
- Assets procéduraux documentés dans ASSETS.md. Pas de photo, son ou asset tiers ajouté. L'équilibrage compétitif reste à confirmer en partie humaine.

## Historique : dix champions initiaux

## Fait
- **M1** : moteur de sorts serveur autoritaire. Validation des rangs, ressources, cooldowns, CC, visibilité, cible et portée ; approche automatique des cibles hors portée, annulable par un nouvel ordre. Incantations interruptibles, animations `cast_Q/W/E/R`, haste, recasts et charges.
- Primitives : projectiles avec collisions balayées triées par distance (première cible / perçants / ciblés / retour), zones instantanées, retardées avec télégraphie et persistantes, cônes, dashes, blinks respectant les murs, liens interruptibles par la distance.
- Buffs : stun/root/slow/silence/knockup/taunt/suppress/blind/grounded ; ténacité sauf knockup/suppress ; ralentissement le plus fort, y compris plusieurs sorts du même lanceur. Boucliers temporaires indépendants, invisibilité, révélation, anti-soin et hooks de stase.
- Recalcul des stats par statAtLevel + objets + buffs + hooks ; supports des unités non championnes sans accumulation des modificateurs. Mana, énergie de Nyx et absence de ressource de Kaelen ; régénération, XP partagée à 1400 u, niveaux 1–18, points de compétence, rangs QWE et paliers R 6/11/16.
- **M2** : Grondak et Ilyra, quatre sorts et passifs testés ; neuf invocateurs : Flash, Ignite, Heal, Barrier, Exhaust, Teleport, Smite, Ghost, Cleanse. Flash ne laisse pas le rayon du champion dans un mur ; Teleport s'interrompt sur dégâts, CC, ordre ou disparition de la cible.
- **M3** : les dix kits complets : Grondak, Tharox, Kaelen, Vrenna, Ilyra, Zephyr, Sable, Corvin, Nyx, Solenne. Attaques renforcées, marques, saignement, riposte, trois fentes/rayons, orbe aller-retour, pièges limités, invisibilité, marque de mort et soins/boucliers amplifiés.
- **M4 — garde-fous** : 90 duels (45 paires × niveaux 6 et 11, équipements par classe), déterministes, sur les vrais systèmes combat/movement/vision. Contrôle des ressources, valeurs finies, dégâts aberrants et répétabilité. 40 tests de coûts/CD des sorts ; descriptions FR alignées sur les paramètres. Cela ne remplace pas l'équilibrage en partie humaine.
- Dernière validation complète : **331 tests / 32 fichiers**, `npm run typecheck`, `npm test`, `npm run build` verts sur Node 24.22.1. Avertissement de chunk client >500 Ko préexistant, hors de ce domaine.
- Synchronisation avec main jusqu'à `acc60fbe479e866fb4ecb115dbd8750853464703`. Conflits résolus en conservant **strictement l'API damage.ts de claude-server**, et les deux ensembles d'événements partagés. Aucun changement des systèmes des autres domaines.
- Aucune dépendance ni asset ajouté.

## En cours
- Publication par GitHub CLI authentifié et ouverture de la PR pour revue par Fable.

## Validation réseau
- Serveur réel et 8 clients WebSocket : draft, chargement, puis 15 secondes de commandes simultanées.
- **20,0 snapshots/s par client, aucune erreur**, test terminé avec code 0.
- Débit JSON décompressé mesuré : 36,8–37,2 Ko/s par client (compression WebSocket active). Ce chiffre brut ne certifie pas le budget réseau compressé <30 Ko/s.
- `node --import tsx` utilisé pour contourner uniquement le pipe IPC du CLI tsx indisponible dans cet environnement.

## Bloqué / Besoins
- L'intégration ChatGPT utilisée pour la lecture refuse les écritures (403). Une connexion GitHub CLI officielle a été validée par l'utilisateur ; la publication utilise cette connexion.
- Les nouvelles règles d'AGENTS.md sont prises en compte : publication via `pr/codex-champions`, YOUR_PLAN de main conservé, revue/merge par Fable uniquement.
- Le réglage compétitif, les visuels et l'ergonomie nécessitent les parties humaines et les branches client/FX/UI ; le moteur expose les événements et états requis.

## Contrats modifiés
- Ajouts **optionnels** dans `shared/types.ts`, commit `contract:` : `fxKey` sur abilityCast, summonerCast, buffApplied, buffRemoved, levelUp ; `stacks` sur buffApplied. Événements respawn et extensions autoAttack de claude-server préservés. Aucun champ supprimé/renommé.
- `SummonerDef.params?` (schema possédé) contient les valeurs de gameplay des invocateurs.
- L'API de dégâts de secours livrée au premier jalon a été **retirée lors de la synchronisation** : le fichier final `combat/damage.ts` est identique à celui de main. Tous les sorts passent par cette API et ses hooks.

## API disponibles / intégration

### Buffs — `server/src/combat/buffs.ts`
- `addBuff(world, target, id, { duration, source?, stacks?, params?, definition? })`, `removeBuff`, `getBuff`, `getBuffs`, `hasBuff`, `hasCC`, `canAct/canMove/canAttack/canCast`, `getTenacity`, `getStatMods`, `cleanse`.
- `params.statMods` / `statPercentMods` : bonus plats / relatifs ; `amount` : ralentissement ; `tenacity`, `damageReduction`, `damageDealtReduction`, `grievous`, `invisible`, `ghosted`, `revealed` : effets.
- `registerStatSource((world, entity) => ({ flat, percent, tenacity }))` retourne un désabonnement. Les stats brutes d'ITEMS[champ.items] sont déjà additionnées ; le hook ajoute les passifs, sans compter deux fois ces stats.
- `setBaseStats(world, entity, stats)` : mise à jour de la base des sbires/monstres dont les stats progressent alors qu'un buff est actif.
- Enregistrement automatique sur `world.hooks.canMove/canAttack/isTargetable/beforeAttackHit/beforeDamage/beforeHeal/entitySnapshot`.
- Snapshot : buffs `{id, remaining, stacks?}` via entitySnapshot ; `data.invisible`, `data.ghost`, `data.revealedUntil` compatibles avec movement/vision de claude-server.

### Combat, boucliers et passifs
- `abilities/effects.ts` re-exporte applyDamage/applyHeal et adapte applyShield avec `{ source, target, amount, duration?, buffId?, fxKey? }`. Le bouclier réel reste exclusivement dans `combatData(target).shields`, via l'API de claude-server ; les buffs n'en stockent qu'une vue/identité.
- `abilities/combatHooks.ts` : registerDamageModifier(world, fn), branchement direct sur hooks.beforeDamage.
- Passifs : **hooks.afterDamage**, précision non arrondie et `ctx.isAutoAttack` pour les impacts confirmés. L'événement `autoAttack` de départ brise l'invisibilité, sans compter deux fois les impacts. La marque de Nyx stocke seulement les PV effectivement retirés, pas les boucliers ni l'overkill.
- Dashes : primitives.startDash produit le contrat `DashData` de movement (targetId, stopDistance, ignoreWalls, onArrive). Le mouvement avance une seule fois et émet l'événement ; abilities assure collisions/CC/fin. Secours local seulement si movementSystem absent du World.

### Progression et FX
- `grantXP`, `levelAbility`, `maxAbilityRank`, `xpToNextLevel`, `xpForLevel`, `cooldownWithHaste` dans systems/leveling.ts.
- `world.hooks.selfState` renseigne xpToNext. XP de mort : `data.xpReward`, sinon `data.minion.xp` / `data.jungle.xp` / `data.monster.xp` ; `data.xpGlobal` pour les objectifs globaux.
- abilityCast, projectileSpawn/Hit, dash, areaEffect, buffApplied/Removed, summonerCast, levelUp avec fxKey. Télégraphies `<sort>_telegraph`, impacts/variantes conservent les clés des données.

## Vérification reproductible
```bash
npm ci
npm run typecheck
npm test
npm run build
# CI réseau : démarrer le serveur, puis scripts/bots.ts --bots 8 --seconds 15.
# L'environnement local n'autorise pas le pipe IPC du CLI tsx : utiliser node --import tsx.
```
