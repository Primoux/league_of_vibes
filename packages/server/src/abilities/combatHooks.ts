import type { DamageType } from '@lov/shared';
import type { Entity, World } from '../world.ts';
export interface DamageContext {
  source: Entity;
  target: Entity;
  amount: number;
  type: DamageType;
  ability?: string;
  crit?: boolean;
  /** False for triggered damage: prevents recursive on-hit effects. */
  onHit?: boolean;
}
type DamageHook = (context: DamageContext) => void;
const hooks = new WeakMap<World, Set<DamageHook>>();
export function registerDamageModifier(world: World, fn: DamageHook): () => void {
  let set = hooks.get(world);
  if (!set) hooks.set(world, set = new Set());
  set.add(fn);
  return () => { set.delete(fn); };
}
export function beforeDamage(world: World, context: DamageContext): void {
  hooks.get(world)?.forEach((fn) => fn(context));
}
