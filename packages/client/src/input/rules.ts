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

/**
 * Touches de la rangée du haut (position physique, indépendante du clavier AZERTY/QWERTY) :
 * 1 2 3 5 6 7 = objets (slots 0 à 5), 4 = balise (trinket).
 */
export function digitAction(code: string): { kind: 'item'; slot: number } | { kind: 'ward' } | null {
  switch (code) {
    case 'Digit1':
      return { kind: 'item', slot: 0 };
    case 'Digit2':
      return { kind: 'item', slot: 1 };
    case 'Digit3':
      return { kind: 'item', slot: 2 };
    case 'Digit4':
      return { kind: 'ward' };
    case 'Digit5':
      return { kind: 'item', slot: 3 };
    case 'Digit6':
      return { kind: 'item', slot: 4 };
    case 'Digit7':
      return { kind: 'item', slot: 5 };
    default:
      return null;
  }
}
