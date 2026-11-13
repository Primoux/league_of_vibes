import { ITEMS, clamp, getChampion, statAtLevel, type Stats } from '@lov/shared';
import { BUFFS } from '../../../shared/src/data/buffs.ts';
import type { BuffDef, CCType } from '@lov/shared';
import { removeShield } from './damage.ts';
import { cancelAttack } from '../systems/combat.ts';
import type { Entity, World } from '../world.ts';

export interface BuffParams {
  [key: string]: number | string | boolean | Partial<Stats> | undefined;
  amount?: number;
  tenacity?: number;
  damageReduction?: number;
  damageDealtReduction?: number;
  grievous?: number;
  invisible?: boolean;
  ghosted?: boolean;
  statMods?: Partial<Stats>;
  statPercentMods?: Partial<Stats>;
}
export interface BuffInstance {
  id: string;
  source?: number;
  remaining: number;
  duration: number;
  stacks: number;
  params: BuffParams;
  def: BuffDef;
  shieldRemaining?: number;
}
export interface BuffOptions {
  duration: number;
  source?: Entity | number;
  stacks?: number;
  params?: BuffParams;
  definition?: Partial<BuffDef>;
}
export interface StatModifiers {
  flat?: Partial<Stats>;
  percent?: Partial<Stats>;
  tenacity?: number;
}
export type StatSource = (world: World, entity: Entity) => StatModifiers | undefined;
const statSources = new Set<StatSource>();
/** Adds item passives/other stats after the built-in sum of ITEMS[champ.items]. Returns an unsubscribe. */
export function registerStatSource(fn: StatSource): () => void {
  statSources.add(fn);
  return () => { statSources.delete(fn); };
}
export function getBuffs(e: Entity): BuffInstance[] {
  return (e.data.buffs ??= []) as BuffInstance[];
}
export function getBuff(e: Entity, id: string, source?: number): BuffInstance | undefined {
  return getBuffs(e).find((b) => b.id === id && (source === undefined || b.source === source));
}
export const hasBuff = (e: Entity, id: string): boolean => !!getBuff(e, id);
export const hasCC = (e: Entity, cc: CCType): boolean => getBuffs(e).some((b) => b.remaining > 0 && b.def.cc === cc);
const hardCC = (e: Entity): boolean => ['stun', 'knockup', 'suppress', 'fear', 'taunt'].some((c) => hasCC(e, c as CCType));
export const canAct = (e: Entity): boolean => !e.dead && !hardCC(e) && !e.data.stasis;
export const canMove = (e: Entity): boolean => !e.dead && !e.data.stasis && !['stun', 'knockup', 'suppress', 'fear', 'root'].some((c) => hasCC(e, c as CCType))
  && (!hasCC(e, 'taunt') || e.data.tauntForcedMove === true) && !e.data.casting && !e.data.channeling;
export const canAttack = (e: Entity): boolean => !e.dead && !e.data.stasis && !['stun', 'knockup', 'suppress', 'fear'].some((c) => hasCC(e, c as CCType)) && !e.data.casting && !e.data.channeling;
export const canCast = (e: Entity): boolean => canAct(e) && !hasCC(e, 'silence') && !e.data.casting && !e.data.channeling;
export function getTenacity(e: Entity): number {
  let remaining = 1 - clamp(Number(e.data.itemTenacity ?? 0), 0, 0.9);
  for (const b of getBuffs(e)) remaining *= 1 - clamp(b.params.tenacity ?? 0, 0, 0.9);
  return clamp(1 - remaining, 0, 0.9);
}
function syncFlags(world: World, e: Entity): void {
  const buffs = getBuffs(e);
  e.data.invisible = buffs.some((b) => b.id === 'invisible' || b.params.invisible);
  e.data.ghosted = buffs.some((b) => b.params.ghosted);
  e.data.revealed = buffs.some((b) => b.params.revealed);
  e.data.ghost = e.data.ghosted;
  e.data.revealedUntil = Math.max(-Infinity, ...buffs.filter((b) => b.params.revealed).map((b) => world.time + b.remaining));
}
export function addBuff(world: World, target: Entity, buffId: string, opts: BuffOptions): BuffInstance | undefined {
  initBuffs(world);
  if (target.dead || !Number.isFinite(opts.duration) || opts.duration <= 0) return;
  const source = typeof opts.source === 'number' ? opts.source : opts.source?.id;
  const def: BuffDef = { id: buffId, name: buffId, kind: 'buff', ...BUFFS[buffId], ...opts.definition };
  const reducible = def.cc && !['knockup', 'suppress', 'slow', 'grounded'].includes(def.cc);
  const duration = opts.duration * (reducible ? 1 - getTenacity(target) : 1);
  let buff = getBuffs(target).find((b) => b.id === buffId && b.source === source);
  if (buff) {
    buff.remaining = Math.max(buff.remaining, duration);
    buff.duration = Math.max(buff.duration, duration);
    buff.stacks = Math.min(def.maxStacks ?? 1, buff.stacks + (opts.stacks ?? 1));
    buff.params = { ...buff.params, ...opts.params };
  } else {
    buff = { id: buffId, source, remaining: duration, duration, stacks: Math.min(def.maxStacks ?? 1, opts.stacks ?? 1), params: { ...opts.params }, def };
    getBuffs(target).push(buff);
  }
  syncFlags(world, target);
  recalculateStats(world, target);
  world.events.emit({ type: 'buffApplied', target: target.id, buffId, source, duration, stacks: buff.stacks, fxKey: def.fxKey ?? buffId });
  return buff;
}
export function removeBuff(world: World, target: Entity, buffId: string, source?: number | null): void {
  const buffs = getBuffs(target);
  for (let i = buffs.length - 1; i >= 0; i--) {
    const b = buffs[i]!;
    if (b.id !== buffId || (source !== undefined && b.source !== (source === null ? undefined : source))) continue;
    if (typeof b.params.shieldId === 'string') removeShield(target, b.params.shieldId);
    buffs.splice(i, 1);
    world.events.emit({ type: 'buffRemoved', target: target.id, buffId, fxKey: b.def.fxKey ?? buffId });
  }
  syncFlags(world, target);
  recalculateStats(world, target);
}
export function cleanse(world: World, e: Entity): void {
  for (const b of [...getBuffs(e)]) {
    if (b.def.cc && b.def.cc !== 'knockup' && b.def.cc !== 'suppress') removeBuff(world, e, b.id, b.source ?? null);
  }
}
export function breakInvisibility(world: World, e: Entity): void {
  for (const b of [...getBuffs(e)]) if (b.id === 'invisible' || b.params.invisible) removeBuff(world, e, b.id, b.source ?? null);
}
function mergeStats(out: Partial<Stats>, add?: Partial<Stats>, stacks = 1): void {
  if (!add) return;
  for (const key of Object.keys(add) as (keyof Stats)[]) out[key] = (out[key] ?? 0) + (add[key] ?? 0) * stacks;
}
export function getStatMods(e: Entity): StatModifiers {
  const flat: Partial<Stats> = {}, percent: Partial<Stats> = {};
  for (const b of getBuffs(e)) {
    mergeStats(flat, b.def.statMods, b.stacks);
    mergeStats(flat, b.params.statMods, b.stacks);
    mergeStats(percent, b.def.statPercentMods, b.stacks);
    mergeStats(percent, b.params.statPercentMods, b.stacks);
  }
  return { flat, percent, tenacity: getTenacity(e) };
}
/** Keeps changing minion/monster base stats separate from temporary modifiers. */
export function setBaseStats(world: World, e: Entity, stats: Stats): void {
  if (e.champ) return;
  if (getBuffs(e).length) e.data.buffBaseStats = { ...stats };
  else Object.assign(e.stats, stats);
  recalculateStats(world, e);
}
export function recalculateStats(world: World, e: Entity): void {
  const def = e.champ ? getChampion(e.champ.championId) : undefined;
  if (!def && getBuffs(e).length === 0) {
    const original = e.data.buffBaseStats as Stats | undefined;
    if (original) { Object.assign(e.stats, original); e.hp = Math.min(e.hp, e.stats.maxHp); delete e.data.buffBaseStats; }
    return;
  }
  const baseStats = def?.baseStats ?? (e.data.buffBaseStats ??= { ...e.stats }) as Stats;
  const { flat = {}, percent = {} } = getStatMods(e);
  e.data.baseAd = def ? statAtLevel(def.baseStats.ad, def.perLevel.ad ?? 0, e.champ!.level) : baseStats.ad;
  for (const id of e.champ?.items ?? []) if (id && ITEMS[id]) mergeStats(flat, ITEMS[id].stats);
  let tenacity = 0;
  for (const fn of statSources) {
    const mods = fn(world, e);
    if (!mods) continue;
    mergeStats(flat, mods.flat);
    mergeStats(percent, mods.percent);
    tenacity = 1 - (1 - tenacity) * (1 - (mods.tenacity ?? 0));
  }
  e.data.itemTenacity = tenacity;
  for (const key of Object.keys(baseStats) as (keyof Stats)[]) {
    const base = def ? statAtLevel(baseStats[key], def.perLevel[key] ?? 0, e.champ!.level) : baseStats[key];
    e.stats[key] = (base + (flat[key] ?? 0)) * (1 + (percent[key] ?? 0));
  }
  let slow = 0;
  for (const b of getBuffs(e)) if (b.def.cc === 'slow') slow = Math.max(slow, Number(b.params.amount ?? b.params.slow ?? 0));
  e.stats.moveSpeed = Math.max(0, e.stats.moveSpeed * (1 - clamp(slow, 0, 0.99)));
  e.stats.attackSpeed = e.stats.attackSpeed > 0 ? clamp(e.stats.attackSpeed, 0.1, 2.5) : 0;
  e.stats.critChance = clamp(e.stats.critChance, 0, 1);
  e.stats.maxHp = Math.max(1, e.stats.maxHp);
  e.stats.maxMana = Math.max(0, e.stats.maxMana);
  e.hp = Math.min(e.hp, e.stats.maxHp);
  e.mana = clamp(e.mana, 0, e.stats.maxMana);
}
export function strongestParam(e: Entity, key: string): number {
  let out = 0;
  for (const b of getBuffs(e)) out = Math.max(out, Number(b.params[key] ?? 0));
  return out;
}
export function updateBuffs(world: World, dt: number): void {
  initBuffs(world);
  for (const e of world.entities.values()) {
    const buffs = getBuffs(e);
    for (let i = buffs.length - 1; i >= 0; i--) {
      const b = buffs[i]!;
      b.remaining -= dt;
      if (b.remaining <= 1e-8 || (e.dead && b.params.persistOnDeath !== true)) removeBuff(world, e, b.id, b.source ?? null);
    }
    if (e.champ || buffs.length || e.data.buffBaseStats) recalculateStats(world, e);
    e.data.tauntForcedMove = false;
    const taunt = buffs.find((b) => b.def.cc === 'taunt');
    const taunter = world.get(taunt?.source);
    if (taunter && !taunter.dead && canAttack(e)) {
      e.attackTarget = taunter.id;
      e.data.tauntForcedMove = true;
      const outOfRange = Math.hypot(taunter.pos.x - e.pos.x, taunter.pos.y - e.pos.y) > e.stats.attackRange + e.radius + taunter.radius;
      e.moveTarget = outOfRange ? { ...taunter.pos } : undefined;
      e.path = undefined;
    }
    if (!canMove(e)) { e.moveTarget = undefined; e.path = undefined; }
    if (!canAttack(e)) { cancelAttack(e); e.attackTarget = undefined; }

  }
}

const initialized = new WeakSet<World>();
/** Controls and mitigation plug into the server's hooks; no edits to another system. */
export function initBuffs(world: World): void {
  if (initialized.has(world)) return;
  initialized.add(world);
  world.hooks.entitySnapshot.push((e, snapshot) => {
    const buffs = getBuffs(e);
    if (buffs.length) snapshot.buffs = buffs.map((b) => ({ id: b.id, remaining: Math.max(0, Math.round(b.remaining * 10) / 10), ...(b.stacks > 1 ? { stacks: b.stacks } : {}) }));
  });
  world.hooks.canMove.push((e) => canMove(e));
  world.hooks.canAttack.push((e) => canAttack(e));
  world.hooks.isTargetable.push((e) => !e.data.stasis);
  world.hooks.beforeAttackHit.push((ctx) => { if (hasCC(ctx.attacker, 'blind')) { ctx.amount = 0; ctx.bonus = 0; } });
  world.hooks.beforeDamage.push((ctx) => {
    if (!Number.isFinite(ctx.amount) || ctx.target.data.stasis) { ctx.amount = 0; return; }
    const source = ctx.credit ?? ctx.source;
    const parry = getBuff(ctx.target, 'kaelen_w');
    if (parry && source?.champ && source.team !== ctx.target.team) {
      removeBuff(world, ctx.target, parry.id, parry.source);
      ctx.amount = 0;
      if (Math.hypot(source.pos.x - ctx.target.pos.x, source.pos.y - ctx.target.pos.y) <= 250 + source.radius + ctx.target.radius)
        addBuff(world, source, 'stun', { duration: Number(parry.params.stun ?? 1), source: ctx.target });
      return;
    }
    if (ctx.type !== 'true') {
      ctx.multiplier *= 1 - Math.min(.95, source ? strongestParam(source, 'damageDealtReduction') : 0);
      ctx.multiplier *= 1 - Math.min(.95, strongestParam(ctx.target, 'damageReduction'));
    }
  });
  world.hooks.afterDamage.push((ctx) => {
    const source = ctx.credit ?? ctx.source;
    if (source?.champ && ctx.target.champ && source.team !== ctx.target.team) ctx.target.data.lastChampionDamageAt = world.time;
  });
  world.hooks.beforeHeal.push((ctx) => { ctx.amount *= 1 - Math.min(1, strongestParam(ctx.target, 'grievous')); });
}
