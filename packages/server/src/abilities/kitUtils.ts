export { delayedArea, enemiesInRadius, alliesInRadius, startDash, spawnZone, removeZone, spawnProjectile, blink, tether, cone, direction, endpoint, schedule } from './primitives.ts';
import { dist2 } from '@lov/shared';
import type { Entity } from '../world.ts';
export const distWithin = (a: Entity, b: Entity, range: number): boolean => !b.dead && dist2(a.pos, b.pos) <= (range + a.radius + b.radius) ** 2;
