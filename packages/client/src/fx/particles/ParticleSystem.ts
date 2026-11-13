/**
 * Particules GPU instanciées.
 *
 * Principe : chaque particule est un quad instancié dont les attributs (position initiale, vitesse, naissance,
 * durée de vie, gravité, traînée, style) sont écrits UNE fois à l'émission dans un tampon circulaire.
 * Le vertex shader calcule la trajectoire analytiquement à partir de l'âge (p0 + v·k(t) − ½g·t²) et lit
 * couleur/alpha/taille dans une texture de courbes. Aucun travail CPU par particule et par frame.
 *
 * Deux lots : additif (lumières, étincelles, magie) et alpha classique (fumée, poussière, débris).
 */
import * as THREE from 'three';
import { CURVE_RES, CURVES, bakeStyle, type ColorStop, type Curve } from '../core/curves.ts';
import { RingAllocator } from '../core/ring.ts';
import { rng } from '../core/rng.ts';
import { TEX, getParticleAtlas, type TexName } from './textures.ts';

export type Range = number | readonly [number, number];
export type Blend = 'add' | 'normal';

export interface ParticleStyleDef {
  texture: TexName;
  /** Dégradé de couleur sur la vie (linéaire, HDR possible). Défaut : blanc (la teinte d'émission colore). */
  color?: readonly ColorStop[];
  alpha?: Curve;
  /** Multiplicateur de taille sur la vie. */
  size?: Curve;
  blend?: Blend;
}

export interface ParticleStyle {
  readonly index: number;
  readonly tex: number;
  readonly blend: Blend;
}

export type EmitShape = 'point' | 'sphere' | 'disc' | 'ring' | 'cone' | 'line';

export interface EmitOptions {
  count: number;
  /** Durée de vie (s). */
  life: Range;
  /** Taille (unités monde, 1 = 100 unités de jeu). */
  size: Range;
  /** Vitesse (unités monde / s). */
  speed?: Range;
  shape?: EmitShape;
  /** Rayon de la forme d'émission (unités monde). */
  radius?: number;
  /** Axe du cône (normalisé), défaut +Y. */
  dir?: THREE.Vector3;
  /** Demi-angle du cône (rad). */
  angle?: number;
  /** Fin du segment pour shape 'line'. */
  to?: THREE.Vector3;
  /** Vitesse verticale ajoutée (unités/s). */
  up?: Range;
  /** Vitesse ajoutée à toutes les particules (ex. vitesse du projectile). */
  inherit?: THREE.Vector3;
  /** Gravité (unités/s², positive = vers le bas). */
  gravity?: number;
  /** Traînée exponentielle (1/s). */
  drag?: number;
  /** Vitesse de rotation (rad/s). */
  spin?: Range;
  /** > 0 : quad étiré dans le sens de la vitesse (étincelles). */
  stretch?: number;
  /** Teinte multipliée à la couleur du style. */
  tint?: THREE.Color;
  /** Multiplicateur de luminosité (> 1 = bloom). */
  intensity?: number;
  /** Retard d'apparition (s) — utile pour étaler une salve. */
  delay?: Range;
}

const MAX_STYLES = 128;

const VERT = /* glsl */ `
uniform float uTime;
uniform sampler2D uCurves;
attribute vec3 aP0;
attribute vec3 aVel;
attribute vec4 aTime;  // naissance, durée de vie, taille, vitesse de rotation
attribute vec4 aMisc;  // gravité, traînée, style + 256*texture, étirement
attribute vec4 aTint;  // rgb, rotation initiale
varying vec2 vUv;
varying vec4 vColor;

vec4 curveAt(int row, float t) {
  float x = clamp(t, 0.0, 1.0) * ${CURVE_RES - 1}.0;
  int i = int(floor(x));
  int j = min(i + 1, ${CURVE_RES - 1});
  return mix(texelFetch(uCurves, ivec2(i, row), 0), texelFetch(uCurves, ivec2(j, row), 0), fract(x));
}

void main() {
  float age = uTime - aTime.x;
  float t = age / aTime.y;
  if (t < 0.0 || t > 1.0) { gl_Position = vec4(2.0, 2.0, 2.0, 1.0); return; }

  float drag = aMisc.y;
  float k = drag > 0.0 ? (1.0 - exp(-drag * age)) / drag : age;
  vec3 pos = aP0 + aVel * k - vec3(0.0, 0.5 * aMisc.x * age * age, 0.0);

  float style = mod(aMisc.z, 256.0);
  float tex = floor(aMisc.z / 256.0);
  int row = int(style) * 2;
  vec4 col = curveAt(row, t);
  float size = curveAt(row + 1, t).r * aTime.z;

  vec4 mv = modelViewMatrix * vec4(pos, 1.0);
  vec2 corner = position.xy;
  if (aMisc.w > 0.0) {
    vec3 vel = aVel * (drag > 0.0 ? exp(-drag * age) : 1.0) - vec3(0.0, aMisc.x * age, 0.0);
    vec2 vv = (modelViewMatrix * vec4(vel, 0.0)).xy;
    float sp = length(vv);
    vec2 d = sp > 1e-4 ? vv / sp : vec2(1.0, 0.0);
    vec2 n = vec2(-d.y, d.x);
    float len = size + sp * aMisc.w;
    mv.xy += d * corner.x * len + n * corner.y * size;
  } else {
    float a = aTint.w + aTime.w * age;
    float c = cos(a), s = sin(a);
    mv.xy += mat2(c, s, -s, c) * corner * size;
  }
  gl_Position = projectionMatrix * mv;
  vUv = (uv + vec2(mod(tex, 4.0), 3.0 - floor(tex / 4.0))) * 0.25;
  vColor = vec4(col.rgb * aTint.rgb, col.a);
}
`;

const FRAG = /* glsl */ `
uniform sampler2D uAtlas;
varying vec2 vUv;
varying vec4 vColor;
void main() {
  vec4 tx = texture2D(uAtlas, vUv);
  float a = tx.a * vColor.a;
  if (a < 0.004) discard;
  gl_FragColor = vec4(vColor.rgb * tx.rgb, a);
  #include <tonemapping_fragment>
  #include <colorspace_fragment>
}
`;

/** Un lot de particules partageant un mode de mélange. */
class ParticleBatch {
  readonly mesh: THREE.Mesh;
  private readonly ring: RingAllocator;
  private readonly p0: THREE.InstancedBufferAttribute;
  private readonly vel: THREE.InstancedBufferAttribute;
  private readonly time: THREE.InstancedBufferAttribute;
  private readonly misc: THREE.InstancedBufferAttribute;
  private readonly tint: THREE.InstancedBufferAttribute;
  private readonly geo: THREE.InstancedBufferGeometry;
  private readonly attrs: THREE.InstancedBufferAttribute[];
  private used = 0;
  /** Instant où la dernière particule écrite meurt : au-delà, le lot est vide. */
  private endTime = -1;

  constructor(capacity: number, blend: Blend, uniforms: Record<string, THREE.IUniform>) {
    this.ring = new RingAllocator(capacity);
    const quad = new THREE.PlaneGeometry(1, 1);
    this.geo = new THREE.InstancedBufferGeometry();
    this.geo.index = quad.index;
    this.geo.setAttribute('position', quad.getAttribute('position'));
    this.geo.setAttribute('uv', quad.getAttribute('uv'));
    const attr = (name: string, size: number): THREE.InstancedBufferAttribute => {
      const a = new THREE.InstancedBufferAttribute(new Float32Array(capacity * size), size);
      a.setUsage(THREE.DynamicDrawUsage);
      this.geo.setAttribute(name, a);
      return a;
    };
    this.p0 = attr('aP0', 3);
    this.vel = attr('aVel', 3);
    this.time = attr('aTime', 4);
    this.misc = attr('aMisc', 4);
    this.tint = attr('aTint', 4);
    this.attrs = [this.p0, this.vel, this.time, this.misc, this.tint];
    // Naissance très loin dans le futur : les emplacements jamais écrits restent invisibles.
    (this.time.array as Float32Array).fill(1e9);
    this.geo.instanceCount = 0;

    const mat = new THREE.ShaderMaterial({
      uniforms,
      vertexShader: VERT,
      fragmentShader: FRAG,
      transparent: true,
      depthWrite: false,
      blending: blend === 'add' ? THREE.AdditiveBlending : THREE.NormalBlending,
    });
    this.mesh = new THREE.Mesh(this.geo, mat);
    this.mesh.frustumCulled = false;
    this.mesh.renderOrder = blend === 'add' ? 11 : 10;
    this.mesh.name = `fx-particles-${blend}`;
  }

  /** Écrit une particule. */
  write(
    px: number, py: number, pz: number,
    vx: number, vy: number, vz: number,
    birth: number, life: number, size: number, spin: number,
    gravity: number, drag: number, styleCode: number, stretch: number,
    r: number, g: number, b: number, rot: number,
  ): void {
    const i = this.ring.next();
    if (this.used < this.ring.capacity) this.used++;
    if (birth + life > this.endTime) this.endTime = birth + life;
    this.mesh.visible = true;
    const p0 = this.p0.array as Float32Array;
    const v = this.vel.array as Float32Array;
    const t = this.time.array as Float32Array;
    const m = this.misc.array as Float32Array;
    const c = this.tint.array as Float32Array;
    p0[i * 3] = px; p0[i * 3 + 1] = py; p0[i * 3 + 2] = pz;
    v[i * 3] = vx; v[i * 3 + 1] = vy; v[i * 3 + 2] = vz;
    t[i * 4] = birth; t[i * 4 + 1] = life; t[i * 4 + 2] = size; t[i * 4 + 3] = spin;
    m[i * 4] = gravity; m[i * 4 + 1] = drag; m[i * 4 + 2] = styleCode; m[i * 4 + 3] = stretch;
    c[i * 4] = r; c[i * 4 + 1] = g; c[i * 4 + 2] = b; c[i * 4 + 3] = rot;
  }

  /**
   * Téléverse uniquement les plages écrites depuis la frame précédente. Quand tout est mort, le lot est
   * masqué et repart de zéro : le vertex shader ne retraite pas des milliers d'instances mortes après un combat.
   */
  flush(time: number): void {
    if (this.used > 0 && time > this.endTime) {
      this.used = 0;
      this.ring.reset();
      this.geo.instanceCount = 0;
      this.mesh.visible = false;
      return;
    }
    const n = this.ring.flush();
    if (n === 0) return;
    this.geo.instanceCount = this.used;
    for (const a of this.attrs) {
      a.clearUpdateRanges();
      for (let k = 0; k < n; k++) a.addUpdateRange(this.ring.rangeStart[k]! * a.itemSize, this.ring.rangeCount[k]! * a.itemSize);
      a.needsUpdate = true;
    }
  }
}

const _v = new THREE.Vector3();
const _q = new THREE.Quaternion();
const _up = new THREE.Vector3(0, 1, 0);
const _white = new THREE.Color(1, 1, 1);

export class ParticleSystem {
  readonly group = new THREE.Group();
  private readonly batches: Record<Blend, ParticleBatch>;
  private readonly curveData = new Float32Array(CURVE_RES * 4 * MAX_STYLES * 2);
  private readonly curveTex: THREE.DataTexture;
  private readonly styles: ParticleStyle[] = [];
  private readonly uniforms: Record<string, THREE.IUniform>;
  /** Horloge des particules (s). */
  time = 0;
  /** Nombre de particules émises depuis le début (stat de debug / budget). */
  emitted = 0;
  /** Multiplicateur global de nombre de particules (qualité basse = 0.5). */
  density = 1;

  constructor(capacity = 16384) {
    this.curveTex = new THREE.DataTexture(this.curveData, CURVE_RES, MAX_STYLES * 2, THREE.RGBAFormat, THREE.FloatType);
    this.curveTex.minFilter = this.curveTex.magFilter = THREE.NearestFilter;
    this.curveTex.needsUpdate = true;
    this.uniforms = { uTime: { value: 0 }, uCurves: { value: this.curveTex }, uAtlas: { value: getParticleAtlas() } };
    this.batches = {
      normal: new ParticleBatch(capacity >> 2, 'normal', this.uniforms),
      add: new ParticleBatch(capacity, 'add', this.uniforms),
    };
    this.group.add(this.batches.normal.mesh, this.batches.add.mesh);
    this.group.name = 'fx-particles';
  }

  defineStyle(def: ParticleStyleDef): ParticleStyle {
    if (this.styles.length >= MAX_STYLES) throw new Error('Trop de styles de particules');
    const index = this.styles.length;
    bakeStyle(def.color ?? [[0, 1, 1, 1]], def.alpha ?? CURVES.fadeOut, def.size ?? CURVES.one, this.curveData, index * 2);
    this.curveTex.needsUpdate = true;
    const style: ParticleStyle = { index, tex: TEX[def.texture], blend: def.blend ?? 'add' };
    this.styles.push(style);
    return style;
  }

  /** Émet `opts.count` particules autour de `origin` (coordonnées monde). */
  emit(style: ParticleStyle, origin: THREE.Vector3, o: EmitOptions): void {
    const batch = this.batches[style.blend];
    const count = Math.max(1, Math.round(o.count * this.density));
    const shape = o.shape ?? 'sphere';
    const radius = o.radius ?? 0;
    const tint = o.tint ?? _white;
    const inten = o.intensity ?? 1;
    const code = style.index + style.tex * 256;
    if (shape === 'cone') _q.setFromUnitVectors(_up, o.dir ?? _up);
    for (let n = 0; n < count; n++) {
      let px = origin.x;
      let py = origin.y;
      let pz = origin.z;
      const speed = o.speed === undefined ? 0 : rng.pick(o.speed);
      switch (shape) {
        case 'point':
        case 'sphere': {
          randomUnit(_v);
          const r = shape === 'sphere' ? radius * Math.cbrt(rng.next()) : 0;
          px += _v.x * r;
          py += _v.y * r;
          pz += _v.z * r;
          _v.multiplyScalar(speed);
          break;
        }
        case 'disc':
        case 'ring': {
          const a = rng.next() * Math.PI * 2;
          const r = shape === 'ring' ? radius : radius * Math.sqrt(rng.next());
          const c = Math.cos(a);
          const s = Math.sin(a);
          px += c * r;
          pz += s * r;
          _v.set(c * speed, 0, s * speed);
          break;
        }
        case 'cone': {
          const ang = (o.angle ?? 0.4) * Math.sqrt(rng.next());
          const az = rng.next() * Math.PI * 2;
          _v.set(Math.sin(ang) * Math.cos(az), Math.cos(ang), Math.sin(ang) * Math.sin(az)).applyQuaternion(_q).multiplyScalar(speed);
          if (radius > 0) {
            px += (rng.next() - 0.5) * radius;
            pz += (rng.next() - 0.5) * radius;
          }
          break;
        }
        case 'line': {
          const k = rng.next();
          const to = o.to ?? origin;
          px += (to.x - origin.x) * k;
          py += (to.y - origin.y) * k;
          pz += (to.z - origin.z) * k;
          randomUnit(_v).multiplyScalar(speed);
          if (radius > 0) {
            px += (rng.next() - 0.5) * radius;
            pz += (rng.next() - 0.5) * radius;
          }
          break;
        }
      }
      if (o.up !== undefined) _v.y += rng.pick(o.up);
      if (o.inherit) _v.add(o.inherit);
      const birth = this.time + (o.delay === undefined ? 0 : rng.pick(o.delay));
      batch.write(
        px, py, pz,
        _v.x, _v.y, _v.z,
        birth, rng.pick(o.life), rng.pick(o.size), o.spin === undefined ? 0 : rng.pick(o.spin),
        o.gravity ?? 0, o.drag ?? 0, code, o.stretch ?? 0,
        tint.r * inten, tint.g * inten, tint.b * inten, rng.next() * Math.PI * 2,
      );
    }
    this.emitted += count;
  }

  update(dt: number): void {
    this.time += dt;
    this.uniforms.uTime!.value = this.time;
    this.batches.add.flush(this.time);
    this.batches.normal.flush(this.time);
  }
}

function randomUnit(out: THREE.Vector3): THREE.Vector3 {
  const z = rng.next() * 2 - 1;
  const a = rng.next() * Math.PI * 2;
  const r = Math.sqrt(1 - z * z);
  return out.set(r * Math.cos(a), z, r * Math.sin(a));
}
