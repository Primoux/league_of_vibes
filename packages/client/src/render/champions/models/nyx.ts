/**
 * Nyx, l'Ombre Sans Nom — assassin encapuchonné, voiles d'ombre, deux lames courbes à fil rose.
 */
import { easeIn, easeOut } from '../anim/ease.ts';
import { J, setOffset, setRot, type Clip } from '../anim/pose.ts';
import { geo } from '../builder.ts';
import type { ChampionModelDef } from '../ChampionView.ts';

/** Garde basse, lames vers l'avant. */
function crouch(p: Float32Array): void {
  setRot(p, J.spine, 0, 0, -0.3);
  setRot(p, J.head, 0, 0, 0.2);
  setRot(p, J.armL, 0.4, 0, 0.6);
  setRot(p, J.armR, -0.4, 0, 0.6);
  setRot(p, J.foreL, 0, 0, 0.9);
  setRot(p, J.foreR, 0, 0, 0.9);
  setRot(p, J.legL, 0, 0, 0.45);
  setRot(p, J.shinL, 0, 0, -0.7);
  setRot(p, J.legR, 0, 0, -0.15);
  setRot(p, J.shinR, 0, 0, -0.5);
  setOffset(p, 0, -0.14, 0);
}

const idle: Clip = {
  loop: true,
  duration: 2,
  sample(t, p) {
    crouch(p);
    const b = Math.sin((t / 2) * Math.PI * 2);
    setRot(p, J.spine, 0, b * 0.08, -0.3);
  },
};
/** Double entaille croisée (auto-attaque, Q). */
const strike: Clip = {
  loop: false,
  duration: 0.3,
  sample(t, p) {
    crouch(p);
    const k = easeOut(t / 0.07) * (1 - easeIn((t - 0.12) / 0.18));
    setRot(p, J.armL, 0.4 - 1.0 * k, 0, 0.6 + 0.9 * k);
    setRot(p, J.armR, -0.4 + 1.0 * k, 0, 0.6 + 0.9 * k);
    setRot(p, J.foreL, 0, 0, 0.9 - 0.7 * k);
    setRot(p, J.foreR, 0, 0, 0.9 - 0.7 * k);
    setOffset(p, 0.15 * k, -0.14, 0);
  },
};

export const nyxModel: ChampionModelDef = {
  rig: { hipY: 0.84, hipW: 0.12, thigh: 0.42, shin: 0.42, spine: 0.52, shoulderW: 0.22, upperArm: 0.31, forearm: 0.29, neck: 0.14 },
  scale: 0.98,
  ctx: { energy: 1.25, hand: 1 },
  clips: { idle, strike, cast_Q: strike },
  build(b, c) {
    const shadow = c.primary;
    const violet = c.secondary;
    b.add('hips', geo.box(0.22, 0.2, 0.32), shadow);
    b.add('hips', geo.box(0.24, 0.05, 0.34), c.team, { pos: [0, 0.08, 0] });
    for (const z of [-0.1, 0.1]) b.add('hips', geo.box(0.06, 0.5, 0.12), violet, { pos: [-0.1, -0.22, z], rot: [z, 0, 0.15] }); // voiles
    b.add('spine', geo.box(0.22, 0.4, 0.32), shadow, { pos: [0, 0.26, 0] });
    b.add('spine', geo.box(0.05, 0.3, 0.2), violet, { pos: [0.12, 0.3, 0] });
    b.add('spine', geo.box(0.06, 0.5, 0.3), violet, { pos: [-0.13, 0.2, 0], rot: [0, 0, 0.15] }); // cape courte
    // capuche profonde, aucun visage : deux yeux roses
    b.add('head', geo.ball(0.15, 1), shadow, { pos: [0, 0.12, 0] });
    b.add('head', geo.cone(0.16, 0.3, 6), violet, { pos: [-0.06, 0.24, 0], rot: [0, 0, 0.6] });
    b.glow('head', geo.box(0.02, 0.022, 0.05), c.glow, { pos: [0.15, 0.12, -0.05] });
    b.glow('head', geo.box(0.02, 0.022, 0.05), c.glow, { pos: [0.15, 0.12, 0.05] });
    for (const [arm, fore] of [['armL', 'foreL'], ['armR', 'foreR']] as const) {
      b.add(arm, geo.box(0.1, 0.31, 0.1), shadow, { pos: [0, -0.15, 0] });
      b.add(fore, geo.box(0.1, 0.27, 0.1), violet, { pos: [0, -0.12, 0] });
      // lame courbe le long de l'avant-bras, fil rose
      b.add(fore, geo.box(0.04, 0.5, 0.02), '#2a2a3a', { pos: [0.06, -0.38, 0], rot: [0, 0, 0.35] });
      b.glow(fore, geo.box(0.012, 0.46, 0.025), c.glow, { pos: [0.085, -0.38, 0], rot: [0, 0, 0.35] });
    }
    for (const [leg, shin] of [['legL', 'shinL'], ['legR', 'shinR']] as const) {
      b.add(leg, geo.box(0.13, 0.42, 0.13), shadow, { pos: [0, -0.2, 0] });
      b.add(shin, geo.box(0.13, 0.4, 0.13), violet, { pos: [0, -0.19, 0] });
      b.add(shin, geo.box(0.2, 0.07, 0.12), shadow, { pos: [0.04, -0.39, 0] });
    }
  },
};
