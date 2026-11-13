/**
 * Modèles low-poly des structures (tours, inhibiteurs, nexus) et balises. Propriétaire : claude-client.
 */
import * as THREE from 'three';
import { RigView, cachedGeo, glow, material, part, pivot, type Rig } from './rig.ts';

export type TeamSide = 'blue' | 'red' | 'neutral';

export const TEAM_HEX: Record<TeamSide, number> = { blue: 0x3f8cff, red: 0xff4a3a, neutral: 0xe0b040 };
/** Teinte de pierre par équipe (bleu : pierre claire froide, rouge : pierre sombre chaude). */
const STONE: Record<TeamSide, number> = { blue: 0xa7a9a6, red: 0x7c6458, neutral: 0x8f8a7c };
const TRIM: Record<TeamSide, number> = { blue: 0xd9c38a, red: 0x8f5a3a, neutral: 0xb8a070 };

export function towerModel(team: TeamSide, tier = 'outer'): RigView {
  const root = new THREE.Group();
  const stone = material(STONE[team], { rough: 0.9 });
  const dark = material(0x4a4642, { rough: 1 });
  const trim = material(TRIM[team], { rough: 0.5, metal: 0.4 });
  const c = TEAM_HEX[team];
  const scale = tier === 'nexus' ? 0.9 : 1;

  part(cachedGeo('tw_foot', () => new THREE.CylinderGeometry(1.25, 1.45, 0.45, 8)), dark, 0, 0.22, 0, root);
  part(cachedGeo('tw_base', () => new THREE.CylinderGeometry(1.0, 1.2, 0.9, 8)), stone, 0, 0.9, 0, root);
  part(cachedGeo('tw_band1', () => new THREE.CylinderGeometry(1.06, 1.06, 0.14, 8)), trim, 0, 1.38, 0, root);
  part(cachedGeo('tw_shaft', () => new THREE.CylinderGeometry(0.62, 0.9, 2.6, 8)), stone, 0, 2.75, 0, root);
  part(cachedGeo('tw_band2', () => new THREE.CylinderGeometry(0.7, 0.7, 0.14, 8)), trim, 0, 3.6, 0, root);
  part(cachedGeo('tw_head', () => new THREE.CylinderGeometry(1.0, 0.68, 0.6, 8)), stone, 0, 4.3, 0, root);
  // 4 griffes qui tiennent le cristal
  for (let i = 0; i < 4; i++) {
    const a = (i / 4) * Math.PI * 2 + Math.PI / 4;
    const claw = part(cachedGeo('tw_claw', () => new THREE.ConeGeometry(0.14, 1.1, 4)), trim, Math.cos(a) * 0.72, 5.0, Math.sin(a) * 0.72, root);
    claw.rotation.set(Math.sin(a) * 0.35, 0, -Math.cos(a) * 0.35);
  }
  const crystalPivot = pivot(0, 5.35, 0, root);
  const crystal = part(cachedGeo('tw_crystal', () => new THREE.OctahedronGeometry(0.55, 0).scale(1, 1.5, 1)), glow(c, 1.8), 0, 0, 0, crystalPivot);
  crystal.castShadow = false;
  root.scale.setScalar(scale);
  const rig: Rig = { root, height: 6, spinners: [crystalPivot], floaters: [crystalPivot], structure: true };
  return new RigView(rig);
}

export function inhibitorModel(team: TeamSide): RigView {
  const root = new THREE.Group();
  const stone = material(STONE[team], { rough: 0.9 });
  const trim = material(TRIM[team], { rough: 0.5, metal: 0.4 });
  const c = TEAM_HEX[team];
  part(cachedGeo('ih_base', () => new THREE.CylinderGeometry(1.6, 1.8, 0.4, 10)), stone, 0, 0.2, 0, root);
  part(cachedGeo('ih_step', () => new THREE.CylinderGeometry(1.2, 1.4, 0.3, 10)), trim, 0, 0.55, 0, root);
  for (let i = 0; i < 5; i++) {
    const a = (i / 5) * Math.PI * 2;
    const p = part(cachedGeo('ih_post', () => new THREE.BoxGeometry(0.22, 1.2, 0.22)), stone, Math.cos(a) * 1.35, 1.0, Math.sin(a) * 1.35, root);
    p.rotation.y = -a;
  }
  const core = pivot(0, 1.7, 0, root);
  part(cachedGeo('ih_core', () => new THREE.IcosahedronGeometry(0.55, 0)), glow(c, 2), 0, 0, 0, core).castShadow = false;
  const ring = part(cachedGeo('ih_ring', () => new THREE.TorusGeometry(0.95, 0.07, 6, 28)), glow(c, 1.2), 0, 0, 0, core);
  ring.rotation.x = Math.PI / 2.4;
  return new RigView({ root, height: 2.4, spinners: [core], floaters: [core], structure: true });
}

export function nexusModel(team: TeamSide): RigView {
  const root = new THREE.Group();
  const stone = material(STONE[team], { rough: 0.9 });
  const trim = material(TRIM[team], { rough: 0.45, metal: 0.5 });
  const c = TEAM_HEX[team];
  part(cachedGeo('nx_base', () => new THREE.CylinderGeometry(2.6, 3.0, 0.6, 12)), stone, 0, 0.3, 0, root);
  part(cachedGeo('nx_step', () => new THREE.CylinderGeometry(2.1, 2.4, 0.5, 12)), trim, 0, 0.85, 0, root);
  part(cachedGeo('nx_ped', () => new THREE.CylinderGeometry(1.2, 1.7, 0.9, 8)), stone, 0, 1.5, 0, root);
  for (let i = 0; i < 6; i++) {
    const a = (i / 6) * Math.PI * 2;
    const spike = part(cachedGeo('nx_spike', () => new THREE.ConeGeometry(0.28, 2.2, 5)), trim, Math.cos(a) * 2.0, 1.9, Math.sin(a) * 2.0, root);
    spike.rotation.set(Math.sin(a) * 0.25, 0, -Math.cos(a) * 0.25);
  }
  const core = pivot(0, 3.4, 0, root);
  part(cachedGeo('nx_core', () => new THREE.OctahedronGeometry(1.1, 0).scale(1, 1.6, 1)), glow(c, 2), 0, 0, 0, core).castShadow = false;
  const orbit = pivot(0, 3.4, 0, root);
  for (let i = 0; i < 4; i++) {
    const a = (i / 4) * Math.PI * 2;
    part(cachedGeo('nx_shard', () => new THREE.OctahedronGeometry(0.28, 0).scale(1, 1.8, 1)), glow(c, 1.4), Math.cos(a) * 1.8, Math.sin(i * 2) * 0.3, Math.sin(a) * 1.8, orbit).castShadow = false;
  }
  return new RigView({ root, height: 4.5, spinners: [core, orbit], floaters: [core], structure: true });
}

export function wardModel(team: TeamSide): RigView {
  const root = new THREE.Group();
  const c = TEAM_HEX[team];
  part(cachedGeo('wd_post', () => new THREE.CylinderGeometry(0.04, 0.06, 0.7, 5)), material(0x5a4632), 0, 0.35, 0, root);
  const lantern = pivot(0, 0.8, 0, root);
  part(cachedGeo('wd_gem', () => new THREE.OctahedronGeometry(0.14, 0).scale(1, 1.4, 1)), glow(c, 2.2), 0, 0, 0, lantern).castShadow = false;
  return new RigView({ root, height: 0.9, spinners: [lantern], floaters: [lantern], structure: true });
}
