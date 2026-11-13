/** Règles d'affichage du HUD (pures, testées). */
import { ULT_LEVELS, type AbilitySlot, type Role, type Team } from '@lov/shared';

/** Le joueur peut-il monter ce sort maintenant ? (PLAN §5 : R à 6/11/16, QWE ≤ ceil(niveau/2), max 5). */
export function canRankUp(slot: AbilitySlot, level: number, rank: number, skillPoints: number): boolean {
  if (skillPoints <= 0) return false;
  if (slot === 'R') return rank < 3 && ULT_LEVELS.filter((l) => level >= l).length > rank;
  return rank < 5 && rank < Math.ceil(level / 2);
}

export interface ScoreRow {
  playerId: string;
  name: string;
  team: Team;
  championId: string;
  role?: Role;
  level: number;
  kills: number;
  deaths: number;
  assists: number;
  cs: number;
  items: (string | null)[];
  summoners: [string, string];
  dead: boolean;
  respawnIn?: number;
  isMe: boolean;
}

const ROLE_ORDER: Role[] = ['top', 'jungle', 'mid', 'bot'];

/** Tri : par rôle (haut, jungle, milieu, bas) puis par nom. */
export function sortScoreRows(rows: readonly ScoreRow[]): ScoreRow[] {
  return [...rows].sort((a, b) => {
    const ra = a.role ? ROLE_ORDER.indexOf(a.role) : 9;
    const rb = b.role ? ROLE_ORDER.indexOf(b.role) : 9;
    return ra - rb || a.name.localeCompare(b.name);
  });
}

/** Score « MVP » simple pour l'écran de fin. */
export function mvpScore(r: { kills: number; deaths: number; assists: number; cs: number; damageToChampions?: number }): number {
  return r.kills * 3 + r.assists * 1.5 - r.deaths * 1.5 + r.cs * 0.02 + (r.damageToChampions ?? 0) / 1500;
}

/** Fraction 0..1 d'un temps de recharge, bornée. */
export function cooldownFraction(remaining: number, total: number): number {
  if (remaining <= 0 || total <= 0) return 0;
  return Math.min(1, remaining / total);
}

/** Coût affiché d'un sort à un rang donné (rang 1 = index 0). */
export function rankValue(values: readonly number[], rank: number): number {
  if (values.length === 0) return 0;
  return values[Math.max(0, Math.min(values.length - 1, rank - 1))] ?? 0;
}

/** Recharge effective avec accélération : cd * 100 / (100 + haste). */
export function hastedCooldown(base: number, haste: number): number {
  return (base * 100) / (100 + Math.max(0, haste));
}
