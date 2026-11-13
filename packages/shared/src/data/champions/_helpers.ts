import type { AbilityDef, ChampionDef } from '../../schema.ts';
import type { Stats } from '../../types.ts';

export const DEFAULT_STATS: Stats = {
  maxHp: 600,
  hpRegen: 8,
  maxMana: 340,
  manaRegen: 8,
  ad: 60,
  ap: 0,
  armor: 32,
  mr: 32,
  attackSpeed: 0.65,
  attackRange: 175,
  moveSpeed: 340,
  critChance: 0,
  abilityHaste: 0,
  lifesteal: 0,
  armorPen: 0,
  magicPen: 0,
};

export const stats = (s: Partial<Stats>): Stats => ({ ...DEFAULT_STATS, ...s });

type AbilityInput = Omit<AbilityDef, 'maxRank' | 'cost' | 'cooldown'> & Partial<Pick<AbilityDef, 'maxRank' | 'cost' | 'cooldown'>>;

export const ability = (a: AbilityInput): AbilityDef => ({
  maxRank: a.slot === 'R' ? 3 : a.slot === 'P' ? 0 : 5,
  cost: [0],
  cooldown: [0],
  ...a,
});

export const defineChampion = (c: ChampionDef): ChampionDef => c;
