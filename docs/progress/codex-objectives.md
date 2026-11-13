# Progress — codex-objectives

Session : 2026-11-13, branche `codex-objectives`.

## Fait
- Lecture complète des plans et progress ; synchronisation `origin/main` (déjà à jour).
- Baseline : installation, typecheck et 4 tests verts avec Node 24.22.3. Le Node 21 du shell est incompatible avec Vitest 5/Vite 8 ; aucune dépendance ajoutée.
- M1 : données PvE, 22 tours/6 inhibiteurs/2 nexus, vagues des trois voies, scaling, canons/super-sbires, ciblage, cadence, priorité d'agression, ramp des tours, plaques, fortification, anti-backdoor, ordre de destruction, respawn inhibiteur, victoire.
- M2 : scénarios headless de rencontre mid symétrique et de défense sans joueurs.
- M3 : camps complets/petits monstres, aggro de camp, leash/reset/régénération, respawn après nettoyage complet, buffs transférables, soins Smite, sanctuaire du crabe ; drakes/Faille/âme/Ancien, larves/larvelettes, œil et invocation du Colosse, Léviathan et sbires renforcés, timers et annonces.

## En cours
- M4 : compléter les effets d'âme, durcir les cas limites, mesures headless et tests de non-régression.

## Bloqué / Besoins (des autres agents)
- `combat` et `buffs` sont encore des stubs. Adaptateurs locaux `world-ai/runtime.ts` (`configurePveApis`) utilisables immédiatement, à remplacer par les API centrales dès disponibilité. Aucun fichier d'un autre domaine modifié.
- claude-server : brancher `configurePveApis(world, { applyDamage, addBuff })` avec adaptateurs de signatures ; appeler `structureDamageMultiplier(world, request)` **avant** les résistances dans TOUS les dégâts aux structures. Le mock le fait déjà. Le combat général doit ignorer `entity.data.pveCombat` pour éviter les doubles auto-attaques. Les IA exposent aussi `attackTarget`/`moveTarget`.
- claude-server : intégrer `getObjectiveTimers(world, team)` et `getObjectiveState(world).teams` au snapshot/score ; le protocole actuel a déjà les champs nécessaires.
- mistral-economy : lire `data.rewards` (sbires : lastHitOnly/xpRadius ; camps : smiteRequired ; objectifs : teamGold/xp), les données `shared/src/data/{minions,monsters,structures}.ts` et les événements existants. Aucun versement d'or/XP direct pour ne pas doubler votre système.
- Plaques : `structureDestroyed` avec `plate: true`, équipe défenseuse ; `target.data.plateReward` est disponible **pendant** l'événement (gold/radius/source). Tours : récompenses `data.rewards.localGold/teamGold`.
- mistral-economy : œil hors inventaire dans `killer.data.heraldEye = { expiresAt }`, actif via `useHeraldEye(world, owner)` (consomme si valide). Il manque encore une commande client d'objet actif dans le protocole ; l'intégrateur doit raccorder l'UI/commande.
- mistral-economy : `data.leviathanBuff = { expiresAt, recallSeconds: 4 }`, supprimé à la mort/expiration. `data.elderBuff` suit la même durée sans rappel.
- codex-champions : buffs `dragon_<element>` cumulables/permanents, `soul_<element>`, `blue_buff`, `red_buff`, `red_slow`, `scuttle_speed`, `grub_siege`, `elder`, `leviathan`. Paramètres explicites via `addBuff`. Le mock utilise `data.pveBuffs` et recalcule ses bonus uniquement ; l'API réelle doit gérer les stats/CC sans exécuter le mock.
- L'équilibrage d'une partie de 25–30 minutes à huit humains dépend des autres domaines et devra être validé en intégration.

## Contrats modifiés / API disponibles
- Aucun fichier de contrat partagé modifié. Utilisation des événements existants et données namespacées.
- Exports : `isStructureAttackable`, `structureDamageMultiplier`, `structureState`, `spawnWave`, `getCampTimers`, `getObjectiveTimers`, `getObjectiveState`, `useHeraldEye`, `configurePveApis`.
- Données de récompenses accessibles par imports directs, pas de modification du barrel partagé.
