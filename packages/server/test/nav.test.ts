import { describe, expect, it } from 'vitest';
import { WALLS, dist, type Polygon } from '@lov/shared';
import { NavGrid, navGrid } from '../src/nav/grid.ts';
import { coreWorld, champ, run } from './helpers.ts';

const rect = (x: number, y: number, w: number, h: number): Polygon => [
  { x, y }, { x: x + w, y }, { x: x + w, y: y + h }, { x, y: y + h },
];

describe('NavGrid', () => {
  const grid = new NavGrid([rect(2000, 1000, 400, 3000)], 6000);

  it('ligne droite quand rien ne bloque', () => {
    expect(grid.findPath({ x: 500, y: 500 }, { x: 1500, y: 1500 })).toEqual([{ x: 1500, y: 1500 }]);
  });

  it('contourne un mur et le chemin lissé reste accessible', () => {
    const from = { x: 1500, y: 2500 };
    const to = { x: 3000, y: 2500 };
    const path = grid.findPath(from, to);
    expect(path.length).toBeGreaterThan(1);
    expect(path.length).toBeLessThan(6);
    expect(path[path.length - 1]).toEqual(to);
    let prev = from;
    let len = 0;
    for (const p of path) {
      expect(grid.lineWalkable(prev, p)).toBe(true);
      len += dist(prev, p);
      prev = p;
    }
    // Contourne par le haut (y=4000) ou le bas (y=1000) : ~ 2 × 1500 + 1500
    expect(len).toBeLessThan(5200);
  });

  it('clic dans un mur → point accessible le plus proche', () => {
    const p = grid.nearestWalkable({ x: 2050, y: 2500 });
    expect(grid.walkable(p.x, p.y)).toBe(true);
    expect(p.x).toBeLessThan(2000);
    const path = grid.findPath({ x: 1500, y: 2500 }, { x: 2050, y: 2500 });
    expect(path[path.length - 1]!.x).toBeLessThan(2000);
  });

  it('point totalement enfermé : chemin vide', () => {
    const boxed = new NavGrid([rect(900, 900, 200, 1200), rect(2900, 900, 200, 1200), rect(900, 900, 2200, 200), rect(900, 1900, 2200, 200)], 4000);
    expect(boxed.findPath({ x: 2000, y: 1500 }, { x: 3500, y: 3500 })).toEqual([]);
  });

  it('grille réelle : A* rapide sur toute la carte', () => {
    const g = navGrid();
    for (const w of WALLS) {
      // centroïde (les murs sont convexes) : indépendant de la géométrie exacte de map.ts
      const c = { x: w.reduce((s, p) => s + p.x, 0) / w.length, y: w.reduce((s, p) => s + p.y, 0) / w.length };
      expect(g.walkable(c.x, c.y)).toBe(false);
    }
    const t0 = performance.now();
    for (let i = 0; i < 50; i++) g.findPath({ x: 600, y: 600 }, { x: 11400, y: 11400 });
    const per = (performance.now() - t0) / 50;
    expect(per).toBeLessThan(15);
  });
});

/** Boîte englobante d'un mur (les tests ne dépendent pas de la géométrie exacte de map.ts). */
function wallBox(p: { x: number; y: number }[]): { minX: number; maxX: number; minY: number; maxY: number } {
  return { minX: Math.min(...p.map((q) => q.x)), maxX: Math.max(...p.map((q) => q.x)), minY: Math.min(...p.map((q) => q.y)), maxY: Math.max(...p.map((q) => q.y)) };
}

describe('movement', () => {
  it('un champion contourne un mur de la carte pour atteindre sa destination', () => {
    const w = coreWorld();
    const b = wallBox(WALLS[0]!);
    const from = { x: b.minX - 250, y: (b.minY + b.maxY) / 2 };
    const to = { x: b.maxX + 200, y: from.y };
    expect(navGrid().walkable(from.x, from.y) && navGrid().walkable(to.x, to.y)).toBe(true);
    const c = champ(w, 'blue', from);
    w.commands.push({ playerId: c.champ!.playerId, msg: { t: 'move', pos: to } });
    let crossed = false;
    for (let i = 0; i < 30 * 15; i++) {
      w.step();
      if (navGrid().walkable(c.pos.x, c.pos.y) === false) crossed = true;
    }
    expect(crossed).toBe(false);
    expect(c.pos.x).toBeCloseTo(to.x, 0);
  });

  it('dash : atteint la destination, émet dash, appelle onArrive, s’arrête aux murs', () => {
    const w = coreWorld();
    const c = champ(w, 'blue', { x: 6000, y: 3000 });
    let arrived = 0;
    c.data.dash = { to: { x: 6400, y: 3000 }, speed: 1200, onArrive: () => arrived++ };
    run(w, 0.5);
    expect(c.pos).toEqual({ x: 6400, y: 3000 });
    expect(arrived).toBe(1);
    expect(c.data.dash).toBeUndefined();
    expect(w.events.drain().filter((e) => e.type === 'dash')).toHaveLength(1);

    const b = wallBox(WALLS[0]!);
    const y = (b.minY + b.maxY) / 2;
    const d = champ(w, 'blue', { x: b.minX - 250, y });
    d.data.dash = { to: { x: b.maxX + 300, y }, speed: 1500 };
    run(w, 1);
    expect(d.pos.x).toBeLessThan(b.minX);
    expect(navGrid().walkable(d.pos.x, d.pos.y)).toBe(true);
  });

  it('collisions : deux unités ne se superposent pas, ghost les ignore', () => {
    const w = coreWorld();
    const a = champ(w, 'blue', { x: 6000, y: 3000 });
    const b = champ(w, 'blue', { x: 6001, y: 3000 });
    run(w, 1);
    expect(dist(a.pos, b.pos)).toBeGreaterThan(80);
    const c = champ(w, 'blue', { x: 7000, y: 3000 });
    const d = champ(w, 'blue', { x: 7001, y: 3000 });
    c.data.ghost = true;
    run(w, 1);
    expect(dist(c.pos, d.pos)).toBeLessThan(5);
  });

  it('les structures bloquent les champions', () => {
    const w = coreWorld();
    const tower = w.spawn({ kind: 'tower', team: 'red', pos: { x: 6000, y: 3000 }, radius: 120, hp: 5000, mana: 0, stats: { ...champ(w, 'red', { x: 0, y: 0 }).stats, moveSpeed: 0 } });
    const c = champ(w, 'blue', { x: 5500, y: 3000 });
    w.commands.push({ playerId: c.champ!.playerId, msg: { t: 'move', pos: { x: 6500, y: 3000 } } });
    let minD = Infinity;
    for (let i = 0; i < 90; i++) {
      w.step();
      minD = Math.min(minD, dist(c.pos, tower.pos));
    }
    expect(minD).toBeGreaterThan(tower.radius + c.radius - 15);
  });
});
