/**
 * Kaelen, la Lame de l'Aube — chevalier solaire : armure dorée, tabard bleu nuit, plumet, grande épée lumineuse.
 */
import { clamp01, easeOut, easeIn } from '../anim/ease.ts';
import { J, setOffset, setRot, type Clip } from '../anim/pose.ts';
import { geo } from '../builder.ts';
import type { ChampionModelDef } from '../ChampionView.ts';


/** Taille horizontale (auto-attaque) : l'épée balaie devant lui. */
const strike: Clip = {
  loop: false,
  duration: 0.38,
  sample(t, p) {
    const k = easeOut(t / 0.1) * (1 - easeIn((t - 0.16) / 0.22));
    setRot(p, J.armR, -1.2 + 0.4 * k, 0, 0.9 + 0.4 * k);
    setRot(p, J.foreR, 0, 0, 0.2);
    setRot(p, J.spine, 0, 0.6 - 1.4 * k, -0.1);
    setRot(p, J.armL, 0.3, 0, 0.2);
    setRot(p, J.legL, 0, 0, 0.3);
    setRot(p, J.legR, 0, 0, -0.25);
    setOffset(p, 0.08 * k, -0.03, 0);
  },
};

/** R : épée levée à deux mains puis abattue. */
const castR: Clip = {
  loop: false,
  duration: 0.75,
  sample(t, p) {
    const up = easeOut(t / 0.3);
    const down = easeIn((t - 0.32) / 0.1);
    const rec = easeIn((t - 0.55) / 0.2);
    const raise = (2.9 * up - 2.2 * down) * (1 - rec);
    setRot(p, J.armR, -0.3, 0, raise);
    setRot(p, J.armL, 0.3, 0, raise);
    setRot(p, J.foreR, 0, 0, 0.3 * up);
    setRot(p, J.foreL, 0, 0, 0.3 * up);
    setRot(p, J.spine, 0, 0, (0.25 * up - 0.5 * down) * (1 - rec));
    setOffset(p, 0.1 * down * (1 - rec), (0.08 * up - 0.15 * down) * (1 - rec), 0);
  },
};

/** W : parade, épée en travers. */
const castW: Clip = {
  loop: false,
  duration: 1.2,
  sample(t, p) {
    const k = easeOut(t / 0.12) * (1 - easeIn((t - 1.0) / 0.2));
    setRot(p, J.armR, -0.6 * k, 0, 1.3 * k);
    setRot(p, J.foreR, 0, 0, 1.2 * k);
    setRot(p, J.prop, 0, 0, -0.9 * k);
    setRot(p, J.armL, 0.4 * k, 0, 1.0 * k);
    setRot(p, J.foreL, 0, 0, 1.3 * k);
    setRot(p, J.legL, 0, 0, 0.35 * k);
    setRot(p, J.legR, 0, 0, -0.3 * k);
    setOffset(p, 0, -0.08 * k, 0);
  },
};

export const kaelenModel: ChampionModelDef = {
  rig: { hipY: 0.88, hipW: 0.15, thigh: 0.44, shin: 0.43, spine: 0.6, shoulderW: 0.3, upperArm: 0.34, forearm: 0.31, neck: 0.15 },
  ctx: { energy: 1.05, hand: 1 },
  clips: { strike, cast_R: castR, cast_W: castW },
  build(b, c) {
    const gold = c.primary;
    const navy = c.secondary;
    const steel = '#c9ced8';
    b.add('hips', geo.box(0.26, 0.24, 0.42), steel);
    b.add('hips', geo.box(0.06, 0.5, 0.3), navy, { pos: [0.14, -0.2, 0] }); // tabard
    b.add('hips', geo.box(0.28, 0.06, 0.44), c.team, { pos: [0, 0.1, 0] });
    b.add('spine', geo.box(0.32, 0.46, 0.48), gold, { pos: [0, 0.32, 0] });
    b.add('spine', geo.box(0.06, 0.4, 0.28), navy, { pos: [0.17, 0.3, 0] });
    b.glow('spine', geo.crystal(0.06, 0.16), c.glow, { pos: [0.21, 0.4, 0] }); // soleil sur le tabard
    for (const z of [-0.3, 0.3]) b.add('spine', geo.ball(0.15, 0), gold, { pos: [0, 0.56, z], scale: [1, 0.7, 1.1] });
    // heaume + plumet
    b.add('head', geo.box(0.28, 0.3, 0.28), steel, { pos: [0.02, 0.12, 0] });
    b.add('head', geo.box(0.04, 0.05, 0.22), '#1a1f2b', { pos: [0.16, 0.13, 0] }); // visière
    b.add('head', geo.box(0.32, 0.06, 0.32), gold, { pos: [0.02, 0.27, 0] });
    b.add('head', geo.box(0.34, 0.12, 0.06), navy, { pos: [-0.1, 0.34, 0], rot: [0, 0, 0.5] });
    for (const [arm, fore] of [['armL', 'foreL'], ['armR', 'foreR']] as const) {
      b.add(arm, geo.box(0.13, 0.34, 0.13), steel, { pos: [0, -0.16, 0] });
      b.add(fore, geo.box(0.13, 0.3, 0.13), gold, { pos: [0, -0.14, 0] });
      b.add(fore, geo.box(0.09, 0.09, 0.09), steel, { pos: [0, -0.31, 0] });
    }
    for (const [leg, shin] of [['legL', 'shinL'], ['legR', 'shinR']] as const) {
      b.add(leg, geo.box(0.16, 0.44, 0.16), navy, { pos: [0, -0.2, 0] });
      b.add(shin, geo.box(0.15, 0.4, 0.15), steel, { pos: [0, -0.19, 0] });
      b.add(shin, geo.box(0.25, 0.08, 0.16), gold, { pos: [0.04, -0.39, 0] });
    }
    // grande épée : garde dorée, lame claire, fil lumineux
    b.add('prop', geo.box(0.05, 0.06, 0.3), gold, { pos: [0.04, 0, 0] });
    b.add('prop', geo.box(1.05, 0.08, 0.025), '#e8ecf4', { pos: [0.6, 0, 0] });
    b.glow('prop', geo.box(0.95, 0.02, 0.03), c.glow, { pos: [0.6, 0.045, 0] });
  },
};

