import { describe, expect, it } from 'vitest';
import { CURVE_RES, bakeStyle, sampleCurve, sampleGradient } from '../../src/fx/core/curves.ts';
import { Pool } from '../../src/fx/core/pool.ts';
import { RingAllocator } from '../../src/fx/core/ring.ts';
import { Rng } from '../../src/fx/core/rng.ts';
import { Scheduler } from '../../src/fx/core/scheduler.ts';

describe('courbes', () => {
  it('interpole linéairement et borne aux extrémités', () => {
    const c = [[0, 0], [0.5, 1], [1, 0]] as const;
    expect(sampleCurve(c, -1)).toBe(0);
    expect(sampleCurve(c, 0.25)).toBeCloseTo(0.5);
    expect(sampleCurve(c, 0.5)).toBe(1);
    expect(sampleCurve(c, 0.75)).toBeCloseTo(0.5);
    expect(sampleCurve(c, 2)).toBe(0);
  });

  it('courbe vide = 1, courbe à une clé = constante', () => {
    expect(sampleCurve([], 0.3)).toBe(1);
    expect(sampleCurve([[0, 0.4]], 0.9)).toBe(0.4);
  });

  it('dégradé : interpolation par canal', () => {
    const out = { r: 0, g: 0, b: 0 };
    sampleGradient([[0, 1, 0, 0], [1, 0, 0, 2]], 0.5, out);
    expect(out).toEqual({ r: 0.5, g: 0, b: 1 });
    sampleGradient([[0.2, 1, 1, 1], [0.8, 0, 0, 0]], 0.1, out);
    expect(out.r).toBe(1);
    sampleGradient([[0.2, 1, 1, 1], [0.8, 0, 0, 0]], 0.9, out);
    expect(out.r).toBe(0);
  });

  it('bakeStyle écrit couleur+alpha puis taille sur 2 lignes', () => {
    const data = new Float32Array(CURVE_RES * 4 * 4);
    bakeStyle([[0, 1, 2, 3]], [[0, 1], [1, 0]], [[0, 0.5], [1, 1]], data, 2);
    const row2 = 2 * CURVE_RES * 4;
    const row3 = 3 * CURVE_RES * 4;
    expect([data[row2], data[row2 + 1], data[row2 + 2], data[row2 + 3]]).toEqual([1, 2, 3, 1]);
    expect(data[row2 + (CURVE_RES - 1) * 4 + 3]).toBe(0); // alpha final
    expect(data[row3]).toBe(0.5);
    expect(data[row3 + (CURVE_RES - 1) * 4]).toBe(1);
    expect(data[0]).toBe(0); // lignes 0-1 intactes
  });
});

describe('Pool', () => {
  it('réutilise les objets libérés et applique reset', () => {
    let n = 0;
    const pool = new Pool(() => ({ id: n++, used: false }), (o) => (o.used = false));
    const a = pool.acquire()!;
    a.used = true;
    pool.release(a);
    const b = pool.acquire()!;
    expect(b).toBe(a);
    expect(b.used).toBe(false);
    expect(pool.size).toBe(1);
  });

  it('respecte la limite max', () => {
    const pool = new Pool(() => ({}), undefined, 2);
    expect(pool.acquire()).not.toBeNull();
    expect(pool.acquire()).not.toBeNull();
    expect(pool.acquire()).toBeNull();
  });
});

describe('RingAllocator', () => {
  it('suit une plage contiguë', () => {
    const r = new RingAllocator(8);
    r.next();
    r.next();
    r.next();
    expect(r.flush()).toBe(1);
    expect([r.rangeStart[0], r.rangeCount[0]]).toEqual([0, 3]);
    expect(r.flush()).toBe(0);
  });

  it('découpe en 2 plages quand le tampon boucle', () => {
    const r = new RingAllocator(8);
    for (let i = 0; i < 6; i++) r.next();
    r.flush();
    for (let i = 0; i < 4; i++) r.next(); // 6,7,0,1
    expect(r.flush()).toBe(2);
    expect([r.rangeStart[0], r.rangeCount[0], r.rangeStart[1], r.rangeCount[1]]).toEqual([6, 2, 0, 2]);
  });

  it('reset repart de l’emplacement 0 sans plage en attente', () => {
    const r = new RingAllocator(8);
    for (let i = 0; i < 5; i++) r.next();
    r.reset();
    expect(r.flush()).toBe(0);
    expect(r.next()).toBe(0);
  });

  it('plage complète si plus de capacity écritures', () => {
    const r = new RingAllocator(4);
    for (let i = 0; i < 10; i++) r.next();
    expect(r.flush()).toBe(1);
    expect([r.rangeStart[0], r.rangeCount[0]]).toEqual([0, 4]);
  });
});

describe('Rng', () => {
  it('est déterministe et borné', () => {
    const a = new Rng(42);
    const b = new Rng(42);
    for (let i = 0; i < 100; i++) {
      const x = a.range(2, 5);
      expect(x).toBe(b.range(2, 5));
      expect(x).toBeGreaterThanOrEqual(2);
      expect(x).toBeLessThan(5);
    }
    expect(a.pick(7)).toBe(7);
  });
});

describe('Scheduler', () => {
  it('exécute les tâches à échéance, y compris celles planifiées pendant update', () => {
    const s = new Scheduler();
    const log: string[] = [];
    s.after(0.5, () => {
      log.push('a');
      s.after(0.2, () => log.push('b'));
    });
    s.update(0.3);
    expect(log).toEqual([]);
    s.update(0.3);
    expect(log).toEqual(['a']);
    s.update(0.1);
    expect(log).toEqual(['a']);
    s.update(0.15);
    expect(log).toEqual(['a', 'b']);
    expect(s.pending).toBe(0);
  });
});
