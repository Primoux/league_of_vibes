/**
 * Modèles low-poly des sbires (mêlée, lanceur, canon, super-sbire), couleur d'équipe. Propriétaire : claude-client.
 */
import * as THREE from 'three';
import { RigView, cachedGeo, glow, material, part, pivot, type Rig } from './rig.ts';
import { TEAM_HEX, type TeamSide } from './structures.ts';

const ARMOR: Record<TeamSide, number> = { blue: 0x3567c9, red: 0xb83a30, neutral: 0x9a8a5a };
const SKIN = 0xd8b48a;
const METAL = 0xb9bcc2;

/** Jambe : pivot à la hanche, géométrie vers le bas. */
function leg(parent: THREE.Object3D, x: number, y: number, z: number, len: number, w: number, mat: THREE.Material): THREE.Group {
  const p = pivot(x, y, z, parent);
  part(cachedGeo(`leg_${len}_${w}`, () => new THREE.BoxGeometry(w, len, w).translate(0, -len / 2, 0)), mat, 0, 0, 0, p);
  return p;
}
function arm(parent: THREE.Object3D, x: number, y: number, z: number, len: number, w: number, mat: THREE.Material): THREE.Group {
  const p = pivot(x, y, z, parent);
  part(cachedGeo(`arm_${len}_${w}`, () => new THREE.BoxGeometry(w, len, w).translate(0, -len / 2, 0)), mat, 0, 0, 0, p);
  return p;
}

export function meleeMinion(team: TeamSide): RigView {
  const root = new THREE.Group();
  const armor = material(ARMOR[team], { rough: 0.6, metal: 0.2 });
  const metal = material(METAL, { rough: 0.35, metal: 0.7 });
  const dark = material(0x3a3430);
  const legs = [leg(root, 0, 0.32, 0.1, 0.32, 0.12, dark), leg(root, 0, 0.32, -0.1, 0.32, 0.12, dark)];
  const body = pivot(0, 0.32, 0, root);
  part(cachedGeo('mm_torso', () => new THREE.BoxGeometry(0.3, 0.34, 0.36).translate(0, 0.17, 0)), armor, 0, 0, 0, body);
  const head = pivot(0, 0.42, 0, body);
  part(cachedGeo('mm_helm', () => new THREE.SphereGeometry(0.15, 6, 5)), armor, 0, 0.08, 0, head);
  part(cachedGeo('mm_crest', () => new THREE.ConeGeometry(0.05, 0.16, 4)), glow(TEAM_HEX[team], 1), 0, 0.26, 0, head);
  const sword = arm(body, 0.02, 0.3, -0.24, 0.24, 0.09, armor);
  part(cachedGeo('mm_blade', () => new THREE.BoxGeometry(0.05, 0.4, 0.03).translate(0, -0.38, 0)), metal, 0, 0, 0, sword).rotation.z = 0.3;
  const shieldArm = arm(body, 0.05, 0.3, 0.24, 0.22, 0.09, armor);
  part(cachedGeo('mm_shield', () => new THREE.CylinderGeometry(0.16, 0.16, 0.04, 6).rotateX(Math.PI / 2)), metal, 0.08, -0.2, 0.05, shieldArm);
  const rig: Rig = { root, body, head, legs, arms: [sword, shieldArm], weapon: sword, height: 0.9, stride: 2.4 };
  return new RigView(rig);
}

export function casterMinion(team: TeamSide): RigView {
  const root = new THREE.Group();
  const robe = material(ARMOR[team], { rough: 0.85 });
  const trim = material(0xe9d7a8, { rough: 0.6 });
  const legs = [leg(root, 0, 0.22, 0.08, 0.22, 0.09, material(0x3a3430)), leg(root, 0, 0.22, -0.08, 0.22, 0.09, material(0x3a3430))];
  const body = pivot(0, 0.18, 0, root);
  part(cachedGeo('mc_robe', () => new THREE.ConeGeometry(0.26, 0.6, 7).translate(0, 0.3, 0)), robe, 0, 0, 0, body);
  part(cachedGeo('mc_belt', () => new THREE.CylinderGeometry(0.17, 0.19, 0.05, 7)), trim, 0, 0.32, 0, body);
  const head = pivot(0, 0.62, 0, body);
  part(cachedGeo('mc_head', () => new THREE.SphereGeometry(0.12, 6, 5)), material(SKIN), 0, 0, 0, head);
  part(cachedGeo('mc_hood', () => new THREE.ConeGeometry(0.15, 0.3, 6)), robe, -0.02, 0.14, 0, head);
  const staffArm = arm(body, 0.05, 0.48, -0.2, 0.2, 0.07, robe);
  part(cachedGeo('mc_staff', () => new THREE.CylinderGeometry(0.02, 0.02, 0.8, 4).translate(0, -0.15, 0)), material(0x6a4a2a), 0.04, 0, 0, staffArm);
  part(cachedGeo('mc_orb', () => new THREE.IcosahedronGeometry(0.08, 0)), glow(TEAM_HEX[team], 2), 0.04, 0.27, 0, staffArm).castShadow = false;
  return new RigView({ root, body, head, legs, arms: [staffArm], weapon: staffArm, height: 0.9, stride: 2.2 });
}

export function cannonMinion(team: TeamSide): RigView {
  const root = new THREE.Group();
  const plate = material(ARMOR[team], { rough: 0.55, metal: 0.3 });
  const wood = material(0x6a4a2e);
  const iron = material(0x3d3d42, { rough: 0.4, metal: 0.7 });
  const body = pivot(0, 0.32, 0, root);
  part(cachedGeo('cn_cart', () => new THREE.BoxGeometry(0.85, 0.3, 0.6)), wood, 0, 0.05, 0, body);
  part(cachedGeo('cn_plate', () => new THREE.BoxGeometry(0.88, 0.1, 0.64)), plate, 0, 0.24, 0, body);
  const barrel = pivot(0.05, 0.4, 0, body);
  part(cachedGeo('cn_barrel', () => new THREE.CylinderGeometry(0.11, 0.15, 0.75, 8).rotateZ(-Math.PI / 2).translate(0.3, 0, 0)), iron, 0, 0, 0, barrel);
  barrel.rotation.z = 0.25;
  // roues = « jambes » (rotation continue simulée par le balancement)
  const legs: THREE.Object3D[] = [];
  for (const [x, z] of [[0.28, 0.33], [-0.28, 0.33], [0.28, -0.33], [-0.28, -0.33]] as const) {
    const w = pivot(x, 0.18, z, root);
    part(cachedGeo('cn_wheel', () => new THREE.CylinderGeometry(0.18, 0.18, 0.07, 8).rotateX(Math.PI / 2)), wood, 0, 0, 0, w);
    legs.push(w);
  }
  part(cachedGeo('cn_flag', () => new THREE.BoxGeometry(0.02, 0.22, 0.16).translate(0, 0, 0.08)), glow(TEAM_HEX[team], 0.8), -0.38, 0.75, 0, body);
  return new RigView({ root, body, legs, weapon: barrel, height: 0.9, stride: 3, quadruped: true });
}

export function superMinion(team: TeamSide): RigView {
  const root = new THREE.Group();
  const armor = material(ARMOR[team], { rough: 0.5, metal: 0.35 });
  const dark = material(0x2e2a28);
  const metal = material(METAL, { rough: 0.3, metal: 0.8 });
  const legs = [leg(root, 0, 0.55, 0.2, 0.55, 0.22, dark), leg(root, 0, 0.55, -0.2, 0.55, 0.22, dark)];
  const body = pivot(0, 0.55, 0, root);
  part(cachedGeo('sm_torso', () => new THREE.BoxGeometry(0.6, 0.7, 0.75).translate(0, 0.35, 0)), armor, 0, 0, 0, body);
  part(cachedGeo('sm_pauldron', () => new THREE.SphereGeometry(0.22, 6, 4)), metal, 0, 0.68, 0.42, body);
  part(cachedGeo('sm_pauldron', () => new THREE.SphereGeometry(0.22, 6, 4)), metal, 0, 0.68, -0.42, body);
  const head = pivot(0.05, 0.8, 0, body);
  part(cachedGeo('sm_head', () => new THREE.BoxGeometry(0.3, 0.26, 0.28)), armor, 0, 0.1, 0, head);
  part(cachedGeo('sm_eyes', () => new THREE.BoxGeometry(0.02, 0.05, 0.2)), glow(TEAM_HEX[team], 2.5), 0.16, 0.12, 0, head);
  const fist1 = arm(body, 0, 0.62, -0.5, 0.55, 0.2, armor);
  part(cachedGeo('sm_fist', () => new THREE.DodecahedronGeometry(0.17, 0)), metal, 0, -0.6, 0, fist1);
  const fist2 = arm(body, 0, 0.62, 0.5, 0.55, 0.2, armor);
  part(cachedGeo('sm_fist', () => new THREE.DodecahedronGeometry(0.17, 0)), metal, 0, -0.6, 0, fist2);
  return new RigView({ root, body, head, legs, arms: [fist1, fist2], weapon: fist1, height: 1.6, stride: 1.6 });
}

export function minionModel(subtype: string | undefined, team: TeamSide): RigView {
  switch (subtype) {
    case 'caster':
      return casterMinion(team);
    case 'cannon':
      return cannonMinion(team);
    case 'super':
      return superMinion(team);
    default:
      return meleeMinion(team);
  }
}
