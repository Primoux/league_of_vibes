/**
 * Modèles low-poly des monstres : jungle, drakes (6 éléments + Ancien), larves, Colosse du Rift, Léviathan.
 * Propriétaire : claude-client. Tous regardent +x.
 */
import * as THREE from 'three';
import type { DragonElement } from '@lov/shared';
import { RigView, cachedGeo, glow, material, part, pivot, type Rig } from './rig.ts';

function limb(parent: THREE.Object3D, x: number, y: number, z: number, len: number, w: number, mat: THREE.Material): THREE.Group {
  const p = pivot(x, y, z, parent);
  part(cachedGeo(`limb_${len}_${w}`, () => new THREE.BoxGeometry(w, len, w).translate(0, -len / 2, 0)), mat, 0, 0, 0, p);
  return p;
}

function quadruped(opts: { len: number; h: number; w: number; legLen: number; color: number; belly?: number; headSize: number; snout?: number; tail?: number; eyes?: number }): { rig: Rig; body: THREE.Group } {
  const root = new THREE.Group();
  const fur = material(opts.color, { rough: 0.9 });
  const legMat = material(opts.belly ?? opts.color, { rough: 0.9 });
  const lw = opts.w * 0.28;
  const legs = [
    limb(root, opts.len * 0.32, opts.legLen, opts.w * 0.3, opts.legLen, lw, legMat),
    limb(root, opts.len * 0.32, opts.legLen, -opts.w * 0.3, opts.legLen, lw, legMat),
    limb(root, -opts.len * 0.32, opts.legLen, -opts.w * 0.3, opts.legLen, lw, legMat),
    limb(root, -opts.len * 0.32, opts.legLen, opts.w * 0.3, opts.legLen, lw, legMat),
  ];
  const body = pivot(0, opts.legLen, 0, root);
  part(cachedGeo(`q_body_${opts.len}_${opts.h}_${opts.w}`, () => new THREE.BoxGeometry(opts.len, opts.h, opts.w).translate(0, opts.h / 2, 0)), fur, 0, 0, 0, body);
  const head = pivot(opts.len / 2, opts.h * 0.8, 0, body);
  part(cachedGeo(`q_head_${opts.headSize}`, () => new THREE.BoxGeometry(opts.headSize, opts.headSize * 0.85, opts.headSize * 0.9)), fur, opts.headSize * 0.35, 0, 0, head);
  if (opts.snout) part(cachedGeo(`q_snout_${opts.snout}`, () => new THREE.BoxGeometry(opts.snout!, opts.snout! * 0.5, opts.snout! * 0.6)), fur, opts.headSize * 0.8, -opts.headSize * 0.15, 0, head);
  if (opts.eyes !== undefined) {
    for (const z of [-1, 1]) part(cachedGeo('q_eye', () => new THREE.BoxGeometry(0.04, 0.05, 0.05)), glow(opts.eyes, 2), opts.headSize * 0.86, opts.headSize * 0.15, z * opts.headSize * 0.25, head).castShadow = false;
  }
  let tail: THREE.Group | undefined;
  if (opts.tail) {
    tail = pivot(-opts.len / 2, opts.h * 0.8, 0, body);
    part(cachedGeo(`q_tail_${opts.tail}`, () => new THREE.ConeGeometry(opts.w * 0.15, opts.tail!, 4).rotateZ(Math.PI / 2).translate(-opts.tail! / 2, 0, 0)), fur, 0, 0, 0, tail);
  }
  return { rig: { root, body, head, legs, tail, height: opts.legLen + opts.h, quadruped: true, stride: 2.6 }, body };
}

export function wolfModel(small: boolean): RigView {
  const { rig } = quadruped({ len: 0.9, h: 0.38, w: 0.36, legLen: 0.35, color: small ? 0x6f6a72 : 0x4f4a58, belly: 0x3a3640, headSize: 0.32, snout: 0.22, tail: 0.5, eyes: 0x9fe8ff });
  if (!small) rig.root.scale.setScalar(1.35);
  else rig.root.scale.setScalar(0.85);
  return new RigView(rig);
}

export function raptorModel(small: boolean): RigView {
  const root = new THREE.Group();
  const skin = material(small ? 0xc75a3a : 0xa23a2a, { rough: 0.8 });
  const dark = material(0x3a2420);
  const legs = [limb(root, 0, 0.42, 0.12, 0.42, 0.07, dark), limb(root, 0, 0.42, -0.12, 0.42, 0.07, dark)];
  const body = pivot(0, 0.42, 0, root);
  part(cachedGeo('rp_body', () => new THREE.SphereGeometry(0.26, 7, 5).scale(1.4, 0.9, 0.9)), skin, 0, 0.18, 0, body);
  const head = pivot(0.3, 0.42, 0, body);
  part(cachedGeo('rp_neck', () => new THREE.CylinderGeometry(0.06, 0.09, 0.32, 5)), skin, 0, -0.1, 0, head).rotation.z = -0.5;
  part(cachedGeo('rp_head', () => new THREE.ConeGeometry(0.11, 0.38, 5).rotateZ(-Math.PI / 2)), skin, 0.12, 0.05, 0, head);
  part(cachedGeo('rp_crest', () => new THREE.ConeGeometry(0.05, 0.25, 3)), material(0xf2c14a), -0.02, 0.18, 0, head).rotation.z = 0.6;
  const tail = pivot(-0.34, 0.2, 0, body);
  part(cachedGeo('rp_tail', () => new THREE.ConeGeometry(0.1, 0.55, 5).rotateZ(Math.PI / 2).translate(-0.27, 0, 0)), skin, 0, 0, 0, tail);
  root.scale.setScalar(small ? 0.85 : 1.25);
  return new RigView({ root, body, head, legs, tail, height: 0.9, stride: 3.2 });
}

export function krugModel(small: boolean): RigView {
  const root = new THREE.Group();
  const rock = material(0x8a7d6a, { rough: 1 });
  const moss = material(0x5c7a3a, { rough: 1 });
  const legs = [limb(root, 0, 0.3, 0.2, 0.3, 0.18, rock), limb(root, 0, 0.3, -0.2, 0.3, 0.18, rock)];
  const body = pivot(0, 0.3, 0, root);
  part(cachedGeo('kr_body', () => new THREE.DodecahedronGeometry(0.45, 0).scale(1, 0.85, 1)), rock, 0, 0.38, 0, body);
  part(cachedGeo('kr_moss', () => new THREE.DodecahedronGeometry(0.3, 0).scale(1.2, 0.4, 1.2)), moss, -0.05, 0.75, 0, body);
  const head = pivot(0.35, 0.45, 0, body);
  part(cachedGeo('kr_eye', () => new THREE.BoxGeometry(0.06, 0.06, 0.25)), glow(0xffa040, 2), 0.06, 0, 0, head).castShadow = false;
  const a1 = limb(body, 0, 0.5, 0.48, 0.45, 0.2, rock);
  const a2 = limb(body, 0, 0.5, -0.48, 0.45, 0.2, rock);
  root.scale.setScalar(small ? 0.7 : 1.3);
  return new RigView({ root, body, head, legs, arms: [a1, a2], height: 1.2, stride: 1.4 });
}

export function grompModel(): RigView {
  const root = new THREE.Group();
  const skin = material(0x5f8a3a, { rough: 0.7 });
  const belly = material(0xc9c27a, { rough: 0.8 });
  const body = pivot(0, 0, 0, root);
  part(cachedGeo('gr_body', () => new THREE.SphereGeometry(0.6, 8, 6).scale(1.15, 0.75, 1)), skin, 0, 0.5, 0, body);
  part(cachedGeo('gr_belly', () => new THREE.SphereGeometry(0.45, 8, 6).scale(0.8, 0.6, 0.9)), belly, 0.25, 0.38, 0, body);
  const head = pivot(0.5, 0.75, 0, body);
  for (const z of [-0.25, 0.25]) {
    part(cachedGeo('gr_eyeball', () => new THREE.SphereGeometry(0.14, 6, 5)), material(0xf2eed0), 0, 0.12, z, head);
    part(cachedGeo('gr_pupil', () => new THREE.SphereGeometry(0.06, 5, 4)), material(0x111111), 0.11, 0.14, z, head);
  }
  const legs = [limb(root, 0.3, 0.3, 0.45, 0.3, 0.14, skin), limb(root, 0.3, 0.3, -0.45, 0.3, 0.14, skin), limb(root, -0.35, 0.35, -0.5, 0.35, 0.2, skin), limb(root, -0.35, 0.35, 0.5, 0.35, 0.2, skin)];
  root.scale.setScalar(1.3);
  return new RigView({ root, body, head, legs, height: 1.2, quadruped: true, stride: 1.2 });
}

/** Golem gardien de buff : pierre + cristaux lumineux (bleu) ou magma (rouge). */
export function buffGolemModel(kind: 'blue' | 'red'): RigView {
  const root = new THREE.Group();
  const c = kind === 'blue' ? 0x4ab0ff : 0xff5a2a;
  const stone = material(kind === 'blue' ? 0x5d6a78 : 0x5a3a30, { rough: 1 });
  const legs = [limb(root, 0, 0.6, 0.32, 0.6, 0.3, stone), limb(root, 0, 0.6, -0.32, 0.6, 0.3, stone)];
  const body = pivot(0, 0.6, 0, root);
  part(cachedGeo('bg_torso', () => new THREE.DodecahedronGeometry(0.65, 0).scale(0.9, 1, 1.1)), stone, 0, 0.6, 0, body);
  part(cachedGeo('bg_core', () => new THREE.OctahedronGeometry(0.22, 0)), glow(c, 2.4), 0.5, 0.65, 0, body).castShadow = false;
  for (let i = 0; i < 5; i++) {
    const a = (i / 5) * Math.PI - Math.PI / 2;
    const cr = part(cachedGeo('bg_shard', () => new THREE.ConeGeometry(0.1, 0.5, 4)), glow(c, 1.6), -0.2, 1.2, Math.sin(a) * 0.4, body);
    cr.rotation.x = a * 0.5;
  }
  const head = pivot(0.35, 1.25, 0, body);
  part(cachedGeo('bg_head', () => new THREE.BoxGeometry(0.4, 0.32, 0.4)), stone, 0, 0, 0, head);
  part(cachedGeo('bg_eyes', () => new THREE.BoxGeometry(0.03, 0.06, 0.28)), glow(c, 3), 0.21, 0.03, 0, head).castShadow = false;
  const a1 = limb(body, 0, 1.0, 0.75, 0.8, 0.28, stone);
  const a2 = limb(body, 0, 1.0, -0.75, 0.8, 0.28, stone);
  part(cachedGeo('bg_fist', () => new THREE.DodecahedronGeometry(0.25, 0)), stone, 0, -0.85, 0, a1);
  part(cachedGeo('bg_fist', () => new THREE.DodecahedronGeometry(0.25, 0)), stone, 0, -0.85, 0, a2);
  return new RigView({ root, body, head, legs, arms: [a1, a2], height: 2.2, stride: 1.2 });
}

export function scuttleModel(): RigView {
  const root = new THREE.Group();
  const shell = material(0x3a8a9a, { rough: 0.5, metal: 0.2 });
  const body = pivot(0, 0.32, 0, root);
  part(cachedGeo('sc_shell', () => new THREE.SphereGeometry(0.5, 8, 5, 0, Math.PI * 2, 0, Math.PI / 2).scale(1.2, 0.6, 1)), shell, 0, 0, 0, body);
  part(cachedGeo('sc_glow', () => new THREE.IcosahedronGeometry(0.12, 0)), glow(0x9ff6ff, 2), 0, 0.32, 0, body).castShadow = false;
  const head = pivot(0.5, 0.05, 0, body);
  for (const z of [-0.15, 0.15]) part(cachedGeo('sc_eye', () => new THREE.CylinderGeometry(0.03, 0.03, 0.22, 4)), material(0x222222), 0, 0.12, z, head);
  const legs: THREE.Object3D[] = [];
  for (let i = 0; i < 6; i++) {
    const side = i < 3 ? 1 : -1;
    const x = ((i % 3) - 1) * 0.3;
    const l = limb(root, x, 0.32, side * 0.45, 0.4, 0.06, shell);
    l.rotation.x = side * 0.6;
    legs.push(l);
  }
  root.scale.setScalar(1.2);
  return new RigView({ root, body, head, legs, height: 0.7, stride: 4 });
}

export const DRAGON_COLORS: Record<DragonElement | 'elder', { body: number; accent: number }> = {
  infernal: { body: 0x9a2a1a, accent: 0xff7a1a },
  ocean: { body: 0x1a5a8a, accent: 0x4ff0e0 },
  mountain: { body: 0x6a5a40, accent: 0xd8b060 },
  cloud: { body: 0xa8c0d8, accent: 0xf0f8ff },
  hextech: { body: 0x2a3a6a, accent: 0x5ae0ff },
  chemtech: { body: 0x3a5a2a, accent: 0xa8ff3a },
  elder: { body: 0x3a2a5a, accent: 0xc89aff },
};

export function dragonModel(element: DragonElement | 'elder'): RigView {
  const col = DRAGON_COLORS[element];
  const { rig, body } = quadruped({ len: 1.8, h: 0.8, w: 0.9, legLen: 0.65, color: col.body, headSize: 0.55, snout: 0.45, tail: 1.6, eyes: col.accent });
  const acc = glow(col.accent, element === 'elder' ? 2 : 1.3);
  const wingMat = material(col.body, { rough: 0.6 });
  const wings: THREE.Object3D[] = [];
  for (const side of [1, -1]) {
    const w = pivot(0.2, 0.75, side * 0.45, body);
    const g = cachedGeo(`dr_wing_${side}`, () => {
      const geo = new THREE.BufferGeometry();
      const pts = new Float32Array([0, 0, 0, -1.2, 0.1, side * 1.6, 0.5, 0.2, side * 1.3, 0, 0, 0, -1.4, -0.1, side * 0.8, -1.2, 0.1, side * 1.6]);
      geo.setAttribute('position', new THREE.BufferAttribute(pts, 3));
      geo.computeVertexNormals();
      return geo;
    });
    const wingMesh = part(g, wingMat, 0, 0, 0, w);
    (wingMesh.material as THREE.MeshStandardMaterial).side = THREE.DoubleSide;
    wings.push(w);
  }
  // crête lumineuse sur le dos
  for (let i = 0; i < 5; i++) part(cachedGeo('dr_spike', () => new THREE.ConeGeometry(0.08, 0.3, 4)), acc, 0.6 - i * 0.3, 0.9, 0, body).castShadow = false;
  if (rig.head) for (const z of [-0.18, 0.18]) part(cachedGeo('dr_horn', () => new THREE.ConeGeometry(0.06, 0.4, 4)), acc, 0.1, 0.38, z, rig.head).rotation.z = 0.7;
  rig.wings = wings;
  rig.stride = 1.4;
  rig.root.scale.setScalar(element === 'elder' ? 1.45 : 1.2);
  return new RigView(rig);
}

export function grubModel(): RigView {
  const root = new THREE.Group();
  const skin = material(0x7a4ab0, { rough: 0.7 });
  const body = pivot(0, 0, 0, root);
  const segs: THREE.Object3D[] = [];
  for (let i = 0; i < 4; i++) {
    const s = 0.24 - i * 0.03;
    segs.push(part(cachedGeo(`gb_seg_${i}`, () => new THREE.SphereGeometry(s, 6, 5)), skin, -i * 0.28, s, 0, body));
  }
  const head = pivot(0.2, 0.28, 0, body);
  part(cachedGeo('gb_eye', () => new THREE.SphereGeometry(0.06, 5, 4)), glow(0xf0a0ff, 2.5), 0.1, 0.05, 0, head).castShadow = false;
  // les segments ondulent : on les traite comme « jambes » (rotation z sur leur centre)
  return new RigView({ root, body, head, legs: segs, height: 0.5, stride: 2.5 });
}

/** Colosse du Rift : grand scarabée cuirassé à œil unique. */
export function heraldModel(): RigView {
  const root = new THREE.Group();
  const shell = material(0x4a2a6a, { rough: 0.45, metal: 0.3 });
  const plate = material(0x7a5aa0, { rough: 0.5, metal: 0.3 });
  const eye = glow(0xff4ad8, 3);
  const legs: THREE.Object3D[] = [];
  for (let i = 0; i < 6; i++) {
    const side = i < 3 ? 1 : -1;
    const l = limb(root, ((i % 3) - 1) * 0.6, 0.8, side * 0.8, 0.85, 0.16, shell);
    l.rotation.x = side * 0.45;
    legs.push(l);
  }
  const body = pivot(0, 0.75, 0, root);
  part(cachedGeo('hr_shell', () => new THREE.SphereGeometry(1, 9, 6, 0, Math.PI * 2, 0, Math.PI / 2).scale(1.35, 0.9, 1)), shell, 0, 0, 0, body);
  for (let i = 0; i < 3; i++) part(cachedGeo('hr_plate', () => new THREE.BoxGeometry(0.2, 0.15, 1.6)), plate, -0.6 + i * 0.5, 0.82 - Math.abs(i - 1) * 0.12, 0, body);
  const head = pivot(1.15, 0.25, 0, body);
  part(cachedGeo('hr_head', () => new THREE.DodecahedronGeometry(0.45, 0)), plate, 0, 0, 0, head);
  part(cachedGeo('hr_eye', () => new THREE.SphereGeometry(0.2, 8, 6)), eye, 0.35, 0.05, 0, head).castShadow = false;
  for (const z of [-0.3, 0.3]) part(cachedGeo('hr_mandible', () => new THREE.ConeGeometry(0.08, 0.6, 4).rotateZ(-Math.PI / 2)), plate, 0.45, -0.25, z, head);
  root.scale.setScalar(1.3);
  return new RigView({ root, body, head, legs, height: 2.2, stride: 1.6 });
}

/** Léviathan : serpent abyssal géant dressé, tête cornue. */
export function leviathanModel(): RigView {
  const root = new THREE.Group();
  const skin = material(0x2a1a4a, { rough: 0.55, metal: 0.15 });
  const scale = material(0x5a3a8a, { rough: 0.5 });
  const acc = glow(0xb06aff, 2);
  const body = pivot(0, 0, 0, root);
  // corps en arc : anneaux de tailles décroissantes
  const segs = 9;
  const spine: THREE.Object3D[] = [];
  for (let i = 0; i < segs; i++) {
    const t = i / (segs - 1);
    const r = 0.75 - t * 0.35;
    const x = -1.8 + t * 2.6;
    const y = 0.5 + Math.sin(t * Math.PI * 0.85) * 2.2;
    const seg = part(cachedGeo(`lv_seg_${i}`, () => new THREE.DodecahedronGeometry(r, 0)), i % 2 ? scale : skin, x, y, 0, body);
    spine.push(seg);
    if (i % 2 === 0) part(cachedGeo('lv_fin', () => new THREE.ConeGeometry(0.12, 0.55, 4)), acc, x, y + r * 0.9, 0, body).castShadow = false;
  }
  const head = pivot(1.1, 2.6, 0, body);
  part(cachedGeo('lv_head', () => new THREE.BoxGeometry(1.1, 0.6, 0.8)), skin, 0.3, 0, 0, head);
  part(cachedGeo('lv_jaw', () => new THREE.BoxGeometry(0.9, 0.2, 0.7)), scale, 0.4, -0.35, 0, head);
  for (const z of [-0.3, 0.3]) {
    part(cachedGeo('lv_eye', () => new THREE.BoxGeometry(0.1, 0.1, 0.12)), glow(0xffe04a, 3), 0.86, 0.12, z, head).castShadow = false;
    part(cachedGeo('lv_horn', () => new THREE.ConeGeometry(0.12, 0.9, 5)), material(0xd8cfb8), -0.1, 0.5, z, head).rotation.z = 0.9;
  }
  // tentacules à la base = « jambes »
  const legs: THREE.Object3D[] = [];
  for (let i = 0; i < 4; i++) {
    const a = (i / 4) * Math.PI * 2 + 0.4;
    const l = limb(root, -1.8 + Math.cos(a) * 0.7, 0.6, Math.sin(a) * 0.7, 0.9, 0.18, scale);
    l.rotation.x = Math.sin(a) * 0.7;
    legs.push(l);
  }
  root.scale.setScalar(1.35);
  return new RigView({ root, body, head, legs, height: 4, stride: 0.8, floaters: [head] });
}

export function monsterModel(subtype: string | undefined, element?: DragonElement): RigView | null {
  switch (subtype) {
    case 'wolves':
      return wolfModel(false);
    case 'wolf_small':
      return wolfModel(true);
    case 'raptors':
      return raptorModel(false);
    case 'raptor_small':
      return raptorModel(true);
    case 'krugs':
      return krugModel(false);
    case 'krug_small':
      return krugModel(true);
    case 'gromp':
      return grompModel();
    case 'blue_buff':
      return buffGolemModel('blue');
    case 'red_buff':
      return buffGolemModel('red');
    case 'scuttle':
      return scuttleModel();
    case 'dragon':
      return dragonModel(element ?? 'infernal');
    case 'elder_dragon':
      return dragonModel('elder');
    case 'grub':
      return grubModel();
    case 'herald':
      return heraldModel();
    case 'leviathan':
      return leviathanModel();
    default:
      return null;
  }
}
