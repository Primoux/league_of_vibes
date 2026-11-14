import { getChampion } from '@lov/shared';
import { describe, expect, it } from 'vitest';
import { aimFor, clampToRange, towards } from '../../src/fx/telegraph/aim.ts';

const caster = { id: 1, pos: { x: 1000, y: 1000 } };
const target = { id: 2, pos: { x: 1600, y: 1000 } };
void target.id;

describe('aim', () => {
  it('clampToRange / towards', () => {
    expect(clampToRange({ x: 0, y: 0 }, { x: 300, y: 400 }, 1000)).toEqual({ x: 300, y: 400 });
    expect(clampToRange({ x: 0, y: 0 }, { x: 300, y: 400 }, 250)).toEqual({ x: 150, y: 200 });
    expect(towards({ x: 0, y: 0 }, { x: 3, y: 4 }, 10)).toEqual({ x: 6, y: 8 });
  });

  it('forme selon le ciblage', () => {
    const ilyra = getChampion('ilyra');
    const q = aimFor(ilyra.abilities.Q, caster.pos, target.pos);
    expect(q.aim?.kind).toBe('line');
    expect(q.range).toBe(950);
    if (q.aim?.kind === 'line') expect(q.aim.to).toEqual({ x: 1950, y: 1000 });

    const w = aimFor(ilyra.abilities.W, caster.pos, { x: 5000, y: 1000 });
    expect(w.aim).toEqual({ kind: 'circle', center: { x: 1850, y: 1000 }, radius: 280 });

    expect(aimFor(ilyra.abilities.E, caster.pos, target.pos).aim).toBeNull();

    const cone = aimFor(getChampion('sable').abilities.W, caster.pos, target.pos);
    expect(cone.aim?.kind).toBe('cone');
    if (cone.aim?.kind === 'cone') expect(cone.aim.halfAngle).toBeCloseTo((25 * Math.PI) / 180);

    const globalUlt = aimFor(getChampion('sable').abilities.R, caster.pos, target.pos);
    expect(globalUlt.range).toBeNull(); // pas de cercle de 12 000 u
  });
});
