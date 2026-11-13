/**
 * Zephyr, l'Arcaniste Instable — mage en longue cape violette, capuche, gantelets cyan, éclats arcaniques
 * qui gravitent autour de lui (de plus en plus vite quand il incante).
 */
import * as THREE from 'three';
import { easeIn, easeOut } from '../anim/ease.ts';
import { J, setOffset, setRot, type Clip } from '../anim/pose.ts';
import { geo } from '../builder.ts';
import type { ChampionModelDef } from '../ChampionView.ts';

/** Lévitation légère, mains ouvertes. */
const idle: Clip = {
  loop: true,
  duration: 2.2,
  sample(t, p) {
    const b = Math.sin((t / 2.2) * Math.PI * 2);
    setRot(p, J.armL, 0.4, 0, 0.35 + b * 0.05);
    setRot(p, J.armR, -0.4, 0, 0.35 - b * 0.05);
    setRot(p, J.foreL, 0, 0, 0.7);
    setRot(p, J.foreR, 0, 0, 0.7);
    setRot(p, J.legL, 0, 0, 0.1);
    setRot(p, J.shinL, 0, 0, -0.25);
    setOffset(p, 0, 0.12 + b * 0.04, 0);
  },
};

/** Incantation : deux mains projetées vers l'avant. */
const cast: Clip = {
  loop: false,
  duration: 0.45,
  sample(t, p) {
    const k = easeOut(t / 0.1) * (1 - easeIn((t - 0.25) / 0.2));
    setRot(p, J.armL, 0.3, 0, 0.35 + 1.2 * k);
    setRot(p, J.armR, -0.3, 0, 0.35 + 1.2 * k);
    setRot(p, J.foreL, 0, 0, 0.7 - 0.6 * k);
    setRot(p, J.foreR, 0, 0, 0.7 - 0.6 * k);
    setRot(p, J.spine, 0, 0, -0.15 * k);
    setOffset(p, -0.06 * k, 0.12, 0);
  },
};

/** R : bras levés, le corps s'élève. */
const castR: Clip = {
  loop: false,
  duration: 0.9,
  sample(t, p) {
    const k = easeOut(t / 0.35) * (1 - easeIn((t - 0.65) / 0.25));
    setRot(p, J.armL, 0.6, 0, 2.6 * k);
    setRot(p, J.armR, -0.6, 0, 2.6 * k);
    setRot(p, J.spine, 0, 0, 0.3 * k);
    setRot(p, J.head, 0, 0, 0.35 * k);
    setOffset(p, 0, 0.12 + 0.35 * k, 0);
  },
};

export const zephyrModel: ChampionModelDef = {
  rig: { hipY: 0.88, hipW: 0.12, thigh: 0.44, shin: 0.43, spine: 0.56, shoulderW: 0.25, upperArm: 0.31, forearm: 0.3, neck: 0.15 },
  ctx: { energy: 0.9, hand: 1 },
  clips: { idle, cast, cast_R: castR, strike: cast },
  build(b, c) {
    const violet = c.primary;
    const night = c.secondary;
    const skin = '#d9c3e8';
    b.add('hips', geo.cone(0.38, 0.9, 7), night, { pos: [0, -0.4, 0] });
    b.add('hips', geo.box(0.22, 0.07, 0.34), c.team, { pos: [0, 0.05, 0] });
    b.add('spine', geo.box(0.24, 0.44, 0.36), violet, { pos: [0, 0.27, 0] });
    b.add('spine', geo.cone(0.34, 1.0, 6), night, { pos: [-0.14, 0.05, 0], rot: [0, 0, 0.12] }); // cape
    b.glow('spine', geo.ball(0.06, 0), c.glow, { pos: [0.13, 0.36, 0] });
    // capuche pointue, visage dans l'ombre, yeux cyan
    b.add('head', geo.ball(0.13, 1), skin, { pos: [0.04, 0.09, 0] });
    b.add('head', geo.cone(0.2, 0.42, 6), violet, { pos: [-0.04, 0.24, 0], rot: [0, 0, 0.35] });
    b.glow('head', geo.box(0.02, 0.025, 0.045), c.glow, { pos: [0.16, 0.1, -0.05] });
    b.glow('head', geo.box(0.02, 0.025, 0.045), c.glow, { pos: [0.16, 0.1, 0.05] });
    for (const [arm, fore] of [['armL', 'foreL'], ['armR', 'foreR']] as const) {
      b.add(arm, geo.box(0.11, 0.3, 0.11), violet, { pos: [0, -0.14, 0] });
      b.add(fore, geo.cone(0.11, 0.3, 6), night, { pos: [0, -0.14, 0], rot: [Math.PI, 0, 0] });
      b.glow(fore, geo.ball(0.06, 0), c.glow, { pos: [0, -0.3, 0] });
    }
    for (const shin of ['shinL', 'shinR'] as const) b.add(shin, geo.box(0.18, 0.07, 0.1), night, { pos: [0.04, -0.4, 0] });
    for (const leg of ['legL', 'legR'] as const) b.add(leg, geo.box(0.1, 0.4, 0.1), night, { pos: [0, -0.2, 0] });
  },
  extras(rig, _built, c) {
    // éclats arcaniques en orbite irrégulière
    const mat = new THREE.MeshBasicMaterial({ color: new THREE.Color(c.accent).multiplyScalar(1.8), toneMapped: false });
    const g = new THREE.OctahedronGeometry(0.06, 0);
    const shards = [0, 1, 2, 3].map(() => {
      const m = new THREE.Mesh(g, mat);
      m.userData.noDissolve = true;
      rig.joint('body').add(m);
      return m;
    });
    return (_dt, s) => {
      const speed = s.anim.startsWith('cast') ? 5 : 1.6;
      shards.forEach((m, i) => {
        const a = s.time * speed * (i % 2 ? 1 : -1.3) + i * 1.7;
        m.visible = !s.dead;
        m.position.set(Math.cos(a) * (0.45 + i * 0.05), 1.0 + Math.sin(s.time * 2.3 + i) * 0.35, Math.sin(a) * (0.45 + i * 0.05));
        m.rotation.set(s.time * 3, s.time * 2, 0);
      });
    };
  },
};
