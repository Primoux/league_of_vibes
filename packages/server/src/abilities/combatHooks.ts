import type { DamageContext, World } from '../world.ts';
export type { DamageContext } from '../world.ts';
/** Compatibility helper: registers directly on claude-server's authoritative pre-damage pipeline. */
export function registerDamageModifier(world: World, fn: (context: DamageContext) => void): () => void {
  world.hooks.beforeDamage.push(fn);
  return () => { const index = world.hooks.beforeDamage.indexOf(fn); if (index >= 0) world.hooks.beforeDamage.splice(index, 1); };
}
