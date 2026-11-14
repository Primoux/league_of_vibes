import type { TowerTier } from '../types.ts';
// Équilibrage (mesures headless, branche codex-objectives-balance) : une tour externe à 5000 PV + 40 armure
// + backdoor +200 ne mourait pas en 90 s même pour un ADC niveau 6. PV -25 % environ, armure -10,
// résistance anti-backdoor 200 -> 60 (la fortification à -50 % avant 3:00 reste).
export const STRUCTURES = {
  towers: {
    outer: { hp: 3400, ad: 152, armor: 30, localGold: 250, teamGold: 50 },
    inner: { hp: 2600, ad: 170, armor: 45, localGold: 300, teamGold: 50 },
    inhib: { hp: 2700, ad: 190, armor: 60, localGold: 300, teamGold: 50 },
    nexus: { hp: 2200, ad: 210, armor: 60, localGold: 50, teamGold: 50 },
  } satisfies Record<TowerTier, { hp: number; ad: number; armor: number; localGold: number; teamGold: number }>,
  inhibitorHp: 3200, nexusHp: 4500, range: 750, plateGold: 125, plateRadius: 1000,
  backdoorRadius: 1000, backdoorResistance: 60, fortificationUntil: 180,
};
