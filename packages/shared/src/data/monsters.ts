import type { MonsterType } from '../types.ts';
export interface MonsterDef { hp: number; ad: number; armor: number; range: number; gold: number; xp: number; teamGold?: number }
// Équilibrage (mesures headless, branche codex-objectives-balance) : les camps étaient ~3x trop longs à clear
// pour un jungler niveau 3 (gromp ~60 s) et leurs dégâts de base tuaient un jungler 1v1.
// PV réduits d'environ 35-40 %, dégâts réduits d'environ 25-30 %, armure 20 -> 15.
const small = { hp: 300, ad: 9, armor: 0, range: 125, gold: 7, xp: 25 };
export const MONSTERS: Record<MonsterType, MonsterDef> = {
  blue_buff: { hp: 1500, ad: 50, armor: 15, range: 175, gold: 90, xp: 280 },
  red_buff: { hp: 1500, ad: 50, armor: 15, range: 175, gold: 90, xp: 280 },
  gromp: { hp: 1350, ad: 55, armor: 15, range: 250, gold: 90, xp: 280 },
  wolves: { hp: 1100, ad: 32, armor: 15, range: 125, gold: 55, xp: 160 },
  wolf_small: small, raptors: { hp: 750, ad: 15, armor: 15, range: 125, gold: 30, xp: 110 },
  raptor_small: { ...small, hp: 240 }, krugs: { hp: 1100, ad: 45, armor: 15, range: 125, gold: 65, xp: 180 }, krug_small: small,
  scuttle: { hp: 1100, ad: 0, armor: 15, range: 0, gold: 55, xp: 180 },
  dragon: { hp: 5000, ad: 70, armor: 25, range: 500, gold: 25, xp: 240, teamGold: 25 },
  elder_dragon: { hp: 9000, ad: 115, armor: 50, range: 500, gold: 250, xp: 750, teamGold: 250 },
  grub: { hp: 1300, ad: 28, armor: 15, range: 200, gold: 20, xp: 120, teamGold: 20 },
  herald: { hp: 6500, ad: 75, armor: 40, range: 250, gold: 100, xp: 320, teamGold: 100 },
  leviathan: { hp: 10000, ad: 120, armor: 65, range: 600, gold: 300, xp: 900, teamGold: 300 },
};
export const DRAGON_ELEMENTS = ['infernal', 'ocean', 'mountain', 'cloud', 'hextech', 'chemtech'] as const;
