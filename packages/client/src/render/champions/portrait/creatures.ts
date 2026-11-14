import * as THREE from 'three';
import { geo } from '../builder.ts';
import type { PortraitBuild } from './humanoids.ts';
import { cloak } from './anatomy.ts';

const grondak: PortraitBuild = (s, c) => {
  const stone = '#827361', dark = '#484842', edge = '#a28e72';
  s.form('hips', dark, [[-.19, .22, .31], [.04, .3, .37], [.2, .25, .32]], false);
  s.part('spine', geo.rock(.55, 2, 1), stone, { pos: [0, .46, 0], scale: [.88, 1, 1.05] });
  for (const side of [-1, 1]) {
    s.part('spine', geo.rock(.33, 8 + side), edge, { pos: [-.02, .79, side * .48], scale: [1.15, .82, 1.07] });
    s.panel('spine', '#716552', [[0, .14], [side * .26, .21], [side * .38, -.08], [side * .08, -.21]], [.38, .6, 0], false, .085);
    s.tube('spine', '#3d3c33', [[.37, .67, side * .17], [.43, .49, side * .18], [.4, .29, side * .28]], [.024, .025, .015]);
    s.b.glow('spine', new THREE.TubeGeometry(new THREE.CatmullRomCurve3([new THREE.Vector3(.38, .67, side * .17), new THREE.Vector3(.445, .49, side * .18), new THREE.Vector3(.41, .29, side * .28)]), 8, .009, 4, false), c.glow);
    for (let i = 0; i < 3; i++) s.part('spine', geo.rock(.13, i + 20 + side), stone, { pos: [-.1 - i * .09, .9 + i * .025, side * (.27 - i * .06)] });
  }
  s.panel('spine', dark, [[-.14, .16], [.14, .15], [.08, -.18], [0, -.25], [-.1, -.12]], [.43, .38, 0], false, .08);
  s.gem('spine', c.glow, [.485, .49, 0], [.024, .12, .08]);
  // Visage anguleux, grand front, sourcils de roche et mâchoire taillée du portrait.
  s.part('head', geo.rock(.29, 7, 1), stone, { pos: [.1, .09, 0], scale: [1, 1.1, 1.05] });
  s.panel('head', edge, [[-.2, .2], [-.12, .32], [0, .27], [.14, .32], [.24, .17], [.12, .1], [-.12, .1]], [.28, .14, 0], false, .11);
  for (const side of [-1, 1]) {
    s.panel('head', dark, [[side * .035, .045], [side * .2, .09], [side * .19, .015], [side * .06, -.015]], [.345, .15, 0], false, .012);
    s.gem('head', c.glow, [.36, .18, side * .115], [.012, .026, .066]);
    s.panel('head', '#766752', [[side * .05, .08], [side * .23, .08], [side * .21, -.08], [side * .13, -.14]], [.31, .03, 0], false, .065);
  }
  s.panel('head', edge, [[-.035, .12], [.035, .12], [.06, -.055], [0, -.09], [-.055, -.055]], [.4, .105, 0], false, .06);
  s.panel('head', dark, [[-.13, .025], [.13, .025], [.12, -.055], [-.12, -.055]], [.32, -.085, 0], false, .012);
  s.panel('head', edge, [[-.18, .04], [.18, .04], [.14, -.11], [0, -.14], [-.16, -.08]], [.3, -.14, 0], false, .08);
  for (const [arm, fore, leg, shin, side] of [['armL', 'foreL', 'legL', 'shinL', -1], ['armR', 'foreR', 'legR', 'shinR', 1]] as const) {
    s.part(arm, geo.rock(.23, 30 + side, 1), stone, { pos: [0, -.23, 0], scale: [1, 1.25, 1] });
    s.part(fore, geo.rock(.23, 40 + side), dark, { pos: [0, -.21, 0], scale: [1.1, 1.3, 1] });
    s.part(fore, geo.rock(.31, 50 + side, 1), stone, { pos: [.06, -.5, 0], scale: [1.2, .9, 1.08] });
    for (let i = 0; i < 4; i++) s.part(fore, geo.rock(.09, 60 + i), edge, { pos: [.28, -.49, (i - 1.5) * .13], scale: [.75, 1.25, 1] });
    s.gem(fore, c.glow, [.32, -.47, 0], [.015, .027, .17]);
    s.part(leg, geo.rock(.23, 70 + side), stone, { pos: [0, -.18, 0], scale: [1.1, 1.15, 1.1] });
    s.part(shin, geo.rock(.22, 80 + side), dark, { pos: [0, -.19, 0], scale: [1, 1.15, 1] });
    s.part(shin, geo.rock(.24, 90 + side), stone, { pos: [.1, -.34, 0], scale: [1.3, .5, 1.05] });
  }
};

const tharox: PortraitBuild = (s, c, rig) => {
  const fur = '#593a2c', tan = '#ae8458', ivory = '#dfcfaa', armor = '#655e5a';
  s.form('hips', '#493026', [[-.16, .22, .3], [.1, .25, .34], [.18, .2, .27]]);
  s.form('spine', '#6f4430', [[-.02, .21, .29], [.23, .31, .41], [.55, .29, .46], [.73, .19, .3]]);
  cloak(s, '#74513b', '#916c48', .94, .48);
  for (const side of [-1, 1]) {
    s.ell('spine', tan, [-.08, .6, side * .36], [.25, .22, .26], 10);
    s.ell('spine', armor, [.035, .52, side * .41], [.23, .2, .27], 12, true);
    s.tube('spine', ivory, [[0, .66, side * .43], [.04, .88, side * .46], [.13, .92, side * .42]], [.07, .042, .002]);
    for (let i = 0; i < 8; i++) {
      const z = side * (.16 + i * .04), y = .6 - i % 3 * .06;
      s.tube('spine', i % 2 ? tan : '#77573f', [[.1, y + .06, z], [.16, y - .04, z + side * .06], [.18, y - .17, z + side * .035]], [.055, .04, .003], false, 5);
    }
  }
  s.panel('spine', armor, [[-.13, .17], [.13, .17], [.17, -.14], [0, -.25], [-.17, -.14]], [.3, .27, 0], true, .06);
  s.gem('spine', c.glow, [.345, .27, 0], [.026, .09, .07]);
  // Tête de sanglier : groin modelé, narines, défenses courbes et crinière rouge.
  s.ell('head', fur, [.06, .12, 0], [.235, .25, .235], 16);
  s.ell('head', '#976550', [.25, .055, 0], [.15, .12, .17], 14);
  s.ell('head', '#b17b65', [.355, .065, 0], [.03, .09, .123], 12);
  for (const side of [-1, 1]) {
    s.ell('head', '#35272b', [.382, .075, side * .054], [.007, .028, .022], 8);
    s.ell('head', '#261f22', [.245, .2, side * .125], [.02, .032, .06], 10);
    s.gem('head', c.glow, [.26, .203, side * .122], [.008, .015, .033]);
    s.tube('head', '#3c2926', [[.16, .25, side * .04], [.25, .25, side * .115], [.22, .235, side * .19]], [.014, .03, .012]);
    s.tube('head', ivory, [[.25, -.005, side * .135], [.37, .045, side * .21], [.35, .22, side * .24], [.28, .285, side * .2]], [.055, .047, .025, .003], false, 8);
    s.panel('head', '#704533', [[-.04, .01], [0, .22], [.1, .08], [.065, -.03]], [-.03, .28, side * .17], false, .035);
    s.panel('head', '#a27155', [[0, .02], [.025, .15], [.07, .075]], [-.005, .29, side * .18], false, .01);
  }
  for (let i = 0; i < 7; i++) s.tube('head', i % 2 ? '#853c2d' : '#a34c31', [[-.1, .26, (i - 3) * .042], [-.015, .42 + i % 2 * .04, (i - 3) * .045], [-.2, .42, (i - 3) * .05]], [.045, .05, .002]);
  for (const [arm, fore, leg, shin, side] of [['armL', 'foreL', 'legL', 'shinL', -1], ['armR', 'foreR', 'legR', 'shinR', 1]] as const) {
    s.form(arm, fur, [[-rig.upperArm, .12, .14], [-.1, .18, .19], [.06, .16, .18]]);
    s.form(fore, '#8c6247', [[-rig.forearm, .12, .14], [-.16, .15, .16], [0, .145, .15]]);
    s.form(fore, armor, [[-rig.forearm + .06, .146, .16], [-rig.forearm + .15, .17, .18]], true);
    s.ell(fore, fur, [.025, -rig.forearm - .04, 0], [.14, .12, .16], 10);
    for (let i = -1; i <= 1; i++) s.tube(fore, ivory, [[.11, -rig.forearm - .02, i * .09], [.17, -rig.forearm - .08, i * .09]], [.026, .003]);
    s.form(leg, fur, [[-rig.thigh, .12, .14], [0, .15, .17]]);
    s.form(shin, '#685044', [[-rig.shin + .08, .1, .12], [0, .13, .14]]);
    for (const z of [-.07, .07]) s.ell(shin, '#302b2b', [.075, -rig.shin + .045, z], [.18, .095, .071], 10);
  }
};

export const creaturePortraits: Record<string, PortraitBuild> = { grondak, tharox };
