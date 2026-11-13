import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { describe, expect, it } from 'vitest';
import { CHAMPION_LIST } from '@lov/shared';
import manifest from '../src/assets/champions/manifest.json';
import { championArtworkUrl } from '../src/ui/icons/ChampionArtwork.tsx';

function dimensions(data: Buffer): [number, number] {
  for (let offset = 12; offset + 8 < data.length;) {
    const tag = data.toString('ascii', offset, offset + 4), size = data.readUInt32LE(offset + 4), start = offset + 8;
    if (tag === 'VP8 ') return [data.readUInt16LE(start + 6) & 0x3fff, data.readUInt16LE(start + 8) & 0x3fff];
    if (tag === 'VP8X') return [data.readUIntLE(start + 4, 3) + 1, data.readUIntLE(start + 7, 3) + 1];
    if (tag === 'VP8L') { const bits = data.readUInt32LE(start + 1); return [(bits & 0x3fff) + 1, ((bits >>> 14) & 0x3fff) + 1]; }
    offset += 8 + size + (size & 1);
  }
  throw new Error('Dimensions WebP introuvables');
}

describe('portraits de sélection', () => {
  it('couvre chaque champion avec une image distincte, valide et légère', () => {
    expect(manifest.portraits.map(p => p.id).sort()).toEqual(CHAMPION_LIST.map(c => c.id).sort());
    const hashes = new Set<string>(); let total = 0;
    for (const entry of manifest.portraits) {
      const bytes = readFileSync(new URL(`../src/assets/champions/${entry.file}`, import.meta.url));
      expect(bytes.toString('ascii', 0, 4)).toBe('RIFF'); expect(bytes.toString('ascii', 8, 12)).toBe('WEBP');
      expect(dimensions(bytes)).toEqual([512, 512]); expect(bytes.length).toBeLessThan(90000);
      hashes.add(createHash('sha256').update(bytes).digest('hex')); total += bytes.length;
      expect(championArtworkUrl(entry.id)).toContain(`${entry.id}.webp`);
    }
    expect(hashes.size).toBe(CHAMPION_LIST.length); expect(total).toBeLessThan(1_000_000);
  });
  it('laisse le portrait SVG de secours aux identifiants inconnus', () => {
    expect(championArtworkUrl('inconnu')).toBeUndefined();
  });
});
