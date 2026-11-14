# YOUR_PLAN — mistral-economy (Mistral 2)

Branche : `mistral-economy`

## Mission
Tu implémentes **l'économie et les objets** côté serveur (or, primes, boutique, ~30 objets, rappel, balises/trinket) et **l'audio** côté client (sons de tout le jeu + musique). Règles : PLAN §8 et §12.

## Tu possèdes
- `packages/server/src/systems/economy.ts`, `packages/server/src/items/**` (à créer : passifs/actifs d'objets)
- `packages/shared/src/data/items.ts`, `ItemDef` dans `shared/src/schema.ts`
- `packages/client/src/audio/**` (+ assets audio CC0 éventuels dans `packages/client/public/audio/`, listés dans `ASSETS.md`)
- Tests `packages/server/test/{economy,items,recall,wards}*.test.ts`

## Interdit
Autres systèmes serveur, `client/src/{ui,render,net,input,fx}/**`.

## Dépendances
- Événement `death` (claude-server) : killer, assists → or des kills, primes, assists, last hits (sbires/monstres : utilise `data/minions.ts` et `data/monsters.ts` de codex-objectives pour les montants, ou tes propres constantes en attendant).
- `structureDestroyed`, `objectiveTaken` (codex-objectives) → or local/global.
- Stats d'objets : enregistre une source de stats via `registerStatSource` (codex-champions, `combat/buffs.ts`) ; mocke si absent. Passifs d'objets : hooks EventBus (`damage`, `autoAttack`, `abilityCast`).
- Rappel : lit `data.leviathanBuff` (codex-objectives) pour le rappel renforcé ; interrompu par `damage` reçu ou commande de déplacement.
- Boutique UI = mistral-ui ; tu fournis la logique serveur et `self.canShop`, `self.gold` (via le snapshot — demande à claude-server si un champ manque, ou commit `contract:` additif).

## Jalons
### M1 — or et boutique
1. Or de départ, or passif (dès 1:05, `PASSIVE_GOLD_PER_10S`), last hits, kills (300 + primes de série / morts consécutives, premier sang), assists, émission `goldGain`.
2. **Boutique** serveur : `buy` (recettes : coût réduit des composants possédés, retire les composants), `sell` (70 %), `undoShop` (pile d'actions, vidée en sortant de la fontaine), contrôle de `FOUNTAIN_RADIUS` ou mort, inventaire 6 + trinket, objets uniques, consommables empilables. Événements `itemBought`/`itemSold`.
3. **Rappel** (B) : 8 s, interruption, téléportation fontaine, `recallStart`/`recallEnd`.

### M2 — objets
4. Les ~30 objets du PLAN §8 dans `data/items.ts` (noms originaux FR, coûts et stats LoL-like, recettes cohérentes, descriptions). Passifs/actifs simples dans `server/src/items/` (ex. anti-soin, sablier = stase 2,5 s, lifesteal, sort-lame, bouclier de support, potions, Œil du colosse avec codex-objectives).
5. **Balises** : trinket 2 charges (recharge 90 s), `placeWard` → entité `ward` (90 s, invisible pour l'ennemi, rayon de vision 900 lu par la vision de claude-server), destruction en 3 auto-attaques.

### M3 — audio (client)
6. `AudioManager` : WebAudio, canaux (master, effets, voix/annonces, musique, interface) exposés pour les paramètres de mistral-ui (`setVolume(channel, v)`), spatialisation simple (atténuation selon distance à la caméra), limitation de polyphonie.
7. Sons générés (zzfx ou synthèse WebAudio) ou CC0 : auto-attaques (mêlée/distance), impacts physique/magique, chaque `fxKey` des 40 sorts (avec fallback par type de sort), sorts d'invocateur, morts, level-up, or, achat, tour qui tire, structure détruite, drakes/Léviathan, annonces (jingles + voix synthétique via SpeechSynthesis optionnelle : « Premier sang », « Tour détruite »…), musique d'ambiance douce (générée/loop CC0) et musiques de victoire/défaite.

### M4
8. Équilibrage économique : tests de simulation (or moyen à 10/20/30 min cohérent avec LoL, objets complets ~2 500–3 300 or), mixage audio.

## Tests
Achat avec recette, undo, vente, refus hors fontaine, prime de série, assists partagés, or passif, rappel interrompu, rappel renforcé, ward expire.
