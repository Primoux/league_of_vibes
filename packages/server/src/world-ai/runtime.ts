/** Adaptateurs PvE temporaires : remplacer via configurePveApis dès disponibilité des API combat/buffs. */
import { damageMultiplier, dist2, type DamageType, type Stats } from '@lov/shared';
import type { Entity, World } from '../world.ts';
export const stats = (hp: number, ad: number, armor = 0, range = 125, speed = 0, attackSpeed = 1): Stats => ({
  maxHp: hp, hpRegen: 0, maxMana: 0, manaRegen: 0, ad, ap: 0, armor, mr: armor,
  attackSpeed, attackRange: range, moveSpeed: speed, critChance: 0, abilityHaste: 0, lifesteal: 0, armorPen: 0, magicPen: 0,
});
export interface DamageRequest { source: Entity; target: Entity; amount: number; type: DamageType; ability?: string }
export interface BuffOptions { duration: number; source?: Entity; stacks?: number; params?: Record<string, number> }
interface Apis { applyDamage: (world: World, request: DamageRequest) => void; addBuff: (world: World, target: Entity, id: string, options: BuffOptions) => void }
export const isStructure = (e: Entity): boolean => e.kind === 'tower' || e.kind === 'inhibitor' || e.kind === 'nexus';
const guards = new WeakMap<World, (request: DamageRequest) => number>();
export function setStructureDamageGuard(world: World, guard: (request: DamageRequest) => number): void { guards.set(world, guard); }
const fallback: Apis = {
  applyDamage(world, request) {
    const { source, target, type, ability } = request;
    if (target.dead || target.team === source.team) return;
    const multiplier = isStructure(target) ? (guards.get(world)?.(request) ?? 1) : 1;
    let amount = Math.max(0, request.amount * multiplier);
    if (type !== 'true') amount *= damageMultiplier(type === 'physical' ? target.stats.armor - source.stats.armorPen : target.stats.mr - source.stats.magicPen);
    const absorbed = Math.min(target.shield, amount);
    target.shield -= absorbed;
    amount -= absorbed;
    target.hp = Math.max(0, target.hp - amount);
    world.events.emit({ type: 'damage', source: source.id, target: target.id, amount, damageType: type, ability });
    if (target.hp <= 0 && !target.dead) {
      target.dead = true; target.anim = 'death'; target.moveTarget = undefined; target.attackTarget = undefined;
      world.events.emit({ type: 'death', target: target.id, killer: source.id });
    }
  },
  addBuff(world, target, id, options) {
    const buffs = (target.data.pveBuffs ??= {}) as Record<string, { expiresAt: number; stacks: number; params?: Record<string, number> }>;
    buffs[id] = { expiresAt: world.time + options.duration, stacks: options.stacks ?? 1, params: options.params };
    world.events.emit({ type: 'buffApplied', target: target.id, buffId: id, duration: options.duration, source: options.source?.id });
  },
};
const apis = new WeakMap<World, Apis>();
export function configurePveApis(world: World, overrides: Partial<Apis>): void { apis.set(world, { ...fallback, ...overrides }); }
export function applyDamage(world: World, request: DamageRequest): void { (apis.get(world) ?? fallback).applyDamage(world, request); }
export function addBuff(world: World, target: Entity, id: string, options: BuffOptions): void { (apis.get(world) ?? fallback).addBuff(world, target, id, options); }
export function inRange(a: Entity, b: Entity, range: number): boolean { return dist2(a.pos, b.pos) <= (range + b.radius) ** 2; }
export function nearest(world: World, source: Entity, range: number, predicate: (e: Entity) => boolean): Entity | undefined {
  let best: Entity | undefined; let distance = range * range;
  for (const e of world.entities.values()) {
    if (e.dead || e.id === source.id || !predicate(e)) continue;
    const d = dist2(source.pos, e.pos);
    if (d < distance || (d === distance && (!best || e.id < best.id))) { best = e; distance = d; }
  }
  return best;
}
/** Le combat général doit ignorer les entités data.pveCombat ; cet adaptateur gère leur cadence. */
export function attack(world: World, source: Entity, target: Entity, amount = source.stats.ad): boolean {
  source.attackTarget = target.id;
  if (!inRange(source, target, source.stats.attackRange)) { source.moveTarget = target.pos; return false; }
  source.moveTarget = undefined;
  if (world.time < Number(source.data.pveNextAttack ?? 0)) return false;
  source.data.pveNextAttack = world.time + 1 / Math.max(0.1, source.stats.attackSpeed);
  source.anim = 'attack';
  world.events.emit({ type: 'autoAttack', source: source.id, target: target.id, ranged: source.stats.attackRange > 250 });
  applyDamage(world, { source, target, amount, type: 'physical' });
  return true;
}
export function retire(world: World, entity: Entity, delay = 2): void {
  if (!entity.dead) return;
  entity.data.pveRemoveAt ??= world.time + delay;
  if (world.time >= Number(entity.data.pveRemoveAt)) world.remove(entity.id);
}
