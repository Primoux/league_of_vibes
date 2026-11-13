/**
 * Fonctions géométriques pures sur la carte (testables, sans DOM). Propriétaire : claude-client.
 */
import { LANE_HALF_WIDTH, LANE_PATHS, MAP_SIZE, RIVER_HALF_WIDTH, type Vec2 } from '@lov/shared';

const SQRT1_2 = Math.SQRT1_2;

export function distToSegment(px: number, py: number, a: Vec2, b: Vec2): number {
  const dx = b.x - a.x;
  const dy = b.y - a.y;
  const l2 = dx * dx + dy * dy;
  const t = l2 === 0 ? 0 : Math.max(0, Math.min(1, ((px - a.x) * dx + (py - a.y) * dy) / l2));
  return Math.hypot(px - a.x - t * dx, py - a.y - t * dy);
}

const LANES = Object.values(LANE_PATHS);

/** Distance à la voie la plus proche (axe central). */
export function distToLane(x: number, y: number): number {
  let best = Infinity;
  for (const path of LANES) for (let i = 0; i < path.length - 1; i++) best = Math.min(best, distToSegment(x, y, path[i]!, path[i + 1]!));
  return best;
}

/** Distance (perpendiculaire) à l'axe de la rivière x + y = MAP_SIZE. */
export function distToRiver(x: number, y: number): number {
  return Math.abs(x + y - MAP_SIZE) * SQRT1_2;
}

const smooth = (e0: number, e1: number, x: number): number => {
  const t = Math.max(0, Math.min(1, (x - e0) / (e1 - e0)));
  return t * t * (3 - 2 * t);
};

export const RIVER_DEPTH = 32; // unités de jeu

/**
 * Hauteur du terrain (unités de jeu) : la rivière est creusée en pente douce, léger relief dans la jungle,
 * voies plates. Les unités sont posées à cette hauteur.
 */
export function terrainHeight(x: number, y: number): number {
  const dr = distToRiver(x, y);
  // la rivière s'arrête avant les coins des voies (embouchures)
  const along = Math.abs(x - y) * SQRT1_2;
  const riverMask = 1 - smooth(5800, 6600, along);
  let h = -RIVER_DEPTH * (1 - smooth(RIVER_HALF_WIDTH - 220, RIVER_HALF_WIDTH + 120, dr)) * riverMask;
  const laneFlat = smooth(LANE_HALF_WIDTH, LANE_HALF_WIDTH + 400, distToLane(x, y));
  h += (Math.sin(x * 0.0011) * Math.cos(y * 0.0013) * 9 + Math.sin((x + y) * 0.0023) * 4) * laneFlat * (1 - (1 - smooth(0, RIVER_HALF_WIDTH + 300, dr)) * riverMask);
  return h;
}
