/**
 * Sable, la Tireuse des Dunes — pistolera du désert : chapeau à large bord, cache-poussière, long fusil à canon cuivré.
 */
import { easeIn, easeOut } from '../anim/ease.ts';
import { J, setOffset, setRot, type Clip } from '../anim/pose.ts';
import { geo } from '../builder.ts';
import type { ChampionModelDef } from '../ChampionView.ts';

/** Fusil épaulé : les deux bras vers l'avant. */
function aim(p: Float32Array, k: number): void {
  setRot(p, J.armR, -0.15, 0, 0.3 + 1.1 * k);
  setRot(p, J.foreR, 0, 0, 0.6 - 0.5 * k);
  setRot(p, J.prop, 0, 0, -0.9 + 0.75 * k);
  setRot(p, J.armL, 0.5, 0, 0.3 + 1.2 * k);
  setRot(p, J.foreL, 0, 0, 0.9 - 0.4 * k);
}

const idle: Clip = {
  loop: true,
  duration: 2.6,
  sample(t, p) {
    aim(p, 0);
    const b = Math.sin((t / 2.6) * Math.PI * 2);
    setRot(p, J.spine, 0, 0, b * 0.02);
    setRot(p, J.head, 0, Math.sin((t / 5.2) * Math.PI * 2) * 0.2, 0);
    setOffset(p, 0, b * 0.015, 0);
  },
};
const attackReady: Clip = {
  loop: true,
  duration: 1,
  sample(t, p) {
    aim(p, easeOut(t / 0.15));
    setRot(p, J.spine, 0, -0.2, 0);
  },
};
/** Tir : recul du fusil. */
const strike: Clip = {
  loop: false,
  duration: 0.3,
  sample(t, p) {
    aim(p, 1);
    const kick = easeOut(t / 0.04) * (1 - easeIn((t - 0.05) / 0.2));
    setRot(p, J.armR, -0.15, 0, 1.4 + 0.35 * kick);
    setRot(p, J.spine, 0, -0.2, 0.12 * kick);
    setOffset(p, -0.06 * kick, 0, 0);
  },
};
/** R : visée longue, un genou à terre. */
const castR: Clip = {
  loop: false,
  duration: 1.25,
  sample(t, p) {
    const k = easeOut(t / 0.25);
    aim(p, k);
    setRot(p, J.legL, 0, 0, 1.0 * k);
    setRot(p, J.shinL, 0, 0, -1.4 * k);
    setRot(p, J.legR, 0, 0, -0.5 * k);
    setRot(p, J.shinR, 0, 0, -1.5 * k);
    const kick = t > 1 ? easeOut((t - 1) / 0.05) * (1 - easeIn((t - 1.05) / 0.2)) : 0;
    setRot(p, J.spine, 0, -0.2, 0.2 * kick);
    setOffset(p, -0.1 * kick, -0.38 * k, 0);
  },
};

export const sableModel: ChampionModelDef = {
  rig: { hipY: 0.86, hipW: 0.13, thigh: 0.43, shin: 0.42, spine: 0.55, shoulderW: 0.24, upperArm: 0.32, forearm: 0.3, neck: 0.15 },
  ctx: { energy: 1, hand: 1 },
  clips: { idle, attackReady, strike, cast_R: castR },
  build(b, c) {
    const sand = c.primary;
    const brown = c.secondary;
    const skin = '#b98563';
    b.add('hips', geo.box(0.24, 0.22, 0.36), brown);
    b.add('hips', geo.box(0.26, 0.06, 0.38), c.team, { pos: [0, 0.08, 0] });
    b.add('hips', geo.box(0.08, 0.5, 0.42), sand, { pos: [-0.12, -0.2, 0] }); // pan du cache-poussière
    b.add('spine', geo.box(0.26, 0.42, 0.38), sand, { pos: [0, 0.28, 0] });
    b.add('spine', geo.box(0.04, 0.38, 0.18), brown, { pos: [0.14, 0.28, 0] });
    b.add('spine', geo.box(0.06, 0.06, 0.4), '#7a4a26', { pos: [0.1, 0.18, 0], rot: [0.6, 0, 0] }); // cartouchière
    b.add('head', geo.ball(0.13, 1), skin, { pos: [0.03, 0.08, 0] });
    b.add('head', geo.cyl(0.32, 0.32, 0.03, 10), brown, { pos: [0, 0.2, 0] }); // bord du chapeau
    b.add('head', geo.cyl(0.14, 0.16, 0.16, 8), brown, { pos: [-0.01, 0.29, 0] });
    b.add('head', geo.box(0.04, 0.05, 0.3), '#3a2414', { pos: [0, 0.22, 0] });
    b.add('head', geo.box(0.06, 0.08, 0.24), sand, { pos: [0.13, 0.02, 0] }); // foulard sur le visage
    b.glow('head', geo.box(0.02, 0.022, 0.04), c.glow, { pos: [0.14, 0.11, -0.05] });
    b.glow('head', geo.box(0.02, 0.022, 0.04), c.glow, { pos: [0.14, 0.11, 0.05] });
    for (const [arm, fore] of [['armL', 'foreL'], ['armR', 'foreR']] as const) {
      b.add(arm, geo.box(0.11, 0.32, 0.11), sand, { pos: [0, -0.15, 0] });
      b.add(fore, geo.box(0.11, 0.28, 0.11), brown, { pos: [0, -0.13, 0] });
      b.add(fore, geo.ball(0.055, 0), skin, { pos: [0, -0.29, 0] });
    }
    for (const [leg, shin] of [['legL', 'shinL'], ['legR', 'shinR']] as const) {
      b.add(leg, geo.box(0.14, 0.43, 0.14), brown, { pos: [0, -0.2, 0] });
      b.add(shin, geo.box(0.15, 0.4, 0.15), '#5a3a22', { pos: [0, -0.19, 0] });
      b.add(shin, geo.box(0.24, 0.08, 0.15), '#3a2414', { pos: [0.04, -0.39, 0] });
    }
    // long fusil le long de +y local (pointé vers l'avant quand le bras est levé)
    b.add('prop', geo.box(0.07, 0.42, 0.07), brown, { pos: [0, 0.02, 0] }); // crosse
    b.add('prop', geo.cyl(0.035, 0.035, 0.85, 6), '#b87333', { pos: [0, 0.62, 0] }); // canon cuivré
    b.add('prop', geo.box(0.05, 0.12, 0.06), '#2e2e2e', { pos: [0, 0.3, 0.05] });
    b.glow('prop', geo.cyl(0.03, 0.03, 0.04, 6), c.glow, { pos: [0, 1.05, 0] });
  },
};
