/**
 * Passifs et actifs des objets. Propriétaire : mistral-economy.
 * Les passifs s'enregistrent via world.hooks / world.events (registerItemHooks) ;
 * les actifs sont déclenchés par la commande `useItem` (useItem).
 * Les dégâts passent EXCLUSIVEMENT par applyDamage/applyHeal/applyShield (combat/damage.ts).
 */
import { ITEMS, dist, type Stats, type Team } from '@lov/shared';
import { applyHeal, applyShield, creditOf } from '../combat/damage.ts';
import type { Entity, World } from '../world.ts';
import { econ } from './state.ts';

export const STAT_KEYS = [
  'maxHp', 'hpRegen', 'maxMana', 'manaRegen', 'ad', 'ap', 'armor', 'mr',
  'attackSpeed', 'attackRange', 'moveSpeed', 'critChance', 'abilityHaste',
  'lifesteal', 'armorPen', 'magicPen',
] as const;

/** Liste des itemIds possédés (6 slots + trinket). */
export function ownedItems(ent: Entity): string[] {
  const st = econ(ent);
  const out: string[] = [];
  for (const id of ent.champ?.items ?? []) if (id) out.push(id);
  if (st.trinket) out.push(st.trinket);
  return out;
}

export function hasPassive(ent: Entity, passiveId: string): boolean {
  return ownedItems(ent).some((id) => ITEMS[id]?.passive?.id === passiveId);
}

export function hasItem(ent: Entity, itemId: string): boolean {
  return ownedItems(ent).includes(itemId);
}

/** Somme des stats plates des objets possédés. */
export function itemStatsOf(ent: Entity): Partial<Stats> {
  const out: Partial<Stats> = {};
  for (const id of ownedItems(ent)) {
    const def = ITEMS[id];
    if (!def) continue;
    for (const k of STAT_KEYS) {
      const v = def.stats[k];
      if (v) out[k] = ((out[k] ?? 0) as number) + v;
    }
  }
  return out;
}

/**
 * Applique les stats d'objets (plus effets temporaires type rafale) dans `ent.stats` par delta
 * (on retire l'ancien bonus, on applique le nouveau) : compose avec les autres systèmes de stats.
 * Un gain de PV max soigne du delta, une perte clamp les PV (comme dans LoL).
 * Quand le système buffs de codex-champions fera le recalcul global, il devra utiliser itemStatsOf().
 */
export function applyItemStats(world: World, ent: Entity): void {
  const st = econ(ent);
  const prev = st.appliedItemStats ?? {};
  const bonus = itemStatsOf(ent);
  if (st.rafaleUntil > world.time) {
    const baseMove = ent.stats.moveSpeed - ((prev.moveSpeed ?? 0) as number);
    bonus.moveSpeed = (bonus.moveSpeed ?? 0) + Math.round(baseMove * 0.4);
  }
  for (const k of STAT_KEYS) {
    const oldV = (prev[k] ?? 0) as number;
    const newV = (bonus[k] ?? 0) as number;
    if (oldV !== newV) ent.stats[k] = ((ent.stats[k] ?? 0) as number) - oldV + newV;
  }
  const dMaxHp = (bonus.maxHp ?? 0) - (prev.maxHp ?? 0);
  if (dMaxHp > 0) ent.hp = Math.min(ent.hp + dMaxHp, ent.stats.maxHp);
  else if (dMaxHp < 0) ent.hp = Math.min(ent.hp, ent.stats.maxHp);
  st.appliedItemStats = bonus;
}

/** Enregistre tous les hooks de passifs d'objets. Appelé dans init() du système economy. */
export function registerItemHooks(world: World): void {
  // Dégâts bonus contre les monstres (machete) et exécution (griffe de l'ombre).
  world.hooks.beforeDamage.push((ctx) => {
    const credit = ctx.credit;
    if (!credit?.champ) return;
    if (hasPassive(credit, 'chasseur') && ctx.target.kind === 'monster') ctx.amount += 15;
    if (
      hasPassive(credit, 'ombre') &&
      ctx.target.stats.maxHp > 0 &&
      ctx.target.hp / ctx.target.stats.maxHp < 0.4
    ) {
      ctx.multiplier *= 1.1;
    }
  });

  // Blessure grave (hache du bourreau) : réduit les soins subis par la cible.
  world.hooks.afterDamage.push((ctx, dealt, w) => {
    const credit = ctx.credit;
    if (credit?.champ && ctx.target.team !== credit.team && dealt > 0 && hasPassive(credit, 'grievous')) {
      econ(ctx.target).grievousUntil = w.time + 3;
    }
  });

  // Soins : anti-soin (blessure grave) et amplification (bénédiction de l'encensoir).
  world.hooks.beforeHeal.push((ctx, w) => {
    if (econ(ctx.target).grievousUntil > w.time) ctx.amount *= 0.6;
    const src = ctx.source;
    if (src?.champ && hasPassive(src, 'benediction')) ctx.amount *= 1.2;
  });

  // Auto-attaques : balises (1 dégât/attaque), sort-lame, étincelle.
  world.hooks.beforeAttackHit.push((ctx) => {
    if (ctx.target.kind === 'ward') {
      ctx.amount = 1;
      ctx.crit = false;
      ctx.critMultiplier = 1;
      ctx.bonus = 0;
      return;
    }
    const a = ctx.attacker;
    if (!a.champ) return;
    if (econ(a).spellbladeReady && hasPassive(a, 'spellblade')) {
      ctx.bonus += a.stats.ap * 0.8;
      ctx.damageType = 'magic';
      econ(a).spellbladeReady = false;
    } else if (hasPassive(a, 'etincelle')) {
      ctx.bonus += 20;
    }
  });

  // Stase (sablier) : incoblable, invulnérable, immobile.
  world.hooks.isTargetable.push((target, _attacker, w) => econ(target).staseUntil <= w.time);
  world.hooks.preventDeath.push((target, _killer, w) => {
    if (econ(target).staseUntil > w.time) {
      target.hp = Math.max(target.hp, 1);
      return true;
    }
    return false;
  });
  world.hooks.canMove.push((e, w) => econ(e).staseUntil <= w.time);
  world.hooks.canAttack.push((e, w) => econ(e).staseUntil <= w.time);

  // Un sort lancé arme le sort-lame.
  world.events.on('abilityCast', (e) => {
    const s = world.get(e.source);
    if (s?.champ && hasPassive(s, 'spellblade')) econ(s).spellbladeReady = true;
  });

  // Passifs au last hit / kill de monstre.
  world.events.on('death', (e) => {
    const k = world.get(e.killer);
    const credit = k ? creditOf(world, k) : undefined;
    if (!credit?.champ) return;
    const t = world.get(e.target);
    if (!t) return;
    if (t.kind === 'minion' && hasPassive(credit, 'last_hit_heal')) {
      applyHeal(world, { source: credit, target: credit, amount: 4 });
    }
    if (t.kind === 'monster' && hasPassive(credit, 'chasseur')) {
      applyHeal(world, { source: credit, target: credit, amount: 10 });
    }
  });
}

/** Effets périodiques des objets (potion, voile de la nuit). Appelé chaque tick par economy. */
export function tickItems(world: World, dt: number): void {
  for (const c of world.query('champion')) {
    const st = econ(c);
    if (st.hot) {
      const h = st.hot;
      applyHeal(world, { source: c, target: c, amount: (h.total * dt) / h.duration, silent: true });
      h.remaining -= dt;
      if (h.remaining <= 0) st.hot = null;
    }
    if (hasPassive(c, 'voile') && world.time >= st.voileShieldAt) {
      applyShield(world, { source: c, target: c, amount: 100, duration: 8, id: 'voile' });
      st.voileShieldAt = world.time + 40;
      world.events.emit({ type: 'buffApplied', target: c.id, buffId: 'item_voile', duration: 8, source: c.id });
    }
  }
}

/**
 * Commande `useItem` : active d'un objet de l'inventaire (`slot` 0..5) ou du trinket (`slot` -1).
 */
export function useItem(world: World, ent: Entity, slot: number): void {
  if (!ent.champ || ent.dead) return;
  const st = econ(ent);
  // Œil du colosse donné par codex-objectives hors inventaire (data.heraldEye).
  if (slot === -1 && ent.data.heraldEye) {
    delete ent.data.heraldEye;
    st.colossusCall = { team: ent.team as Team, x: ent.pos.x, y: ent.pos.y };
    world.events.emit({ type: 'announcement', key: 'colosse_invoque', team: ent.team as Team });
    world.events.emit({ type: 'itemUsed', target: ent.id, itemId: 'colossus_eye' });
    return;
  }
  const id = slot === -1 ? st.trinket : ent.champ.items[slot];
  if (!id) return;
  const def = ITEMS[id];
  if (!def?.active) return;
  if ((st.itemCooldowns[def.active.id] ?? -Infinity) > world.time) return;

  switch (def.active.id) {
    case 'potion': {
      const ch = st.charges[slot] ?? 1;
      st.hot = { total: 120, remaining: 15, duration: 15 };
      st.charges[slot] = ch - 1;
      if (ch - 1 <= 0) {
        ent.champ.items[slot] = null;
        delete st.charges[slot];
        delete st.paidPrice[slot];
      }
      break;
    }
    case 'stase':
      st.staseUntil = world.time + 2.5;
      world.events.emit({ type: 'buffApplied', target: ent.id, buffId: 'item_stase', duration: 2.5, source: ent.id });
      break;
    case 'rafale':
      st.rafaleUntil = world.time + 2;
      world.events.emit({ type: 'buffApplied', target: ent.id, buffId: 'item_rafale', duration: 2, source: ent.id });
      break;
    case 'bouclier_allie': {
      applyShield(world, { source: ent, target: ent, amount: 120, duration: 2.5, id: 'encensoir' });
      let best: Entity | undefined;
      let bestD = 800;
      for (const o of world.query('champion')) {
        if (o === ent || o.team !== ent.team || o.dead) continue;
        const d = dist(ent.pos, o.pos);
        if (d < bestD) {
          bestD = d;
          best = o;
        }
      }
      if (best) applyShield(world, { source: ent, target: best, amount: 120, duration: 2.5, id: 'encensoir' });
      break;
    }
    case 'colosse':
      // L'invocation du Colosse est résolue par codex-objectives (entité invoquée).
      st.trinket = null;
      st.colossusCall = { team: ent.team as Team, x: ent.pos.x, y: ent.pos.y };
      world.events.emit({ type: 'announcement', key: 'colosse_invoque', team: ent.team as Team });
      break;
    default:
      return;
  }
  st.itemCooldowns[def.active.id] = world.time + def.active.cooldown;
  world.events.emit({ type: 'itemUsed', target: ent.id, itemId: id });
}
