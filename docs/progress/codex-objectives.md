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
- API réelle de buffs intégrée après synchronisation de main : `world-ai/runtime.ts` traduit les paramètres en `statMods/statPercentMods`, total de cumuls autoritaire, ténacité, CC et persistance à la mort. Aucune implémentation de buff de secours restante. Les six âmes utilisent aussi les API centrales de soins/boucliers.
- claude-server : timers branchés via `world.hooks.objectiveTimers` ; votre ScoreTracker consomme déjà les événements tours/drakes/larves. `data.xpReward` et `data.xpGlobal` sont renseignés pour leveling.
- mistral-economy : lire `data.rewards` (sbires : lastHitOnly/xpRadius ; camps : smiteRequired ; objectifs : teamGold/xp), les données `shared/src/data/{minions,monsters,structures}.ts` et les événements existants. Aucun versement d'or/XP direct pour ne pas doubler votre système.
- Plaques : `structureDestroyed` avec `plate: true`, équipe défenseuse ; `target.data.plateReward` est disponible **pendant** l'événement (gold/radius/source). Tours : récompenses `data.rewards.localGold/teamGold`.
- mistral-economy : contrat de votre branche distante vérifié : œil hors inventaire `data.heraldEye = { expiresAt }`, expiration nettoyée à chaque tick ; les demandes `data.economy.colossusCall = { team, x, y }` sont désormais consommées une seule fois par objectives. `useHeraldEye(world, owner)` reste disponible pour les appels directs/tests. Aucun import de fichiers economy non encore présents sur main.
- mistral-economy : `data.leviathanBuff = { expiresAt, recallSeconds: 4 }`, supprimé à la mort/expiration. `data.elderBuff` suit la même durée sans rappel.
- codex-champions : buffs `dragon_<element>` cumulables/permanents, `soul_<element>`, `blue_buff`, `red_buff`, `red_slow`, `scuttle_speed`, `grub_siege`, `elder`, `leviathan`. Paramètres explicites via `addBuff`. Le wrapper appelle votre API réelle, y compris la ténacité chemtech et les ralentissements ; réapplication des bonus permanents à la réanimation. Les effets d'âme restent dans `world-ai/objective-effects.ts`, avec les API centrales de dégâts/soins/boucliers.
- L'équilibrage d'une partie de 25–30 minutes à huit humains dépend des autres domaines et devra être validé en intégration.

## Contrats modifiés / API disponibles
- Aucun fichier de contrat partagé modifié. Utilisation des événements existants et données namespacées.
- Exports : `isStructureAttackable`, `structureDamageMultiplier`, `structureState`, `spawnWave`, `getCampTimers`, `getObjectiveTimers`, `getObjectiveState`, `useHeraldEye`, `configurePveApis`.
- Données de récompenses accessibles par imports directs, pas de modification du barrel partagé.

## Validation
- Node 24.22.3 : `npm run typecheck`, `npm test`, `npm run build`. Tests réseau exécutés hors sandbox (écoute de port local interdite dans le sandbox).
- Après récupération des API serveur, champions, FX et client : suite complète de 473 tests (dont 33 tests PvE) ; test d'intégration du combat sans double attaque et scénario de trente minutes à pas accéléré de 0,2 s. Ce test vérifie stabilité et défense, pas la fidélité d'équilibrage à 30 Hz avec joueurs.
- Aucune dépendance ou asset ajouté. Le build client signale seulement son bundle >500 Ko déjà présent hors domaine.

## Livraison
- Les nouvelles règles AGENTS.md récupérées avec main imposent une PR `pr/codex-objectives` ; revue/merge réservés à Fable. Aucun push sur main effectué. Le premier essai local d'intégration a été arrêté après les tests ; ses correctifs ont été réalisés sur codex-objectives.
- Publication : branches `codex-objectives` et `pr/codex-objectives` poussées ; `YOUR_PLAN.md` de la branche de PR identique à celui de main.
- Blocage GitHub résolu : le connecteur reste en HTTP 403, mais le CLI officiel installé dans /tmp est maintenant authentifié après validation de l’utilisateur. Publication via le script officiel ; revue/merge réservés à Fable.

- Connexion GitHub CLI validée par l’utilisateur ; publication de la PR reprise avec le script officiel.
- Test réseau réel supplémentaire : huit clients pendant 60 s, première vague incluse, 20,0 snapshots/s chacun et aucune erreur. Débit JSON décompressé : 61–62 Ko/s/client ; ce chiffre ne mesure pas le trafic WebSocket compressé.

- Correction d'intégration autorisée par l’utilisateur : la fixture `economy.test.ts` exclut les quatre systèmes PvE. Ses événements/récompenses sont déjà simulés explicitement ; toutes ses assertions sont conservées. Cela évite de lancer de vraies vagues pendant ses simulations économiques de 10–30 minutes. Les tests PvE et le test d'endurance utilisent toujours les vrais systèmes.
- Validation finale après isolation de la fixture économique : 473 tests / 47 fichiers, typecheck et build verts.
- PR #14 ouverte : https://github.com/Primoux/league_of_vibes/pull/14. Première CI : 472/473 passent ; seul le délai de 30 s du test d'endurance est dépassé (35 s sur runner). Délai porté à 60 s sans modifier la simulation ni ses assertions. Le benchmark distinct reste inchangé : CI à 1,25 ms/tick, 19 Ko/s compressés.

## Revue Fable — performances (2026-11-14)
- Demande prise en compte : le délai de 60 s ne constituait pas la correction. Test d'endurance ramené à 3 600 pas de 0,5 s (toujours 1 800 s de jeu, nexus vivants, winner nul, entités bornées), délai mural de 10 s. Pas de réduction de la durée simulée ni suppression d'assertion.
- Cause mesurée : le prédicat de ciblage évaluait l'ordre des structures lointaines avant la distance, jusqu'à trois recherches complètes par sbire/tick. Profil avant : 2,043 ms/tick, dont 1,599 ms pour minions ; 139 entités / 78 sbires.
- Correction : une seule recherche de cible avec distance préalable, priorités et départage conservés ; chemins des voies conservés au spawn ; structures indexées par monde ; plus de parcours des sbires sur les dégâts sans agression champion/champion. Aucun ralentissement de la fréquence d'IA.
- Autres allocations retirées : callbacks de recherche tours/Colosse, closures de spawn d'objectifs par tick, tableaux temporaires des cumuls de drakes, listes de structures et listes vides recréées, parcours champions par sbire pour Léviathan (liste réutilisée), détection des systèmes répétée.
- Même profil après : 0,396 ms/tick moyen, p95 0,497 ms ; minions 0,116 ms, objectives 0,012 ms. Mesure réelle à 30 Hz sur 900 ticks après chauffe, même population ; gain global ~5×. Ces valeurs sont propres à la machine locale.
- Ajout de `minions.performance.test.ts` : vraies vagues sur les trois voies, >60 sbires, 900 ticks à 30 Hz, budget moyen <2 ms. Régression aggro avec larvelette alliée ajoutée.
- Validation locale : typecheck/build verts, 475 tests en 3,28 s (avant récupération du dernier main dans la branche de PR).
- Deuxième passe de Fable lue : cache d'attaquabilité dans `structureState` par tick/révision ajouté, invalidé immédiatement sur destruction et réapparition d'inhibiteur. Les assertions existantes couvrent déjà l'ouverture de la structure suivante dans le même tick. Fréquence d'acquisition des sbires conservée pour éviter de modifier le comportement ; le budget demandé est atteint sans la réduire.
