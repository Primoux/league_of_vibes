import { describe, expect, it } from 'vitest';
import { DynamicResolution } from '../src/render/DynamicResolution.ts';

/** Avance de `seconds` avec des frames de `dt` ; retourne les ratios appliqués. */
function run(d: DynamicResolution, dt: number, seconds: number): number[] {
  const out: number[] = [];
  for (let t = 0; t < seconds; t += dt) {
    const r = d.update(dt);
    if (r !== null) out.push(r);
  }
  return out;
}

describe('DynamicResolution', () => {
  it('reste au maximum tant que les frames sont fluides', () => {
    const d = new DynamicResolution(2);
    expect(run(d, 1 / 60, 20)).toEqual([]);
    expect(d.ratio).toBe(2);
  });

  it('descend par paliers quand les frames sont lentes, sans passer sous le minimum', () => {
    const d = new DynamicResolution(2, 1);
    const steps = run(d, 1 / 30, 30);
    expect(steps[0]).toBe(1.75);
    expect(steps.at(-1)).toBe(1);
    expect(d.ratio).toBe(1);
    // paliers espacés (pas de chute brutale en une frame)
    expect(steps.length).toBe(4);
  });

  it('ignore les pics isolés (onglet masqué, chargement)', () => {
    const d = new DynamicResolution(2);
    run(d, 1 / 60, 2);
    expect(d.update(0.5)).toBeNull();
    expect(run(d, 1 / 60, 2)).toEqual([]);
  });

  it('remonte après une longue période fluide, et se bloque si la remontée rechute', () => {
    const d = new DynamicResolution(2, 1);
    run(d, 1 / 30, 3.5); // 1 ou 2 paliers vers le bas
    const low = d.ratio;
    expect(low).toBeLessThan(2);
    const up = run(d, 1 / 60, 12);
    expect(up[0]).toBe(low + 0.25);
    // rechute juste après la remontée : on redescend…
    const down = run(d, 1 / 30, 1.6);
    expect(down[0]).toBe(low);
    // …et on ne retente pas avant une minute
    expect(run(d, 1 / 60, 40)).toEqual([]);
    expect(run(d, 1 / 60, 40).length).toBeGreaterThan(0);
  });

  it('reset repart du nouveau maximum', () => {
    const d = new DynamicResolution(2);
    run(d, 1 / 30, 5);
    d.reset(1.5);
    expect(d.ratio).toBe(1.5);
  });
});
