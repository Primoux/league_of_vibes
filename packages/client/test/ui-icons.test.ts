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

import { ITEM_LIST } from '@lov/shared';
import { ITEM_GLYPH, itemGlyphKey, itemTint, itemVariation } from '../src/ui/icons/glyphs.ts';

describe('icônes d’objets', () => {
  it('ont un glyphe défini pour chaque objet', () => {
    for (const it of ITEM_LIST) expect(ITEM_GLYPH[itemGlyphKey(it)], it.id).toBeTruthy();
  });
  it('sont variées sur tout le catalogue', () => {
    const glyphs = new Set(ITEM_LIST.map(itemGlyphKey));
    expect(glyphs.size).toBeGreaterThanOrEqual(30);
    // combinaison glyphe + teinte + variation : quasiment toutes distinctes
    const combos = new Set(ITEM_LIST.map((i) => `${itemGlyphKey(i)}|${itemTint(i)}|${itemVariation(i).hue}|${itemVariation(i).tilt}`));
    expect(combos.size).toBeGreaterThanOrEqual(ITEM_LIST.length - 2);
  });
});
