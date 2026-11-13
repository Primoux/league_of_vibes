import type { BuffDef, CCType } from '../schema.ts';

/** Catalogue commun. Les effets de kit peuvent fournir leurs paramètres à addBuff. */
export const BUFFS: Record<string, BuffDef> = {
  ...Object.fromEntries((['stun', 'root', 'slow', 'silence', 'knockup', 'taunt', 'suppress', 'fear', 'blind', 'grounded'] satisfies CCType[])
    .map((cc) => [cc, { id: cc, name: { stun: 'Étourdissement', root: 'Immobilisation', slow: 'Ralentissement', silence: 'Silence', knockup: 'Projection', taunt: 'Provocation', suppress: 'Suppression', fear: 'Peur', blind: 'Aveuglement', grounded: 'Ancrage' }[cc], kind: 'cc', cc, fxKey: `cc_${cc}` } satisfies BuffDef])),
  shield: { id: 'shield', name: 'Bouclier', kind: 'buff', fxKey: 'shield' },
  invisible: { id: 'invisible', name: 'Invisibilité', kind: 'buff', fxKey: 'invisible' },
  grievous: { id: 'grievous', name: 'Blessure profonde', kind: 'debuff', fxKey: 'grievous' },
  haste: { id: 'haste', name: 'Célérité', kind: 'buff' },
  cleanse: { id: 'cleanse', name: 'Résilience', kind: 'buff' },
  exhaust: { id: 'exhaust', name: 'Fatigue', kind: 'debuff', cc: 'slow', fxKey: 'sum_exhaust' },
  ignite: { id: 'ignite', name: 'Embrasement', kind: 'debuff', fxKey: 'sum_ignite' },
};
