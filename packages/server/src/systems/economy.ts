/**
 * Système économie. Propriétaire : mistral-economy (PLAN.md §8).
 * Or (passif, last hits, kills/primes, structures, objectifs), boutique (buy/sell/undoShop),
 * rappel (B), balises (trinket). Stats et passifs d'objets : voir `server/src/items/`.
 *
 * Contrats consommés : `world.hooks.bounty` (fourni ici, lu par combat/damage.ts),
 * événements `death` / `structureDestroyed` / `objectiveTaken`, commandes `buy`/`sell`/`undoShop`/
 * `recall`/`placeWard`/`useItem`, `data.leviathanBuff` (codex-objectives, optionnel).
 */
import {
  FOUNTAINS,
  FOUNTAIN_RADIUS,
  INVENTORY_SLOTS,
  ITEMS,
  MAP_SIZE,
  PASSIVE_GOLD_PER_10S,
  PASSIVE_GOLD_START,
  RECALL_SECONDS,
  clamp,
  dist,
  isInWall,
  otherTeam,
  scaled,
  type EntityId,
  type MonsterType,
  type Team,
  type Vec2,
} from '@lov/shared';
import { kill } from '../combat/damage.ts';
import { healItemMaxHpDelta, itemStatsOf, registerItemHooks, tickItems, useItem } from '../items/index.ts';
import { econ, type ShopAction } from '../items/state.ts';
import type { Entity, System, World } from '../world.ts';

// ---------------------------------------------------------------- Constantes d'économie
export const KILL_GOLD = 300;
export const FIRST_BLOOD_GOLD = 100;
export const ASSIST_SHARE = 0.5; // fraction de la prime partagée entre les assistants
export const SELL_RATIO = 0.7;
export const LOCAL_GOLD_RADIUS = 1000;
/** Récompenses de structures (fallback ; codex-objectives pose `data.rewards` sur les structures). */
export const STRUCTURE_TEAM_GOLD = 50;
export const STRUCTURE_LOCAL_GOLD: Record<string, number> = { outer: 250, inner: 300, inhib: 300, nexus: 50 };
export const PLATE_GOLD = 125;
/** Or du last hit par type de sbire (fallback ; codex-objectives pose `data.rewards.gold`, déjà scalé avec le temps). */
export const MINION_GOLD: Record<string, number> = { melee: 21, caster: 14, cannon: 60, super: 90 };
/** Or du last hit par monstre (fallback ; idem via data.rewards). */
export const MONSTER_GOLD: Record<string, number> = {
  blue_buff: 90, red_buff: 90, gromp: 90, wolves: 55, wolf_small: 7,
  raptors: 30, raptor_small: 7, krugs: 65, krug_small: 7, scuttle: 55, grub: 20,
};
/** Or d'équipe par objectif épique (aligné sur `teamGold` de data/monsters.ts de codex-objectives). */
export const OBJECTIVE_GOLD: Partial<Record<MonsterType, number>> = {
  dragon: 25, elder_dragon: 250, grub: 20, herald: 100, leviathan: 300,
};

/** Récompenses posées par codex-objectives sur les entités (sbires, monstres, structures, plaques). */
export interface RewardData {
  gold?: number;
  xp?: number;
  lastHitOnly?: boolean;
  smiteRequired?: boolean;
  xpRadius?: number;
  localGold?: number;
  teamGold?: number;
}
interface PlateReward {
  gold: number;
  radius: number;
  source: EntityId;
}
export const WARD_LIFETIME = 90;
export const WARD_SIGHT = 900;
export const WARD_CAST_RANGE = 600;
export const WARD_MAX_CHARGES = 2;
export const WARD_RECHARGE = 90;
export const WARD_HP = 3; // détruite en 3 auto-attaques (voir beforeAttackHit dans items/handlers.ts)
export const RECALL_LEVIATHAN_SECONDS = 4; // rappel renforcé avec le buff du Léviathan

const firstBlood = new WeakMap<World, boolean>();

/** Ajoute de l'or à un champion et émet `goldGain` (pour le HUD, les FX et l'audio). */
export function addGold(world: World, ent: Entity, amount: number, pos?: Vec2): void {
  if (!ent.champ || amount <= 0) return;
  ent.champ.gold += amount;
  world.events.emit({ type: 'goldGain', target: ent.id, amount: Math.round(amount), ...(pos ? { pos } : {}) });
}

/** Prime de kill d'un champion : base + série de la victime − morts consécutives (min 100). */
export function killBounty(victim: Entity): number {
  if (!victim.champ) return 0;
  const st = econ(victim);
  let b = KILL_GOLD;
  if (st.killStreak >= 5) b += 300;
  else if (st.killStreak >= 3) b += 150;
  b -= Math.min(4, st.deathStreak) * 25;
  return Math.max(100, b);
}

/** Boutique : achat possible à la fontaine de son équipe ou en étant mort. */
export function canShopNow(ent: Entity): boolean {
  if (ent.dead) return true;
  return dist(ent.pos, FOUNTAINS[ent.team as Team]) <= FOUNTAIN_RADIUS;
}

function ownsItem(ent: Entity, itemId: string): boolean {
  const st = econ(ent);
  return ent.champ!.items.includes(itemId) || st.trinket === itemId;
}

function buyItem(world: World, ent: Entity, itemId: string): void {
  const def = ITEMS[itemId];
  const st = econ(ent);
  if (!def || def.shop === false || !ent.champ || !canShopNow(ent)) return;

  if (def.category === 'trinket') {
    if (st.trinket) return;
    st.trinket = def.id;
    if (def.id === 'stealth_ward') {
      st.trinketCharges = WARD_MAX_CHARGES;
      st.trinketRechargeAt = 0;
    }
    st.shopHistory.push({ kind: 'buy', itemId: def.id, slot: -1, gold: 0, paidPrice: 0 });
    world.events.emit({ type: 'itemBought', target: ent.id, itemId: def.id });
    return;
  }

  if (def.unique && ownsItem(ent, def.id)) return;

  // Consommable : on empile dans le slot existant tant qu'il reste de la place.
  if (def.maxStacks) {
    const existing = ent.champ.items.findIndex((id) => id === def.id);
    if (existing >= 0) {
      const ch = st.charges[existing] ?? 1;
      if (ch < def.maxStacks && ent.champ.gold >= def.cost) {
        ent.champ.gold -= def.cost;
        st.charges[existing] = ch + 1;
        st.paidPrice[existing] = (st.paidPrice[existing] ?? 0) + def.cost; // prix payé cumulé (vente à 70 %)
        st.shopHistory.push({ kind: 'buy', itemId: def.id, slot: existing, gold: def.cost, paidPrice: st.paidPrice[existing], charges: ch });
        world.events.emit({ type: 'itemBought', target: ent.id, itemId: def.id });
      }
      return;
    }
  }

  // Recette : les composants possédés sont consommés et leur coût est déduit.
  const consumed: ShopAction['consumed'] = [];
  let reduction = 0;
  for (const compId of def.buildsFrom) {
    const cslot = ent.champ.items.findIndex((id) => id === compId);
    if (cslot >= 0) {
      consumed.push({ slot: cslot, itemId: compId, paidPrice: st.paidPrice[cslot] ?? ITEMS[compId]?.cost ?? 0 });
      reduction += ITEMS[compId]?.cost ?? 0;
    }
  }
  const cost = Math.max(0, def.cost - reduction);
  const slot = ent.champ.items.findIndex((id) => id === null);
  if (slot < 0 || ent.champ.gold < cost) return;

  ent.champ.gold -= cost;
  for (const c of consumed) {
    ent.champ.items[c.slot] = null;
    delete st.paidPrice[c.slot];
    delete st.charges[c.slot];
  }
  ent.champ.items[slot] = def.id;
  st.paidPrice[slot] = cost + (consumed ?? []).reduce((s, c) => s + c.paidPrice, 0);
  if (def.maxStacks) st.charges[slot] = 1;
  st.shopHistory.push({ kind: 'buy', itemId: def.id, slot, gold: cost, paidPrice: st.paidPrice[slot], consumed, charges: 0 });
  world.events.emit({ type: 'itemBought', target: ent.id, itemId: def.id });
}

function sellItem(world: World, ent: Entity, slot: number): void {
  const st = econ(ent);
  if (!ent.champ || slot < 0 || slot >= INVENTORY_SLOTS || !canShopNow(ent)) return;
  const id = ent.champ.items[slot];
  if (!id) return;
  const def = ITEMS[id];
  const paid = st.paidPrice[slot] ?? def?.cost ?? 0;
  const refund = Math.round(paid * SELL_RATIO);
  ent.champ.gold += refund;
  ent.champ.items[slot] = null;
  delete st.paidPrice[slot];
  delete st.charges[slot];
  st.shopHistory.push({ kind: 'sell', itemId: id, slot, gold: refund, paidPrice: paid });
  world.events.emit({ type: 'itemSold', target: ent.id, itemId: id });
}

/** Annule la dernière action de boutique (tant qu'on est encore dans la fontaine). */
function undoShop(world: World, ent: Entity): void {
  const st = econ(ent);
  if (!ent.champ || !st.canShop) return;
  const act = st.shopHistory.pop();
  if (!act) return;
  if (act.kind === 'buy') {
    if (act.slot === -1) {
      st.trinket = null;
      st.trinketCharges = 0;
    } else if (ITEMS[act.itemId]?.maxStacks && ent.champ.items[act.slot] === act.itemId && act.charges !== undefined) {
      // Undo d'un achat empilé : on rend la charge, pas tout le stack.
      if (act.charges <= 0) {
        ent.champ.items[act.slot] = null;
        delete st.paidPrice[act.slot];
        delete st.charges[act.slot];
      } else {
        st.charges[act.slot] = act.charges;
      }
    } else {
      ent.champ.items[act.slot] = null;
      delete st.paidPrice[act.slot];
      delete st.charges[act.slot];
      for (const c of act.consumed ?? []) {
        const free = ent.champ.items.findIndex((x) => x === null);
        if (free >= 0) {
          ent.champ.items[free] = c.itemId;
          st.paidPrice[free] = c.paidPrice;
        }
      }
    }
    ent.champ.gold += act.gold;
  } else {
    if (ent.champ.gold < act.gold) {
      st.shopHistory.push(act); // pas assez d'or pour racheter : on annule l'undo
      return;
    }
    ent.champ.gold -= act.gold;
    if (act.slot === -1) st.trinket = act.itemId;
    else {
      ent.champ.items[act.slot] = act.itemId;
      st.paidPrice[act.slot] = act.paidPrice;
      if (act.charges !== undefined) st.charges[act.slot] = act.charges;
    }
  }
}

function startRecall(world: World, ent: Entity): void {
  const st = econ(ent);
  if (ent.dead || st.recall) return;
  // Buff du Léviathan (codex-objectives) : rappel renforcé, `recallSeconds` dans data.leviathanBuff.
  const lb = ent.data.leviathanBuff as { expiresAt: number; recallSeconds?: number } | undefined;
  const total = lb && lb.expiresAt > world.time ? (lb.recallSeconds ?? RECALL_LEVIATHAN_SECONDS) : RECALL_SECONDS;
  st.recall = { remaining: total, total };
  world.events.emit({ type: 'recallStart', target: ent.id });
}

function cancelRecall(world: World, ent: Entity): void {
  const st = econ(ent);
  if (!st.recall) return;
  st.recall = null;
  world.events.emit({ type: 'recallEnd', target: ent.id, completed: false });
}

function placeWard(world: World, ent: Entity, pos: Vec2): void {
  const st = econ(ent);
  if (ent.dead || st.trinket !== 'stealth_ward' || st.trinketCharges <= 0) return;
  const p = { x: clamp(pos.x, 0, MAP_SIZE), y: clamp(pos.y, 0, MAP_SIZE) };
  if (dist(ent.pos, p) > WARD_CAST_RANGE || isInWall(p)) return;
  world.spawn({
    kind: 'ward',
    team: ent.team,
    pos: p,
    radius: 30,
    hp: WARD_HP,
    mana: 0,
    stats: { maxHp: WARD_HP, hpRegen: 0, maxMana: 0, manaRegen: 0, ad: 0, ap: 0, armor: 0, mr: 0, attackSpeed: 0, attackRange: 0, moveSpeed: 0, critChance: 0, abilityHaste: 0, lifesteal: 0, armorPen: 0, magicPen: 0 },
    subtype: 'stealth_ward',
    data: {
      invisible: true, // invisible pour l'équipe ennemie (vision.ts)
      sightRadius: WARD_SIGHT,
      ward: { ownerId: ent.champ?.playerId, expiresAt: world.time + WARD_LIFETIME },
    },
  });
  st.trinketCharges--;
  if (st.trinketCharges < WARD_MAX_CHARGES) st.trinketRechargeAt = world.time + WARD_RECHARGE;
  world.events.emit({ type: 'itemUsed', target: ent.id, itemId: 'stealth_ward' });
}

function grantKillGold(world: World, e: { target: EntityId; killer?: EntityId; assists?: EntityId[]; bounty?: number }): void {
  const target = world.get(e.target);
  if (!target) return;
  const killer = e.killer !== undefined ? world.get(e.killer) : undefined;

  if (target.kind === 'champion' && target.champ) {
    const vst = econ(target);
    const wasStreak = vst.killStreak;
    vst.deathStreak++;
    vst.killStreak = 0;
    if (vst.recall) cancelRecall(world, target);
    if (killer?.champ && killer.team !== target.team) {
      const kst = econ(killer);
      kst.killStreak++;
      let gold = e.bounty ?? KILL_GOLD;
      if (!firstBlood.get(world)) {
        firstBlood.set(world, true);
        gold += FIRST_BLOOD_GOLD;
        // Pas d'`announcement` ici : mistral-ui dérive premier sang / séries / multi-kills de l'événement `death`.
      }
      void wasStreak;
      addGold(world, killer, gold, target.pos);
    }
    const assists = (e.assists ?? []).map((id) => world.get(id)).filter((a): a is Entity => !!a?.champ);
    if (assists.length > 0 && e.bounty !== undefined) {
      const share = (e.bounty * ASSIST_SHARE) / assists.length;
      for (const a of assists) addGold(world, a, share, target.pos);
    }
    return;
  }

  // Last hit sur sbire / monstre : or + CS pour le champion tueur.
  if ((target.kind === 'minion' || target.kind === 'monster') && killer?.champ && killer.team !== target.team) {
    const rw = target.data.rewards as RewardData | undefined;
    let gold = rw?.gold ?? (target.kind === 'minion' ? (MINION_GOLD[target.subtype ?? 'melee'] ?? 21) : (MONSTER_GOLD[target.subtype ?? ''] ?? 50));
    // Camps de jungle : or complet uniquement avec Smite (PLAN §7).
    if (rw?.smiteRequired && !hasSmite(killer)) gold = Math.round(gold / 2);
    killer.champ.cs++;
    addGold(world, killer, gold, target.pos);
  }
}

function hasSmite(ent: Entity): boolean {
  const s = ent.champ!.summoners;
  return s.D === 'smite' || s.F === 'smite';
}

function grantStructureGold(world: World, e: { target: EntityId; kind: 'tower' | 'inhibitor' | 'nexus'; team: Team; plate?: boolean }): void {
  const struct = world.get(e.target);
  const pos = struct?.pos ?? { x: 0, y: 0 };
  const team = otherTeam(e.team); // l'équipe qui a détruit la structure
  if (e.plate) {
    // Plaque : or local, lu dans data.plateReward posé par codex-objectives pendant l'événement.
    const plate = struct?.data.plateReward as PlateReward | undefined;
    const gold = plate?.gold ?? PLATE_GOLD;
    const radius = plate?.radius ?? LOCAL_GOLD_RADIUS;
    for (const c of world.query('champion')) {
      if (c.champ && c.team === team && dist(c.pos, pos) <= radius) addGold(world, c, gold, pos);
    }
    return;
  }
  const rw = struct?.data.rewards as RewardData | undefined;
  const teamGold = rw?.teamGold ?? STRUCTURE_TEAM_GOLD;
  const localGold = rw?.localGold ?? (STRUCTURE_LOCAL_GOLD[struct?.subtype ?? ''] ?? STRUCTURE_LOCAL_GOLD.outer!);
  for (const c of world.query('champion')) {
    if (!c.champ || c.team !== team) continue;
    addGold(world, c, teamGold);
    if (dist(c.pos, pos) <= LOCAL_GOLD_RADIUS) addGold(world, c, localGold);
  }
}

function grantObjectiveGold(world: World, e: { monster: MonsterType; team: Team }): void {
  const gold = OBJECTIVE_GOLD[e.monster] ?? 0;
  if (!gold) return;
  for (const c of world.query('champion')) {
    if (c.champ && c.team === e.team) addGold(world, c, gold);
  }
}

export const economySystem: System = {
  name: 'economy',

  init(world) {
    registerItemHooks(world);

    // Prime de kill lue par combat/damage.ts (champ `bounty` de l'événement death).
    world.hooks.bounty.push((victim) => (victim.champ ? killBounty(victim) : undefined));

    // Rappel : on ne peut pas bouger tant qu'on canalise.
    world.hooks.canMove.push((e, w) => {
      const st = e.data.economy as ReturnType<typeof econ> | undefined;
      return !st?.recall;
    });

    world.events.on('death', (e) => grantKillGold(world, e));
    world.events.on('structureDestroyed', (e) => grantStructureGold(world, e));
    world.events.on('objectiveTaken', (e) => grantObjectiveGold(world, e));

    // mistral-ui attend `champ.items` de longueur 7 dans le snapshot (index 6 = trinket).
    world.hooks.entitySnapshot.push((e, s) => {
      if (!e.champ || !s.champ) return;
      s.champ.items = [...e.champ.items, econ(e).trinket];
    });

    // Complète le SelfState envoyé au client (hook ajouté par claude-server).
    world.hooks.selfState.push((e, s, w) => {
      if (!e.champ) return;
      const st = econ(e);
      s.canShop = st.canShop;
      if (st.recall) s.recalling = Math.max(0, st.recall.remaining);
      if (st.trinket === 'stealth_ward') {
        s.wardCharges = { charges: st.trinketCharges, cooldownRemaining: Math.max(0, st.trinketRechargeAt - w.time) };
      }
    });

    // Le rappel est interrompu par les dégâts subis.
    world.events.on('damage', (e) => {
      const t = world.get(e.target);
      if (t?.champ) cancelRecall(world, t);
    });
  },

  update(world, dt) {
    // --- commandes boutique / rappel / balise / objet actif
    for (const { playerId, msg } of world.commands) {
      const ent = world.championOf(playerId);
      if (!ent?.champ) continue;
      switch (msg.t) {
        case 'buy': {
          const before = itemStatsOf(ent);
          buyItem(world, ent, msg.itemId);
          healItemMaxHpDelta(ent, before);
          break;
        }
        case 'sell': {
          const before = itemStatsOf(ent);
          sellItem(world, ent, msg.slot);
          healItemMaxHpDelta(ent, before);
          break;
        }
        case 'undoShop': {
          const before = itemStatsOf(ent);
          undoShop(world, ent);
          healItemMaxHpDelta(ent, before);
          break;
        }
        case 'recall':
          startRecall(world, ent);
          break;
        case 'placeWard':
          placeWard(world, ent, msg.pos);
          break;
        case 'useItem':
          useItem(world, ent, msg.slot, msg.targetPos);
          break;
        case 'move':
        case 'attackMove':
        case 'attack':
          if (!ent.dead) cancelRecall(world, ent);
          break;
        default:
          break;
      }
    }

    for (const c of world.query('champion')) {
      const st = econ(c);

      // Or passif (dès scaled(PASSIVE_GOLD_START), toutes les 10 s, même mort).
      if (st.nextPassiveGoldAt === 0) st.nextPassiveGoldAt = scaled(PASSIVE_GOLD_START);
      while (world.time >= st.nextPassiveGoldAt) {
        addGold(world, c, PASSIVE_GOLD_PER_10S);
        st.nextPassiveGoldAt += 10;
      }

      // Achat autorisé : fontaine ou mort. Quitter la fontaine vide l'historique d'undo.
      st.canShop = canShopNow(c);
      if (!st.canShop && st.shopHistory.length > 0) st.shopHistory = [];

      // Rappel en cours.
      if (st.recall) {
        st.recall.remaining -= dt;
        if (c.dead) {
          cancelRecall(world, c);
        } else if (st.recall.remaining <= 0) {
          const f = FOUNTAINS[c.team as Team];
          c.pos = { x: f.x + 60, y: f.y + 60 };
          c.moveTarget = undefined;
          c.path = undefined;
          st.recall = null;
          world.events.emit({ type: 'recallEnd', target: c.id, completed: true });
        }
      }

      // Recharge du trinket de vision.
      if (st.trinket === 'stealth_ward' && st.trinketCharges < WARD_MAX_CHARGES && world.time >= st.trinketRechargeAt) {
        st.trinketCharges++;
        if (st.trinketCharges < WARD_MAX_CHARGES) st.trinketRechargeAt = world.time + WARD_RECHARGE;
      }

      // Les stats d'objets sont recalculées par buffs (codex-champions) ; ici on gère juste le PV max.
    }

    tickItems(world, dt);

    // Balises : expiration + révélation si un ennemi entre dans leur rayon.
    for (const w of world.query('ward')) {
      const wd = w.data.ward as { expiresAt: number } | undefined;
      if (wd && world.time >= wd.expiresAt) {
        kill(world, w);
        continue;
      }
      for (const c of world.query('champion')) {
        if (c.team !== w.team && !c.dead && dist(c.pos, w.pos) <= WARD_SIGHT) {
          w.data.revealedUntil = world.time + 1;
          break;
        }
      }
    }
  },
};
