/**
 * Rig procédural générique et vue animée pour les modèles low-poly. Propriétaire : claude-client.
 *
 * Un modèle se compose de pivots nommés (jambes, bras, ailes, queue…) : les géométries sont décalées pour que
 * chaque pivot soit à l'articulation, et l'animateur fait tourner les pivots selon l'animation courante
 * (idle / run / attack / cast_* / death). Le modèle regarde +x local.
 */
import * as THREE from 'three';
import type { EntitySnapshot } from '@lov/shared';
import type { EntityView } from '../EntityView.ts';

// ---------------------------------------------------------------- caches

const geoCache = new Map<string, THREE.BufferGeometry>();
const matCache = new Map<string, THREE.Material>();

export function cachedGeo(key: string, make: () => THREE.BufferGeometry): THREE.BufferGeometry {
  let g = geoCache.get(key);
  if (!g) {
    g = make();
    geoCache.set(key, g);
  }
  return g;
}

export function material(color: number, opts: { emissive?: number; intensity?: number; rough?: number; metal?: number } = {}): THREE.Material {
  const key = `${color}|${opts.emissive ?? 0}|${opts.intensity ?? 1}|${opts.rough ?? 0.8}|${opts.metal ?? 0}`;
  let m = matCache.get(key);
  if (!m) {
    m = new THREE.MeshStandardMaterial({
      color,
      roughness: opts.rough ?? 0.8,
      metalness: opts.metal ?? 0,
      flatShading: true,
      emissive: opts.emissive ?? 0,
      emissiveIntensity: opts.intensity ?? 1,
    });
    if (!opts.emissive) addRim(m as THREE.MeshStandardMaterial);
    matCache.set(key, m);
  }
  return m;
}

/**
 * Liseré de lumière (rim) : les bords tournés à l'opposé de la caméra s'éclaircissent légèrement, ce qui
 * détache les silhouettes du sol en vue plongeante (quelques instructions par pixel, aucun passage en plus).
 */
function addRim(m: THREE.MeshStandardMaterial): void {
  m.onBeforeCompile = (sh) => {
    sh.fragmentShader = sh.fragmentShader.replace(
      '#include <emissivemap_fragment>',
      `#include <emissivemap_fragment>
      float rimF = 1.0 - clamp(dot(normal, normalize(vViewPosition)), 0.0, 1.0);
      totalEmissiveRadiance += diffuseColor.rgb * vec3(0.95, 0.97, 1.1) * rimF * rimF * rimF * 0.6;`,
    );
  };
  m.customProgramCacheKey = () => 'rim';
}

export function glow(color: number, intensity = 1.6): THREE.Material {
  return material(color, { emissive: color, intensity, rough: 0.4 });
}

/** Crée un mesh positionné. */
export function part(geo: THREE.BufferGeometry, mat: THREE.Material, x = 0, y = 0, z = 0, parent?: THREE.Object3D): THREE.Mesh {
  const m = new THREE.Mesh(geo, mat);
  m.position.set(x, y, z);
  m.castShadow = true;
  m.receiveShadow = true;
  parent?.add(m);
  return m;
}

/** Pivot (articulation) vide positionné. */
export function pivot(x: number, y: number, z: number, parent: THREE.Object3D): THREE.Group {
  const g = new THREE.Group();
  g.position.set(x, y, z);
  parent.add(g);
  return g;
}

// ---------------------------------------------------------------- rig

export interface Rig {
  /** Racine du modèle animé (enfant de object3d). */
  root: THREE.Group;
  /** Corps : rebond / inclinaison. */
  body?: THREE.Object3D;
  head?: THREE.Object3D;
  /** Jambes : balancées en opposition (rotation autour de z local = avant/arrière quand on regarde +x). */
  legs?: THREE.Object3D[];
  arms?: THREE.Object3D[];
  /** Bras/arme qui frappe pendant l'attaque (sinon arms[0]). */
  weapon?: THREE.Object3D;
  wings?: THREE.Object3D[];
  tail?: THREE.Object3D;
  /** Éléments qui tournent en continu (cristaux). */
  spinners?: THREE.Object3D[];
  /** Éléments qui flottent (rebond sinusoïdal). */
  floaters?: THREE.Object3D[];
  /** Hauteur totale (pour la mort/enfoncement). */
  height: number;
  /** Fréquence de pas (cycles / s) en course. */
  stride?: number;
  /** Quadrupède : jambes avant/arrière alternées en diagonale. */
  quadruped?: boolean;
  /** Pas d'animation de chute à la mort (structures : effondrement). */
  structure?: boolean;
}

const _base = new WeakMap<THREE.Object3D, number>();
const _baseX = new WeakMap<THREE.Object3D, number>();
function baseX(o: THREE.Object3D): number {
  let x = _baseX.get(o);
  if (x === undefined) {
    x = o.position.x;
    _baseX.set(o, x);
  }
  return x;
}
function baseY(o: THREE.Object3D): number {
  let y = _base.get(o);
  if (y === undefined) {
    y = o.position.y;
    _base.set(o, y);
  }
  return y;
}

export class RigView implements EntityView {
  readonly object3d = new THREE.Group();
  private anim = 'idle';
  private t = Math.random() * 10;
  private animT = 0;
  private deathT = 0;
  private moveBlend = 0;

  constructor(readonly rig: Rig) {
    this.object3d.add(rig.root);
    for (const f of rig.floaters ?? []) baseY(f);
    if (rig.body) baseY(rig.body);
    if (rig.head) baseX(rig.head);
  }

  playAnim(name: string): void {
    if (name !== this.anim) this.animT = 0;
    this.anim = name;
  }

  update(snap: EntitySnapshot, dt: number): void {
    const r = this.rig;
    this.t += dt;
    this.animT += dt;
    const dead = !!snap.dead;

    for (const s of r.spinners ?? []) s.rotation.y += dt * (dead ? 0 : 0.9);
    for (const [i, f] of (r.floaters ?? []).entries()) f.position.y = baseY(f) + (dead ? -0.3 : Math.sin(this.t * 1.6 + i) * 0.12);

    if (dead) {
      this.deathT = Math.min(1, this.deathT + dt * (r.structure ? 0.6 : 1.4));
      const k = this.deathT * this.deathT;
      if (r.structure) {
        r.root.position.y = -r.height * 0.55 * k;
        r.root.rotation.z = 0.12 * k;
        r.root.rotation.x = 0.08 * k;
      } else {
        r.root.rotation.x = (Math.PI / 2) * Math.min(1, this.deathT * 1.6);
        r.root.position.y = -r.height * 0.6 * Math.max(0, this.deathT - 0.5) * 2;
      }
      return;
    }
    if (this.deathT > 0) {
      this.deathT = 0;
      r.root.rotation.set(0, 0, 0);
      r.root.position.y = 0;
    }

    const running = this.anim === 'run';
    this.moveBlend += ((running ? 1 : 0) - this.moveBlend) * Math.min(1, dt * 10);
    const mb = this.moveBlend;
    const stride = r.stride ?? 2.2;
    const ph = this.t * stride * Math.PI * 2;

    // jambes
    if (r.legs) {
      r.legs.forEach((leg, i) => {
        const phase = r.quadruped ? (i % 2 === 0 ? 0 : Math.PI) + (i >= 2 ? Math.PI : 0) : i % 2 === 0 ? 0 : Math.PI;
        leg.rotation.z = Math.sin(ph + phase) * 0.6 * mb;
      });
    }
    // corps : respiration / rebond de course
    if (r.body) {
      const breathe = Math.sin(this.t * 2) * 0.02;
      r.body.position.y = baseY(r.body) + breathe * (1 - mb) + Math.abs(Math.sin(ph)) * 0.06 * mb;
      r.body.rotation.z = -0.08 * mb;
    }
    // bras : balancier en course
    if (r.arms) {
      r.arms.forEach((arm, i) => {
        arm.rotation.z = Math.sin(ph + (i % 2 ? 0 : Math.PI)) * 0.5 * mb;
      });
    }
    if (r.wings) {
      const flap = Math.sin(this.t * (running ? 7 : 2.2)) * (running ? 0.5 : 0.18);
      r.wings.forEach((w, i) => (w.rotation.x = (i % 2 ? -1 : 1) * (0.25 + flap)));
    }
    if (r.tail) r.tail.rotation.y = Math.sin(this.t * 3) * 0.3;
    if (r.head) {
      r.head.rotation.z = Math.sin(this.t * 1.3) * 0.05;
      r.head.position.x = baseX(r.head);
    }

    // attaque / incantation : frappe avec l'arme
    const attacking = this.anim === 'attack' || this.anim.startsWith('cast');
    if (attacking) {
      const k = Math.min(1, this.animT / 0.35);
      const swing = Math.sin(k * Math.PI);
      const w = r.weapon ?? r.arms?.[0];
      if (w) w.rotation.z = -1.6 * swing;
      if (r.body) r.body.rotation.z = 0.15 * swing;
      if (r.head && !r.weapon && !r.arms) r.head.position.x = baseX(r.head) + 0.15 * swing; // morsure
      if (this.animT > 0.6) this.animT = 0; // boucle si l'animation reste « attack »
    }
  }

  dispose(): void {
    this.object3d.removeFromParent();
  }
}
