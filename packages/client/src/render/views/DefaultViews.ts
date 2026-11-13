/**
 * Vues par défaut des entités (formes simples), en attendant les modèles low-poly (M2) et les champions
 * de claude-fx. Propriétaire : claude-client.
 * Géométries et matériaux partagés (pas d'allocation par entité au-delà des Mesh).
 */
import * as THREE from 'three';
import { CHAMPIONS, type EntitySnapshot } from '@lov/shared';
import type { EntityView } from '../EntityView.ts';

export const TEAM_COLOR = { blue: 0x3a7bff, red: 0xff4a4a, neutral: 0xc9a227 } as const;

const geoCache = new Map<string, THREE.BufferGeometry>();
const matCache = new Map<string, THREE.Material>();

function geo(key: string, make: () => THREE.BufferGeometry): THREE.BufferGeometry {
  let g = geoCache.get(key);
  if (!g) {
    g = make();
    geoCache.set(key, g);
  }
  return g;
}
export function mat(color: number | string, opts: { emissive?: number; flat?: boolean } = {}): THREE.Material {
  const key = `${color}|${opts.emissive ?? 0}|${opts.flat ? 1 : 0}`;
  let m = matCache.get(key);
  if (!m) {
    m = new THREE.MeshStandardMaterial({ color, roughness: 0.75, metalness: 0.05, flatShading: opts.flat ?? true, emissive: opts.emissive ?? 0 });
    matCache.set(key, m);
  }
  return m;
}

/** Vue simple : un groupe avec un corps, animations procédurales génériques. */
export class SimpleView implements EntityView {
  readonly object3d = new THREE.Group();
  protected body: THREE.Object3D;
  private anim = 'idle';
  private t = 0;
  private deathT = 0;

  constructor(body: THREE.Object3D, protected readonly bodyHeight: number) {
    this.body = body;
    body.traverse((o) => {
      if ((o as THREE.Mesh).isMesh) {
        o.castShadow = true;
        o.receiveShadow = true;
      }
    });
    this.object3d.add(body);
  }

  update(snap: EntitySnapshot, dt: number): void {
    this.t += dt;
    const b = this.body;
    if (snap.dead) {
      this.deathT = Math.min(1, this.deathT + dt * 1.5);
      b.position.y = -this.bodyHeight * this.deathT;
      b.rotation.z = this.deathT * 0.6;
      return;
    }
    this.deathT = 0;
    b.rotation.z = 0;
    switch (this.anim) {
      case 'run':
        b.position.y = Math.abs(Math.sin(this.t * 10)) * 0.08;
        b.rotation.x = Math.sin(this.t * 10) * 0.05;
        b.position.x = 0;
        break;
      case 'attack':
      case 'cast_Q':
      case 'cast_W':
      case 'cast_E':
      case 'cast_R':
        b.position.y = 0;
        b.position.x = Math.max(0, Math.sin(this.t * 12)) * 0.12;
        break;
      default:
        b.position.y = Math.sin(this.t * 2) * 0.02;
        b.position.x = 0;
        b.rotation.x = 0;
    }
  }

  playAnim(name: string): void {
    if (name !== this.anim) this.t = 0;
    this.anim = name;
  }

  dispose(): void {
    this.object3d.removeFromParent();
  }
}

function mesh(g: THREE.BufferGeometry, m: THREE.Material, y = 0): THREE.Mesh {
  const me = new THREE.Mesh(g, m);
  me.position.y = y;
  return me;
}

/** Capsule colorée + « nez » indiquant l'orientation (+x). */
export function championPlaceholder(snap: EntitySnapshot): EntityView {
  const def = snap.subtype ? CHAMPIONS[snap.subtype] : undefined;
  const color = def?.palette.primary ?? '#cccccc';
  const team = TEAM_COLOR[snap.team];
  const g = new THREE.Group();
  g.add(mesh(geo('champ', () => new THREE.CapsuleGeometry(0.42, 0.9, 4, 10)), mat(color), 0.9));
  g.add(mesh(geo('champ_nose', () => new THREE.ConeGeometry(0.16, 0.4, 6).rotateZ(-Math.PI / 2)), mat(def?.palette.accent ?? '#ffffff'), 1.25).translateX(0.45));
  const ring = mesh(geo('team_ring', () => new THREE.TorusGeometry(0.55, 0.05, 6, 24).rotateX(Math.PI / 2)), mat(team, { emissive: team }), 0.05);
  g.add(ring);
  return new SimpleView(g, 1.8);
}

export function minionView(snap: EntitySnapshot): EntityView {
  const team = TEAM_COLOR[snap.team];
  const g = new THREE.Group();
  switch (snap.subtype) {
    case 'caster':
      g.add(mesh(geo('m_caster', () => new THREE.ConeGeometry(0.28, 0.75, 6)), mat(team), 0.38));
      g.add(mesh(geo('m_orb', () => new THREE.IcosahedronGeometry(0.12)), mat(0xfff2b0, { emissive: 0xffd35a }), 0.85));
      break;
    case 'cannon':
      g.add(mesh(geo('m_cannon', () => new THREE.BoxGeometry(0.8, 0.45, 0.6)), mat(team), 0.35));
      g.add(mesh(geo('m_barrel', () => new THREE.CylinderGeometry(0.1, 0.12, 0.6, 8).rotateZ(Math.PI / 2)), mat(0x444444), 0.6).translateX(0.35));
      break;
    case 'super':
      g.add(mesh(geo('m_super', () => new THREE.BoxGeometry(0.75, 1.0, 0.75)), mat(team), 0.5));
      break;
    default:
      g.add(mesh(geo('m_melee', () => new THREE.BoxGeometry(0.45, 0.55, 0.45)), mat(team), 0.28));
  }
  return new SimpleView(g, 1);
}

export function towerView(team: EntitySnapshot['team']): EntityView {
  const c = TEAM_COLOR[team];
  const g = new THREE.Group();
  g.add(mesh(geo('t_base', () => new THREE.CylinderGeometry(0.9, 1.1, 0.6, 8)), mat(0x6b6458), 0.3));
  g.add(mesh(geo('t_shaft', () => new THREE.CylinderGeometry(0.5, 0.75, 3.6, 8)), mat(0x8a8274), 2.4));
  g.add(mesh(geo('t_crystal', () => new THREE.OctahedronGeometry(0.55)), mat(c, { emissive: c }), 4.8));
  return new SimpleView(g, 5.2);
}

export function inhibitorView(team: EntitySnapshot['team']): EntityView {
  const c = TEAM_COLOR[team];
  const g = new THREE.Group();
  g.add(mesh(geo('i_base', () => new THREE.CylinderGeometry(1.3, 1.5, 0.4, 10)), mat(0x5b564c), 0.2));
  g.add(mesh(geo('i_ring', () => new THREE.TorusGeometry(0.9, 0.18, 8, 20)), mat(c, { emissive: c }), 1.4));
  return new SimpleView(g, 2);
}

export function nexusView(team: EntitySnapshot['team']): EntityView {
  const c = TEAM_COLOR[team];
  const g = new THREE.Group();
  g.add(mesh(geo('n_base', () => new THREE.CylinderGeometry(2.2, 2.6, 0.8, 10)), mat(0x5b564c), 0.4));
  g.add(mesh(geo('n_core', () => new THREE.OctahedronGeometry(1.4)), mat(c, { emissive: c }), 2.4));
  return new SimpleView(g, 3.4);
}

const MONSTER_SIZE: Record<string, number> = {
  dragon: 2.2,
  elder_dragon: 2.6,
  leviathan: 3.2,
  herald: 2.2,
  blue_buff: 1.3,
  red_buff: 1.3,
  gromp: 1.1,
  wolves: 0.9,
  raptors: 0.8,
  krugs: 1.1,
  scuttle: 0.9,
  grub: 0.6,
};

export function monsterView(snap: EntitySnapshot): EntityView {
  const size = MONSTER_SIZE[snap.subtype ?? ''] ?? 0.5;
  const color = snap.subtype === 'blue_buff' ? 0x4aa3ff : snap.subtype === 'red_buff' ? 0xff6a3a : 0x9a8a5a;
  const g = new THREE.Group();
  const body = mesh(geo('mon', () => new THREE.DodecahedronGeometry(0.5)), mat(color), size * 0.5);
  body.scale.setScalar(size);
  g.add(body);
  return new SimpleView(g, size);
}

export function wardView(snap: EntitySnapshot): EntityView {
  const c = TEAM_COLOR[snap.team];
  const g = new THREE.Group();
  g.add(mesh(geo('ward', () => new THREE.ConeGeometry(0.15, 0.6, 5)), mat(c, { emissive: c }), 0.3));
  return new SimpleView(g, 0.6);
}

/** Vue par défaut selon le type d'entité (null = pas de vue : projectiles et zones sont gérés par les FX). */
export function defaultViewFor(snap: EntitySnapshot): EntityView | null {
  switch (snap.kind) {
    case 'champion':
      return championPlaceholder(snap);
    case 'minion':
      return minionView(snap);
    case 'tower':
      return towerView(snap.team);
    case 'inhibitor':
      return inhibitorView(snap.team);
    case 'nexus':
      return nexusView(snap.team);
    case 'monster':
      return monsterView(snap);
    case 'ward':
      return wardView(snap);
    default:
      return null;
  }
}
