import { ITEMS, clamp, getChampion, statAtLevel, type Stats } from '@lov/shared';
import { BUFFS } from '../../../shared/src/data/buffs.ts';
import type { BuffDef, CCType } from '@lov/shared';
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
export const canMove = (e: Entity): boolean => canAct(e) && !hasCC(e, 'root') && !e.data.casting && !e.data.channeling;
export const canAttack = (e: Entity): boolean => canAct(e) && !e.data.casting && !e.data.channeling;
export const canCast = (e: Entity): boolean => canAct(e) && !hasCC(e, 'silence') && !e.data.casting && !e.data.channeling;
export function getTenacity(e: Entity): number {
  let remaining = 1 - clamp(Number(e.data.itemTenacity ?? 0), 0, 0.9);
  for (const b of getBuffs(e)) remaining *= 1 - clamp(b.params.tenacity ?? 0, 0, 0.9);
  return clamp(1 - remaining, 0, 0.9);
}
function syncFlags(e: Entity): void {
  const buffs = getBuffs(e);
  e.data.invisible = buffs.some((b) => b.id === 'invisible' || b.params.invisible);
  e.data.ghosted = buffs.some((b) => b.params.ghosted);
}
export function addBuff(world: World, target: Entity, buffId: string, opts: BuffOptions): BuffInstance | undefined {
  if (target.dead || !Number.isFinite(opts.duration) || opts.duration <= 0) return;
  const source = typeof opts.source === 'number' ? opts.source : opts.source?.id;
  const def: BuffDef = { id: buffId, name: buffId, kind: 'buff', ...BUFFS[buffId], ...opts.definition };
  const reducible = def.cc && !['knockup', 'suppress', 'slow', 'grounded'].includes(def.cc);
  const duration = opts.duration * (reducible ? 1 - getTenacity(target) : 1);
  let buff = getBuff(target, buffId, source);
  if (buff) {
    buff.remaining = Math.max(buff.remaining, duration);
    buff.duration = Math.max(buff.duration, duration);
    buff.stacks = Math.min(def.maxStacks ?? 1, buff.stacks + (opts.stacks ?? 1));
    buff.params = { ...buff.params, ...opts.params };
  } else {
    buff = { id: buffId, source, remaining: duration, duration, stacks: Math.min(def.maxStacks ?? 1, opts.stacks ?? 1), params: { ...opts.params }, def };
    getBuffs(target).push(buff);
  }
  syncFlags(target);
  if (target.champ) recalculateStats(world, target);
  world.events.emit({ type: 'buffApplied', target: target.id, buffId, source, duration, stacks: buff.stacks, fxKey: def.fxKey ?? buffId });
  return buff;
}
export function removeBuff(world: World, target: Entity, buffId: string, source?: number): void {
  const buffs = getBuffs(target);
  for (let i = buffs.length - 1; i >= 0; i--) {
    const b = buffs[i]!;
    if (b.id !== buffId || (source !== undefined && b.source !== source)) continue;
    target.shield = Math.max(0, target.shield - (b.shieldRemaining ?? 0));
    buffs.splice(i, 1);
    world.events.emit({ type: 'buffRemoved', target: target.id, buffId, fxKey: b.def.fxKey ?? buffId });
  }
  syncFlags(target);
  if (target.champ) recalculateStats(world, target);
}
export function cleanse(world: World, e: Entity): void {
  for (const b of [...getBuffs(e)]) {
    if (b.def.cc && b.def.cc !== 'knockup' && b.def.cc !== 'suppress') removeBuff(world, e, b.id, b.source);
  }
}
export function breakInvisibility(world: World, e: Entity): void {
  for (const b of [...getBuffs(e)]) if (b.id === 'invisible' || b.params.invisible) removeBuff(world, e, b.id, b.source);
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
export function recalculateStats(world: World, e: Entity): void {
  if (!e.champ) return;
  const def = getChampion(e.champ.championId);
  const { flat = {}, percent = {} } = getStatMods(e);
  for (const id of e.champ.items) if (id && ITEMS[id]) mergeStats(flat, ITEMS[id].stats);
  let tenacity = 0;
  for (const fn of statSources) {
    const mods = fn(world, e);
    if (!mods) continue;
    mergeStats(flat, mods.flat);
    mergeStats(percent, mods.percent);
    tenacity = 1 - (1 - tenacity) * (1 - (mods.tenacity ?? 0));
  }
  e.data.itemTenacity = tenacity;
  for (const key of Object.keys(def.baseStats) as (keyof Stats)[]) {
    const base = statAtLevel(def.baseStats[key], def.perLevel[key] ?? 0, e.champ.level);
    e.stats[key] = (base + (flat[key] ?? 0)) * (1 + (percent[key] ?? 0));
  }
  let slow = 0;
  for (const b of getBuffs(e)) if (b.def.cc === 'slow') slow = Math.max(slow, Number(b.params.amount ?? b.params.slow ?? 0));
  e.stats.moveSpeed = Math.max(0, e.stats.moveSpeed * (1 - clamp(slow, 0, 0.99)));
  e.stats.attackSpeed = clamp(e.stats.attackSpeed, 0.1, 2.5);
  e.stats.critChance = clamp(e.stats.critChance, 0, 1);
  e.stats.maxHp = Math.max(1, e.stats.maxHp);
  e.stats.maxMana = Math.max(0, e.stats.maxMana);
  e.hp = Math.min(e.hp, e.stats.maxHp);
  e.mana = clamp(e.mana, 0, e.stats.maxMana);
}
/** Consume shields with the earliest expiry first, then any shield owned by another system. */
export function absorbShield(world: World, e: Entity, amount: number): number {
  let left = Math.min(amount, e.shield);
  const absorbed = left;
  const buckets = getBuffs(e).filter((b) => (b.shieldRemaining ?? 0) > 0).sort((a, b) => a.remaining - b.remaining);
  for (const b of buckets) {
    const used = Math.min(left, b.shieldRemaining!);
    b.shieldRemaining! -= used;
    left -= used;
    // Keep the empty bucket until expiry: associated effects (e.g. frost shield) still have a duration.
    if (left <= 0) break;
  }
  e.shield = Math.max(0, e.shield - absorbed);
  return amount - absorbed;
}
export function strongestParam(e: Entity, key: string): number {
  let out = 0;
  for (const b of getBuffs(e)) out = Math.max(out, Number(b.params[key] ?? 0));
  return out;
}
export function updateBuffs(world: World, dt: number): void {
  for (const e of world.entities.values()) {
    const buffs = getBuffs(e);
    for (let i = buffs.length - 1; i >= 0; i--) {
      const b = buffs[i]!;
      b.remaining -= dt;
      if (b.remaining <= 1e-8 || (e.dead && b.params.persistOnDeath !== true)) removeBuff(world, e, b.id, b.source);
    }
    if (e.champ) recalculateStats(world, e);
    // Enforce controls even with the bootstrap movement system.
    if (!canMove(e)) { e.moveTarget = undefined; e.path = undefined; }
    if (!canAttack(e) || hasCC(e, 'blind')) e.attackTarget = undefined;
    const taunt = buffs.find((b) => b.def.cc === 'taunt');
    if (taunt && !e.dead && world.get(taunt.source) && !world.get(taunt.source)!.dead) e.attackTarget = taunt.source;
  }
}
