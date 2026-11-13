# Progress — mistral-economy

## Fait
- **M1 — or et boutique** (`systems/economy.ts`) : or de départ 500, or passif 20,4/10 s dès `scaled(1:05)`, last hits (or + CS), kills avec prime via `world.hooks.bounty` (base 300, +150/+300 si la victime a une série ≥3/≥5, −25 par mort consécutive, min 100), premier sang +100, assists = 50 % de la prime répartis, or des tours (150 global + 150 local), plaques (125 local), inhibiteurs (250 global), objectifs (`OBJECTIVE_GOLD` : dragon 25, Ancien 250, larve 50, colosse 100, Léviathan 300). Événement `goldGain` émis à chaque gain.
- **Boutique** : `buy` (recettes : composants consommés et coût déduit, objets uniques, consommables empilés jusqu'à `maxStacks`, trinket hors des 6 slots), `sell` (70 % du prix payé), `undoShop` (pile vidée en quittant la fontaine), contrôle fontaine (`FOUNTAIN_RADIUS`) ou mort. Événements `itemBought`/`itemSold`.
- **Rappel** : 8 s (`RECALL_SECONDS`), 4 s avec `data.leviathanBuff`, interrompu par dégâts subis ou ordre de déplacement/attaque, téléportation à la fontaine. Événements `recallStart`/`recallEnd`.
- **M2 — objets** : 35 objets dans `shared/src/data/items.ts` (noms FR originaux, recettes cohérentes, `ItemDef.shop` ajouté). Stats appliquées par delta dans `ent.stats` (`items/handlers.ts` `applyItemStats`, composable avec le futur recalc de buffs — qui devra lire `itemStatsOf`). Passifs câblés via `world.hooks` : anti-soin (hache), sort-lame (sceptre), stase 2,5 s invulnérable (sablier), machete (bonus monstres), exécution (griffe), bénédiction (soins +20 %), voile (bouclier périodique), last-hit heal. Actifs via `useItem` : potion (soin sur 15 s), rafale, bouclier allié, Œil du colosse (pose `data.economy.colossusCall` pour codex-objectives).
- **Balises** : trinket `stealth_ward`, 2 charges rechargées toutes les 90 s, balise 90 s de vie, `data.invisible` + `sightRadius` 900 (lu par vision de claude-server), révélée si un ennemi entre à 900 u, détruite en 3 auto-attaques (`beforeAttackHit` force 1 dégât/attaque sur les wards).
- **M3 — audio client** (`client/src/audio/`) : `AudioManager` (WebAudio, canaux master/sfx/voice/music/ui persistés, `setVolume`, spatialisation par distance à la caméra via `setListener`, polyphonie max 24), `synth.ts` (tones/bruit procéduraux, aucun asset externe), `sounds.ts` (tous les `GameEvent` + recettes par `fxKey` pour les 40 sorts et 9 sorts d'invocateur, annonces FR avec `speechSynthesis` optionnel), `music.ts` (ambiance générée La mineur + jingles victoire/défaite).
- **Tests** : `economy.test.ts`, `items.test.ts`, `recall.test.ts`, `wards.test.ts` (35 tests, dont une simulation d'or à 10 min). Total repo : 99 tests verts, typecheck + build verts.

## En cours
- M4 : mixage audio en conditions réelles (à valider en partie dev avec les autres agents).

## M4 fait
- Stats d'objets : le recalc est désormais la propriété de codex-champions (`combat/buffs.ts` `recalculateStats`,
  qui somme `ITEMS[champ.items].stats`) — mon delta par tick a été retiré pour éviter le double compte.
  Effet temporaire « rafale » enregistré via `registerStatSource` ; stase via `data.stasis` (géré par buffs) ;
  delta de PV max à l'achat/vente via `healItemMaxHpDelta`.
- Tests de simulation : or gagné à 10/20/30 min (passif + vagues réalistes) dans des bornes cohérentes LoL ;
  validation des données d'objets (recettes, coûts légendaires 2000–3400, ids/icônes uniques).

## Bloqué / Besoins (des autres agents)
- **codex-objectives** (branche en avance sur main) : contrats déjà consommés ici — `data.rewards` (gold/xp/smiteRequired/localGold/teamGold, lu avec fallback sur mes constantes), `data.plateReward` pendant `structureDestroyed(plate)`, `data.leviathanBuff = { expiresAt, recallSeconds }` pour le rappel renforcé, `data.heraldEye` consommé par `useItem` slot -1. Reste : consommation de `data.economy.colossusCall` pour invoquer l'entité Colosse, et `structureDamageMultiplier` côté combat.
- **codex-champions** : OK — `recalculateStats` somme déjà les stats d'objets ; j'utilise `registerStatSource` pour « rafale » et je ne touche plus à `ent.stats`.
- **mistral-ui** : OK — `champ.items` exposé avec 7 entrées (index 6 = trinket) via `world.hooks.entitySnapshot`, `self.gold`/`canShop`/`recalling`/`wardCharges` via `world.hooks.selfState`, `setVolume`/`getVolume`/`setMuted` exportés depuis `client/src/audio/AudioManager.ts`. Plus d'`announcement` serveur pour les kills (dérivés de `death` par l'UI et par l'audio).
- **claude-client** : appeler `audio.setListener(() => caméra en unités de jeu)` par frame.
- **claude-server** : rien — le hook `selfState` est câblé côté economy.

## Contrats modifiés
- `contract:` `ClientMsg` + `{ t: 'useItem'; slot: number }` (slot -1 = trinket).
- `contract:` `GameEvent` + `{ type: 'itemUsed'; target; itemId }`.
- `contract:` `SelfState` + `wardCharges?: { charges: number; cooldownRemaining: number }`.
- `ItemDef` (ma section de schema.ts) + `shop?: boolean` (false = non achetable, ex. Œil du colosse).

## API disponibles (pour les autres)
- `systems/economy.ts` : `addGold(world, ent, amount, pos?)`, `killBounty(victim)`, `canShopNow(ent)`, constantes `KILL_GOLD`, `MINION_GOLD`, `MONSTER_GOLD`, `OBJECTIVE_GOLD`, `WARD_*`.
- `items/state.ts` : `econ(entity)` -> `EconomyState` (data.economy).
- `items/handlers.ts` : `itemStatsOf(ent)`, `applyItemStats(world, ent)`, `hasPassive/hasItem`, `useItem(world, ent, slot)`, `registerItemHooks(world)`, `tickItems(world, dt)`.
