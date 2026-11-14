import { describe, expect, it } from 'vitest';
import { CHAMPION_LIST, CAMPS, INHIBITORS, MAP_SIZE, TOWERS, isInWall, mirror } from '../src/index.ts';

describe('shared data', () => {
  it('a 12 champions avec 4 sorts + passif et des ids uniques', () => {
    expect(CHAMPION_LIST).toHaveLength(12);
    expect(new Set(CHAMPION_LIST.map((c) => c.id)).size).toBe(12);
    for (const c of CHAMPION_LIST) {
      expect(Object.keys(c.abilities).sort()).toEqual(['E', 'Q', 'R', 'W']);
      expect(c.abilities.R.maxRank).toBe(3);
      expect(c.passive.slot).toBe('P');
      for (const a of Object.values(c.abilities)) expect(a.fxKey.startsWith(c.id)).toBe(true);
    }
  });

  it('la carte est symétrique et les structures hors des murs', () => {
    expect(TOWERS).toHaveLength(22);
    expect(INHIBITORS).toHaveLength(6);
    const blue = TOWERS.filter((t) => t.team === 'blue');
    const red = TOWERS.filter((t) => t.team === 'red');
    blue.forEach((t, i) => expect(red[i]!.pos).toEqual(mirror(t.pos)));
    for (const s of [...TOWERS, ...INHIBITORS, ...CAMPS]) {
      expect(isInWall(s.pos)).toBe(false);
      expect(s.pos.x).toBeGreaterThan(0);
      expect(s.pos.x).toBeLessThan(MAP_SIZE);
    }
  });
});
