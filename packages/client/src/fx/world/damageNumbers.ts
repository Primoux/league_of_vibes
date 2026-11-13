/**
 * Chiffres flottants (dégâts, soins, or) sur GPU : un quad instancié par glyphe, atlas de glyphes dessiné
 * dans un canvas (contour sombre), montée / grossissement / fondu calculés dans le vertex shader.
 * Aucune allocation par frame ; tampon circulaire de glyphes.
 */
import * as THREE from 'three';
import { RingAllocator } from '../core/ring.ts';
import { rng } from '../core/rng.ts';

const GLYPHS = '0123456789+-!';
const CELL_W = 48;
const CELL_H = 64;

function makeAtlas(): THREE.CanvasTexture {
  const c = document.createElement('canvas');
  c.width = CELL_W * 16;
  c.height = CELL_H;
  const g = c.getContext('2d')!;
  g.font = `900 ${CELL_H * 0.78}px system-ui, "Segoe UI", sans-serif`;
  g.textAlign = 'center';
  g.textBaseline = 'middle';
  g.lineJoin = 'round';
  for (let i = 0; i < GLYPHS.length; i++) {
    const x = i * CELL_W + CELL_W / 2;
    const y = CELL_H / 2 + 2;
    g.lineWidth = 9;
    g.strokeStyle = 'rgba(0,0,0,0.85)';
    g.strokeText(GLYPHS[i]!, x, y);
    g.fillStyle = '#ffffff';
    g.fillText(GLYPHS[i]!, x, y);
  }
  const t = new THREE.CanvasTexture(c);
  t.colorSpace = THREE.NoColorSpace;
  t.minFilter = THREE.LinearFilter;
  t.generateMipmaps = false;
  return t;
}

const VERT = /* glsl */ `
uniform float uTime;
attribute vec3 aAnchor;
attribute vec4 aGlyph;  // glyphe, décalage x (en largeurs), taille, nombre de glyphes
attribute vec4 aTime;   // naissance, durée, dérive x, dérive z
attribute vec3 aColor;
varying vec2 vUv;
varying vec3 vColor;
varying float vAlpha;
void main() {
  float age = uTime - aTime.x;
  float t = age / aTime.y;
  if (t < 0.0 || t > 1.0) { gl_Position = vec4(2.0, 2.0, 2.0, 1.0); return; }
  float rise = 1.0 - pow(1.0 - min(1.0, t * 1.6), 3.0);
  vec3 p = aAnchor + vec3(aTime.z * t, 0.9 * rise, aTime.w * t);
  float size = aGlyph.z * (1.0 + 0.7 * exp(-age * 14.0));
  vec4 mv = modelViewMatrix * vec4(p, 1.0);
  float w = 0.62;
  mv.x += (aGlyph.y - (aGlyph.w - 1.0) * 0.5) * w * size + position.x * w * size;
  mv.y += position.y * size;
  gl_Position = projectionMatrix * mv;
  vUv = vec2((aGlyph.x + uv.x) / 16.0, uv.y);
  vColor = aColor;
  vAlpha = 1.0 - smoothstep(0.65, 1.0, t);
}
`;
const FRAG = /* glsl */ `
uniform sampler2D uAtlas;
varying vec2 vUv;
varying vec3 vColor;
varying float vAlpha;
void main() {
  vec4 tx = texture2D(uAtlas, vUv);
  float a = tx.a * vAlpha;
  if (a < 0.02) discard;
  gl_FragColor = vec4(vColor * tx.rgb, a);
  #include <colorspace_fragment>
}
`;

export class FloatingNumbers {
  readonly mesh: THREE.Mesh;
  private readonly ring: RingAllocator;
  private readonly anchor: THREE.InstancedBufferAttribute;
  private readonly glyph: THREE.InstancedBufferAttribute;
  private readonly time: THREE.InstancedBufferAttribute;
  private readonly color: THREE.InstancedBufferAttribute;
  private readonly attrs: THREE.InstancedBufferAttribute[];
  private readonly geo = new THREE.InstancedBufferGeometry();
  private readonly uniforms: { uTime: THREE.IUniform<number>; uAtlas: THREE.IUniform<THREE.Texture> };
  private used = 0;
  private clock = 0;
  private endTime = -1;

  constructor(capacity = 768) {
    this.ring = new RingAllocator(capacity);
    const quad = new THREE.PlaneGeometry(1, 1);
    this.geo.index = quad.index;
    this.geo.setAttribute('position', quad.getAttribute('position'));
    this.geo.setAttribute('uv', quad.getAttribute('uv'));
    const attr = (name: string, size: number): THREE.InstancedBufferAttribute => {
      const a = new THREE.InstancedBufferAttribute(new Float32Array(capacity * size), size).setUsage(THREE.DynamicDrawUsage);
      this.geo.setAttribute(name, a);
      return a as THREE.InstancedBufferAttribute;
    };
    this.anchor = attr('aAnchor', 3);
    this.glyph = attr('aGlyph', 4);
    this.time = attr('aTime', 4);
    this.color = attr('aColor', 3);
    (this.time.array as Float32Array).fill(1e9);
    this.attrs = [this.anchor, this.glyph, this.time, this.color];
    this.geo.instanceCount = 0;
    this.uniforms = { uTime: { value: 0 }, uAtlas: { value: makeAtlas() } };
    const mat = new THREE.ShaderMaterial({
      uniforms: this.uniforms,
      vertexShader: VERT,
      fragmentShader: FRAG,
      transparent: true,
      depthTest: false,
      depthWrite: false,
    });
    this.mesh = new THREE.Mesh(this.geo, mat);
    this.mesh.frustumCulled = false;
    this.mesh.renderOrder = 50;
    this.mesh.name = 'fx-numbers';
  }

  /** Affiche `text` (chiffres, + - !) au-dessus de `pos` (monde). */
  spawn(pos: THREE.Vector3, text: string, color: THREE.Color, size = 0.42, life = 0.9): void {
    const n = text.length;
    const dx = rng.range(-0.35, 0.35);
    const dz = rng.range(-0.15, 0.15);
    for (let k = 0; k < n; k++) {
      const g = GLYPHS.indexOf(text[k]!);
      if (g < 0) continue;
      const i = this.ring.next();
      if (this.used < this.ring.capacity) this.used++;
      this.endTime = Math.max(this.endTime, this.clock + life);
      this.mesh.visible = true;
      const a = this.anchor.array as Float32Array;
      const gl = this.glyph.array as Float32Array;
      const t = this.time.array as Float32Array;
      const c = this.color.array as Float32Array;
      a[i * 3] = pos.x; a[i * 3 + 1] = pos.y; a[i * 3 + 2] = pos.z;
      gl[i * 4] = g; gl[i * 4 + 1] = k; gl[i * 4 + 2] = size; gl[i * 4 + 3] = n;
      t[i * 4] = this.clock; t[i * 4 + 1] = life; t[i * 4 + 2] = dx; t[i * 4 + 3] = dz;
      c[i * 3] = color.r; c[i * 3 + 1] = color.g; c[i * 3 + 2] = color.b;
    }
  }

  update(dt: number): void {
    this.clock += dt;
    this.uniforms.uTime.value = this.clock;
    if (this.used > 0 && this.clock > this.endTime) {
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
