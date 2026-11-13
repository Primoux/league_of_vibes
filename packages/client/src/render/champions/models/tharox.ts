/**
 * Tharox, la Charge Sauvage — colosse à tête de sanglier, défenses d'ivoire, mantelet de fourrure, braises.
 */
import { clamp01, easeOut } from '../anim/ease.ts';
import { J, setOffset, setRot, type Clip } from '../anim/pose.ts';
import { geo } from '../builder.ts';
import type { ChampionModelDef } from '../ChampionView.ts';


/** Charge tête baissée (dash Q/R). */
const dash: Clip = {
  loop: true,
  duration: 0.36,
  sample(t, p) {
    const s = Math.sin((t / 0.36) * Math.PI * 2);
    setRot(p, J.body, 0, 0, -0.55);
    setRot(p, J.head, 0, 0, -0.3);
    setRot(p, J.armL, 0.3, 0, -0.6 + s * 0.4);
    setRot(p, J.armR, -0.3, 0, -0.6 - s * 0.4);
    setRot(p, J.legL, 0, 0, s * 1.0);
    setRot(p, J.legR, 0, 0, -s * 1.0);
    setRot(p, J.shinL, 0, 0, -0.4);
    setRot(p, J.shinR, 0, 0, -0.4);
    setOffset(p, 0, Math.abs(s) * 0.08 - 0.05, 0);
  },
};

/** E : piétinement, une jambe levée puis abattue. */
const castE: Clip = {
  loop: false,
  duration: 0.55,
  sample(t, p) {
    const up = easeOut(t / 0.2) * (1 - clamp01((t - 0.2) / 0.08));
    setRot(p, J.legR, 0, 0, 1.2 * up);
    setRot(p, J.shinR, 0, 0, -1.0 * up);
    setRot(p, J.armL, 0.6, 0, 0.4);
    setRot(p, J.armR, -0.6, 0, 0.4);
    setRot(p, J.spine, 0, 0, 0.2 * up - 0.15);
    setOffset(p, 0, 0.1 * up - 0.06 * (1 - up), 0);
  },
};

export const tharoxModel: ChampionModelDef = {
  rig: { hipY: 0.8, hipW: 0.22, thigh: 0.38, shin: 0.4, spine: 0.7, shoulderW: 0.46, upperArm: 0.4, forearm: 0.38, neck: 0.12 },
  scale: 1.08,
  ctx: { energy: 0.8, hand: 1 },
  clips: { dash, cast_E: castE },
  build(b, c) {
    const fur = c.primary;
    const tan = c.secondary;
    const ivory = '#efe3c8';
    b.add('hips', geo.box(0.36, 0.26, 0.5), fur, { pos: [0, 0, 0] });
    b.add('hips', geo.box(0.38, 0.07, 0.54), c.team, { pos: [0, 0.1, 0] });
    // torse massif + bosse de fourrure
    b.add('spine', geo.box(0.5, 0.56, 0.72), fur, { pos: [0, 0.34, 0] });
    b.add('spine', geo.rock(0.38, 4), tan, { pos: [-0.12, 0.66, 0], scale: [1, 0.7, 1.3] });
    b.add('spine', geo.box(0.06, 0.34, 0.42), tan, { pos: [0.25, 0.32, 0] });
    for (const z of [-0.42, 0.42]) b.add('spine', geo.cone(0.12, 0.3, 5), ivory, { pos: [0, 0.75, z], rot: [z > 0 ? -0.4 : 0.4, 0, 0] });
    // tête de sanglier : groin, défenses, yeux de braise
    b.add('head', geo.box(0.38, 0.3, 0.34), fur, { pos: [0.08, 0.1, 0] });
    b.add('head', geo.box(0.22, 0.18, 0.24), tan, { pos: [0.3, 0.02, 0] });
    for (const z of [-0.11, 0.11]) b.add('head', geo.cone(0.045, 0.32, 5), ivory, { pos: [0.36, 0.08, z], rot: [0, 0, -0.6] });
    for (const z of [-0.12, 0.12]) b.add('head', geo.cone(0.06, 0.2, 4), fur, { pos: [-0.02, 0.3, z], rot: [z > 0 ? -0.5 : 0.5, 0, 0] });
    b.glow('head', geo.box(0.03, 0.03, 0.05), c.glow, { pos: [0.26, 0.16, -0.1] });
    b.glow('head', geo.box(0.03, 0.03, 0.05), c.glow, { pos: [0.26, 0.16, 0.1] });
    for (const [arm, fore] of [['armL', 'foreL'], ['armR', 'foreR']] as const) {
      b.add(arm, geo.box(0.2, 0.42, 0.2), fur, { pos: [0, -0.18, 0] });
      b.add(fore, geo.box(0.2, 0.36, 0.2), tan, { pos: [0, -0.16, 0] });
      b.add(fore, geo.box(0.22, 0.14, 0.22), fur, { pos: [0, -0.4, 0] });
    }
    for (const [leg, shin] of [['legL', 'shinL'], ['legR', 'shinR']] as const) {
      b.add(leg, geo.box(0.22, 0.4, 0.22), fur, { pos: [0, -0.18, 0] });
      b.add(shin, geo.box(0.18, 0.36, 0.18), tan, { pos: [0, -0.18, 0] });
      b.add(shin, geo.box(0.28, 0.1, 0.2), '#2b1d14', { pos: [0.05, -0.38, 0] });
    }
  },
};
