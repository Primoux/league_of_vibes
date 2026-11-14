/** 67 Master : streetwear arcanique et balancier des deux paumes, même pendant la course. */
import { J, setRot, setOffset, type Clip, type Pose } from '../anim/pose.ts';
import { geo } from '../builder.ts';
import type { ChampionModelDef } from '../ChampionView.ts';
import { Color } from 'three';

/** Export pur pour vérifier l'alternance des mains sans WebGL. */
export function sampleSixSeven(t: number, p: Pose, energy = 1): void {
  const wave = Math.sin(t / 1.34 * Math.PI * 2) * energy;
  setRot(p, J.armL, .18, 0, .12 + wave * .18);
  setRot(p, J.armR, -.18, 0, .12 - wave * .18);
  setRot(p, J.foreL, 0, 0, Math.PI / 2 + wave * .32);
  setRot(p, J.foreR, 0, 0, Math.PI / 2 - wave * .32);
  setRot(p, J.head, 0, wave * .06, -.04);
}

const idle: Clip = { loop: true, duration: 1.34, sample(t, p) {
  sampleSixSeven(t, p);
  setOffset(p, 0, Math.sin(t * Math.PI * 2 / 1.34) * .018, 0);
} };
const run: Clip = { loop: true, duration: .67, sample(t, p) {
  sampleSixSeven(t, p, 1.15);
  const s = Math.sin(t * Math.PI * 2 / .67);
  setRot(p, J.legL, 0, 0, s * .7); setRot(p, J.legR, 0, 0, -s * .7);
  setRot(p, J.shinL, 0, 0, -.25 - Math.max(0, -s) * .7);
  setRot(p, J.shinR, 0, 0, -.25 - Math.max(0, s) * .7);
  setRot(p, J.spine, 0, s * .05, -.1);
  setOffset(p, 0, Math.abs(s) * .045, 0);
} };
const cast: Clip = { loop: false, duration: .67, sample(t, p) {
  sampleSixSeven(t, p, 1.6);
  setRot(p, J.spine, 0, 0, -.12 * Math.sin(Math.PI * t / .67));
} };
const castR: Clip = { loop: false, duration: 1.67, sample(t, p) {
  sampleSixSeven(t, p, 1.7);
  setOffset(p, 0, Math.sin(Math.min(1, t / 1.67) * Math.PI) * .12, 0);
} };
const recall: Clip = { loop: true, duration: 1.34, sample(t, p) {
  sampleSixSeven(t, p, 1.3); setOffset(p, 0, .12 + Math.sin(t * 3) * .07, 0);
} };

export const master67Model: ChampionModelDef = {
  rig: { hipY: .87, hipW: .15, thigh: .43, shin: .42, spine: .56, shoulderW: .29, upperArm: .29, forearm: .29, neck: .16 },
  ctx: { energy: 1, hand: 1 },
  clips: { idle, run, dash: run, attackReady: idle, strike: cast, cast, cast_Q: cast, cast_W: cast, cast_E: cast, cast_R: castR, recall },
  build(b, c) {
    const skin = '#b97f5b', hair = '#241b29', shoes = '#eef7ff';
    const violetGlow = new Color('#e4a7ff').multiplyScalar(1.5);
    b.add('hips', geo.box(.28, .25, .42), c.secondary);
    b.add('hips', geo.box(.3, .055, .44), c.team, { pos: [0, .1, 0] });
    b.add('spine', geo.box(.36, .5, .58), c.primary, { pos: [0, .28, 0] });
    b.add('head', geo.ball(.235, 1), c.primary, { pos: [-.075, .14, 0], scale: [.7, 1.13, 1.1] });
    for (const z of [-.21, .21]) b.add('head', geo.box(.07, .33, .035), c.accent, { pos: [.015, .14, z], rot: [0, 0, -.16] });
    b.add('spine', geo.box(.025, .12, .25), c.secondary, { pos: [.19, .13, 0] }); // poche kangourou
    for (const z of [-.09, .09]) b.add('spine', geo.cyl(.008, .008, .22), c.accent, { pos: [.19, .4, z] });
    b.add('head', geo.ball(.18, 1), skin, { pos: [.03, .1, 0], scale: [1, 1.1, .95] });
    for (let i = 0; i < 9; i++) {
      const a = i * 2.399;
      b.add('head', geo.ball(.075, 0), hair, { pos: [-.015 + Math.cos(a) * .1, .25 + (i % 3) * .022, Math.sin(a) * .115] });
    }
    for (const z of [-.068, .068]) {
      b.add('head', geo.box(.025, .035, .043), '#142137', { pos: [.195, .12, z] });
      b.add('head', geo.box(.025, .012, .06), hair, { pos: [.181, .175, z], rot: [0, 0, -.1] });
    }
    b.add('head', geo.box(.025, .035, .135), '#fff7df', { pos: [.19, .035, 0], rot: [0, 0, -.1] }); // grand sourire
    b.add('head', geo.box(.035, .055, .07), hair, { pos: [.14, -.055, 0] }); // bouc
    b.add('head', geo.torus(.035, .008, 8), '#ecc57b', { pos: [.01, .02, .18], rot: [0, Math.PI / 2, 0] });
    for (const [arm, fore] of [['armL', 'foreL'], ['armR', 'foreR']] as const) {
      b.add(arm, geo.cyl(.14, .12, .29, 8), c.primary, { pos: [0, -.13, 0] });
      b.add(fore, geo.cyl(.11, .095, .27, 8), c.primary, { pos: [0, -.13, 0] });
      b.add(fore, geo.cyl(.1, .1, .055, 8), c.accent, { pos: [0, -.27, 0] });
      // Après rotation du coude, +x local pointe vers le ciel : paumes réellement vers le haut.
      b.add(fore, geo.box(.065, .17, .23), skin, { pos: [.02, -.36, 0] });
      for (let finger = 0; finger < 4; finger++) b.add(fore, geo.box(.055, .12 - Math.abs(finger - 1.5) * .016, .045), skin, { pos: [.02, -.48, (finger - 1.5) * .055], rot: [0, 0, -.12] });
      b.add(fore, geo.box(.065, .12, .055), skin, { pos: [.035, -.35, fore === 'foreL' ? -.16 : .16], rot: [fore === 'foreL' ? -.5 : .5, 0, 0] });
    }
    // Chiffres 6/7 en segments lumineux, intégrés au même SkinnedMesh émissif.
    const segments = [[0, .12, true], [.07, .06, false], [.07, -.06, false], [0, -.12, true], [-.07, -.06, false], [-.07, .06, false], [0, 0, true]] as const;
    for (const [digit, centerZ] of [[6, .105], [7, -.105]] as const) {
      const active = digit === 6 ? [0, 2, 3, 4, 5, 6] : [0, 1, 2];
      for (const i of active) {
        const [z, y, horizontal] = segments[i]!;
        b.glow('spine', geo.box(.018, horizontal ? .018 : .1, horizontal ? .13 : .018), digit === 6 ? c.glow : violetGlow, { pos: [.19, .33 + y * .65, centerZ - z * .65], scale: .65 });
      }
    }
    for (const [leg, shin] of [['legL', 'shinL'], ['legR', 'shinR']] as const) {
      b.add(leg, geo.box(.18, .42, .18), c.secondary, { pos: [0, -.2, 0] });
      b.add(shin, geo.box(.16, .38, .16), c.secondary, { pos: [0, -.18, 0] });
      b.add(shin, geo.box(.31, .12, .2), shoes, { pos: [.065, -.37, 0] });
      b.glow(shin, geo.box(.29, .025, .205), c.glow, { pos: [.065, -.415, 0] });
    }
  },
};
