import { describe, expect, it } from 'vitest';
import {
  BRUSHES,
  CAMPS,
  FOUNTAINS,
  INHIBITORS,
  LANE_HALF_WIDTH,
  LANE_PATHS,
  MAP_SIZE,
  NEXUS,
  PITS,
  TOWERS,
  WALLS,
  isInWall,
  mirror,
  polygonArea,
  type Polygon,
  type Vec2,
} from '../src/index.ts';

function isConvexCCW(p: Polygon): boolean {
  for (let i = 0; i < p.length; i++) {
    const a = p[i]!;
    const b = p[(i + 1) % p.length]!;
    const c = p[(i + 2) % p.length]!;
    if ((b.x - a.x) * (c.y - b.y) - (b.y - a.y) * (c.x - b.x) < 0) return false;
  }
  return polygonArea(p) > 0;
}

/** Distance d'un point à un segment. */
function distSeg(p: Vec2, a: Vec2, b: Vec2): number {
  const dx = b.x - a.x;
  const dy = b.y - a.y;
  const t = Math.max(0, Math.min(1, ((p.x - a.x) * dx + (p.y - a.y) * dy) / (dx * dx + dy * dy)));
  return Math.hypot(p.x - a.x - t * dx, p.y - a.y - t * dy);
}

describe('carte', () => {
  it('murs convexes, anti-horaires', () => {
    for (const w of WALLS) expect(isConvexCCW(w)).toBe(true);
  });

  it('murs et brousses symétriques par rotation 180°', () => {
    const half = WALLS.length / 2;
    for (let i = 0; i < half; i++) expect(WALLS[i + half]).toEqual(WALLS[i]!.map(mirror));
    const bh = BRUSHES.length / 2;
    for (let i = 0; i < bh; i++) expect(BRUSHES[i + bh]).toEqual(BRUSHES[i]!.map(mirror));
  });

  it('camps symétriques et ids uniques', () => {
    expect(new Set(CAMPS.map((c) => c.id)).size).toBe(CAMPS.length);
    for (const c of CAMPS.filter((c) => c.side === 'blue')) {
      const red = CAMPS.find((r) => r.id === c.id.replace('blue_', 'red_'))!;
      expect(red.pos).toEqual(mirror(c.pos));
    }
  });

  it('aucun mur sur les voies', () => {
    for (const path of Object.values(LANE_PATHS)) {
      for (let i = 0; i < path.length - 1; i++) {
        for (const w of WALLS) for (const p of w) expect(distSeg(p, path[i]!, path[i + 1]!)).toBeGreaterThan(LANE_HALF_WIDTH);
        for (let k = 0; k <= 40; k++) {
          const a = path[i]!;
          const b = path[i + 1]!;
          expect(isInWall({ x: a.x + ((b.x - a.x) * k) / 40, y: a.y + ((b.y - a.y) * k) / 40 })).toBe(false);
        }
      }
    }
  });

  it('camps, fosses et structures dégagés (rayon 150)', () => {
    const pts = [...CAMPS.map((c) => c.pos), ...TOWERS.map((t) => t.pos), ...INHIBITORS.map((i) => i.pos), NEXUS.blue, NEXUS.red, PITS.dragon.pos, PITS.leviathan.pos];
    for (const p of pts) for (let a = 0; a < 8; a++) expect(isInWall({ x: p.x + Math.cos(a) * 150, y: p.y + Math.sin(a) * 150 })).toBe(false);
  });

  it('tout est accessible depuis la fontaine bleue (flood fill, grille 50 u, unité de rayon 40)', () => {
    const CELL = 50;
    const N = MAP_SIZE / CELL;
    const free = new Uint8Array(N * N);
    for (let j = 0; j < N; j++)
      for (let i = 0; i < N; i++) {
        const c = { x: i * CELL + 25, y: j * CELL + 25 };
        let ok = !isInWall(c);
        for (let a = 0; ok && a < 6; a++) ok = !isInWall({ x: c.x + Math.cos(a) * 40, y: c.y + Math.sin(a) * 40 });
        free[j * N + i] = ok ? 1 : 0;
      }
    const seen = new Uint8Array(N * N);
    const idx = (p: Vec2) => Math.floor(p.y / CELL) * N + Math.floor(p.x / CELL);
    const stack = [idx(FOUNTAINS.blue)];
    seen[stack[0]!] = 1;
    while (stack.length) {
      const k = stack.pop()!;
      const i = k % N;
      const j = (k - i) / N;
      for (const [di, dj] of [[1, 0], [-1, 0], [0, 1], [0, -1]] as const) {
        const ni = i + di;
        const nj = j + dj;
        if (ni < 0 || nj < 0 || ni >= N || nj >= N) continue;
        const nk = nj * N + ni;
        if (!seen[nk] && free[nk]) {
          seen[nk] = 1;
          stack.push(nk);
        }
      }
    }
    for (const c of CAMPS) expect(seen[idx(c.pos)], c.id).toBe(1);
    expect(seen[idx(FOUNTAINS.red)]).toBe(1);
    expect(seen[idx(PITS.dragon.pos)]).toBe(1);
    // aucune poche isolée : toute cellule libre est atteignable
    let unreachable = 0;
    for (let k = 0; k < N * N; k++) if (free[k] && !seen[k]) unreachable++;
    expect(unreachable).toBe(0);
  });
});
