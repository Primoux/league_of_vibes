/**
 * Matériaux FX réutilisables : fresnel (boucliers, dômes, auras), givre cristallin, eau stylisée,
 * distorsion de chaleur (rendue dans une couche dédiée, appliquée par PostFx).
 */
import * as THREE from 'three';

// ---------------------------------------------------------------- Fresnel / rim glow
const FRESNEL_VERT = /* glsl */ `
varying vec3 vN;
varying vec3 vV;
varying vec3 vLocal;
void main() {
  vLocal = position;
  vec4 mv = modelViewMatrix * vec4(position, 1.0);
  vN = normalize(normalMatrix * normal);
  vV = normalize(-mv.xyz);
  gl_Position = projectionMatrix * mv;
}
`;
const FRESNEL_FRAG = /* glsl */ `
uniform vec3 uColor;
uniform float uPower;
uniform float uBase;
uniform float uOpacity;
uniform float uTime;
uniform float uBands;
varying vec3 vN;
varying vec3 vV;
varying vec3 vLocal;
void main() {
  float f = pow(1.0 - abs(dot(normalize(vN), normalize(vV))), uPower);
  float bands = uBands * (0.5 + 0.5 * sin(vLocal.y * 18.0 - uTime * 3.0));
  float a = (uBase + f + bands * f) * uOpacity;
  gl_FragColor = vec4(uColor, clamp(a, 0.0, 1.0));
  #include <tonemapping_fragment>
  #include <colorspace_fragment>
}
`;

export interface FresnelOptions {
  color?: THREE.Color;
  power?: number;
  base?: number;
  bands?: number;
  opacity?: number;
}

export function createFresnelMaterial(o: FresnelOptions = {}): THREE.ShaderMaterial {
  return new THREE.ShaderMaterial({
    uniforms: {
      uColor: { value: (o.color ?? new THREE.Color(1, 1, 1)).clone() },
      uPower: { value: o.power ?? 2.5 },
      uBase: { value: o.base ?? 0.05 },
      uBands: { value: o.bands ?? 0 },
      uOpacity: { value: o.opacity ?? 1 },
      uTime: { value: 0 },
    },
    vertexShader: FRESNEL_VERT,
    fragmentShader: FRESNEL_FRAG,
    transparent: true,
    depthWrite: false,
    blending: THREE.AdditiveBlending,
    side: THREE.DoubleSide,
  });
}

// ---------------------------------------------------------------- Givre cristallin (facettes)
const ICE_VERT = /* glsl */ `
varying vec3 vViewPos;
varying vec3 vTint;
void main() {
  vec4 p = vec4(position, 1.0);
  vTint = vec3(1.0);
  #ifdef USE_INSTANCING
  p = instanceMatrix * p;
  #endif
  #ifdef USE_INSTANCING_COLOR
  vTint = instanceColor;
  #endif
  vec4 mv = modelViewMatrix * p;
  vViewPos = mv.xyz;
  gl_Position = projectionMatrix * mv;
}
`;
const ICE_FRAG = /* glsl */ `
uniform vec3 uColor;
uniform vec3 uDeep;
uniform float uOpacity;
varying vec3 vViewPos;
varying vec3 vTint;
void main() {
  // normale de facette via dérivées écran : look low-poly sans dupliquer les sommets
  vec3 n = normalize(cross(dFdx(vViewPos), dFdy(vViewPos)));
  vec3 v = normalize(-vViewPos);
  float rim = pow(1.0 - abs(dot(n, v)), 2.0);
  float light = 0.5 + 0.5 * dot(n, normalize(vec3(0.3, 0.8, 0.5)));
  float glint = pow(max(0.0, dot(reflect(-v, n), normalize(vec3(0.2, 0.9, 0.4)))), 24.0);
  vec3 col = (mix(uDeep, uColor, light) * 0.85 + rim * uColor * 0.7) * vTint + glint * 1.2;
  gl_FragColor = vec4(col, uOpacity * (0.75 + 0.25 * rim));
  #include <tonemapping_fragment>
  #include <colorspace_fragment>
}
`;

export function createIceMaterial(color = new THREE.Color(0.35, 0.65, 0.95), deep = new THREE.Color(0.03, 0.1, 0.25)): THREE.ShaderMaterial {
  return new THREE.ShaderMaterial({
    uniforms: { uColor: { value: color.clone() }, uDeep: { value: deep.clone() }, uOpacity: { value: 0.92 } },
    vertexShader: ICE_VERT,
    fragmentShader: ICE_FRAG,
    transparent: true,
  });
}

// ---------------------------------------------------------------- Eau stylisée (zones, souffle océan)
const WATER_VERT = /* glsl */ `
varying vec2 vUv;
void main() {
  vUv = uv;
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}
`;
const WATER_FRAG = /* glsl */ `
uniform vec3 uColor;
uniform vec3 uFoam;
uniform float uTime;
uniform float uOpacity;
varying vec2 vUv;
void main() {
  vec2 p = vUv * 2.0 - 1.0;
  float r = length(p);
  if (r > 1.0) discard;
  float w = sin(p.x * 9.0 + uTime * 1.7) * sin(p.y * 7.0 - uTime * 1.3);
  float caustic = smoothstep(0.55, 0.95, 0.5 + 0.5 * w);
  float foam = smoothstep(0.82, 0.97, r) * (1.0 - smoothstep(0.97, 1.0, r));
  vec3 col = uColor * (0.8 + 0.2 * w) + uFoam * (caustic * 0.12 + foam * 0.8);
  gl_FragColor = vec4(col, uOpacity * (0.55 + 0.45 * foam) * (1.0 - smoothstep(0.98, 1.0, r)));
  #include <tonemapping_fragment>
  #include <colorspace_fragment>
}
`;

export function createWaterMaterial(color = new THREE.Color(0.03, 0.18, 0.42), foam = new THREE.Color(0.75, 0.9, 1)): THREE.ShaderMaterial {
  return new THREE.ShaderMaterial({
    uniforms: { uColor: { value: color.clone() }, uFoam: { value: foam.clone() }, uTime: { value: 0 }, uOpacity: { value: 0.85 } },
    vertexShader: WATER_VERT,
    fragmentShader: WATER_FRAG,
    transparent: true,
    depthWrite: false,
  });
}

// ---------------------------------------------------------------- Distorsion (chaleur, ondes)
/** Couche Three.js réservée aux objets de distorsion : invisibles dans le rendu principal, lus par PostFx. */
export const DISTORTION_LAYER = 3;

const HEAT_FRAG = /* glsl */ `
uniform float uTime;
uniform float uStrength;
varying vec2 vUv;
void main() {
  vec2 p = vUv * 2.0 - 1.0;
  float fade = 1.0 - smoothstep(0.4, 1.0, length(p));
  vec2 off = vec2(sin(p.y * 14.0 + uTime * 6.0), cos(p.x * 12.0 - uTime * 5.0)) * uStrength * fade;
  // offset encodé autour de 0.5 ; alpha = masque
  gl_FragColor = vec4(0.5 + off * 0.5, 0.0, fade);
}
`;

export function createHeatMaterial(strength = 0.3): THREE.ShaderMaterial {
  return new THREE.ShaderMaterial({
    uniforms: { uTime: { value: 0 }, uStrength: { value: strength } },
    vertexShader: WATER_VERT,
    fragmentShader: HEAT_FRAG,
    transparent: true,
    depthWrite: false,
    side: THREE.DoubleSide,
  });
}

// ---------------------------------------------------------------- Dissolve (mort)
export interface DissolveUniforms {
  uDissolve: THREE.IUniform<number>;
  uEdgeColor: THREE.IUniform<THREE.Color>;
  uNoiseScale: THREE.IUniform<number>;
}

export function createDissolveUniforms(edge = new THREE.Color(3, 1.6, 0.6)): DissolveUniforms {
  return { uDissolve: { value: 0 }, uEdgeColor: { value: edge }, uNoiseScale: { value: 3.5 } };
}

/** GLSL commun : bruit de dissolution, identique pour le rendu et l'ombre (sinon l'ombre ne disparaît pas en même temps). */
const DISSOLVE_PARS = /* glsl */ `
varying vec3 vDissolvePos;
uniform float uDissolve;
uniform vec3 uEdgeColor;
uniform float uNoiseScale;
float dHash(vec3 p) { p = fract(p * 0.3183099 + 0.1); p *= 17.0; return fract(p.x * p.y * p.z * (p.x + p.y + p.z)); }
float dNoise(vec3 x) {
  vec3 i = floor(x); vec3 f = fract(x); f = f * f * (3.0 - 2.0 * f);
  return mix(mix(mix(dHash(i), dHash(i + vec3(1,0,0)), f.x), mix(dHash(i + vec3(0,1,0)), dHash(i + vec3(1,1,0)), f.x), f.y),
             mix(mix(dHash(i + vec3(0,0,1)), dHash(i + vec3(1,0,1)), f.x), mix(dHash(i + vec3(0,1,1)), dHash(i + vec3(1,1,1)), f.x), f.y), f.z);
}
// bruit + biais vertical : le haut du modèle disparaît en premier
float dissolveValue() { return dNoise(vDissolvePos * uNoiseScale) * 0.8 + (1.0 - clamp(vDissolvePos.y / 2.5, 0.0, 1.0)) * 0.2; }
`;

type ShaderLike = { uniforms: Record<string, THREE.IUniform>; vertexShader: string; fragmentShader: string };

function patchVertex(shader: ShaderLike, u: DissolveUniforms): void {
  Object.assign(shader.uniforms, u);
  shader.vertexShader = shader.vertexShader
    .replace('#include <common>', '#include <common>\nvarying vec3 vDissolvePos;')
    // après project_vertex : `transformed` est déjà skinné (la position suit les membres animés)
    .replace('#include <project_vertex>', '#include <project_vertex>\nvDissolvePos = (modelMatrix * vec4(transformed, 1.0)).xyz;');
}

/**
 * Ajoute un effet de dissolution à un matériau standard (MeshStandard/Toon/Lambert…) sans le réécrire :
 * on injecte du code dans son shader via onBeforeCompile. Les uniforms sont partagés par toutes les pièces
 * d'un même modèle : une seule valeur pilote la dissolution du champion entier.
 */
export function makeDissolvable<M extends THREE.Material>(mat: M, u: DissolveUniforms): M {
  mat.onBeforeCompile = (shader) => {
    patchVertex(shader, u);
    shader.fragmentShader = shader.fragmentShader
      .replace('#include <common>', `#include <common>\n${DISSOLVE_PARS}`)
      .replace(
        '#include <dithering_fragment>',
        `#include <dithering_fragment>
if (uDissolve > 0.0) {
  float dn = dissolveValue();
  if (dn < uDissolve) discard;
  gl_FragColor.rgb = mix(gl_FragColor.rgb, uEdgeColor, 1.0 - smoothstep(0.0, 0.07, dn - uDissolve));
}`,
      );
  };
  mat.customProgramCacheKey = () => 'lov-dissolve';
  return mat;
}

/** Matériau d'ombre (profondeur) qui se dissout avec le modèle. */
export function makeDissolveDepthMaterial(u: DissolveUniforms): THREE.MeshDepthMaterial {
  const mat = new THREE.MeshDepthMaterial({ depthPacking: THREE.RGBADepthPacking });
  mat.onBeforeCompile = (shader) => {
    patchVertex(shader, u);
    shader.fragmentShader = shader.fragmentShader
      .replace('#include <common>', `#include <common>\n${DISSOLVE_PARS}`)
      .replace('#include <clipping_planes_fragment>', '#include <clipping_planes_fragment>\nif (uDissolve > 0.0 && dissolveValue() < uDissolve) discard;');
  };
  mat.customProgramCacheKey = () => 'lov-dissolve-depth';
  return mat;
}

/** Rend dissolvables toutes les pièces d'un modèle (matériaux + ombres). */
export function applyDissolve(root: THREE.Object3D, u: DissolveUniforms): void {
  const depth = makeDissolveDepthMaterial(u);
  root.traverse((o) => {
    if (!(o instanceof THREE.Mesh) || o.userData.noDissolve) return;
    const mats = Array.isArray(o.material) ? o.material : [o.material];
    for (const m of mats) makeDissolvable(m as THREE.Material, u);
    o.customDepthMaterial = depth;
  });
}
