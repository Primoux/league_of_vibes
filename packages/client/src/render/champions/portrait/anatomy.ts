import * as THREE from 'three';
import type { JointName } from '../anim/pose.ts';
import type { ModelColors } from '../ChampionView.ts';
import type { RigSpec } from '../rig.ts';
import { cloth, loft, plate, Sculpt, type Point } from './geometry.ts';

export interface Outfit {
  cloth: string; pants: string; skin: string; trim: string;
  armor?: boolean; openHands?: boolean; bareArms?: boolean; width?: number;
}

export function human(s: Sculpt, c: ModelColors, rig: RigSpec, o: Outfit): void {
  const w = o.width ?? rig.shoulderW * .84;
  s.form('hips', o.pants, [[-.12, .12, .18], [0, .145, .19], [.13, .13, .17]]);
  s.form('spine', o.cloth, [[0, .13, .17], [.12, .15, w * .78], [.31, .17, w], [rig.spine - .08, .145, w * 1.02], [rig.spine + .02, .09, .12]]);
  s.form('hips', o.trim, [[.055, .151, .197], [.105, .15, .193]]);
  s.panel('hips', c.team, [[-.06, .04], [.06, .04], [.05, -.035], [-.05, -.035]], [.16, .08, 0]);
  s.ell('spine', o.skin, [0, rig.spine + .05, 0], [.07, .115, .075]);
  for (const [arm, fore, leg, shin, side] of [['armL', 'foreL', 'legL', 'shinL', -1], ['armR', 'foreR', 'legR', 'shinR', 1]] as const) {
    s.form(arm, o.bareArms ? o.skin : o.cloth, [[-rig.upperArm, .072, .075], [-rig.upperArm * .6, .09, .09], [-.08, .115, .12], [.035, .09, .1]]);
    s.form(fore, o.cloth, [[-rig.forearm + .025, .067, .07], [-rig.forearm * .65, .085, .09], [-.02, .095, .095]]);
    s.form(fore, o.trim, [[-rig.forearm + .015, .075, .078], [-rig.forearm + .065, .083, .087]], !!o.armor);
    if (!o.openHands) hand(s, fore, o.skin, rig.forearm);
    s.form(leg, o.pants, [[-rig.thigh, .072, .08], [-rig.thigh * .55, .09, .105], [.05, .105, .12]]);
    s.form(shin, o.pants, [[-rig.shin + .07, .065, .075], [-rig.shin * .5, .075, .085], [0, .085, .095]]);
    s.ell(shin, '#282530', [.055, -rig.shin + .045, 0], [.18, .085, .105]);
    s.form(shin, o.trim, [[-rig.shin + .005, .16, .105, .055], [-rig.shin + .03, .17, .11, .055]]);
    if (o.armor) {
      s.ell(arm, o.trim, [0, -.015, side * .015], [.145, .12, .16], 12, true);
      s.panel(fore, o.trim, [[-.08, .12], [.08, .12], [.067, -.12], [0, -.16], [-.067, -.12]], [.085, -.14, 0], true);
      s.panel(shin, o.trim, [[-.075, .09], [.075, .09], [.055, -.19], [0, -.26], [-.055, -.19]], [.083, -.12, 0], true);
    }
  }
}

export function hand(s: Sculpt, joint: JointName, skin: string, length: number): void {
  s.ell(joint, skin, [.01, -length - .015, 0], [.052, .07, .066]);
  for (let i = 0; i < 4; i++) {
    const z = (i - 1.5) * .027;
    s.tube(joint, skin, [[.033, -length - .025, z], [.05, -length - .07, z], [.02, -length - .087, z]], [.016, .015, .01], false, 5);
  }
  s.ell(joint, skin, [.055, -length + .01, -.055], [.027, .04, .025], 8);
}

export interface Face {
  skin: string; hair: string; eye?: string; feminine?: boolean; smile?: boolean; beard?: boolean;
}
export function face(s: Sculpt, o: Face): void {
  const skin = o.skin, shade = new THREE.Color(skin).multiplyScalar(.78), lips = o.feminine ? '#985d5b' : '#704d47';
  const width = o.feminine ? .147 : .16;
  s.part('head', loft([[-.095, .045, .055, .045], [-.055, .105, width * .64, .045], [.02, .138, width * .9, .03], [.12, .148, width, .025], [.24, .125, width * .93, .015], [.3, .08, width * .65, 0], [.32, .025, .03, 0]], 20), skin, { smooth: true });
  for (const side of [-1, 1]) {
    s.ell('head', skin, [-.003, .105, side * width], [.036, .056, .025], 8);
    s.ell('head', shade, [.012, .11, side * (width + .012)], [.017, .032, .01], 8);
    s.ell('head', '#d8d3c9', [.169, .145, side * .067], [.004, .012, .033], 10);
    s.ell('head', o.eye ?? '#5d3829', [.174, .145, side * .064], [.003, .011, .016], 8);
    s.ell('head', '#161a24', [.178, .145, side * .064], [.002, .008, .009], 8);
    s.tube('head', '#423839', [[.169, .149, side * .035], [.179, .158, side * .067], [.151, .15, side * .1]], [.003, .004, .003], false, 4);
    s.tube('head', o.hair, [[.157, .18, side * .035], [.17, .191, side * .067], [.145, .18, side * .104]], [.006, .01, .005], false, 5);
    s.tube('head', shade, [[.167, .158, side * .035], [.17, .166, side * .07], [.143, .157, side * .105]], [.004, .005, .003], false, 5);
  }
  s.ell('head', skin, [.17, .095, 0], [.028, .051, .021], 12);
  s.ell('head', shade, [.188, .064, 0], [.013, .011, .021], 8);
  s.tube('head', lips, [[.154, .022, -.053], [.168, o.smile ? .006 : .018, 0], [.154, .022, .053]], [.004, .008, .004], false, 6);
  if (o.smile) s.panel('head', '#eadfc8', [[-.035, .009], [.035, .009], [.026, -.004], [-.026, -.004]], [.172, .023, 0], false, .003);
  if (o.beard) {
    s.tube('head', o.hair, [[.145, -.005, -.064], [.16, -.052, 0], [.145, -.005, .064]], [.012, .022, .012], false, 6);
    s.tube('head', o.hair, [[.175, .039, -.045], [.19, .044, 0], [.175, .039, .045]], [.006, .009, .006], false, 5);
  }
}

export function hair(s: Sculpt, dark: string, light: string, long = false, curly = false): void {
  s.ell('head', dark, [-.04, .25, 0], [.15, .12, .17], 12);
  if (curly) {
    // Boucles décalées et serrées : une masse irrégulière, pas des rouleaux parallèles.
    for (let i = 0; i < 12; i++) {
      const row = i < 7 ? 0 : 1, n = row ? i - 7 : i;
      s.ell('head', i % 4 ? dark : light, [.11 - row * .09 + Math.sin(i * 2.4) * .012, .25 + row * .07 + Math.sin(i * 1.8) * .017, (n - (row ? 2 : 3)) * .044], [.041, .041 + (i % 3) * .007, .034], 8);
    }
  } else for (let i = 0; i < 5; i++) {
    const z = (i - 2) * .053;
    s.tube('head', i === 2 ? light : dark,
      [[-.1, .24, z * .65], [-.025, .33, z * .65], [.09, .29, z + .025], [.1, .23 + i * .007, z + .055]],
      [.034, .042, .035, .003], false, 6);
  }
  if (long) for (const side of [-1, 1]) for (let i = 0; i < 5; i++) {
    const z = side * (.13 + i * .017);
    s.tube('head', i % 2 ? dark : light, [[-.055 - i * .014, .22, z], [-.1, .03, z + side * .025], [-.04 - i * .02, -.18, z + side * .045], [-.14, -.36 - i * .017, z + side * .035]], [.032, .039, .027, .004], false, 6);
  }
}

export function hood(s: Sculpt, color: string, trim: string, point = false): void {
  const shell = loft([[-.24, .025, .025], [-.18, .18, .18], [-.05, .24, .215], [.085, .23, .2]], 16, false);
  s.part('head', shell, color, { pos: [0, .16, 0], rot: [0, 0, -Math.PI / 2], smooth: true });
  const inside = loft([[-.235, .02, .02], [-.175, .169, .169], [-.045, .228, .202], [.084, .218, .187]], 16, false);
  const index = inside.index!; for (let i = 0; i < index.count; i += 3) { const a = index.getX(i); index.setX(i, index.getX(i + 1)); index.setX(i + 1, a); } inside.computeVertexNormals();
  s.part('head', inside, new THREE.Color(color).multiplyScalar(.45), { pos: [0, .16, 0], rot: [0, 0, -Math.PI / 2], smooth: true });
  s.tube('head', trim, [[.05, -.065, -.17], [.095, .16, -.19], [.015, .36, 0], [.095, .16, .19], [.05, -.065, .17]], [.025, .023, .027, .023, .025], false, 6);
  if (point) s.tube('head', color, [[-.05, .3, 0], [-.11, .46, .015], [-.24, .48, .02]], [.12, .075, .003], false, 8);
}

export function cloak(s: Sculpt, color: string, trim: string, height = .95, width = .4): void {
  s.part('spine', cloth(height, width, .25), color, { smooth: true });
  for (const side of [-1, 1]) s.tube('spine', trim, [[-.19, .48, side * .25], [-.23, .05, side * width * .9], [-.28, .48 - height, side * width]], [.013, .014, .016], false, 5);
}

export function skirt(s: Sculpt, color: string, trim: string, width: number, length = .83): void {
  s.form('hips', color, [[-length, .27, width], [-length * .7, .23, width * .87], [-length * .3, .17, width * .6], [.08, .135, .18]], false, { smooth: true });
  s.form('hips', trim, [[-length - .004, .273, width + .005], [-length + .035, .266, width]]);
  for (let i = 0; i < 7; i++) {
    const a = i / 6 * Math.PI - Math.PI / 2;
    s.tube('hips', new THREE.Color(color).multiplyScalar(.83), [[Math.cos(a) * .14, -.04, Math.sin(a) * .18], [Math.cos(a) * .235, -length * .65, Math.sin(a) * width * .83], [Math.cos(a) * .276, -length + .03, Math.sin(a) * width]], [.006, .012, .008], false, 4);
  }
}

export function sun(s: Sculpt, joint: JointName, color: string, pos: Point, r: number, glow: THREE.Color, metal = true): void {
  s.ring(joint, color, r, r * .12, pos, 'x', metal);
  s.ell(joint, color, pos, [r * .16, r * .68, r * .68], 10, metal);
  s.gem(joint, glow, [pos[0] + r * .18, pos[1], pos[2]], [r * .11, r * .35, r * .35]);
  for (let i = 0; i < 10; i++) {
    const a = i / 10 * Math.PI * 2;
    s.part(joint, plate([[0, 0], [.022, r * .18], [0, r * .5], [-.022, r * .18]], .012, .002), color,
      { pos: [pos[0], pos[1] + Math.cos(a) * r * 1.03, pos[2] + Math.sin(a) * r * 1.03], rot: [a, 0, 0] }, metal);
  }
}
