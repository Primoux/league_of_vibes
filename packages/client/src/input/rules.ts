/**
 * Règles pures de l'input (testables). Propriétaire : claude-client.
 */
import type { EntitySnapshot, Targeting, Team } from '@lov/shared';

const ATTACKABLE_KINDS = new Set(['champion', 'minion', 'tower', 'inhibitor', 'nexus', 'monster', 'ward']);

/** Peut-on lancer une auto-attaque sur cette entité ? (ennemie ou neutre, vivante, d'un type ciblable) */
export function isAttackable(e: EntitySnapshot, myTeam: Team): boolean {
  return !e.dead && e.team !== myTeam && ATTACKABLE_KINDS.has(e.kind);
}

/** Le sort exige-t-il une unité ciblée (clic sur une entité) ? */
export function needsTarget(t: Targeting): boolean {
  return t === 'targeted' || t === 'targetedDash';
}
