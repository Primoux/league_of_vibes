/** Duelliste original, index levé en moulinet permanent ; les doigts restent habillés de gants. */
import * as THREE from 'three';
import type { ChampionModelDef } from '../ChampionView.ts';
import { J, setOffset, setRot, type Clip, type Pose } from '../anim/pose.ts';
import { face, hair, hand, human } from '../portrait/anatomy.ts';
import { cloth, Sculpt } from '../portrait/geometry.ts';

export function sampleTouilleur(t: number, p: Pose, forward = false): void {
  const arm = forward ? .95 : .35, fore = forward ? .62 : Math.PI - .35;
  setRot(p, J.armR, 0, 0, arm); setRot(p, J.foreR, 0, 0, fore);
  setRot(p, J.prop, .22 * Math.cos(t * Math.PI * 2), 0, -arm - fore + .22 * Math.sin(t * Math.PI * 2));
  setRot(p, J.armL, .1, 0, -.12); setRot(p, J.foreL, 0, 0, .2);
  setRot(p, J.head, 0, -.08, -.04);
}
const idle: Clip = { loop: true, duration: 1, sample(t, p) { sampleTouilleur(t, p); setOffset(p, 0, Math.sin(t * 2) * .015, 0); } };
const run: Clip = { loop: true, duration: .7, sample(t, p) {
  sampleTouilleur(t, p); const a = Math.sin(t * Math.PI * 2 / .7);
  setRot(p, J.legL, 0, 0, a * .65); setRot(p, J.legR, 0, 0, -a * .65);
  setRot(p, J.shinL, 0, 0, -.18 - Math.max(0, -a) * .65); setRot(p, J.shinR, 0, 0, -.18 - Math.max(0, a) * .65);
  setRot(p, J.armL, .12, 0, -a * .3); setOffset(p, 0, Math.abs(a) * .035, 0);
} };
const cast: Clip = { loop: false, duration: .6, sample(t, p) { sampleTouilleur(t, p, true); setRot(p, J.spine, 0, Math.sin(t * 5) * .1, 0); } };
const ultimate: Clip = { loop: false, duration: 2.25, sample(t, p) { sampleTouilleur(t * 2, p, true); setRot(p, J.armL, .15, 0, .25); setOffset(p, 0, Math.sin(t * 17) * .015, 0); } };

export const touilleurModel: ChampionModelDef = {
  rig: { hipY: .9, hipW: .14, thigh: .43, shin: .43, spine: .57, shoulderW: .26, upperArm: .3, forearm: .29, neck: .16 },
  clips: { idle, run, dash: run, attackReady: idle, strike: cast, cast, cast_Q: cast, cast_W: cast, cast_E: cast, cast_R: ultimate, recall: idle },
  build(b, c) {
    const s = new Sculpt(b), ivory = '#efe4ca', brass = '#bc914b', copper = '#7b3d23';
    human(s, c, this.rig, { cloth: '#17434a', pants: '#263135', skin: '#c79371', trim: brass, openHands: true, width: .23 });
    face(s, { skin: '#cfa07c', hair: copper, eye: '#6b5334', smile: true }); hair(s, '#864321', '#b96631');
    // Moustache à deux vraies boucles, front dégagé et mèches cuivrées.
    for (const side of [-1, 1]) {
      s.tube('head', copper, [[.183, .052, side * .012], [.186, .038, side * .067], [.168, .049, side * .11], [.17, .085, side * .112], [.18, .09, side * .088]], [.013, .016, .01, .007, .002]);
      s.panel('spine', '#1d5358', [[side * .08, .15], [side * .24, .24], [side * .21, -.09], [side * .065, -.2]], [.15, .48, 0], false, .035);
      s.tube('spine', brass, [[.17, .65, side * .235], [.19, .47, side * .18], [.16, .29, side * .08]], [.009, .011, .007], true);
      s.panel('hips', '#17434a', [[side * .035, .04], [side * .19, .07], [side * .28, -.55], [side * .08, -.64]], [-.16, -.02, 0], false, .025);
      for (let i = 0; i < 3; i++) s.ell('spine', brass, [.172, .15 + i * .1, side * .065], [.013, .02, .02], 8, true);
    }
    s.part('spine', cloth(.78, .31, .2, -.16, .018), '#12393f', { smooth: true });
    for (let i = 0; i < 3; i++) s.panel('spine', ivory, [[-.07 - i * .012, .045], [.07 + i * .012, .045], [.05 + i * .012, -.04], [0, -.07], [-.05 - i * .012, -.04]], [.18 + i * .015, .49 - i * .058, 0], false, .012);
    s.ring('spine', brass, .047, .009, [.202, .5, 0], 'x', true); s.gem('spine', c.glow, [.215, .5, 0], [.014, .038, .027]);
    hand(s, 'foreL', ivory, this.rig.forearm);
    const end = this.rig.forearm;
    s.ell('foreR', ivory, [0, -end - .045, 0], [.054, .07, .07], 12);
    for (let i = 0; i < 3; i++) s.tube('foreR', ivory, [[.035, -end - .025, (i - 1) * .037], [.06, -end - .07, (i - 1) * .037], [.026, -end - .087, (i - 1) * .037]], [.018, .018, .012]);
    s.ell('foreR', ivory, [.06, -end + .005, -.05], [.024, .045, .025], 8);
    // L'index appartient à l'os prop : son mouvement est indépendant du poignet et des autres doigts.
    s.tube('prop', ivory, [[0, .075, .046], [0, .19, .046], [.012, .3, .046]], [.023, .021, .014], false, 8);
    const spiral: THREE.Vector3[] = [];
    for (let i = 0; i <= 24; i++) { const a = i / 24 * Math.PI * 4; spiral.push(new THREE.Vector3(Math.cos(a) * .05, .17 + i / 24 * .22, .046 + Math.sin(a) * .05)); }
    b.glow('prop', new THREE.TubeGeometry(new THREE.CatmullRomCurve3(spiral), 24, .006, 4), c.glow);
  },
  extras(rig) {
    const chain = [J.body, J.hips, J.spine, J.armR, J.foreR].map(j => rig.joints[j]!);
    const parent = new THREE.Quaternion(), desired = new THREE.Quaternion(), euler = new THREE.Euler();
    return (_dt, state) => {
      if (state.dead || state.anim === 'stunned') return;
      // Horloge continue entre les clips ; compensation du bras pour garder le doigt vers le haut.
      const a = state.time * Math.PI * (state.anim === 'cast_R' ? 4 : 2);
      parent.identity(); for (const bone of chain) parent.multiply(bone.quaternion);
      desired.setFromEuler(euler.set(.22 * Math.cos(a), 0, .22 * Math.sin(a)));
      rig.joint('prop').quaternion.copy(parent.invert()).multiply(desired);
    };
  },
};
