/**
 * Grondak, le Rempart de Granit — golem de pierre, fissures de magma ambré, longs bras aux poings massifs.
 */
import { clamp01, easeOut, easeIn } from '../anim/ease.ts';
import { J, addRot, setOffset, setRot, type Clip } from '../anim/pose.ts';
import { geo } from '../builder.ts';
import type { ChampionModelDef } from '../ChampionView.ts';


/** Posture de gorille : bras pendants vers l'avant, dos voûté. */
function stance(p: Float32Array, breathe = 0): void {
  setRot(p, J.spine, 0, 0, -0.22 + breathe * 0.03);
  setRot(p, J.head, 0, 0, 0.18);
  setRot(p, J.armL, 0.18, 0, 0.25);
  setRot(p, J.armR, -0.18, 0, 0.25);
  setRot(p, J.foreL, 0, 0, 0.2);
  setRot(p, J.foreR, 0, 0, 0.2);
  setRot(p, J.legL, 0.08, 0, 0.1);
  setRot(p, J.legR, -0.08, 0, 0.1);
  setRot(p, J.shinL, 0, 0, -0.2);
  setRot(p, J.shinR, 0, 0, -0.2);
  setOffset(p, 0, -0.04 + breathe * 0.015, 0);
}

const idle: Clip = {
  loop: true,
  duration: 3.4,
  sample(t, p) {
    const b = Math.sin((t / 3.4) * Math.PI * 2);
    stance(p, b);
    addRot(p, J.armL, b * 0.04, 0, 0);
    addRot(p, J.armR, -b * 0.04, 0, 0);
    setRot(p, J.head, 0, Math.sin((t / 6.8) * Math.PI * 2) * 0.15, 0.18);
  },
};

/** Double frappe des poings (auto-attaque). */
const strike: Clip = {
  loop: false,
  duration: 0.45,
  sample(t, p) {
    stance(p);
    const hit = easeOut(t / 0.1);
    const back = easeIn((t - 0.18) / 0.27);
    const k = hit * (1 - back);
    addRot(p, J.armL, 0, 0, 1.0 * k);
    addRot(p, J.armR, 0, 0, 1.0 * k);
    addRot(p, J.spine, 0, 0, -0.25 * k);
    setOffset(p, 0.1 * k, -0.08 * k, 0);
  },
};

const attackReady: Clip = {
  loop: true,
  duration: 1,
  sample(t, p) {
    stance(p);
    const k = easeOut(t / 0.3);
    addRot(p, J.armL, 0, 0, 1.8 * k);
    addRot(p, J.armR, 0, 0, 1.8 * k);
    addRot(p, J.foreL, 0, 0, 0.6 * k);
    addRot(p, J.foreR, 0, 0, 0.6 * k);
    addRot(p, J.spine, 0, 0, 0.18 * k);
  },
};

/** Q : poing droit abattu sur le sol. */
const castQ: Clip = {
  loop: false,
  duration: 0.6,
  sample(t, p) {
    stance(p);
    const up = easeOut(t / 0.15);
    const slam = easeIn((t - 0.12) / 0.1);
    const rec = easeIn((t - 0.35) / 0.25);
    addRot(p, J.armR, 0, 0, (2.6 * up - 2.2 * slam) * (1 - rec));
    addRot(p, J.spine, 0, 0, (0.2 * up - 0.55 * slam) * (1 - rec));
    setOffset(p, 0.15 * slam * (1 - rec), -0.18 * slam * (1 - rec), 0);
  },
};

/** W : rugissement, poitrine en avant, bras écartés. */
const castW: Clip = {
  loop: false,
  duration: 0.7,
  sample(t, p) {
    stance(p);
    const k = easeOut(t / 0.15) * (1 - easeIn((t - 0.45) / 0.25));
    addRot(p, J.armL, 1.1 * k, 0, 0.3 * k);
    addRot(p, J.armR, -1.1 * k, 0, 0.3 * k);
    addRot(p, J.spine, 0, 0, 0.45 * k);
    addRot(p, J.head, 0, 0, -0.5 * k);
    setOffset(p, 0, 0.05 * k, 0);
  },
};

/** Charge : épaule en avant (dash du E). */
const dash: Clip = {
  loop: true,
  duration: 0.4,
  sample(t, p) {
    stance(p);
    const s = Math.sin((t / 0.4) * Math.PI * 2);
    setRot(p, J.body, 0, -0.35, -0.3);
    addRot(p, J.armR, -0.3, 0, -0.4);
    addRot(p, J.armL, 0.2, 0, 0.9);
    setRot(p, J.legL, 0, 0, s * 0.9);
    setRot(p, J.legR, 0, 0, -s * 0.9);
    setOffset(p, 0, Math.abs(s) * 0.05, 0);
  },
};

/** R : deux poings levés puis abattus. */
const castR: Clip = {
  loop: false,
  duration: 0.9,
  sample(t, p) {
    stance(p);
    const up = easeOut(t / 0.3);
    const slam = easeIn((t - 0.3) / 0.12);
    const rec = easeIn((t - 0.6) / 0.3);
    const raise = (3.0 * up - 2.5 * slam) * (1 - rec);
    addRot(p, J.armL, 0, 0, raise);
    addRot(p, J.armR, 0, 0, raise);
    addRot(p, J.spine, 0, 0, (0.4 * up - 0.75 * slam) * (1 - rec));
    setOffset(p, 0.1 * slam * (1 - rec), (0.12 * up - 0.3 * slam) * (1 - rec), 0);
  },
};

export const grondakModel: ChampionModelDef = {
  rig: { hipY: 0.82, hipW: 0.26, thigh: 0.38, shin: 0.42, spine: 0.78, shoulderW: 0.55, upperArm: 0.46, forearm: 0.44, neck: 0.1 },
  scale: 1.05,
  ctx: { energy: 0.7, hand: 1 },
  clips: { idle, strike, attackReady, cast_Q: castQ, cast_W: castW, cast_R: castR, dash },
  build(b, c) {
    const stone = c.primary;
    const dark = c.secondary;
    const moss = '#5d6b3f';
    // bassin + ceinture d'équipe
    b.add('hips', geo.rock(0.3, 11), dark, { scale: [0.9, 0.6, 1.25] });
    b.add('hips', geo.torus(0.33, 0.045, 9), c.team, { pos: [0, 0.08, 0], rot: [Math.PI / 2, 0, 0], scale: [0.85, 1.2, 1] });
    // torse : gros bloc + épaules rocheuses
    b.add('spine', geo.rock(0.55, 2), stone, { pos: [0, 0.5, 0], scale: [0.85, 0.95, 1.1] });
    b.add('spine', geo.rock(0.3, 3), stone, { pos: [-0.05, 0.82, -0.45] });
    b.add('spine', geo.rock(0.3, 4), stone, { pos: [-0.05, 0.82, 0.45] });
    b.add('spine', geo.rock(0.18, 5), dark, { pos: [-0.35, 0.95, 0.1] });
    b.add('spine', geo.rock(0.14, 6), moss, { pos: [-0.2, 1.02, -0.2] });
    // fissures de magma (poitrine, dos)
    b.glow('spine', geo.box(0.04, 0.42, 0.05), c.glow, { pos: [0.46, 0.48, 0.05], rot: [0.3, 0, 0] });
    b.glow('spine', geo.box(0.04, 0.22, 0.05), c.glow, { pos: [0.45, 0.62, -0.12], rot: [-0.9, 0, 0] });
    b.glow('spine', geo.box(0.04, 0.18, 0.05), c.glow, { pos: [0.44, 0.34, 0.16], rot: [0.9, 0, 0] });
    b.glow('spine', geo.ball(0.08, 0), c.glow, { pos: [0.42, 0.52, 0] });
    // tête : petit rocher enfoncé, yeux ambrés
    b.add('head', geo.rock(0.2, 7), stone, { pos: [0.12, 0.06, 0], scale: [1, 0.85, 1] });
    b.add('head', geo.box(0.12, 0.06, 0.32), dark, { pos: [0.24, 0.12, 0] }); // arcade
    b.glow('head', geo.box(0.04, 0.035, 0.07), c.glow, { pos: [0.3, 0.06, -0.08] });
    b.glow('head', geo.box(0.04, 0.035, 0.07), c.glow, { pos: [0.3, 0.06, 0.08] });
    // bras : segments rocheux, poings énormes
    for (const [arm, fore, side] of [['armL', 'foreL', -1], ['armR', 'foreR', 1]] as const) {
      b.add(arm, geo.rock(0.2, 20 + side), stone, { pos: [0, -0.22, 0], scale: [1, 1.5, 1] });
      b.add(fore, geo.rock(0.21, 30 + side), stone, { pos: [0, -0.18, 0], scale: [1, 1.4, 1] });
      b.add(fore, geo.rock(0.3, 40 + side), dark, { pos: [0.04, -0.5, 0] });
      b.glow(fore, geo.box(0.05, 0.05, 0.22), c.glow, { pos: [0.28, -0.5, 0] });
    }
    // jambes trapues
    for (const [leg, shin, side] of [['legL', 'shinL', -1], ['legR', 'shinR', 1]] as const) {
      b.add(leg, geo.rock(0.2, 50 + side), stone, { pos: [0, -0.18, 0], scale: [1, 1.3, 1] });
      b.add(shin, geo.rock(0.19, 60 + side), stone, { pos: [0, -0.2, 0], scale: [1, 1.2, 1] });
      b.add(shin, geo.slab(0.4, 0.12, 0.26, 70 + side), dark, { pos: [0.06, -0.38, 0] });
    }
  },
  extras(_rig, built) {
    // les fissures pulsent doucement (respiration du magma) : `color` multiplie les couleurs de sommets
    const mat = built.glowMaterial;
    return (_dt, s) => {
      mat?.color.setScalar(0.75 + 0.25 * Math.sin(s.time * 2.2));
    };
  },
};
