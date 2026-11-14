import { CHAMPION_LIST } from '@lov/shared';
import { describe, expect, it } from 'vitest';
import { abilityGlyphName } from '../src/ui/icons/glyphs.ts';

describe('icônes de sorts', () => {
  it('donnent des glyphes variés par champion', () => {
    for (const c of CHAMPION_LIST) {
      const names = (['Q', 'W', 'E', 'R'] as const).map((s) => abilityGlyphName(c.abilities[s]));
      expect(new Set(names).size, `${c.id}: ${names.join(',')}`).toBeGreaterThanOrEqual(3);
    }
  });
});
