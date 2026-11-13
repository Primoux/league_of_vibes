/**
 * Meshes instanciés animés (un draw call par système) : pics qui jaillissent du sol puis s'enfoncent,
 * débris balistiques qui rebondissent. Pierre (Grondak), glace (Ilyra), éclats divers.
 */
import * as THREE from 'three';
import { rng } from '../core/rng.ts';

export type BurstMode = 'spike' | 'debris' | 'static';

export interface BurstSpawn {
  pos: THREE.Vector3;
  mode: BurstMode;
  /** Taille (multiplicateur de la géométrie). */
  size: number;
  /** Durée de vie (s). */
  life: number;
  /** Vitesse initiale (débris). */
  vel?: THREE.Vector3;
  /** Inclinaison d'un pic vers l'extérieur (rad) et direction (rad, autour de y). */
  tilt?: number;
  heading?: number;
  /** Étirement vertical (pics). */
  stretch?: number;
  color?: THREE.Color;
  delay?: number;
}

const _m = new THREE.Matrix4();
const _q = new THREE.Quaternion();
const _e = new THREE.Euler();
const _p = new THREE.Vector3();
const _s = new THREE.Vector3();
const ZERO = new THREE.Matrix4().makeScale(0, 0, 0);

export class MeshBurstSystem {
  readonly mesh: THREE.InstancedMesh;
  private head = 0;
  private active = 0;
  private readonly px: Float32Array;
  private readonly py: Float32Array;
  private readonly pz: Float32Array;
  private readonly vx: Float32Array;
  private readonly vy: Float32Array;
  private readonly vz: Float32Array;
  private readonly rx: Float32Array;
  private readonly ry: Float32Array;
  private readonly rz: Float32Array;
  private readonly spin: Float32Array;
  private readonly age: Float32Array;
  private readonly life: Float32Array;
  private readonly size: Float32Array;
  private readonly stretch: Float32Array;
  private readonly mode: Uint8Array; // 0 libre, 1 pic, 2 débris, 3 statique
  /** Gravité des débris (monde/s²). */
  gravity = 14;

  constructor(geometry: THREE.BufferGeometry, material: THREE.Material, readonly capacity = 128) {
    this.mesh = new THREE.InstancedMesh(geometry, material, capacity);
    this.mesh.instanceMatrix.setUsage(THREE.DynamicDrawUsage);
    this.mesh.frustumCulled = false;
    this.mesh.castShadow = true;
    this.mesh.count = capacity;
    this.mesh.visible = false;
    for (let i = 0; i < capacity; i++) {
      this.mesh.setMatrixAt(i, ZERO);
      this.mesh.setColorAt(i, new THREE.Color(1, 1, 1));
    }
    const f = (): Float32Array => new Float32Array(capacity);
    this.px = f(); this.py = f(); this.pz = f();
    this.vx = f(); this.vy = f(); this.vz = f();
    this.rx = f(); this.ry = f(); this.rz = f();
    this.spin = f(); this.age = f(); this.life = f(); this.size = f(); this.stretch = f();
    this.mode = new Uint8Array(capacity);
  }

  spawn(o: BurstSpawn): void {
    const i = this.head;
    this.head = (this.head + 1) % this.capacity;
    if (this.mode[i] === 0) this.active++;
    this.mesh.visible = true;
    this.px[i] = o.pos.x; this.py[i] = o.pos.y; this.pz[i] = o.pos.z;
    this.vx[i] = o.vel?.x ?? 0; this.vy[i] = o.vel?.y ?? 0; this.vz[i] = o.vel?.z ?? 0;
    this.age[i] = -(o.delay ?? 0);
    this.life[i] = o.life;
    this.size[i] = o.size;
    this.stretch[i] = o.stretch ?? 1;
    this.mode[i] = o.mode === 'spike' ? 1 : o.mode === 'debris' ? 2 : 3;
    if (o.mode === 'debris') {
      this.rx[i] = rng.next() * 6.28; this.ry[i] = rng.next() * 6.28; this.rz[i] = rng.next() * 6.28;
      this.spin[i] = rng.range(4, 12) * rng.sign();
    } else {
      // pic : inclinaison vers l'extérieur selon `heading`, légère rotation propre
      this.rx[i] = o.tilt ?? rng.range(-0.15, 0.15);
      this.ry[i] = o.heading ?? rng.next() * 6.28;
      this.rz[i] = rng.next() * 6.28;
      this.spin[i] = 0;
    }
    if (o.color) this.mesh.setColorAt(i, o.color);
    if (this.mesh.instanceColor) this.mesh.instanceColor.needsUpdate = true;
  }

  get alive(): number {
    return this.active;
  }

  update(dt: number): void {
    // vide : ni rendu ni passe d'ombre (sinon 1 draw call + 1 d'ombre par système à chaque frame)
    if (this.active === 0) {
      this.mesh.visible = false;
      return;
    }
    for (let i = 0; i < this.capacity; i++) {
      const mode = this.mode[i]!;
      if (mode === 0) continue;
      const age = (this.age[i]! += dt);
      if (age < 0) {
        this.mesh.setMatrixAt(i, ZERO);
        continue;
      }
      const life = this.life[i]!;
      if (age >= life) {
        this.mode[i] = 0;
        this.active--;
        this.mesh.setMatrixAt(i, ZERO);
        continue;
      }
      const t = age / life;
      const size = this.size[i]!;
      if (mode === 2) {
        // débris : balistique + rebond amorti + rotation
        this.vy[i]! -= this.gravity * dt;
        this.px[i]! += this.vx[i]! * dt;
        this.py[i]! += this.vy[i]! * dt;
        this.pz[i]! += this.vz[i]! * dt;
        if (this.py[i]! < size * 0.3 && this.vy[i]! < 0) {
          this.py[i] = size * 0.3;
          this.vy[i] = -this.vy[i]! * 0.3;
          this.vx[i]! *= 0.6;
          this.vz[i]! *= 0.6;
          this.spin[i]! *= 0.5;
        }
        const r = this.spin[i]! * age;
        _q.setFromEuler(_e.set(this.rx[i]! + r, this.ry[i]! + r * 0.7, this.rz[i]!));
        const k = t > 0.75 ? 1 - (t - 0.75) / 0.25 : 1;
        _s.setScalar(size * k);
      } else if (mode === 1) {
        // pic : jaillit (rebond), tient, s'enfonce
        const rise = t < 0.12 ? backOut(t / 0.12) : 1;
        const sink = t > 0.7 ? (t - 0.7) / 0.3 : 0;
        const h = Math.max(0, rise * (1 - sink * sink));
        _q.setFromEuler(_e.set(this.rx[i]!, this.ry[i]!, this.rz[i]! * 0.1, 'YXZ'));
        _s.set(size * (0.6 + 0.4 * rise), size * this.stretch[i]! * h, size * (0.6 + 0.4 * rise));
      } else {
        // statique (prison de glace…) : apparition rapide, disparition par réduction
        const k = Math.min(1, t / 0.08) * (t > 0.85 ? 1 - (t - 0.85) / 0.15 : 1);
        _q.setFromEuler(_e.set(this.rx[i]!, this.ry[i]!, 0, 'YXZ'));
        _s.set(size * k, size * this.stretch[i]! * k, size * k);
      }
      _p.set(this.px[i]!, this.py[i]!, this.pz[i]!);
      this.mesh.setMatrixAt(i, _m.compose(_p, _q, _s));
    }
    this.mesh.instanceMatrix.needsUpdate = true;
  }
}

function backOut(t: number): number {
  const c1 = 2.2;
  const c3 = c1 + 1;
  return 1 + c3 * (t - 1) ** 3 + c1 * (t - 1) ** 2;
}

/** Géométrie de pic : cône à facettes, base à l'origine, pointe vers +y (hauteur 1). */
export function spikeGeometry(sides = 5): THREE.BufferGeometry {
  const g = new THREE.ConeGeometry(0.35, 1, sides, 1);
  g.translate(0, 0.5, 0);
  const ng = g.index ? g.toNonIndexed() : g;
  ng.computeVertexNormals();
  return ng;
}

/** Géométrie de rocher (débris). */
export function rockGeometry(): THREE.BufferGeometry {
  const g = new THREE.DodecahedronGeometry(0.5, 0); // les polyèdres de Three sont déjà non indexés
  const p = g.getAttribute('position') as THREE.BufferAttribute;
  for (let i = 0; i < p.count; i++) {
    const h = Math.sin(p.getX(i) * 91.3 + p.getY(i) * 47.1 + p.getZ(i) * 13.7) * 0.5 + 0.5;
    p.setXYZ(i, p.getX(i) * (0.8 + h * 0.4), p.getY(i) * (0.7 + h * 0.3), p.getZ(i) * (0.8 + h * 0.4));
  }
  g.computeVertexNormals();
  return g;
}

/** Géométrie d'éclat de glace (octaèdre étiré). */
export function shardGeometry(): THREE.BufferGeometry {
  const g = new THREE.OctahedronGeometry(0.5, 0);
  g.scale(0.55, 1.4, 0.55);
  g.translate(0, 0.6, 0);
  const ng = g.index ? g.toNonIndexed() : g;
  ng.computeVertexNormals();
  return ng;
}
