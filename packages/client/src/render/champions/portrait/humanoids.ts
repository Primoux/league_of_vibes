import * as THREE from 'three';
import type { ModelColors } from '../ChampionView.ts';
import type { RigSpec } from '../rig.ts';
import { face, hair, hood, human, cloak, skirt, sun } from './anatomy.ts';
import { Sculpt, loft, plate, type Point } from './geometry.ts';

export type PortraitBuild = (s: Sculpt, c: ModelColors, rig: RigSpec) => void;
const gold = '#c99a48', goldLight = '#ead394';
const whiteGlow = new THREE.Color('#fff1ba').multiplyScalar(1.7);

function staff(s: Sculpt, color: string, trim: string): void {
  s.tube('prop', color, [[0, -.58, 0], [0, .15, 0], [0, .98, 0]], [.026, .023, .02], true, 8);
  for (const y of [-.5, -.2, .35, .85]) s.ring('prop', trim, .031, .008, [0, y, 0], 'y', true);
}

const kaelen: PortraitBuild = (s, c, rig) => {
  human(s, c, rig, { cloth: '#263451', pants: '#263451', skin: '#a28b73', trim: gold, armor: true, width: .26 });
  // Plastron bombé, arrêtes dorées, tabard et soleil de poitrine.
  s.form('spine', '#bb8e3e', [[.1, .16, .2], [.24, .205, .25], [.43, .17, .255], [.53, .11, .14]], true);
  s.panel('spine', '#23365b', [[-.12, .19], [.12, .19], [.1, -.16], [0, -.21], [-.1, -.16]], [.205, .28, 0]);
  s.part('hips', plate([[-.12, .04], [.12, .04], [.15, -.48], [0, -.57], [-.15, -.48]], .018), c.secondary, { pos: [.15, -.03, 0] });
  sun(s, 'spine', goldLight, [.226, .4, 0], .067, whiteGlow);
  for (const side of [-1, 1]) {
    s.panel('arm' + (side < 0 ? 'L' : 'R') as 'armL' | 'armR', goldLight, [[-.17, .06], [-.13, .17], [.07, .18], [.19, .02], [.13, -.12], [-.12, -.11]], [.03, .025, side * .025], true);
    s.tube('spine', goldLight, [[.155, .51, side * .13], [.2, .42, side * .2], [.21, .26, side * .22]], [.014, .014, .008], true);
  }
  cloak(s, '#243659', '#cfaa62', .9, .37);
  // Heaume sculpté fermé, visière en V et plumet souple bleu nuit.
  s.ell('head', gold, [0, .14, 0], [.177, .218, .17], 16, true);
  s.panel('head', '#182030', [[-.115, .08], [0, .038], [.115, .08], [.09, .035], [0, -.025], [-.09, .035]], [.17, .17, 0], false, .008);
  s.panel('head', goldLight, [[-.018, .23], [.02, .23], [.026, -.13], [0, -.18], [-.025, -.13]], [.19, .12, 0], true);
  for (const side of [-1, 1]) s.panel('head', goldLight, [[0, .13], [side * .15, .19], [side * .17, .12], [side * .095, .07]], [.155, .16, 0], true);
  sun(s, 'head', goldLight, [.16, .32, 0], .045, whiteGlow);
  for (let i = 0; i < 5; i++) s.tube('head', i % 2 ? '#263a68' : '#354f87', [[-.04, .34, (i - 2) * .022], [-.09, .53 + i * .012, 0], [-.3, .48, (i - 2) * .035], [-.41, .3, (i - 2) * .04]], [.035, .05, .04, .004]);
  // Épée solaire : lame à quatre arêtes, garde ailée, gravure et pommeau.
  s.tube('prop', '#453426', [[-.18, 0, 0], [-.05, 0, 0], [.1, 0, 0]], [.04, .035, .04]);
  s.panel('prop', gold, [[-.27, -.04], [-.3, .04], [-.13, .07], [0, .03], [.13, .07], [.3, .04], [.27, -.04], [0, -.02]], [.12, 0, 0], true, .045);
  s.part('prop', loft([[0, .025, .09], [.77, .018, .064], [1.07, .002, .002]], 4), '#e7e6df', { pos: [.15, 0, 0], rot: [0, 0, -Math.PI / 2] }, true);
  s.tube('prop', goldLight, [[.2, .026, 0], [.65, .019, 0], [1.1, .005, 0]], [.009, .008, .002], true);
  s.gem('prop', whiteGlow, [.13, .01, 0], [.045, .048, .048]);
};

const vrenna: PortraitBuild = (s, c, rig) => {
  human(s, c, rig, { cloth: '#803638', pants: '#493029', skin: '#bd8d6b', trim: '#b58b54', bareArms: true });
  face(s, { skin: '#c89b79', hair: '#35241d', eye: '#735033', feminine: true }); hair(s, '#35241d', '#5d3b2b');
  // Cuir articulé, coutures, rivets et foulard crème du portrait.
  for (const side of [-1, 1]) {
    s.panel('spine', '#a14844', [[0, .21], [side * .2, .16], [side * .16, -.2], [side * .035, -.25]], [.162, .3, 0]);
    for (let i = 0; i < 4; i++) s.ell('spine', gold, [.187, .2 + i * .065, side * (.12 + i * .01)], [.009, .011, .011], 8, true);
    s.panel(side < 0 ? 'armL' : 'armR', '#85383c', [[-.12, .05], [0, .14], [.13, .05], [.1, -.1], [-.1, -.1]], [0, .01, 0]);
  }
  s.form('spine', '#e0cda3', [[.43, .16, .25], [.53, .115, .16], [.58, .09, .11]]);
  s.tube('spine', '#b9a77e', [[.13, .47, -.2], [.18, .44, 0], [.13, .48, .2]], [.012, .012, .008]);
  s.panel('spine', '#dfcda5', [[-.12, .15], [.07, .13], [.11, -.3], [0, -.41], [-.06, -.25]], [-.22, .26, .14]);
  // Longue tresse avec ses deux brins, bagues et extrémité effilée.
  for (let i = 0; i < 2; i++) {
    const points: Point[] = [];
    for (let j = 0; j < 9; j++) points.push([.02 + Math.cos(j * 1.7 + i * Math.PI) * .026, .18 - j * .065, .16 + Math.sin(j * 1.7 + i * Math.PI) * .026]);
    s.tube('head', i ? '#35241d' : '#59402c', points, [.04, .032, .017]);
  }
  for (const y of [.03, -.18, -.32]) s.ring('head', gold, .042, .007, [.02, y, .16], 'y', true);
  s.tube('prop', '#b6a482', [[0, -.63, 0], [0, .4, 0], [0, 1.34, 0]], [.027, .022, .023]);
  for (const y of [-.35, -.15, .95, 1.2]) s.ring('prop', gold, .033, .009, [0, y, 0], 'y', true);
  s.panel('prop', '#deca99', [[0, .26], [.07, -.01], [.025, -.13], [0, -.08], [-.025, -.13], [-.07, -.01]], [0, 1.43, 0], true);
  s.panel('prop', '#a84440', [[0, .1], [.18, .04], [.12, -.16], [0, -.07]], [0, 1.14, .08], false, .012);
};

const ilyra: PortraitBuild = (s, c, rig) => {
  human(s, c, rig, { cloth: '#234c78', pants: '#193553', skin: '#eadde1', trim: '#abd7ee' });
  skirt(s, '#173b62', '#a1cbdf', .4); cloak(s, '#25517a', '#c5e8f3', .9, .42);
  face(s, { skin: '#e9dfe7', hair: '#9eafc6', eye: '#6195c8', feminine: true }); hair(s, '#bccddd', '#e3eff3', true);
  for (const side of [-1, 1]) {
    s.panel('spine', '#8ebbd8', [[0, .24], [side * .2, .12], [side * .16, -.14], [side * .04, -.21]], [.18, .27, 0], true);
    s.panel(side < 0 ? 'armL' : 'armR', '#b9deea', [[-.13, .04], [0, .21], [.13, .04], [.09, -.11], [-.09, -.11]], [.02, 0, 0], true);
    s.tube('spine', '#c3e4f0', [[.13, .49, side * .09], [.2, .38, side * .15], [.16, .1, side * .1]], [.012, .018, .009], true);
  }
  s.gem('spine', c.glow, [.207, .29, 0], [.035, .1, .055]);
  s.ring('head', '#98bfd4', .165, .014, [0, .255, 0], 'y', true);
  for (let i = -3; i <= 3; i++) {
    s.gem('head', new THREE.Color('#c6eaff').multiplyScalar(1.1), [.015, .34 + (3 - Math.abs(i)) * .025, i * .052], [.028, .12 - Math.abs(i) * .014, .032]);
  }
  staff(s, '#334a6b', '#b5cbdc');
  for (const side of [-1, 1]) s.tube('prop', '#bad7e3', [[0, .84, side * .02], [0, 1.02, side * .14], [0, 1.18, side * .1]], [.028, .022, .004], true);
  s.gem('prop', c.glow, [0, 1.12, 0], [.095, .22, .09]);
};

const zephyr: PortraitBuild = (s, c, rig) => {
  human(s, c, rig, { cloth: '#5d3486', pants: '#241c3a', skin: '#c69ba6', trim: '#70628c' });
  skirt(s, '#2e2248', '#8364a5', .33, .76); cloak(s, '#4d2d73', '#8f62ac', 1.02, .43);
  face(s, { skin: '#cba7a8', hair: '#302432', eye: '#4abecb', beard: true }); hood(s, '#5a3384', '#8d5ca7', true);
  // Frange asymétrique du portrait ; les mèches balayent le front vers la tempe.
  for (let i = 0; i < 4; i++) s.tube('head', i % 2 ? '#392b46' : '#241c31', [[.015, .29, -.09 + i * .035], [.12, .29 - i * .009, -.04 + i * .035], [.15, .22 - i * .012, .05 + i * .028], [.08, .11, .14]], [.033, .04, .026, .002]);
  for (const side of [-1, 1]) {
    const fore = side < 0 ? 'foreL' : 'foreR';
    s.form(fore, '#65728b', [[-.29, .08, .09], [-.2, .13, .13], [-.03, .11, .11]], true);
    s.panel(fore, '#807399', [[-.09, .1], [0, .15], [.09, .1], [.07, -.1], [-.07, -.1]], [.11, -.13, 0], true);
    s.gem(fore, c.glow, [.135, -.12, 0], [.028, .066, .05]);
    s.tube('spine', '#9ba0b4', [[.13, .51, side * .15], [.2, .4, side * .08], [.17, .1, side * .12]], [.02, .019, .008], true);
  }
  s.gem('spine', c.glow, [.19, .36, 0], [.035, .095, .06]);
};

const sable: PortraitBuild = (s, c, rig) => {
  human(s, c, rig, { cloth: '#71432d', pants: '#3c2c29', skin: '#ba8967', trim: '#b28c50' });
  cloak(s, '#77472e', '#b68d57', .98, .34);
  face(s, { skin: '#c08b68', hair: '#3b251e', eye: '#a46b2e', feminine: true }); hair(s, '#3b251e', '#6a432a', true);
  // Foulard enveloppant, chapeau creusé, ceinture et boucle de laiton.
  s.form('head', '#ba975e', [[-.045, .14, .14, .06], [.035, .173, .165, .05], [.075, .142, .16, .04]]);
  s.tube('head', '#907345', [[.196, .045, -.11], [.217, .013, 0], [.196, .045, .11]], [.007, .01, .007]);
  s.form('head', '#4a3025', [[.26, .37, .35, -.025], [.28, .38, .36, -.025], [.3, .17, .17, -.02], [.45, .15, .15, -.035], [.48, .115, .125, -.04]], false, { smooth: true });
  s.form('head', '#8e6538', [[.3, .172, .173, -.02], [.34, .17, .17, -.025]]);
  s.panel('head', gold, [[-.042, .03], [.042, .03], [.042, -.03], [-.042, -.03]], [.153, .32, 0], true);
  s.tube('spine', '#473328', [[.16, .52, -.18], [.195, .27, 0], [.14, .05, .17]], [.035, .037, .035]);
  for (let i = 0; i < 6; i++) s.ell('spine', '#cba66a', [.21, .42 - i * .055, -.11 + i * .045], [.02, .035, .013], 8, true);
  s.panel('prop', '#533726', [[-.055, .2], [.06, .2], [.07, -.2], [.025, -.3], [-.065, -.25]], [0, -.03, 0], false, .065);
  s.part('prop', new THREE.CylinderGeometry(.032, .037, .86, 12), '#b7834e', { pos: [0, .63, 0], smooth: true }, true);
  s.form('prop', '#6b5140', [[.18, .055, .06], [.36, .05, .055]], true);
  s.ell('prop', '#18232d', [0, 1.064, 0], [.027, .003, .027], 10);
  for (const y of [.25, .8, 1.02]) s.ring('prop', '#d3ac70', .039, .009, [0, y, 0], 'y', true);
  s.ring('prop', '#c3a16d', .07, .009, [0, .15, .04], 'x', true);
};

const corvin: PortraitBuild = (s, c, rig) => {
  human(s, c, rig, { cloth: '#303343', pants: '#202634', skin: '#333540', trim: '#7d8a94' });
  cloak(s, '#202735', '#3c4657', 1, .38); hood(s, '#252b39', '#525c6c');
  s.ell('head', '#292e3b', [.025, .13, 0], [.15, .2, .15], 12);
  s.panel('head', '#a2abb2', [[0, .21], [.14, .12], [.09, -.05], [0, -.21], [-.09, -.05], [-.14, .12]], [.145, .14, 0], true, .045);
  s.tube('head', '#7b8793', [[.16, .26, 0], [.27, .08, 0], [.32, -.04, 0]], [.035, .032, .003], true);
  for (const side of [-1, 1]) {
    s.panel('head', '#111c24', [[side * .025, .024], [side * .115, .06], [side * .1, .005], [side * .035, -.01]], [.18, .2, 0]);
    s.gem('head', c.glow, [.187, .218, side * .069], [.008, .013, .037]);
  }
  for (const side of [-1, 1]) for (let i = 0; i < 8; i++) {
    const x = -.08 - (i % 3) * .035, y = .49 - Math.floor(i / 3) * .095, z = side * (.2 + (i % 3) * .08);
    s.panel('spine', i % 2 ? '#363e4d' : '#202734', [[-.035, .07], [.05, .055], [.045, -.08], [0, -.2], [-.04, -.07]], [x, y, z]);
  }
  // Véritable arc recourbé ouvert, poignée et corde (plus de cercle fermé).
  s.tube('foreL', '#594b44', [[.02, -.92, 0], [-.16, -.73, 0], [-.28, -.33, 0], [-.16, .07, 0], [.02, .27, 0]], [.018, .028, .038, .028, .018], true);
  s.tube('foreL', '#9ca6aa', [[.02, -.92, 0], [.075, -.83, 0], [.04, -.76, 0]], [.012, .02, .012], true);
  s.tube('foreL', '#9ca6aa', [[.02, .27, 0], [.075, .18, 0], [.04, .11, 0]], [.012, .02, .012], true);
  s.b.glow('foreL', new THREE.CylinderGeometry(.004, .004, 1.18, 4), c.glow, { pos: [.025, -.325, 0] });
};

const nyx: PortraitBuild = (s, c, rig) => {
  human(s, c, rig, { cloth: '#26233b', pants: '#171e2b', skin: '#28263a', trim: '#594169' });
  cloak(s, '#372547', '#79527d', .85, .34); hood(s, '#392b54', '#805887', true);
  s.ell('head', '#0d1421', [.05, .12, 0], [.13, .178, .135], 14);
  for (const side of [-1, 1]) s.gem('head', c.glow, [.18, .16, side * .06], [.008, .014, .041]);
  for (const side of [-1, 1]) {
    const fore = side < 0 ? 'foreL' : 'foreR';
    s.panel('spine', '#5e426b', [[0, .18], [side * .18, .09], [side * .11, -.16], [0, -.2]], [.15, .3, 0]);
    s.panel(fore, '#6b6a80', [[-.035, -.05], [.045, -.04], [.1, -.22], [.08, -.39], [-.07, -.57], [-.025, -.34]], [.075, -.16, 0], true);
    s.b.glow(fore, plate([[.085, -.22], [.08, -.39], [-.07, -.57], [-.03, -.44], [.048, -.35]], .006, .001), c.glow, { pos: [.1, -.16, 0] });
    s.ring(fore, '#796281', .065, .012, [0, -.28, 0], 'y', true);
  }
};

const solenne: PortraitBuild = (s, c, rig) => {
  human(s, c, rig, { cloth: '#f0e1bc', pants: '#c4ab78', skin: '#e8c5ab', trim: '#c5a15c' });
  skirt(s, '#e8ddbf', '#ba9654', .42); cloak(s, '#f0e5c8', '#c5a464', .95, .36);
  face(s, { skin: '#e7c3aa', hair: '#a97e3d', eye: '#8b6b35', feminine: true }); hair(s, '#c2a263', '#e4ce97', true);
  for (const side of [-1, 1]) {
    s.panel('spine', '#d3b16d', [[0, .23], [side * .2, .12], [side * .13, -.13], [side * .04, -.22]], [.17, .3, 0], true);
    s.tube('head', gold, [[.1, .24, side * .04], [.04, .27, side * .13], [-.02, .2, side * .16]], [.008, .01, .008], true);
    s.panel(side < 0 ? 'armL' : 'armR', '#e2c78f', [[-.1, .04], [0, .14], [.1, .04], [0, -.04]], [.02, 0, 0], true);
  }
  sun(s, 'spine', '#d3ad62', [.204, .34, 0], .067, whiteGlow);
  s.gem('head', new THREE.Color('#74d7dc'), [.16, .25, 0], [.012, .034, .02]);
  s.ring('head', '#d7b66e', .29, .012, [-.12, .23, 0], 'x', true);
  for (let i = 0; i < 12; i++) {
    const a = i / 12 * Math.PI * 2;
    s.tube('head', '#dfc684', [[-.12, .23 + Math.cos(a) * .3, Math.sin(a) * .3], [-.12, .23 + Math.cos(a) * .37, Math.sin(a) * .37]], [.014, .003], true, 5);
  }
  staff(s, '#ad8548', '#dac591'); sun(s, 'prop', '#ddbd76', [0, 1.03, 0], .13, whiteGlow);
};

const master67: PortraitBuild = (s, c, rig) => {
  human(s, c, rig, { cloth: '#7653d2', pants: '#1e2b43', skin: '#ad7655', trim: '#3ed9d4', openHands: true, width: .27 });
  face(s, { skin: '#b27d59', hair: '#282034', eye: '#4c3028', beard: true, smile: true }); hair(s, '#282034', '#44304a', false, true); hood(s, '#7250c8', '#3ed9d4');
  // Coutures, plis de hoodie, poche, cordons, chaîne et bijoux : tous dans le lot principal.
  for (const side of [-1, 1]) {
    s.tube('spine', '#ad8ae7', [[.115, .48, side * .14], [.17, .25, side * .19], [.14, .08, side * .2]], [.009, .014, .009]);
    s.tube('spine', '#3ed9d4', [[.115, .51, side * .1], [.193, .38, side * .09], [.19, .23, side * .085]], [.012, .014, .009]);
    s.tube('spine', '#c3a153', [[.105, .5, side * .13], [.18, .36, side * .09], [.183, .32, 0]], [.007, .009, .007]);
    s.panel('spine', '#6645ac', [[0, .06], [side * .14, .07], [side * .12, -.04], [0, -.05]], [.17, .13, 0], false, .012);
  }
  s.ring('head', '#e1bf6f', .027, .007, [0, .07, .17]);
  s.gem('spine', new THREE.Color('#80eff0'), [.19, .29, 0], [.013, .027, .025]);
  for (const fore of ['foreL', 'foreR'] as const) {
    s.ell(fore, '#b98562', [.02, -.355, 0], [.037, .095, .114], 12);
    for (let i = 0; i < 4; i++) {
      const z = (i - 1.5) * .057, len = .11 - Math.abs(i - 1.5) * .012;
      s.tube(fore, '#b98562', [[.015, -.4, z], [.035, -.46, z], [.075, -.44 - len, z]], [.024, .022, .014]);
    }
    const side = fore === 'foreL' ? -1 : 1;
    s.tube(fore, '#b98562', [[.02, -.34, side * .08], [.045, -.36, side * .15], [.09, -.42, side * .17]], [.03, .024, .016]);
  }
  // 67 brodé sur le torse, dans le plan de face, lisible dans le bon sens.
  const shapes = [
    [[.09, .34], [.065, .37], [.02, .35], [.015, .28], [.035, .255], [.075, .26], [.09, .29], [.065, .315], [.02, .3]],
    [[-.025, .36], [-.095, .36], [-.04, .255]],
  ];
  for (const [i, points] of shapes.entries()) s.tube('spine', i ? '#dcb9ef' : '#56e5df', points.map(([z, y]) => [.185, y!, z!] as Point), [.008, .008, .008]);
  for (const shin of ['shinL', 'shinR'] as const) {
    s.ell(shin, '#eff4ed', [.07, -rig.shin + .05, 0], [.18, .085, .11]);
    s.form(shin, '#3ed9d4', [[-rig.shin + .005, .165, .112, .06], [-rig.shin + .022, .17, .112, .06]]);
  }
};

export const humanoidPortraits: Record<string, PortraitBuild> = { kaelen, vrenna, ilyra, zephyr, sable, corvin, nyx, solenne, master67 };
