/**
 * Post-process : MSAA (cible HDR) + distorsion de chaleur (seulement si un objet l'utilise) + bloom modéré +
 * sortie étalonnée (bloom additionné, tone mapping, saturation, contraste, ombres froides / hautes lumières
 * chaudes, vignette, sRGB) en **une seule** passe plein écran.
 *
 * Coût en pleine résolution réduit au minimum : rendu de la scène (1 résolution MSAA) puis passe de sortie.
 * Le bloom travaille en demi-résolution et n'est plus recomposé sur la cible MSAA (c'est la sortie qui le lit).
 * Filet anti-NaN dans le filtre de luminance du bloom : c'est son flou qui étalait un NaN en blocs noirs
 * (la sortie, en GLSL ES 1.0 sans `isnan`, se contente de plafonner le HDR ; un NaN y reste un pixel isolé).
 *
 * Bloom « sélectif » par luminance : seuls les pixels au-dessus du seuil brillent. Les FX utilisent des
 * couleurs HDR (> 1) et franchissent le seuil, la scène éclairée normalement reste en dessous.
 * Qualité basse : rendu direct sans post-process.
 */
import * as THREE from 'three';
import { EffectComposer } from 'three/examples/jsm/postprocessing/EffectComposer.js';
import { OutputPass } from 'three/examples/jsm/postprocessing/OutputPass.js';
import { RenderPass } from 'three/examples/jsm/postprocessing/RenderPass.js';
import { FullScreenQuad } from 'three/examples/jsm/postprocessing/Pass.js';
import { ShaderPass } from 'three/examples/jsm/postprocessing/ShaderPass.js';
import { UnrealBloomPass } from 'three/examples/jsm/postprocessing/UnrealBloomPass.js';
import { DISTORTION_LAYER, distortionUsers } from '../shaders/materials.ts';

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
  vec2 off = vec2(0.0);
  if (uStrength > 0.0) {
    vec4 d = texture2D(tDistort, vUv);
    off = (d.rg - 0.5) * 2.0 * d.a * uStrength;
  }
  vec4 c = texture2D(tDiffuse, vUv + off);
  // Filet de sécurité : un seul NaN/Inf (shader d'un autre objet, GPU qui diverge de la norme…) serait étalé
  // par le bloom en blocs noirs. On le neutralise ici, avant le bloom, et on plafonne le HDR.
  if (any(isnan(c)) || any(isinf(c))) c = vec4(0.0, 0.0, 0.0, 1.0);
  gl_FragColor = vec4(min(c.rgb, vec3(64.0)), c.a);
}`,
};

/**
 * Bloom de three sans l'étape finale « mélange additif sur l'image » : le résultat (demi-résolution) reste dans
 * `renderTargetsHorizontal[0]` et la passe de sortie l'additionne. Économise un dessin plein écran et une
 * résolution MSAA par frame.
 */
const BLUR_X = new THREE.Vector2(1, 0);
const BLUR_Y = new THREE.Vector2(0, 1);

class CompositeBloomPass extends UnrealBloomPass {
  private readonly quad = new FullScreenQuad();

  constructor(resolution: THREE.Vector2, strength: number, radius: number, threshold: number) {
    super(resolution, strength, radius, threshold);
    this.needsSwap = false;
    this.materialHighPassFilter.fragmentShader = this.materialHighPassFilter.fragmentShader.replace(
      'vec4 texel = texture2D( tDiffuse, vUv );',
      `vec4 texel = texture2D( tDiffuse, vUv );
      if (any(isnan(texel)) || any(isinf(texel))) texel = vec4(0.0, 0.0, 0.0, 1.0);
      texel.rgb = min(texel.rgb, vec3(64.0));`,
    );
  }

  /** Texture du bloom composé (à additionner à l'image). */
  get texture(): THREE.Texture {
    return this.renderTargetsHorizontal[0]!.texture;
  }

  override render(renderer: THREE.WebGLRenderer, _write: THREE.WebGLRenderTarget, readBuffer: THREE.WebGLRenderTarget): void {
    const oldAutoClear = renderer.autoClear;
    renderer.autoClear = false;
    const u = this.highPassUniforms as Record<string, THREE.IUniform>;
    u.tDiffuse!.value = readBuffer.texture;
    u.luminosityThreshold!.value = this.threshold;
    this.quad.material = this.materialHighPassFilter;
    renderer.setRenderTarget(this.renderTargetBright);
    renderer.clear();
    this.quad.render(renderer);
    let input = this.renderTargetBright;
    for (let i = 0; i < this.nMips; i++) {
      const m = this.separableBlurMaterials[i]!;
      this.quad.material = m;
      m.uniforms.colorTexture!.value = input.texture;
      m.uniforms.direction!.value = BLUR_X;
      renderer.setRenderTarget(this.renderTargetsHorizontal[i]!);
      renderer.clear();
      this.quad.render(renderer);
      m.uniforms.colorTexture!.value = this.renderTargetsHorizontal[i]!.texture;
      m.uniforms.direction!.value = BLUR_Y;
      renderer.setRenderTarget(this.renderTargetsVertical[i]!);
      renderer.clear();
      this.quad.render(renderer);
      input = this.renderTargetsVertical[i]!;
    }
    this.quad.material = this.compositeMaterial;
    this.compositeMaterial.uniforms.bloomStrength!.value = this.strength;
    this.compositeMaterial.uniforms.bloomRadius!.value = this.radius;
    this.compositeMaterial.uniforms.bloomTintColors!.value = this.bloomTintColors;
    renderer.setRenderTarget(this.renderTargetsHorizontal[0]!);
    renderer.clear();
    this.quad.render(renderer);
    renderer.autoClear = oldAutoClear;
  }

  override dispose(): void {
    super.dispose();
    this.quad.dispose();
  }
}

/** Réglages d'étalonnage (appliqués après le tone mapping, avant la conversion sRGB). */
export interface GradeSettings {
  /** Saturation (1 = neutre). */
  saturation: number;
  /** Contraste autour du gris moyen (1 = neutre). */
  contrast: number;
  /** Teinte ajoutée aux ombres (linéaire, petites valeurs). */
  lift: THREE.Color;
  /** Multiplicateur des hautes lumières. */
  gain: THREE.Color;
  /** Assombrissement des coins (0 = aucun). */
  vignette: number;
}

/**
 * OutputPass de three (tone mapping + sRGB, defines gérés par three) dont le shader reçoit l'étalonnage :
 * aucune passe plein écran supplémentaire.
 */
class GradedOutputPass extends OutputPass {
  constructor(g: GradeSettings, bloom: THREE.Texture) {
    super();
    const m = this.material as THREE.RawShaderMaterial;
    Object.assign(m.uniforms, {
      tBloom: { value: bloom },
      uBloomOn: { value: 1 },
      uSat: { value: g.saturation },
      uContrast: { value: g.contrast },
      uLift: { value: g.lift },
      uGain: { value: g.gain },
      uVignette: { value: g.vignette },
    });
    m.fragmentShader = m.fragmentShader
      .replace('varying vec2 vUv;', 'varying vec2 vUv;\nuniform sampler2D tBloom;\nuniform float uBloomOn, uSat, uContrast, uVignette;\nuniform vec3 uLift, uGain;')
      .replace(
        'gl_FragColor = texture2D( tDiffuse, vUv );',
        `gl_FragColor = texture2D( tDiffuse, vUv );
        gl_FragColor.rgb = min(gl_FragColor.rgb, vec3(64.0));
        if (uBloomOn > 0.5) gl_FragColor.rgb += texture2D( tBloom, vUv ).rgb;`,
      )
      .replace(
        '#ifdef SRGB_TRANSFER',
        `{
          vec3 c = max(gl_FragColor.rgb, 0.0);
          float l = dot(c, vec3(0.2126, 0.7152, 0.0722));
          c = mix(vec3(l), c, uSat);
          c += uLift * (1.0 - smoothstep(0.0, 0.3, l));
          c *= mix(vec3(1.0), uGain, smoothstep(0.25, 0.9, l));
          c = 0.18 * pow(c / 0.18 + 1e-6, vec3(uContrast));
          vec2 dv = (vUv - 0.5) * vec2(1.0, 0.8);
          c *= 1.0 - uVignette * smoothstep(0.3, 0.75, length(dv));
          gl_FragColor.rgb = clamp(c, 0.0, 1.0);
        }
        #ifdef SRGB_TRANSFER`,
      );
  }

  set(g: Partial<GradeSettings>): void {
    const u = (this.material as THREE.RawShaderMaterial).uniforms;
    if (g.saturation !== undefined) u.uSat!.value = g.saturation;
    if (g.contrast !== undefined) u.uContrast!.value = g.contrast;
    if (g.vignette !== undefined) u.uVignette!.value = g.vignette;
    if (g.lift) (u.uLift!.value as THREE.Color).copy(g.lift);
    if (g.gain) (u.uGain!.value as THREE.Color).copy(g.gain);
  }
}

export const DEFAULT_GRADE: GradeSettings = {
  saturation: 1.07,
  contrast: 1.06,
  lift: new THREE.Color(0.004, 0.008, 0.02),
  gain: new THREE.Color(1.03, 1.0, 0.95),
  vignette: 0.28,
};

export class PostFx {
  quality: FxQuality = 'high';
  private readonly composer: EffectComposer;
  private readonly bloom: CompositeBloomPass;
  private readonly distortRT: THREE.WebGLRenderTarget;
  private readonly clearColor = new THREE.Color(0.5, 0.5, 0);
  private readonly prevClear = new THREE.Color();
  private readonly size = new THREE.Vector2();
  private readonly distort: ShaderPass;
  private readonly output: GradedOutputPass;
  private pixelRatio = 1;

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
    this.distort = new ShaderPass(DistortionShader);
    this.distort.uniforms.tDistort!.value = this.distortRT.texture;
    this.composer.addPass(this.distort);
    this.bloom = new CompositeBloomPass(new THREE.Vector2(this.size.x, this.size.y), 0.55, 0.45, 0.92);
    this.composer.addPass(this.bloom);
    // Antialiasing : MSAA sur la cible HDR (une passe FXAA en plus serait redondante et coûteuse).
    this.output = new GradedOutputPass(DEFAULT_GRADE, this.bloom.texture);
    this.composer.addPass(this.output);
    this.setSize(this.size.x, this.size.y);
  }

  /** Ajuste l'étalonnage (saturation, contraste, teintes, vignette). */
  setGrade(g: Partial<GradeSettings>): void {
    this.output.set(g);
  }

  setBloom(strength: number, radius = this.bloom.radius, threshold = this.bloom.threshold): void {
    this.bloom.strength = strength;
    // bloom nul : on saute aussi ses passes (demi-résolution) et sa lecture dans la sortie
    this.bloom.enabled = strength > 0;
    (this.output.material as THREE.RawShaderMaterial).uniforms.uBloomOn!.value = strength > 0 ? 1 : 0;
    this.bloom.radius = radius;
    this.bloom.threshold = threshold;
  }

  setSize(w: number, h: number): void {
    this.pixelRatio = this.renderer.getPixelRatio();
    // MSAA ×4 en densité normale ; ×2 en HiDPI (les pixels sont déjà petits, et ×4 y coûte très cher)
    const samples = this.pixelRatio >= 1.5 ? 2 : 4;
    for (const rt of [this.composer.renderTarget1, this.composer.renderTarget2]) {
      if (rt.samples !== samples) {
        rt.samples = samples;
        rt.dispose();
      }
    }
    this.composer.setPixelRatio(this.pixelRatio);
    this.composer.setSize(w, h);
    this.distortRT.setSize(Math.max(1, w >> 1), Math.max(1, h >> 1));
  }

  render(): void {
    if (this.quality === 'low') {
      this.renderer.render(this.scene, this.camera);
      return;
    }
    // le pixel ratio peut changer à chaud (qualité, résolution dynamique) : on suit sans attendre un resize
    if (this.renderer.getPixelRatio() !== this.pixelRatio) {
      this.renderer.getSize(this.size);
      this.setSize(this.size.x, this.size.y);
    }
    // la couche de distorsion coûte un second parcours complet de la scène : seulement si elle sert
    const distortOn = distortionUsers.count > 0;
    this.distort.enabled = distortOn;
    if (distortOn) this.renderDistortion();
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
