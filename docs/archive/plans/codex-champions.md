# YOUR_PLAN — codex-champions (Codex 1)

Branche : `codex-champions`

## Mission
Tu implémentes **tout le gameplay des champions côté serveur** : moteur de sorts, buffs/debuffs/contrôles, boucliers, XP et niveaux, recalcul des stats, les **10 kits complets** et les **9 sorts d'invocateur**.

## Tu possèdes
- `packages/server/src/systems/{abilities,buffs,leveling,summoners}.ts`
- `packages/server/src/abilities/**` (à créer : moteur + un fichier par champion, ex. `abilities/champions/grondak.ts`)
- `packages/server/src/combat/buffs.ts` (API de buffs, à créer)
- `packages/shared/src/data/champions/**`, `data/summoners.ts`, `data/buffs.ts` (à créer)
- `packages/shared/src/schema.ts` (sauf `ItemDef` : mistral-economy)
- Tests `packages/server/test/{abilities,buffs,leveling,summoners,champion-*}.test.ts`

## Interdit
Client, systèmes des autres. Pour les dégâts : **utilise `applyDamage/applyHeal/applyShield`** de claude-server (`server/src/combat/damage.ts`). S'il n'existe pas encore, crée-le avec la signature du PLAN §2.1 et une implémentation minimale, préviens claude-server dans ton progress.

## Contrats que tu fournis (documente dans ton progress)
- `combat/buffs.ts` : `addBuff(world, target, buffId, { duration, source?, stacks?, params? })`, `removeBuff`, `hasBuff`, `hasCC(e, cc)`, `canMove(e)`, `canAttack(e)`, `canCast(e)`, ténacité, `getStatMods`. Utilisé par codex-objectives (buffs de jungle/drakes/Léviathan) et mistral-economy (stats d'objets, passifs).
- Recalcul des stats à chaque tick dans `buffs` : `base + perLevel (statAtLevel) + objets (lus dans data/items via champ.items) + buffs`. Expose un hook `registerStatSource(fn)` pour que mistral-economy ajoute les objets sans éditer ton fichier.
- Événements émis : `abilityCast`, `projectileSpawn`, `projectileHit`, `dash`, `areaEffect`, `buffApplied`, `buffRemoved`, `levelUp`, `summonerCast` avec les bonnes `fxKey` (claude-fx en dépend).
- Animations : mets `entity.anim = 'cast_Q'` etc. pendant les casts.

## Jalons
### M1 — moteur
1. **Lecture des commandes** `cast`, `levelAbility`, `summoner` dans `world.commands` ; validation (rang > 0, cooldown, mana/énergie, CC, portée → si hors portée, se déplacer puis lancer pour les sorts ciblés).
2. **Primitives réutilisables** : projectile skillshot (largeur, vitesse, collision premier ennemi / perçant, murs ignorés), projectile ciblé, zone instantanée, zone retardée (télégraphie `areaEffect`), zone persistante (entité `zone`), cône, dash vers point/cible (via `entity.data.dash` consommé par movement — voir progress de claude-server ; mock sinon), blink, tether, recasts, charges, casts canalisés, temps d'incantation.
3. **Buffs/CC** : stun, root, slow (le plus fort s'applique), silence, knockup (non réductible par ténacité), taunt, suppress, blind, boucliers temporaires, invisibilité (`data.invisible`, lue par vision de claude-server), réduction de soins (grievous).
4. **Leveling** : table XP LoL (280 → 18), XP des sbires/champions/monstres via EventBus (`death`) partagée dans 1400 u, points de compétence (`ceil(level/2)`, R à 6/11/16), régénération PV/mana, cooldowns avec ability haste (`cd * 100/(100+AH)`).

### M2 — premiers kits
5. **Grondak** et **Ilyra** complets + tests (chaque sort : dégâts attendus, CC appliqué, cooldown).
6. **Sorts d'invocateur** : les 9 (`data/summoners.ts`). Flash respecte les murs (atterrit au point valide le plus proche). Smite : monstres et sbires. Teleport : canalisation 4 s vers structure/sbire allié.

### M3 — les 10 kits
7. Tharox, Kaelen, Vrenna, Zephyr, Sable, Corvin, Nyx, Solenne : tous les sorts et passifs tels que décrits dans leurs fichiers de données (paramètres dans `params`). Ajuste les données si nécessaire (tu en es propriétaire), garde les `fxKey`.
8. Énergie (Nyx), sans ressource (Kaelen), passifs d'AA (Kaelen, Corvin, Sable, Tharox) via un hook `onAutoAttackHit` fourni par combat (demande-le à claude-server ou expose-le via EventBus `autoAttack`/`damage`).

### M4 — équilibrage
9. Tests de simulation 1v1 (chaque paire de champions niveau 6/11 avec objets standards) pour détecter les valeurs aberrantes ; ajustements. Descriptions FR à jour avec les valeurs.

## Tests
Un fichier de test par champion + moteur (projectile touche/rate, zone retardée, slow le plus fort, ténacité, knockup non réduit, cooldown avec haste, XP partagée, niveau 6 débloque R).
