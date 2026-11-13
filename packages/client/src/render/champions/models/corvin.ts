/**
 * Corvin, l'Archer des Corbeaux — archer en cape de plumes noires, masque à bec, arc recourbé, yeux verts.
 */
import { easeIn, easeOut } from '../anim/ease.ts';
import { J, setOffset, setRot, type Clip } from '../anim/pose.ts';
import { geo } from '../builder.ts';
import type { ChampionModelDef } from '../ChampionView.ts';

/** Arc tenu main gauche ; la main droite tend la corde (k = tension). */
function draw(p: Float32Array, k: number): void {
  setRot(p, J.armL, 0.2, 0, 0.3 + 1.25 * k);
  setRot(p, J.foreL, 0, 0, 0.5 - 0.45 * k);
  setRot(p, J.armR, -0.3, 0, 0.3 + 1.1 * k);
  setRot(p, J.foreR, 0, 0, 0.5 + 1.6 * k);
  setRot(p, J.spine, 0, -0.5 * k, 0);
}

const attackReady: Clip = {
  loop: true,
  duration: 1,
  sample(t, p) {
    draw(p, easeOut(t / 0.2));
  },
};
/** Décoche : la main droite lâche la corde. */
const strike: Clip = {
  loop: false,
  duration: 0.32,
  sample(t, p) {
    draw(p, 1 - easeIn((t - 0.1) / 0.22));
    setRot(p, J.foreR, 0, 0, 2.1 - 1.6 * easeOut(t / 0.05));
  },
};
const castR: Clip = {
  loop: false,
  duration: 0.8,
  sample(t, p) {
    const k = easeOut(t / 0.25) * (1 - easeIn((t - 0.55) / 0.25));
    setRot(p, J.armL, 0.4, 0, 2.5 * k);
    setRot(p, J.armR, -0.6, 0, 2.2 * k);
    setRot(p, J.spine, 0, 0, 0.3 * k);
    setRot(p, J.head, 0, 0, 0.4 * k);
    setOffset(p, 0, 0.05 * k, 0);
  },
};

export const corvinModel: ChampionModelDef = {
  rig: { hipY: 0.87, hipW: 0.13, thigh: 0.44, shin: 0.42, spine: 0.56, shoulderW: 0.25, upperArm: 0.33, forearm: 0.3, neck: 0.15 },
  ctx: { energy: 1, hand: -1 },
  clips: { attackReady, strike, cast_Q: strike, cast_R: castR },
  build(b, c) {
    const black = c.primary;
    const grey = c.secondary;
    b.add('hips', geo.box(0.24, 0.22, 0.36), black);
    b.add('hips', geo.box(0.26, 0.06, 0.38), c.team, { pos: [0, 0.08, 0] });
    b.add('spine', geo.box(0.24, 0.42, 0.36), grey, { pos: [0, 0.28, 0] });
    // cape de plumes : plusieurs pans
    for (let i = 0; i < 5; i++) {
      const z = (i - 2) * 0.1;
      b.add('spine', geo.box(0.04, 0.75 - Math.abs(i - 2) * 0.08, 0.12), black, { pos: [-0.16, 0.12, z], rot: [z * 0.8, 0, 0.12] });
    }
    for (const z of [-0.26, 0.26]) b.add('spine', geo.cone(0.13, 0.26, 5), black, { pos: [-0.02, 0.56, z], rot: [z > 0 ? -1.2 : 1.2, 0, 0] });
    // tête : capuche + masque à bec
    b.add('head', geo.ball(0.15, 1), black, { pos: [0, 0.12, 0] });
    b.add('head', geo.cone(0.07, 0.3, 4), '#1a1a22', { pos: [0.24, 0.07, 0], rot: [0, 0, -Math.PI / 2] });
    b.glow('head', geo.box(0.02, 0.03, 0.04), c.glow, { pos: [0.14, 0.14, -0.06] });
    b.glow('head', geo.box(0.02, 0.03, 0.04), c.glow, { pos: [0.14, 0.14, 0.06] });
    for (const [arm, fore] of [['armL', 'foreL'], ['armR', 'foreR']] as const) {
      b.add(arm, geo.box(0.11, 0.32, 0.11), grey, { pos: [0, -0.15, 0] });
      b.add(fore, geo.box(0.11, 0.28, 0.11), black, { pos: [0, -0.13, 0] });
    }
    for (const [leg, shin] of [['legL', 'shinL'], ['legR', 'shinR']] as const) {
      b.add(leg, geo.box(0.14, 0.44, 0.14), black, { pos: [0, -0.2, 0] });
      b.add(shin, geo.box(0.14, 0.4, 0.14), grey, { pos: [0, -0.19, 0] });
      b.add(shin, geo.box(0.22, 0.08, 0.14), '#1a1a22', { pos: [0.04, -0.39, 0] });
    }
    // arc dans la main gauche (avant-bras gauche) : branches courbes, corde lumineuse
    b.add('foreL', geo.box(0.05, 0.12, 0.05), grey, { pos: [0, -0.32, 0] });
    b.add('foreL', geo.torus(0.42, 0.025, 10), '#3a3040', { pos: [-0.25, -0.32, 0], rot: [0, 0, 0], scale: [0.7, 1, 1] });
    b.glow('foreL', geo.box(0.012, 0.75, 0.012), c.glow, { pos: [-0.08, -0.32, 0] });
  },
};
