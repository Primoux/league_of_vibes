/** Champion-facing shield metadata layered on the authoritative server API. */
import { applyShield as serverShield, combatData } from '../combat/damage.ts';
import { addBuff } from '../combat/buffs.ts';
import type { Entity, World } from '../world.ts';
export { applyDamage, applyHeal } from '../combat/damage.ts';
export interface ShieldOptions {
  source: Entity | number; target: Entity; amount: number;
  duration?: number; buffId?: string; ability?: string; fxKey?: string;
}
export function applyShield(world: World, opts: ShieldOptions): number {
  const source = typeof opts.source === 'number' ? world.get(opts.source) : opts.source;
  if (!source || opts.target.dead || !Number.isFinite(opts.amount) || opts.amount <= 0) return 0;
  const buffId = opts.buffId ?? opts.ability ?? 'shield', id = `${buffId}:${source.id}`;
  const previous = combatData(opts.target).shields.find((s) => s.id === id)?.amount ?? 0;
  const amount = Math.max(previous, opts.amount);
  const buff = addBuff(world, opts.target, buffId, { source, duration: opts.duration ?? 2.5, params: { shieldId: id }, definition: { fxKey: opts.fxKey ?? buffId } });
  if (!buff) return 0;
  serverShield(world, { source, target: opts.target, amount, duration: opts.duration ?? 2.5, id });
  // A live view of the authoritative shield bucket; no second shield ledger.
  if (!Object.getOwnPropertyDescriptor(buff, 'shieldRemaining')) Object.defineProperty(buff, 'shieldRemaining', {
    enumerable: true, get: () => combatData(opts.target).shields.find((s) => s.id === id)?.amount ?? 0,
  });
  return amount - previous;
}
