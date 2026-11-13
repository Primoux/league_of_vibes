/**
 * Vues par défaut des entités (formes simples), en attendant les modèles low-poly (M2) et les champions
 * de claude-fx. Propriétaire : claude-client.
 * Géométries et matériaux partagés (pas d'allocation par entité au-delà des Mesh).
 */
import * as THREE from 'three';
import { CHAMPIONS, type DragonElement, type EntitySnapshot } from '@lov/shared';
import { minionModel } from '../models/minions.ts';
import { monsterModel } from '../models/monsters.ts';
import { inhibitorModel, nexusModel, towerModel, wardModel } from '../models/structures.ts';
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

/** Élément du prochain drake (mis à jour depuis l'événement objectiveSpawn). */
let dragonElement: DragonElement | undefined;
export function setDragonElement(e: DragonElement | undefined): void {
  dragonElement = e;
}

/** Vue par défaut selon le type d'entité (null = pas de vue : projectiles et zones sont gérés par les FX). */
export function defaultViewFor(snap: EntitySnapshot): EntityView | null {
  switch (snap.kind) {
    case 'champion':
      return championPlaceholder(snap);
    case 'minion':
      return minionModel(snap.subtype, snap.team);
    case 'tower':
      return towerModel(snap.team, snap.subtype);
    case 'inhibitor':
      return inhibitorModel(snap.team);
    case 'nexus':
      return nexusModel(snap.team);
    case 'monster':
      return monsterModel(snap.subtype, dragonElement) ?? new SimpleView(new THREE.Mesh(new THREE.DodecahedronGeometry(0.4), mat(0x9a8a5a)), 0.8);
    case 'ward':
      return wardModel(snap.team);
    default:
      return null;
  }
}
