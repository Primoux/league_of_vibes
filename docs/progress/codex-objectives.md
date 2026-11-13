# Progress — codex-objectives

Session : 2026-11-13, branche `codex-objectives`.

## Fait
- Lecture complète des plans et progress ; synchronisation `origin/main` (déjà à jour).
- Baseline : installation, typecheck et 4 tests verts avec Node 24.22.3. Le Node 21 du shell est incompatible avec Vitest 5/Vite 8 ; aucune dépendance ajoutée.
- M1 : données PvE, 22 tours/6 inhibiteurs/2 nexus, vagues des trois voies, scaling, canons/super-sbires, ciblage, cadence, priorité d'agression, ramp des tours, plaques, fortification, anti-backdoor, ordre de destruction, respawn inhibiteur, victoire.
- M2 : scénarios headless de rencontre mid symétrique et de défense sans joueurs.
- M3 : camps complets/petits monstres, aggro de camp, leash/reset/régénération, respawn après nettoyage complet, buffs transférables, soins Smite, sanctuaire du crabe ; drakes/Faille/âme/Ancien, larves/larvelettes, œil et invocation du Colosse, Léviathan et sbires renforcés, timers et annonces.

## En cours
- M4 technique terminé : six âmes actives (explosion, soin, bouclier, vitesse après ultime, chaîne/slow, bonus à PV bas), garde de structure dans les hooks centraux, projectiles/CC via combat réel, test d'endurance sur trente minutes simulées.
- Équilibrage ajusté : tours à 1,1 attaque/s pour les vagues accélérées, dégâts proportionnels bruts aux sbires (45 % mêlée / 70 % caster / 14 % canon-super), dégâts des sbires contre structures ×0,35 (super ×0,6). Le test sans joueurs ne finit pas automatiquement.
- La validation d'une partie de huit humains et de sa durée réelle reste une étape d'intégration commune.

## Bloqué / Besoins (des autres agents)
- Les nouveaux commits de claude-server ont été récupérés pendant la session. Tous les dégâts passent maintenant par `combat/damage.ts`. Les hooks `beforeDamage`, `isTargetable`, `beforeAttackHit` et `afterAttackHit` sont enregistrés directement par les systèmes possédés. Aucune modification nécessaire côté combat, aucun fichier d'un autre domaine modifié.
- En présence du système `combat`, les IA posent uniquement `attackTarget`/`moveTarget` : cadence, projectiles, windup, CC et dégâts viennent du combat central. Attaque simple conservée pour tests isolés sans combat ; `data.pveCombat` est informatif et ne doit **pas** désactiver le combat central.
- L'API de buffs reste absente : `configurePveApis(world, { addBuff })` permet le raccordement ; le fallback est local dans `world-ai/runtime.ts`. Dégâts, soins et boucliers utilisent déjà les API réelles.
- claude-server : intégrer `getObjectiveTimers(world, team)` et `getObjectiveState(world).teams` au snapshot/score ; le protocole actuel a déjà les champs nécessaires.
- mistral-economy : lire `data.rewards` (sbires : lastHitOnly/xpRadius ; camps : smiteRequired ; objectifs : teamGold/xp), les données `shared/src/data/{minions,monsters,structures}.ts` et les événements existants. Aucun versement d'or/XP direct pour ne pas doubler votre système.
- Plaques : `structureDestroyed` avec `plate: true`, équipe défenseuse ; `target.data.plateReward` est disponible **pendant** l'événement (gold/radius/source). Tours : récompenses `data.rewards.localGold/teamGold`.
- mistral-economy : œil hors inventaire dans `killer.data.heraldEye = { expiresAt }`, actif via `useHeraldEye(world, owner)` (consomme si valide). Il manque encore une commande client d'objet actif dans le protocole ; l'intégrateur doit raccorder l'UI/commande.
- mistral-economy : `data.leviathanBuff = { expiresAt, recallSeconds: 4 }`, supprimé à la mort/expiration. `data.elderBuff` suit la même durée sans rappel.
- codex-champions : buffs `dragon_<element>` cumulables/permanents, `soul_<element>`, `blue_buff`, `red_buff`, `red_slow`, `scuttle_speed`, `grub_siege`, `elder`, `leviathan`. Paramètres explicites via `addBuff`. Le mock utilise `data.pveBuffs` et recalcule ses bonus uniquement ; l'API réelle doit gérer les stats/CC (dont ténacité chemtech) sans exécuter le mock. Les effets d'âme restent dans `world-ai/objective-effects.ts`, avec les API centrales de dégâts/soins/boucliers.
- L'équilibrage d'une partie de 25–30 minutes à huit humains dépend des autres domaines et devra être validé en intégration.

## Contrats modifiés / API disponibles
- Aucun fichier de contrat partagé modifié. Utilisation des événements existants et données namespacées.
- Exports : `isStructureAttackable`, `structureDamageMultiplier`, `structureState`, `spawnWave`, `getCampTimers`, `getObjectiveTimers`, `getObjectiveState`, `useHeraldEye`, `configurePveApis`.
- Données de récompenses accessibles par imports directs, pas de modification du barrel partagé.

## Validation
- Node 24.22.3 : `npm run typecheck`, `npm test`, `npm run build`.
- Après récupération des API serveur : 95 tests (dont nouvelles régressions PvE) ; test d'intégration du combat sans double attaque et scénario de trente minutes à pas accéléré de 0,2 s. Ce test vérifie stabilité et défense, pas la fidélité d'équilibrage à 30 Hz avec joueurs.
- Aucune dépendance ou asset ajouté. Le build client signale seulement son bundle >500 Ko déjà présent hors domaine.
