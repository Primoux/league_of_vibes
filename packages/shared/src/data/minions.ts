import type { MinionType } from '../types.ts';

/**
 * Récompenses consommées par economy/leveling via entity.data.rewards.
 * Équilibrage (mesures headless, branche codex-objectives-balance) : dégâts -30 % (avant : une
 * vague tuait un tank lvl6 en ~31 s et un squishy en ~5 s). PV ajustés à la baisse pour garder
 * la même durée de combat de vague (sinon le test d'endurance minions.test.ts ralentit en CI).
 */
export const MINIONS: Record<MinionType, { hp: number; ad: number; armor: number; range: number; speed: number; attackSpeed: number; gold: number; xp: number }> = {
  melee: { hp: 370, ad: 10, armor: 0, range: 125, speed: 325, attackSpeed: 1.25, gold: 21, xp: 60 },
  caster: { hp: 230, ad: 17, armor: 0, range: 550, speed: 325, attackSpeed: 0.67, gold: 14, xp: 29 },
  cannon: { hp: 700, ad: 32, armor: 0, range: 300, speed: 325, attackSpeed: 1, gold: 60, xp: 93 },
  super: { hp: 1240, ad: 190, armor: 100, range: 170, speed: 325, attackSpeed: 0.85, gold: 90, xp: 97 },
};
