/**
 * État économie d'une entité, namespacé dans `entity.data.economy`.
 * Propriétaire : mistral-economy. Lu/écrit par systems/economy.ts et items/handlers.ts.
 */
import type { Stats, Team } from '@lov/shared';
import type { Entity } from '../world.ts';

/** Action de boutique annulable (pile `shopHistory`, vidée quand on quitte la fontaine). */
export interface ShopAction {
  kind: 'buy' | 'sell';
  itemId: string;
  /** Slot d'inventaire (0..5) ; -1 = trinket. */
  slot: number;
  /** Or dépensé (buy) / remboursé (sell). */
  gold: number;
  /** Prix total payé pour l'objet (pour le remboursement à 70 % à la vente). */
  paidPrice: number;
  /** Composants consommés par la recette (buy uniquement). */
  consumed?: { slot: number; itemId: string; paidPrice: number }[];
  /** Charges du consommable avant l'action. */
  charges?: number;
}

export interface EconomyState {
  killStreak: number;
  deathStreak: number;
  /** Prochain temps de jeu où l'or passif est versé. */
  nextPassiveGoldAt: number;
  recall: { remaining: number; total: number } | null;
  canShop: boolean;
  shopHistory: ShopAction[];
  /** ItemId du trinket équipé, null sinon. */
  trinket: string | null;
  trinketCharges: number;
  /** Temps de jeu de recharge de la prochaine charge de trinket. */
  trinketRechargeAt: number;
  /** Charges par slot de consommable. */
  charges: Record<number, number>;
  /** Prix total payé par slot (vente à 70 %, undo). */
  paidPrice: Record<number, number>;
  /** Cooldowns d'actifs : activeId -> temps de jeu de fin. */
  itemCooldowns: Record<string, number>;
  /** Bonus de stats actuellement appliqués à ent.stats (objets + effets temporaires). */
  appliedItemStats: Partial<Stats> | null;
  /** Soin sur la durée (potion) : total, restant (s). */
  hot: { total: number; remaining: number; duration: number } | null;
  staseUntil: number;
  grievousUntil: number;
  spellbladeReady: boolean;
  rafaleUntil: number;
  voileShieldAt: number;
  /** Demande d'invocation du Colosse (lue par codex-objectives). */
  colossusCall: { team: Team; x: number; y: number } | null;
}

export function defaultEconomyState(): EconomyState {
  return {
    killStreak: 0,
    deathStreak: 0,
    nextPassiveGoldAt: 0, // initialisé au premier update (scaled(PASSIVE_GOLD_START))
    recall: null,
    canShop: false,
    shopHistory: [],
    trinket: null,
    trinketCharges: 0,
    trinketRechargeAt: 0,
    charges: {},
    paidPrice: {},
    itemCooldowns: {},
    appliedItemStats: null,
    hot: null,
    staseUntil: -Infinity,
    grievousUntil: -Infinity,
    spellbladeReady: false,
    rafaleUntil: -Infinity,
    voileShieldAt: 0,
    colossusCall: null,
  };
}

export function econ(e: Entity): EconomyState {
  return (e.data.economy ??= defaultEconomyState()) as EconomyState;
}
