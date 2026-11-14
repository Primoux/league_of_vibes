import type { MonsterType } from '../types.ts';
export interface MonsterDef { hp: number; ad: number; armor: number; range: number; gold: number; xp: number; teamGold?: number }
const small = { hp: 450, ad: 12, armor: 0, range: 125, gold: 7, xp: 15 };
export const MONSTERS: Record<MonsterType, MonsterDef> = {
  blue_buff: { hp: 2300, ad: 66, armor: 20, range: 175, gold: 90, xp: 180 },
  red_buff: { hp: 2300, ad: 66, armor: 20, range: 175, gold: 90, xp: 180 },
  gromp: { hp: 2050, ad: 74, armor: 20, range: 250, gold: 90, xp: 180 },
  wolves: { hp: 1600, ad: 42, armor: 20, range: 125, gold: 55, xp: 100 },
  wolf_small: small, raptors: { hp: 1100, ad: 20, armor: 20, range: 125, gold: 30, xp: 70 },
  raptor_small: { ...small, hp: 350 }, krugs: { hp: 1600, ad: 60, armor: 20, range: 125, gold: 65, xp: 120 }, krug_small: small,
  scuttle: { hp: 1550, ad: 0, armor: 20, range: 0, gold: 55, xp: 115 },
  dragon: { hp: 6500, ad: 90, armor: 30, range: 500, gold: 25, xp: 150, teamGold: 25 },
  elder_dragon: { hp: 12000, ad: 150, armor: 60, range: 500, gold: 250, xp: 500, teamGold: 250 },
  grub: { hp: 1800, ad: 35, armor: 20, range: 200, gold: 20, xp: 75, teamGold: 20 },
  herald: { hp: 9000, ad: 100, armor: 50, range: 250, gold: 100, xp: 200, teamGold: 100 },
  leviathan: { hp: 14000, ad: 160, armor: 80, range: 600, gold: 300, xp: 600, teamGold: 300 },
};
export const DRAGON_ELEMENTS = ['infernal', 'ocean', 'mountain', 'cloud', 'hextech', 'chemtech'] as const;
