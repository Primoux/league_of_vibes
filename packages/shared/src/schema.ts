/**
 * Schémas de données : champions, sorts, buffs, objets, sorts d'invocateur.
 * Propriétaires : codex-champions (Champion/Ability/Buff/Summoner), mistral-economy (Item).
 */
import type { AbilitySlot, DamageType, Role, Stats, SummonerId } from './types.ts';

export type ChampionClass = 'tank' | 'fighter' | 'mage' | 'marksman' | 'assassin' | 'support';

export type Targeting =
  | 'skillshot' // projectile en ligne (width, speed, range)
  | 'targeted' // cible unitaire
  | 'ground' // point au sol (zone)
  | 'cone'
  | 'self'
  | 'dash' // déplacement vers un point
  | 'targetedDash' // déplacement vers une cible
  | 'global';

/** Ratio de scaling : valeur = base[rank] + ad*adRatio + ap*apRatio + ... */
export interface Scaling {
  base: number[]; // une valeur par rang du sort
  ad?: number;
  bonusAd?: number;
  ap?: number;
  maxHp?: number;
  bonusHp?: number;
  armor?: number;
  targetMaxHp?: number; // % PV max de la cible (0.05 = 5 %)
  targetMissingHp?: number;
}

export interface AbilityDef {
  id: string; // ex. 'grondak_q'
  slot: AbilitySlot | 'P';
  name: string;
  description: string; // texte joueur (FR)
  targeting: Targeting;
  maxRank: number; // 5 pour QWE, 3 pour R, 0 pour passif
  cooldown: number[]; // secondes par rang
  cost: number[]; // mana par rang
  range: number;
  radius?: number; // zone / cône
  width?: number; // skillshot
  speed?: number; // projectile / dash
  castTime?: number; // secondes d'immobilisation
  damage?: Scaling & { type: DamageType };
  /** Clé FX à utiliser par claude-fx : convention `<championId>_<slot>[_<variante>]`. */
  fxKey: string;
  /** Paramètres libres spécifiques au kit (durées de CC, %, etc.). */
  params?: Record<string, number | number[]>;
}

export interface ChampionDef {
  id: string; // minuscules, ex. 'grondak'
  name: string;
  title: string;
  class: ChampionClass;
  roles: Role[];
  difficulty: 1 | 2 | 3;
  ranged: boolean;
  resource: 'mana' | 'energy' | 'none';
  baseStats: Stats;
  perLevel: Partial<Stats>; // croissance linéaire simplifiée
  attackWindup: number; // fraction de l'attaque avant l'impact (0.25 typique)
  projectileSpeed?: number; // auto-attaques à distance
  passive: AbilityDef;
  abilities: Record<AbilitySlot, AbilityDef>;
  /** Couleurs principales pour modèle/FX (hex). */
  palette: { primary: string; secondary: string; accent: string };
  lore: string;
}

export type BuffKind = 'buff' | 'debuff' | 'cc';
export type CCType = 'stun' | 'root' | 'slow' | 'silence' | 'knockup' | 'taunt' | 'suppress' | 'fear' | 'blind' | 'grounded';

export interface BuffDef {
  id: string;
  name: string;
  kind: BuffKind;
  cc?: CCType;
  maxStacks?: number;
  /** Les buffs sont résolus côté serveur par le système buffs (codex-champions). */
  statMods?: Partial<Stats>;
  statPercentMods?: Partial<Stats>;
  fxKey?: string;
}

export interface SummonerDef {
  id: SummonerId;
  name: string;
  description: string;
  cooldown: number;
  targeting: Targeting;
  range: number;
  fxKey: string;
}

export type ItemCategory = 'starter' | 'consumable' | 'boots' | 'basic' | 'epic' | 'legendary' | 'trinket';

export interface ItemDef {
  id: string;
  name: string;
  description: string;
  category: ItemCategory;
  cost: number; // coût total
  buildsFrom: string[];
  stats: Partial<Stats>;
  /** Passif nommé, résolu côté serveur par le système economy/items (mistral-economy). */
  passive?: { id: string; name: string; description: string; params?: Record<string, number> };
  active?: { id: string; name: string; description: string; cooldown: number };
  unique?: boolean;
  maxStacks?: number; // consommables
  icon: string; // clé d'icône pour l'UI
}
