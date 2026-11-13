/**
 * Post-process : bloom modéré + distorsion de chaleur + sortie (tone mapping/sRGB) + FXAA.
 *
 * Bloom « sélectif » par luminance : seuls les pixels au-dessus du seuil brillent. Les FX utilisent des
 * couleurs HDR (> 1) et franchissent le seuil, la scène éclairée normalement reste en dessous.
 * Qualité basse : rendu direct sans post-process.
 */
import * as THREE from 'three';
import { EffectComposer } from 'three/examples/jsm/postprocessing/EffectComposer.js';
import { FXAAPass } from 'three/examples/jsm/postprocessing/FXAAPass.js';
import { OutputPass } from 'three/examples/jsm/postprocessing/OutputPass.js';
import { RenderPass } from 'three/examples/jsm/postprocessing/RenderPass.js';
import { ShaderPass } from 'three/examples/jsm/postprocessing/ShaderPass.js';
import { UnrealBloomPass } from 'three/examples/jsm/postprocessing/UnrealBloomPass.js';
import { DISTORTION_LAYER } from '../shaders/materials.ts';

export type FxQuality = 'high' | 'low';

const DistortionShader = {
  uniforms: {
    tDiffuse: { value: null as THREE.Texture | null },
    tDistort: { value: null as THREE.Texture | null },
    uStrength: { value: 0.02 },
  },
  vertexShader: /* glsl */ `
varying vec2 vUv;
void main() { vUv = uv; gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0); }`,
  fragmentShader: /* glsl */ `
uniform sampler2D tDiffuse;
uniform sampler2D tDistort;
uniform float uStrength;
varying vec2 vUv;
void main() {
  vec4 d = texture2D(tDistort, vUv);
  vec2 off = (d.rg - 0.5) * 2.0 * d.a * uStrength;
  gl_FragColor = texture2D(tDiffuse, vUv + off);
}`,
};

export class PostFx {
  quality: FxQuality = 'high';
  private readonly composer: EffectComposer;
  private readonly bloom: UnrealBloomPass;
  private readonly fxaa: FXAAPass;
  private readonly distortRT: THREE.WebGLRenderTarget;
  private readonly clearColor = new THREE.Color(0.5, 0.5, 0);
  private readonly prevClear = new THREE.Color();
  private readonly size = new THREE.Vector2();

  constructor(
    private readonly renderer: THREE.WebGLRenderer,
    private readonly scene: THREE.Scene,
    private readonly camera: THREE.Camera,
  ) {
    renderer.getSize(this.size);
    const pr = renderer.getPixelRatio();
    // HalfFloat : garde les valeurs HDR > 1 jusqu'au bloom.
    const rt = new THREE.WebGLRenderTarget(this.size.x * pr, this.size.y * pr, { type: THREE.HalfFloatType, samples: 4 });
    this.composer = new EffectComposer(renderer, rt);
    this.composer.addPass(new RenderPass(scene, camera));
    this.distortRT = new THREE.WebGLRenderTarget(256, 256, { type: THREE.UnsignedByteType });
    const distort = new ShaderPass(DistortionShader);
    distort.uniforms.tDistort!.value = this.distortRT.texture;
    this.composer.addPass(distort);
    this.bloom = new UnrealBloomPass(new THREE.Vector2(this.size.x, this.size.y), 0.55, 0.45, 0.92);
    this.composer.addPass(this.bloom);
    this.composer.addPass(new OutputPass());
    this.fxaa = new FXAAPass();
    this.composer.addPass(this.fxaa);
    this.setSize(this.size.x, this.size.y);
  }

  setBloom(strength: number, radius = this.bloom.radius, threshold = this.bloom.threshold): void {
    this.bloom.strength = strength;
    this.bloom.radius = radius;
    this.bloom.threshold = threshold;
  }

  setSize(w: number, h: number): void {
    this.composer.setPixelRatio(this.renderer.getPixelRatio());
    this.composer.setSize(w, h);
    this.distortRT.setSize(Math.max(1, w >> 1), Math.max(1, h >> 1));
  }

  render(): void {
    if (this.quality === 'low') {
      this.renderer.render(this.scene, this.camera);
      return;
    }
    this.renderDistortion();
    this.composer.render();
  }

  /** Rend uniquement la couche de distorsion dans une petite cible (offsets encodés en rg). */
  private renderDistortion(): void {
    const r = this.renderer;
    const mask = this.camera.layers.mask;
    const prevTarget = r.getRenderTarget();
    const prevAlpha = r.getClearAlpha();
    r.getClearColor(this.prevClear);
    const bg = this.scene.background;
    this.scene.background = null;
    this.camera.layers.set(DISTORTION_LAYER);
    r.setRenderTarget(this.distortRT);
    r.setClearColor(this.clearColor, 0);
    r.clear();
    r.render(this.scene, this.camera);
    r.setRenderTarget(prevTarget);
    r.setClearColor(this.prevClear, prevAlpha);
    this.scene.background = bg;
    this.camera.layers.mask = mask;
  }

  dispose(): void {
    this.composer.dispose();
    this.distortRT.dispose();
  }
}
