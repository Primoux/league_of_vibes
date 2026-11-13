import { MAP_SIZE, angleTo, clamp, moveTowards } from '@lov/shared';
import type { System } from '../world.ts';

/**
 * Propriétaire : claude-server. BOOTSTRAP : déplacement en ligne droite, sans pathfinding ni collisions.
 * TODO(claude-server) : A* sur grille de navigation (WALLS), lissage, collisions unités, dashes (via data.dash).
 */
export const movementSystem: System = {
  name: 'movement',
  update(world, dt) {
    for (const e of world.entities.values()) {
      if (e.dead || !e.moveTarget) {
        if (e.anim === 'run') e.anim = 'idle';
        continue;
      }
      const next = moveTowards(e.pos, e.moveTarget, e.stats.moveSpeed * dt);
      e.facing = angleTo(e.pos, e.moveTarget);
      e.pos = { x: clamp(next.x, 0, MAP_SIZE), y: clamp(next.y, 0, MAP_SIZE) };
      e.anim = 'run';
      if (next.x === e.moveTarget.x && next.y === e.moveTarget.y) e.moveTarget = undefined;
    }
  },
};
