/**
 * Ruban de traînée (trail) : bande de triangles qui suit un point en mouvement (projectile, dash, lame).
 * Capacité fixe, géométrie réécrite sur place à chaque frame (aucune allocation).
 * Mode 'billboard' : face caméra (projectiles) ; mode 'flat' : à plat au sol (dashes, glissades).
 */
import * as THREE from 'three';

const VERT = /* glsl */ `
attribute float aAlpha;
varying float vAlpha;
varying float vSide;
void main() {
  vAlpha = aAlpha;
  vSide = uv.y * 2.0 - 1.0;
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}
`;

const FRAG = /* glsl */ `
uniform vec3 uColor;
uniform vec3 uCore;
uniform float uOpacity;
varying float vAlpha;
varying float vSide;
void main() {
  // max() : pow(x<0) est indéfini en GLSL → NaN, que le bloom étale en grande tache
  float across = max(0.0, 1.0 - vSide * vSide);
  float core = pow(across, 6.0);
  vec3 col = mix(uColor, uCore, core * vAlpha);
  gl_FragColor = vec4(col, across * vAlpha * uOpacity);
  #include <tonemapping_fragment>
  #include <colorspace_fragment>
}
`;

const _tan = new THREE.Vector3();
const _side = new THREE.Vector3();
const _view = new THREE.Vector3();
const _cam = new THREE.Vector3();
const UP = new THREE.Vector3(0, 1, 0);

export class Ribbon {
  readonly mesh: THREE.Mesh<THREE.BufferGeometry, THREE.ShaderMaterial>;
  /** Largeur totale à la tête (monde). */
  width = 0.3;
  /** Durée de vie d'un point de la traînée (s). */
  life = 0.35;
  mode: 'billboard' | 'flat' = 'billboard';
  /** Distance minimale entre deux points (monde). */
  minDist = 0.08;

  private readonly px: Float32Array;
  private readonly py: Float32Array;
  private readonly pz: Float32Array;
  private readonly age: Float32Array;
  private head = 0; // index du point le plus récent
  private count = 0;
  private readonly pos: THREE.BufferAttribute;
  private readonly alpha: THREE.BufferAttribute;

  constructor(readonly capacity = 32) {
    this.px = new Float32Array(capacity);
    this.py = new Float32Array(capacity);
    this.pz = new Float32Array(capacity);
    this.age = new Float32Array(capacity);
    const geo = new THREE.BufferGeometry();
    this.pos = new THREE.BufferAttribute(new Float32Array(capacity * 2 * 3), 3).setUsage(THREE.DynamicDrawUsage);
    this.alpha = new THREE.BufferAttribute(new Float32Array(capacity * 2), 1).setUsage(THREE.DynamicDrawUsage);
    const uv = new Float32Array(capacity * 4);
    const idx: number[] = [];
    for (let i = 0; i < capacity; i++) {
      uv[i * 4] = i / (capacity - 1);
      uv[i * 4 + 1] = 0;
      uv[i * 4 + 2] = i / (capacity - 1);
      uv[i * 4 + 3] = 1;
      if (i < capacity - 1) {
        const a = i * 2;
        idx.push(a, a + 1, a + 2, a + 1, a + 3, a + 2);
      }
    }
    geo.setAttribute('position', this.pos);
    geo.setAttribute('aAlpha', this.alpha);
    geo.setAttribute('uv', new THREE.BufferAttribute(uv, 2));
    geo.setIndex(idx);
    geo.setDrawRange(0, 0);
    const mat = new THREE.ShaderMaterial({
      uniforms: {
        uColor: { value: new THREE.Color(1, 1, 1) },
        uCore: { value: new THREE.Color(2, 2, 2) },
        uOpacity: { value: 1 },
      },
      vertexShader: VERT,
      fragmentShader: FRAG,
      transparent: true,
      depthWrite: false,
      blending: THREE.AdditiveBlending,
      side: THREE.DoubleSide,
    });
    this.mesh = new THREE.Mesh(geo, mat);
    this.mesh.frustumCulled = false;
    this.mesh.renderOrder = 12;
    this.mesh.name = 'fx-ribbon';
  }

  setColor(color: THREE.Color, core?: THREE.Color): this {
    this.mesh.material.uniforms.uColor!.value.copy(color);
    if (core) this.mesh.material.uniforms.uCore!.value.copy(core);
    return this;
  }

  set opacity(v: number) {
    this.mesh.material.uniforms.uOpacity!.value = v;
  }

  reset(): void {
    this.count = 0;
    this.head = 0;
    this.opacity = 1;
    this.mesh.geometry.setDrawRange(0, 0);
  }

  /** Nombre de points vivants (0 = la traînée a fini de s'effacer). */
  get alive(): number {
    return this.count;
  }

  /** Ajoute la position courante de la tête (si elle a assez bougé). */
  push(p: THREE.Vector3): void {
    if (this.count > 0) {
      const h = this.head;
      const d2 = (p.x - this.px[h]!) ** 2 + (p.y - this.py[h]!) ** 2 + (p.z - this.pz[h]!) ** 2;
      if (d2 < this.minDist * this.minDist) {
        // la tête suit quand même exactement le point
        this.px[h] = p.x;
        this.py[h] = p.y;
        this.pz[h] = p.z;
        return;
      }
    }
    this.head = (this.head + 1) % this.capacity;
    this.px[this.head] = p.x;
    this.py[this.head] = p.y;
    this.pz[this.head] = p.z;
    this.age[this.head] = 0;
    if (this.count < this.capacity) this.count++;
  }

  update(dt: number, camera: THREE.Camera): void {
    // vieillit les points et retire ceux qui ont expiré (les plus anciens sont en queue)
    for (let k = 0; k < this.count; k++) {
      const i = (this.head - k + this.capacity) % this.capacity;
      this.age[i]! += dt;
    }
    while (this.count > 0) {
      const tail = (this.head - this.count + 1 + this.capacity) % this.capacity;
      if (this.age[tail]! < this.life) break;
      this.count--;
    }
    const n = this.count;
    if (n < 2) {
      this.mesh.geometry.setDrawRange(0, 0);
      return;
    }
    const pos = this.pos.array as Float32Array;
    const al = this.alpha.array as Float32Array;
    camera.getWorldPosition(_cam);
    // k = 0 → tête ; on écrit du plus récent au plus ancien
    for (let k = 0; k < n; k++) {
      const i = (this.head - k + this.capacity) % this.capacity;
      const prev = (i - 1 + this.capacity) % this.capacity;
      const next = (i + 1) % this.capacity;
      const a = k === n - 1 ? i : prev;
      const b = k === 0 ? i : next;
      _tan.set(this.px[b]! - this.px[a]!, this.py[b]! - this.py[a]!, this.pz[b]! - this.pz[a]!);
      if (_tan.lengthSq() < 1e-10) _tan.set(1, 0, 0);
      if (this.mode === 'flat') _side.crossVectors(_tan, UP);
      else _side.crossVectors(_tan, _view.set(this.px[i]! - _cam.x, this.py[i]! - _cam.y, this.pz[i]! - _cam.z));
      const life = 1 - this.age[i]! / this.life;
      const taper = (1 - k / (n - 1)) * 0.7 + 0.3;
      _side.normalize().multiplyScalar((this.width / 2) * taper * Math.max(0, life));
      const v = k * 2;
      pos[v * 3] = this.px[i]! + _side.x;
      pos[v * 3 + 1] = this.py[i]! + _side.y;
      pos[v * 3 + 2] = this.pz[i]! + _side.z;
      pos[v * 3 + 3] = this.px[i]! - _side.x;
      pos[v * 3 + 4] = this.py[i]! - _side.y;
      pos[v * 3 + 5] = this.pz[i]! - _side.z;
      al[v] = al[v + 1] = Math.max(0, life) * taper;
    }
    this.pos.needsUpdate = true;
    this.alpha.needsUpdate = true;
    this.mesh.geometry.setDrawRange(0, (n - 1) * 6);
  }

  dispose(): void {
    this.mesh.geometry.dispose();
    this.mesh.material.dispose();
  }
}
