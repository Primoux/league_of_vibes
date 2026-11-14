import type { Entity, World } from '../world.ts';

type BurnKey = 'redBurn' | 'elderBurn';
interface BurnState { source: number; next: number; remaining: number }

/** Rafraîchit trois ticks sans repousser celui qui était déjà programmé. */
export function refreshBurn(world: World, target: Entity, key: BurnKey, source: Entity): void {
  const burn = target.data[key] as BurnState | undefined;
  if (burn && burn.remaining > 0) { burn.source = source.id; burn.remaining = 3; }
  else target.data[key] = { source: source.id, next: world.time + 1, remaining: 3 } satisfies BurnState;
}

/** Consomme un tick échu ; le compteur reste borné même après un long pas de simulation. */
export function takeBurnTick(world: World, target: Entity, key: BurnKey): Entity | undefined {
  const burn = target.data[key] as BurnState | undefined;
  if (!burn) return;
  if (target.dead || target.hp <= 0 || burn.remaining <= 0) { delete target.data[key]; return; }
  if (world.time + 1e-8 < burn.next) return;
  const source = world.get(burn.source);
  if (!source) { delete target.data[key]; return; }
  burn.next += 1; burn.remaining--;
  if (burn.remaining === 0) delete target.data[key];
  return source;
}
