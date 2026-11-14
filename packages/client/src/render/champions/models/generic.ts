/**
 * Modèle humanoïde générique aux couleurs du champion, arme selon la classe.
 * Utilisé par les champions qui n'ont pas encore leur modèle dédié (M3).
 */
import type { ChampionClass } from '@lov/shared';
import { geo, type ModelBuilder } from '../builder.ts';
import type { ChampionModelDef, ModelColors } from '../ChampionView.ts';

const SKIN = '#e6c9a8';

function weapon(b: ModelBuilder, cls: ChampionClass, c: ModelColors): void {
  switch (cls) {
    case 'tank': // masse
      b.add('prop', geo.cyl(0.035, 0.035, 0.7), c.secondary, { pos: [0.3, 0, 0], rot: [0, 0, Math.PI / 2] });
      b.add('prop', geo.rock(0.16, 3), c.accent, { pos: [0.65, 0, 0] });
      break;
    case 'fighter': // épée
      b.add('prop', geo.box(0.9, 0.06, 0.02), '#d8dde6', { pos: [0.52, 0, 0] });
      b.add('prop', geo.box(0.06, 0.04, 0.28), c.accent, { pos: [0.06, 0, 0] });
      break;
    case 'mage': // bâton + orbe
      b.add('prop', geo.cyl(0.025, 0.03, 1.5), c.secondary, { pos: [0, 0.25, 0] });
      b.glow('prop', geo.ball(0.09), c.glow, { pos: [0, 1.05, 0] });
      break;
    case 'marksman': // arme longue tenue vers l'avant
      b.add('prop', geo.box(0.7, 0.08, 0.07), c.secondary, { pos: [0.3, 0.02, 0] });
      b.add('prop', geo.cyl(0.03, 0.03, 0.4, 6), '#8a8f99', { pos: [0.8, 0.04, 0], rot: [0, 0, Math.PI / 2] });
      break;
    case 'assassin': // dague
      b.add('prop', geo.crystal(0.04, 0.5), '#cfd4e0', { pos: [0.28, 0, 0], rot: [0, 0, -Math.PI / 2] });
      break;
    case 'support': // bâton à halo
      b.add('prop', geo.cyl(0.025, 0.03, 1.4), c.secondary, { pos: [0, 0.2, 0] });
      b.glow('prop', geo.torus(0.12, 0.02, 14), c.glow, { pos: [0, 0.98, 0] });
      break;
  }
}

export const genericModel: ChampionModelDef = {
  rig: { hipY: 0.85, hipW: 0.15, thigh: 0.42, shin: 0.42, spine: 0.58, shoulderW: 0.28, upperArm: 0.33, forearm: 0.31, neck: 0.15 },
  build(b, c) {
    const cls = c.def.class;
    // bassin + ceinture d'équipe
    b.add('hips', geo.box(0.26, 0.22, 0.4), c.secondary, { pos: [0, 0.02, 0] });
    b.add('hips', geo.box(0.28, 0.06, 0.43), c.team, { pos: [0, 0.1, 0] });
    // torse
    b.add('spine', geo.box(0.3, 0.42, 0.46), c.primary, { pos: [0, 0.32, 0] });
    b.add('spine', geo.box(0.32, 0.12, 0.5), c.secondary, { pos: [0, 0.1, 0] });
    b.add('spine', geo.box(0.06, 0.2, 0.2), c.accent, { pos: [0.15, 0.38, 0] });
    // épaulières
    for (const z of [-0.28, 0.28]) b.add('spine', geo.ball(0.12, 0), cls === 'tank' || cls === 'fighter' ? c.accent : c.primary, { pos: [0, 0.56, z], scale: [1, 0.7, 1] });
    // tête
    b.add('head', geo.ball(0.16, 1), SKIN, { pos: [0.02, 0.12, 0] });
    b.add('head', geo.ball(0.17, 1), c.secondary, { pos: [-0.03, 0.17, 0], scale: [1, 0.75, 1.02] }); // cheveux / capuche
    b.glow('head', geo.box(0.03, 0.03, 0.04), c.glow, { pos: [0.16, 0.13, -0.06] });
    b.glow('head', geo.box(0.03, 0.03, 0.04), c.glow, { pos: [0.16, 0.13, 0.06] });
    // bras
    for (const [arm, fore] of [['armL', 'foreL'], ['armR', 'foreR']] as const) {
      b.add(arm, geo.box(0.13, 0.34, 0.13), c.primary, { pos: [0, -0.16, 0] });
      b.add(fore, geo.box(0.12, 0.3, 0.12), c.secondary, { pos: [0, -0.14, 0] });
      b.add(fore, geo.ball(0.07, 0), SKIN, { pos: [0, -0.31, 0] });
    }
    // jambes
    for (const [leg, shin] of [['legL', 'shinL'], ['legR', 'shinR']] as const) {
      b.add(leg, geo.box(0.16, 0.42, 0.16), c.secondary, { pos: [0, -0.2, 0] });
      b.add(shin, geo.box(0.14, 0.38, 0.14), c.primary, { pos: [0, -0.19, 0] });
      b.add(shin, geo.box(0.24, 0.08, 0.15), c.secondary, { pos: [0.04, -0.38, 0] });
    }
    weapon(b, cls, c);
  },
};

/** Variante de proportions selon la classe (tanks plus massifs, assassins plus fins). */
export function genericFor(cls: ChampionClass): ChampionModelDef {
  const scale = cls === 'tank' ? 1.12 : cls === 'assassin' ? 0.96 : 1;
  const energy = cls === 'tank' ? 0.75 : cls === 'assassin' ? 1.2 : 1;
  // port d'arme selon la classe : arme basse vers l'avant, bâtons droits
  const carry = cls === 'mage' || cls === 'support' ? 0 : cls === 'marksman' ? -0.3 : cls === 'assassin' ? -0.7 : -0.3;
  const dropAngle = cls === 'mage' || cls === 'support' ? -Math.PI / 2 : 0;
  return { ...genericModel, scale, carry, dropAngle, ctx: { energy } };
}
