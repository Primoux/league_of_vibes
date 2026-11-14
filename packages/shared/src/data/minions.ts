import type { MinionType } from '../types.ts';

/** Récompenses consommées par economy/leveling via entity.data.rewards. */
// Équilibrage (mesures headless) : une vague entière tuait un tank niveau 6 en ~30 s et un
// squishy en ~10 s. Dégâts réduits d'environ 30 % (c'était ~93 dps bruts contre un tank).
export const MINIONS: Record<MinionType, { hp: number; ad: number; armor: number; range: number; speed: number; attackSpeed: number; gold: number; xp: number }> = {
  melee: { hp: 477, ad: 10, armor: 0, range: 125, speed: 325, attackSpeed: 1.25, gold: 21, xp: 60 },
  caster: { hp: 296, ad: 17, armor: 0, range: 550, speed: 325, attackSpeed: 0.67, gold: 14, xp: 29 },
  cannon: { hp: 912, ad: 32, armor: 0, range: 300, speed: 325, attackSpeed: 1, gold: 60, xp: 93 },
  super: { hp: 1600, ad: 190, armor: 100, range: 170, speed: 325, attackSpeed: 0.85, gold: 90, xp: 97 },
};
