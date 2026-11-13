/**
 * Vrenna, la Lance des Steppes — guerrière nomade : armure de cuir rouge, foulard crème, longue lance à pointe dorée.
 */
import { easeIn, easeOut } from '../anim/ease.ts';
import { J, setOffset, setRot, type Clip } from '../anim/pose.ts';
import { geo } from '../builder.ts';
import type { ChampionModelDef } from '../ChampionView.ts';

/** Lance tenue à deux mains, pointe vers l'avant. */
function guard(p: Float32Array): void {
  setRot(p, J.armR, -0.2, 0, 0.5);
  setRot(p, J.foreR, 0, 0, 0.9);
  setRot(p, J.prop, 0, 0, -1.4);
  setRot(p, J.armL, 0.35, 0, 0.9);
  setRot(p, J.foreL, 0, 0, 0.8);
}

const idle: Clip = {
  loop: true,
  duration: 2.4,
  sample(t, p) {
    guard(p);
    const b = Math.sin((t / 2.4) * Math.PI * 2);
    setRot(p, J.spine, 0, 0.15, b * 0.03);
    setRot(p, J.legL, 0, 0, 0.2);
    setRot(p, J.legR, 0, 0, -0.15);
    setOffset(p, 0, b * 0.015 - 0.02, 0);
  },
};

/** Estoc (auto-attaque, Q) : la lance part vers l'avant. */
const strike: Clip = {
  loop: false,
  duration: 0.36,
  sample(t, p) {
    guard(p);
    const k = easeOut(t / 0.08) * (1 - easeIn((t - 0.14) / 0.22));
    setRot(p, J.armR, -0.2, 0, 0.5 + 0.9 * k);
    setRot(p, J.foreR, 0, 0, 0.9 - 0.85 * k);
    setRot(p, J.prop, 0, 0, -1.4 - 0.05 * k);
    setRot(p, J.spine, 0, 0.15 - 0.4 * k, -0.15 * k);
    setRot(p, J.legL, 0, 0, 0.45 * k + 0.2);
    setRot(p, J.legR, 0, 0, -0.35 * k);
    setOffset(p, 0.15 * k, -0.04 * k, 0);
  },
};

/** W : cri de guerre, lance brandie. */
const castW: Clip = {
  loop: false,
  duration: 0.7,
  sample(t, p) {
    const k = easeOut(t / 0.15) * (1 - easeIn((t - 0.45) / 0.25));
    setRot(p, J.armR, -0.3, 0, 2.6 * k + 0.4);
    setRot(p, J.foreR, 0, 0, 0.3);
    setRot(p, J.prop, 0, 0, -0.5);
    setRot(p, J.armL, 0.9 * k, 0, 0.3);
    setRot(p, J.spine, 0, 0, 0.3 * k);
    setRot(p, J.head, 0, 0, -0.4 * k);
  },
};

export const vrennaModel: ChampionModelDef = {
  rig: { hipY: 0.88, hipW: 0.14, thigh: 0.44, shin: 0.43, spine: 0.56, shoulderW: 0.26, upperArm: 0.33, forearm: 0.3, neck: 0.15 },
  dropAngle: -Math.PI / 2,
  carry: -0.5,
  ctx: { energy: 1.1, hand: 1 },
  clips: { idle, strike, cast_Q: strike, cast_W: castW },
  build(b, c) {
    const red = c.primary;
    const cream = c.secondary;
    const leather = '#5a2e22';
    const skin = '#c99a74';
    b.add('hips', geo.box(0.24, 0.22, 0.38), leather);
    b.add('hips', geo.cone(0.3, 0.45, 6), red, { pos: [0, -0.22, 0] }); // jupe de cuir
    b.add('hips', geo.box(0.26, 0.06, 0.4), c.team, { pos: [0, 0.09, 0] });
    b.add('spine', geo.box(0.26, 0.42, 0.4), red, { pos: [0, 0.3, 0] });
    b.add('spine', geo.box(0.28, 0.12, 0.44), cream, { pos: [0, 0.5, 0] }); // foulard
    b.add('spine', geo.box(0.18, 0.4, 0.1), cream, { pos: [-0.16, 0.36, 0.08], rot: [0.2, 0, 0.2] }); // pan du foulard
    b.add('head', geo.ball(0.14, 1), skin, { pos: [0.02, 0.1, 0] });
    b.add('head', geo.ball(0.155, 1), '#2b1a14', { pos: [-0.04, 0.15, 0], scale: [1, 0.8, 1] });
    b.add('head', geo.box(0.36, 0.08, 0.08), '#2b1a14', { pos: [-0.2, 0.06, 0], rot: [0, 0, 0.6] }); // tresse
    b.glow('head', geo.box(0.02, 0.025, 0.04), c.glow, { pos: [0.15, 0.11, -0.05] });
    b.glow('head', geo.box(0.02, 0.025, 0.04), c.glow, { pos: [0.15, 0.11, 0.05] });
    for (const [arm, fore] of [['armL', 'foreL'], ['armR', 'foreR']] as const) {
      b.add(arm, geo.box(0.11, 0.32, 0.11), skin, { pos: [0, -0.15, 0] });
      b.add(fore, geo.box(0.12, 0.28, 0.12), leather, { pos: [0, -0.13, 0] });
      b.add(fore, geo.ball(0.055, 0), skin, { pos: [0, -0.29, 0] });
    }
    for (const [leg, shin] of [['legL', 'shinL'], ['legR', 'shinR']] as const) {
      b.add(leg, geo.box(0.14, 0.44, 0.14), leather, { pos: [0, -0.2, 0] });
      b.add(shin, geo.box(0.14, 0.4, 0.14), red, { pos: [0, -0.19, 0] });
      b.add(shin, geo.box(0.22, 0.08, 0.14), leather, { pos: [0.04, -0.39, 0] });
    }
    // lance : hampe crème, fanion rouge, pointe dorée lumineuse (le long de +y local de la main)
    b.add('prop', geo.cyl(0.025, 0.025, 2.0), cream, { pos: [0, 0.35, 0] });
    b.add('prop', geo.box(0.02, 0.16, 0.22), red, { pos: [0, 1.05, 0.12] });
    b.glow('prop', geo.crystal(0.06, 0.34), c.glow, { pos: [0, 1.48, 0] });
  },
};
