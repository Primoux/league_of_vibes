import type { Vec2 } from './types.ts';

export const dist = (a: Vec2, b: Vec2): number => Math.hypot(a.x - b.x, a.y - b.y);
export const dist2 = (a: Vec2, b: Vec2): number => (a.x - b.x) ** 2 + (a.y - b.y) ** 2;
export const clamp = (v: number, lo: number, hi: number): number => Math.max(lo, Math.min(hi, v));
export const lerp = (a: number, b: number, t: number): number => a + (b - a) * t;
export const angleTo = (from: Vec2, to: Vec2): number => Math.atan2(to.y - from.y, to.x - from.x);

/** Avance `from` vers `to` d'au plus `step`. */
export function moveTowards(from: Vec2, to: Vec2, step: number): Vec2 {
  const d = dist(from, to);
  if (d <= step || d === 0) return { x: to.x, y: to.y };
  return { x: from.x + ((to.x - from.x) / d) * step, y: from.y + ((to.y - from.y) / d) * step };
}

/** Réduction de dégâts LoL : 100 / (100 + résistance) (résistance négative : 2 - 100/(100 - r)). */
export function damageMultiplier(resist: number): number {
  return resist >= 0 ? 100 / (100 + resist) : 2 - 100 / (100 - resist);
}

/** Valeur par niveau façon LoL : base + growth * (n-1) * (0.7025 + 0.0175 * (n-1)). */
export function statAtLevel(base: number, growth: number, level: number): number {
  const n = level - 1;
  return base + growth * n * (0.7025 + 0.0175 * n);
}
