/**
 * Ilyra, la Sentinelle du Givre — mage élancée en robe, couronne d'éclats de glace, bâton surmonté d'un cristal,
 * trois éclats en orbite autour d'elle.
 */
import * as THREE from 'three';
import { recall, stunned } from '../anim/clips.ts';
import { J, addRot, setOffset, setRot, type Clip } from '../anim/pose.ts';
import { geo } from '../builder.ts';
import type { ChampionModelDef } from '../ChampionView.ts';
import { createIceMaterial } from '../../../fx/shaders/materials.ts';

const clamp01 = (t: number): number => (t < 0 ? 0 : t > 1 ? 1 : t);
const easeOut = (t: number): number => 1 - (1 - clamp01(t)) ** 3;
const easeIn = (t: number): number => clamp01(t) ** 2;
const bell = (t: number, a: number, b: number): number => Math.sin(clamp01((t - a) / (b - a)) * Math.PI);

/** Bâton tenu verticalement dans la main droite, main gauche ouverte. */
function stance(p: Float32Array): void {
  setRot(p, J.armR, -0.15, 0, 0.35);
  setRot(p, J.foreR, 0, 0, 0.9);
  setRot(p, J.prop, 0, 0, -1.25); // garde le bâton vertical malgré le coude plié
  setRot(p, J.armL, 0.2, 0, 0.1);
  setRot(p, J.foreL, 0, 0, 0.35);
}

const idle: Clip = {
  loop: true,
  duration: 3,
  sample(t, p) {
    stance(p);
    const b = Math.sin((t / 3) * Math.PI * 2);
    setRot(p, J.spine, 0, 0, b * 0.02);
    addRot(p, J.armL, b * 0.05, 0, b * 0.04);
    setRot(p, J.head, 0, Math.sin((t / 6) * Math.PI * 2) * 0.15, -0.05);
    setOffset(p, 0, b * 0.015, 0);
  },
};

/** Auto-attaque : le bâton pointe la cible. */
const strike: Clip = {
  loop: false,
  duration: 0.4,
  sample(t, p) {
    stance(p);
    const k = easeOut(t / 0.08) * (1 - easeIn((t - 0.15) / 0.25));
    addRot(p, J.armR, 0, 0, 0.9 * k);
    addRot(p, J.foreR, 0, 0, -0.5 * k);
    addRot(p, J.prop, 0, 0, -0.5 * k);
    setRot(p, J.spine, 0, 0.3 * k, -0.08 * k);
  },
};

const attackReady: Clip = {
  loop: true,
  duration: 1,
  sample(t, p) {
    stance(p);
    const k = easeOut(t / 0.25);
    addRot(p, J.armR, 0, 0, -0.25 * k);
    addRot(p, J.prop, 0, 0, 0.3 * k);
    setRot(p, J.spine, 0, -0.25 * k, 0.05 * k);
  },
};

/** Q : main gauche projette l'éclat. */
const castQ: Clip = {
  loop: false,
  duration: 0.5,
  sample(t, p) {
    stance(p);
    const wind = easeOut(t / 0.15);
    const throwK = easeOut((t - 0.15) / 0.1);
    const rec = easeIn((t - 0.3) / 0.2);
    addRot(p, J.armL, 0.2 * wind, 0, (-0.6 * wind + 2.0 * throwK) * (1 - rec));
    addRot(p, J.foreL, 0, 0, (1.2 * wind - 1.1 * throwK) * (1 - rec));
    setRot(p, J.spine, 0, (-0.4 * wind + 0.7 * throwK) * (1 - rec), 0);
  },
};

/** W / E : bâton levé, main tendue. */
const cast: Clip = {
  loop: false,
  duration: 0.5,
  sample(t, p) {
    stance(p);
    const k = bell(t, 0, 0.5);
    addRot(p, J.armR, 0, 0, 0.8 * k);
    addRot(p, J.armL, 0.3 * k, 0, 1.4 * k);
    addRot(p, J.foreL, 0, 0, -0.3 * k);
    setRot(p, J.head, 0, 0, 0.1 * k);
  },
};

/** R : bâton brandi au ciel, lévitation. */
const castR: Clip = {
  loop: false,
  duration: 1,
  sample(t, p) {
    stance(p);
    const k = easeOut(t / 0.3) * (1 - easeIn((t - 0.7) / 0.3));
    addRot(p, J.armR, -0.2 * k, 0, 2.4 * k);
    addRot(p, J.foreR, 0, 0, -0.8 * k);
    addRot(p, J.prop, 0, 0, 1.0 * k);
    addRot(p, J.armL, 0.9 * k, 0, 0.4 * k);
    setRot(p, J.spine, 0, 0, 0.2 * k);
    setRot(p, J.head, 0, 0, 0.3 * k);
    setOffset(p, 0, 0.25 * k, 0);
  },
};

/**
 * Garde le bâton vertical : sa direction dépend de la somme des rotations z de la chaîne
 * corps → torse → bras → avant-bras → bâton ; on compense sur le bâton.
 */
function uprightStaff(clip: Clip): Clip {
  return {
    ...clip,
    sample(t, p, ctx) {
      clip.sample(t, p, ctx);
      p[J.prop * 3 + 2] = -(p[J.body * 3 + 2]! + p[J.spine * 3 + 2]! + p[J.armR * 3 + 2]! + p[J.foreR * 3 + 2]!);
    },
  };
}

export const ilyraModel: ChampionModelDef = {
  rig: { hipY: 0.9, hipW: 0.11, thigh: 0.45, shin: 0.43, spine: 0.55, shoulderW: 0.23, upperArm: 0.3, forearm: 0.29, neck: 0.16 },
  ctx: { energy: 0.85, hand: 1 },
  clips: { idle, strike, attackReady, cast, cast_Q: castQ, cast_R: castR, recall: uprightStaff(recall), stunned: uprightStaff(stunned) },
  build(b, c) {
    const skin = '#e8eef8';
    const ice = c.primary;
    const deep = c.secondary;
    // robe (cône depuis la hanche) + liseré + ceinture d'équipe
    b.add('hips', geo.cone(0.44, 0.95, 8), deep, { pos: [0, -0.42, 0] });
    b.add('hips', geo.torus(0.4, 0.03, 12), ice, { pos: [0, -0.84, 0], rot: [Math.PI / 2, 0, 0] });
    b.add('hips', geo.box(0.2, 0.07, 0.3), c.team, { pos: [0, 0.04, 0] });
    // torse fin
    b.add('spine', geo.box(0.22, 0.42, 0.32), ice, { pos: [0, 0.24, 0] });
    b.add('spine', geo.box(0.24, 0.12, 0.34), deep, { pos: [0, 0.06, 0] });
    b.glow('spine', geo.crystal(0.05, 0.14), c.glow, { pos: [0.12, 0.32, 0] });
    // col d'éclats
    for (const z of [-0.2, -0.1, 0.1, 0.2]) b.add('spine', geo.crystal(0.05, 0.28 - Math.abs(z) * 0.4), ice, { pos: [-0.04, 0.52, z], rot: [z * 2, 0, 0] });
    // tête, cheveux longs, couronne de glace
    b.add('head', geo.ball(0.14, 1), skin, { pos: [0.02, 0.1, 0] });
    b.add('head', geo.box(0.16, 0.42, 0.26), c.accent, { pos: [-0.1, -0.02, 0] });
    b.add('head', geo.ball(0.15, 1), c.accent, { pos: [-0.03, 0.15, 0], scale: [1, 0.8, 1.02] });
    for (let i = -2; i <= 2; i++) b.glow('head', geo.crystal(0.03, 0.2 - Math.abs(i) * 0.04), c.glow, { pos: [0, 0.3, i * 0.06], rot: [i * 0.25, 0, 0] });
    b.glow('head', geo.box(0.02, 0.025, 0.04), new THREE.Color(0.8, 1.7, 2.3), { pos: [0.15, 0.11, -0.05] });
    b.glow('head', geo.box(0.02, 0.025, 0.04), new THREE.Color(0.8, 1.7, 2.3), { pos: [0.15, 0.11, 0.05] });
    // bras fins, manches évasées
    for (const [arm, fore] of [['armL', 'foreL'], ['armR', 'foreR']] as const) {
      b.add(arm, geo.box(0.1, 0.3, 0.1), ice, { pos: [0, -0.14, 0] });
      b.add(fore, geo.cone(0.1, 0.28, 6), deep, { pos: [0, -0.13, 0], rot: [Math.PI, 0, 0] });
      b.add(fore, geo.ball(0.055, 0), skin, { pos: [0, -0.29, 0] });
    }
    // pieds (le reste des jambes est sous la robe)
    for (const shin of ['shinL', 'shinR'] as const) b.add(shin, geo.box(0.2, 0.07, 0.1), deep, { pos: [0.05, -0.4, 0] });
    for (const leg of ['legL', 'legR'] as const) b.add(leg, geo.box(0.1, 0.4, 0.1), deep, { pos: [0, -0.2, 0] });
    // bâton : hampe sombre, croissant et cristal lumineux
    b.add('prop', geo.cyl(0.022, 0.028, 1.55), deep, { pos: [0, 0.22, 0] });
    b.add('prop', geo.torus(0.1, 0.018, 10), ice, { pos: [0, 0.98, 0] });
    b.glow('prop', geo.crystal(0.07, 0.3), c.glow, { pos: [0, 1.06, 0] });
  },
  extras(rig) {
    // trois éclats de glace en orbite autour de la taille
    const mat = createIceMaterial();
    const shards: THREE.Mesh[] = [];
    const g = new THREE.OctahedronGeometry(0.07, 0);
    g.scale(1, 2.2, 1);
    for (let i = 0; i < 3; i++) {
      const m = new THREE.Mesh(g, mat);
      m.userData.noDissolve = true;
      rig.joint('body').add(m);
      shards.push(m);
    }
    return (_dt, s) => {
      for (let i = 0; i < 3; i++) {
        const a = s.time * 1.4 + (i / 3) * Math.PI * 2;
        const m = shards[i]!;
        m.visible = !s.dead;
        m.position.set(Math.cos(a) * 0.55, 1.05 + Math.sin(s.time * 2 + i) * 0.08, Math.sin(a) * 0.55);
        m.rotation.set(0, -a, 0.3);
      }
    };
  },
};
