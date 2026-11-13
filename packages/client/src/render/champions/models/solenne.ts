/**
 * Solenne, la Prêtresse de l'Aube — robe ivoire et ambre, auréole solaire flottante, bâton surmonté d'un disque.
 */
import * as THREE from 'three';
import { bell, easeIn, easeOut } from '../anim/ease.ts';
import { J, setOffset, setRot, type Clip } from '../anim/pose.ts';
import { geo } from '../builder.ts';
import type { ChampionModelDef } from '../ChampionView.ts';

function hold(p: Float32Array): void {
  setRot(p, J.armR, -0.15, 0, 0.35);
  setRot(p, J.foreR, 0, 0, 0.9);
  setRot(p, J.prop, 0, 0, -1.25);
  setRot(p, J.armL, 0.25, 0, 0.3);
  setRot(p, J.foreL, 0, 0, 0.9);
}

const idle: Clip = {
  loop: true,
  duration: 3,
  sample(t, p) {
    hold(p);
    const b = Math.sin((t / 3) * Math.PI * 2);
    setRot(p, J.head, 0, 0, -0.05 + b * 0.03);
    setOffset(p, 0, b * 0.015, 0);
  },
};
/** Bénédiction : main gauche ouverte vers la cible. */
const cast: Clip = {
  loop: false,
  duration: 0.55,
  sample(t, p) {
    hold(p);
    const k = bell(t, 0, 0.55);
    setRot(p, J.armL, 0.2, 0, 0.3 + 1.3 * k);
    setRot(p, J.foreL, 0, 0, 0.9 - 0.8 * k);
    setRot(p, J.head, 0, 0, 0.12 * k);
  },
};
const strike: Clip = {
  loop: false,
  duration: 0.38,
  sample(t, p) {
    hold(p);
    const k = easeOut(t / 0.08) * (1 - easeIn((t - 0.14) / 0.24));
    setRot(p, J.armR, -0.15, 0, 0.35 + 0.9 * k);
    setRot(p, J.foreR, 0, 0, 0.9 - 0.5 * k);
    setRot(p, J.prop, 0, 0, -1.25 - 0.4 * k);
  },
};
/** R : bâton levé au ciel, bras ouverts. */
const castR: Clip = {
  loop: false,
  duration: 1,
  sample(t, p) {
    hold(p);
    const k = easeOut(t / 0.3) * (1 - easeIn((t - 0.7) / 0.3));
    setRot(p, J.armR, -0.2, 0, 0.35 + 2.3 * k);
    setRot(p, J.foreR, 0, 0, 0.9 - 0.8 * k);
    setRot(p, J.prop, 0, 0, -1.25 + 0.6 * k);
    setRot(p, J.armL, 1.2 * k, 0, 0.3 + 0.8 * k);
    setRot(p, J.spine, 0, 0, 0.2 * k);
    setRot(p, J.head, 0, 0, 0.35 * k);
    setOffset(p, 0, 0.12 * k, 0);
  },
};

export const solenneModel: ChampionModelDef = {
  rig: { hipY: 0.9, hipW: 0.11, thigh: 0.45, shin: 0.43, spine: 0.55, shoulderW: 0.23, upperArm: 0.3, forearm: 0.29, neck: 0.16 },
  dropAngle: -Math.PI / 2,
  carry: 0,
  ctx: { energy: 0.85, hand: 1 },
  clips: { idle, cast, strike, cast_R: castR },
  build(b, c) {
    const ivory = c.primary;
    const amber = c.secondary;
    const skin = '#f1d9c4';
    b.add('hips', geo.cone(0.42, 0.95, 8), ivory, { pos: [0, -0.42, 0] });
    b.add('hips', geo.torus(0.39, 0.025, 12), amber, { pos: [0, -0.84, 0], rot: [Math.PI / 2, 0, 0] });
    b.add('hips', geo.box(0.2, 0.07, 0.3), c.team, { pos: [0, 0.04, 0] });
    b.add('spine', geo.box(0.22, 0.42, 0.32), ivory, { pos: [0, 0.24, 0] });
    b.add('spine', geo.box(0.24, 0.1, 0.34), amber, { pos: [0, 0.48, 0] });
    b.glow('spine', geo.ball(0.05, 0), c.glow, { pos: [0.12, 0.34, 0] });
    b.add('head', geo.ball(0.14, 1), skin, { pos: [0.02, 0.1, 0] });
    b.add('head', geo.box(0.16, 0.44, 0.28), '#f5e6a8', { pos: [-0.1, -0.02, 0] }); // longs cheveux blonds
    b.add('head', geo.ball(0.15, 1), '#f5e6a8', { pos: [-0.03, 0.15, 0], scale: [1, 0.8, 1.02] });
    b.glow('head', geo.box(0.02, 0.022, 0.04), c.glow, { pos: [0.15, 0.11, -0.05] });
    b.glow('head', geo.box(0.02, 0.022, 0.04), c.glow, { pos: [0.15, 0.11, 0.05] });
    for (const [arm, fore] of [['armL', 'foreL'], ['armR', 'foreR']] as const) {
      b.add(arm, geo.box(0.1, 0.3, 0.1), ivory, { pos: [0, -0.14, 0] });
      b.add(fore, geo.cone(0.1, 0.28, 6), amber, { pos: [0, -0.13, 0], rot: [Math.PI, 0, 0] });
      b.add(fore, geo.ball(0.05, 0), skin, { pos: [0, -0.29, 0] });
    }
    for (const shin of ['shinL', 'shinR'] as const) b.add(shin, geo.box(0.18, 0.07, 0.1), amber, { pos: [0.05, -0.4, 0] });
    for (const leg of ['legL', 'legR'] as const) b.add(leg, geo.box(0.1, 0.4, 0.1), ivory, { pos: [0, -0.2, 0] });
    // bâton : hampe ambre, disque solaire lumineux
    b.add('prop', geo.cyl(0.022, 0.028, 1.5), amber, { pos: [0, 0.2, 0] });
    b.add('prop', geo.torus(0.12, 0.02, 14), amber, { pos: [0, 1.02, 0], rot: [Math.PI / 2, 0, 0] });
    b.glow('prop', geo.ball(0.07, 1), c.glow, { pos: [0, 1.02, 0] });
  },
  extras(rig, _built, c) {
    // auréole flottante derrière la tête
    const mat = new THREE.MeshBasicMaterial({ color: new THREE.Color(c.accent).lerp(new THREE.Color('#ffe9a8'), 0.5).multiplyScalar(1.6), toneMapped: false, transparent: true, opacity: 0.85 });
    const halo = new THREE.Mesh(new THREE.TorusGeometry(0.2, 0.018, 4, 24), mat);
    halo.userData.noDissolve = true;
    rig.joint('head').add(halo);
    halo.position.set(-0.12, 0.2, 0);
    halo.rotation.y = Math.PI / 2;
    return (_dt, s) => {
      halo.visible = !s.dead;
      halo.position.y = 0.2 + Math.sin(s.time * 2) * 0.02;
      halo.rotation.x = s.time * 0.5;
    };
  },
};
