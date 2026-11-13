/** Bootstrap API required by PLAN §2.1. Owned by claude-server at integration.
 * Damage modifiers run BEFORE mitigation; damage/death events run AFTER health changes.
 */
import { damageMultiplier, type DamageType } from '@lov/shared';
import type { Entity, World } from '../world.ts';
import { absorbShield, addBuff, getBuff, removeBuff, strongestParam } from './buffs.ts';
import { beforeDamage } from '../abilities/combatHooks.ts';

export interface DamageOptions {
  source: Entity | number;
  target: Entity | number;
  amount: number;
  type: DamageType;
  ability?: string;
  crit?: boolean;
  onHit?: boolean;
}
export interface HealOptions { source: Entity | number; target: Entity | number; amount: number }
export interface ShieldOptions extends HealOptions { duration?: number; buffId?: string; ability?: string; fxKey?: string }
const resolve = (w: World, e: Entity | number): Entity | undefined => typeof e === 'number' ? w.get(e) : e;
/** Returns actual health lost (excludes absorbed damage and overkill). */
export function applyDamage(world: World, opts: DamageOptions): number {
  const source = resolve(world, opts.source), target = resolve(world, opts.target);
  if (!source || !target || target.dead || target.data.stasis || !Number.isFinite(opts.amount) || opts.amount <= 0) return 0;
  const parry = getBuff(target, 'kaelen_w');
  if (parry && source.champ && source.team !== target.team) {
    removeBuff(world, target, parry.id, parry.source);
    if (Math.hypot(source.pos.x - target.pos.x, source.pos.y - target.pos.y) <= 250 + source.radius + target.radius)
      addBuff(world, source, 'stun', { duration: Number(parry.params.stun ?? 1), source: target });
    return 0;
  }
  const context = { ...opts, source, target };
  beforeDamage(world, context);
  let amount = Math.max(0, context.amount);
  if (opts.type !== 'true') {
    amount *= 1 - Math.min(0.95, strongestParam(source, 'damageDealtReduction'));
    amount *= 1 - Math.min(0.95, strongestParam(target, 'damageReduction'));
    const resist = opts.type === 'physical' ? target.stats.armor - source.stats.armorPen : target.stats.mr - source.stats.magicPen;
    amount *= damageMultiplier(resist);
  }
  const unshielded = absorbShield(world, target, amount);
  const actual = Math.min(target.hp, unshielded);
  target.hp = Math.max(0, target.hp - unshielded);
  if (source.champ && target.champ && source.team !== target.team) {
    source.champ.damageToChampions += actual;
    target.data.lastChampionDamageAt = world.time;
    source.data.lastChampionCombatAt = world.time;
  }
  world.events.emit({ type: 'damage', source: source.id, target: target.id, amount: actual, damageType: opts.type, ability: opts.ability, crit: context.crit });
  if (target.hp <= 0 && !target.dead) {
    target.dead = true;
    target.anim = 'death';
    target.moveTarget = undefined;
    target.attackTarget = undefined;
    if (target.champ) { target.champ.deaths++; target.champ.respawnIn = 6 + target.champ.level * 2.5; }
    if (source.champ && target.champ && source.team !== target.team) source.champ.kills++;
    world.events.emit({ type: 'death', target: target.id, killer: source.id, assists: [], bounty: 300 });
  }
  return actual;
}
export function applyHeal(world: World, opts: HealOptions): number {
  const source = resolve(world, opts.source), target = resolve(world, opts.target);
  if (!source || !target || target.dead || !Number.isFinite(opts.amount) || opts.amount <= 0) return 0;
  const amount = Math.min(target.stats.maxHp - target.hp, opts.amount * (1 - Math.min(1, strongestParam(target, 'grievous'))));
  target.hp += amount;
  if (amount > 0) world.events.emit({ type: 'heal', source: source.id, target: target.id, amount });
  return amount;
}
export function applyShield(world: World, opts: ShieldOptions): number {
  const source = resolve(world, opts.source), target = resolve(world, opts.target);
  if (!source || !target || target.dead || !Number.isFinite(opts.amount) || opts.amount <= 0) return 0;
  const id = opts.buffId ?? opts.ability ?? 'shield';
  const previous = getBuff(target, id, source.id);
  // Refreshing the same shield never stacks; independent shields do stack.
  const amount = Math.max(previous?.shieldRemaining ?? 0, opts.amount);
  const oldAmount = previous?.shieldRemaining ?? 0;
  const buff = addBuff(world, target, id, { duration: opts.duration ?? 2.5, source, definition: { fxKey: opts.fxKey ?? id } });
  if (!buff) return 0;
  buff.shieldRemaining = amount;
  target.shield += amount - oldAmount;
  world.events.emit({ type: 'shield', source: source.id, target: target.id, amount: amount - oldAmount });
  return amount - oldAmount;
}
