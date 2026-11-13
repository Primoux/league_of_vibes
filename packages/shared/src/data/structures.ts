import type { TowerTier } from '../types.ts';
export const STRUCTURES = {
  towers: {
    outer: { hp: 5000, ad: 152, armor: 40, localGold: 250, teamGold: 50 },
    inner: { hp: 3600, ad: 170, armor: 55, localGold: 300, teamGold: 50 },
    inhib: { hp: 3300, ad: 190, armor: 70, localGold: 300, teamGold: 50 },
    nexus: { hp: 2700, ad: 210, armor: 70, localGold: 50, teamGold: 50 },
  } satisfies Record<TowerTier, { hp: number; ad: number; armor: number; localGold: number; teamGold: number }>,
  inhibitorHp: 4000, nexusHp: 5500, range: 750, plateGold: 125, plateRadius: 1000,
  backdoorRadius: 1000, backdoorResistance: 200, fortificationUntil: 180,
};
