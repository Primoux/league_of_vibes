/**
 * Rivière animée (shader eau stylisé). Propriétaire : claude-client (refonte graphique claude-fx).
 * Bande le long de x + y = MAP_SIZE, courant vers le bas-droite.
 *
 * Profondeur simulée par la distance aux berges (bords transparents : on voit le lit sableux et ses
 * caustiques, rendus par le shader du sol), pseudo-normales issues de deux couches de bruit qui défilent,
 * reflet du ciel (Fresnel), reflets du soleil en HDR (captés par le bloom), écume des berges, filets de courant.
 * Tout le bruit vient de la texture partagée : 5 lectures de texture, aucun bruit calculé.
 */
import * as THREE from 'three';
import { MAP_SIZE, RIVER_HALF_WIDTH } from '@lov/shared';
import { S } from '../coords.ts';
import { RIVER_DEPTH } from './geometry.ts';
import { FOG_GLSL, fogUniforms } from '../FogOfWar.ts';
import { decorTime, noiseTexture } from './noise.ts';

const vert = /* glsl */ `
${FOG_GLSL.vertexDecl}
varying vec2 vUv;
varying vec3 vWorld;
void main() {
  vUv = uv;
  vec4 w = modelMatrix * vec4(position, 1.0);
  vWorld = w.xyz;
  ${FOG_GLSL.vertexMain('w')}
  gl_Position = projectionMatrix * viewMatrix * w;
}`;

const frag = /* glsl */ `
${FOG_GLSL.fragmentDecl}
uniform sampler2D uNoise;
uniform float uTime;
uniform vec2 uSize; // longueur, largeur (monde)
uniform vec3 uDeep;
uniform vec3 uShallow;
uniform vec3 uFoam;
uniform vec3 uSky;
uniform vec3 uSun;
uniform vec3 uSunDir;
varying vec2 vUv;
varying vec3 vWorld;

void main() {
  vec2 p = vUv * uSize;
  float t = uTime;
  float across = abs(vUv.y - 0.5) * 2.0; // 0 au centre, 1 aux berges
  float ends = min(vUv.x, 1.0 - vUv.x) * uSize.x;
  float depth = 1.0 - across * across;

  // deux couches de vagues qui défilent avec le courant (+x local)
  vec4 a = texture2D(uNoise, p * vec2(0.07, 0.11) + vec2(-t * 0.05, t * 0.012));
  vec4 b = texture2D(uNoise, p * vec2(0.16, 0.21) + vec2(-t * 0.11, -t * 0.02));
  vec3 n = normalize(vec3((a.g - 0.5) * 0.4 + (b.b - 0.5) * 0.25, 1.0, (a.b - 0.5) * 0.4 + (b.g - 0.5) * 0.25));
  vec3 v = normalize(cameraPosition - vWorld);

  vec3 col = mix(uShallow, uDeep, smoothstep(0.05, 0.85, depth + (a.r - 0.5) * 0.15));
  col *= 0.94 + 0.12 * b.g;
  // filets de courant étirés dans le sens de l'écoulement
  float streak = smoothstep(0.72, 0.9, texture2D(uNoise, vec2(p.x * 0.035 - t * 0.09, p.y * 0.45)).g);
  col += uFoam * streak * 0.045 * depth;
  // reflet du ciel selon l'angle, reflets du soleil (HDR : ils passent le seuil du bloom)
  float fres = pow(1.0 - clamp(dot(n, v), 0.0, 1.0), 4.0);
  col = mix(col, uSky, clamp(fres * 1.6, 0.0, 0.55));
  vec3 h = normalize(uSunDir + v);
  float spec = pow(clamp(dot(n, h), 0.0, 1.0), 220.0);
  col += uSun * spec * 1.4;
  float glint = smoothstep(0.93, 0.99, b.a) * smoothstep(0.62, 0.9, a.g) * depth;
  col += uSun * glint * 0.6;
  // écume des berges, animée
  float foamBand = smoothstep(0.74, 0.95, across + (a.g - 0.5) * 0.18);
  float foamTex = smoothstep(0.35, 0.65, texture2D(uNoise, p * 0.45 + vec2(-t * 0.16, t * 0.05)).a);
  float foam = foamBand * mix(0.45, 1.0, foamTex);
  col = mix(col, uFoam, foam * 0.75);
  float alpha = mix(0.42, 0.86, smoothstep(0.0, 0.65, depth));
  alpha = max(alpha, foam * 0.85);
  alpha *= (1.0 - smoothstep(0.97, 1.0, across)) * smoothstep(0.0, 4.0, ends);
  ${FOG_GLSL.fragmentApply('col')}
  gl_FragColor = vec4(col, alpha);
  #include <tonemapping_fragment>
  #include <colorspace_fragment>
}`;

export class Water {
  readonly mesh: THREE.Mesh;
  private mat: THREE.ShaderMaterial;

  constructor() {
    const len = 9600 * Math.SQRT2 * 0.94; // de l'embouchure top à l'embouchure bot
    const width = (RIVER_HALF_WIDTH + 60) * 2;
    this.mat = new THREE.ShaderMaterial({
      vertexShader: vert,
      fragmentShader: frag,
      transparent: true,
      depthWrite: false,
      uniforms: {
        ...fogUniforms,
        uNoise: { value: noiseTexture() },
        uTime: decorTime,
        uSize: { value: new THREE.Vector2(len * S, width * S) },
        uDeep: { value: new THREE.Color(0x0f4f72) },
        uShallow: { value: new THREE.Color(0x3fb0bf) },
        uFoam: { value: new THREE.Color(0xeefbff) },
        uSky: { value: new THREE.Color(0xa8d8f0) },
        uSun: { value: new THREE.Color(0xfff0d0) },
        uSunDir: { value: new THREE.Vector3(14, 30, 9).normalize() },
      },
    });
    this.mesh = new THREE.Mesh(new THREE.PlaneGeometry(len * S, width * S).rotateX(-Math.PI / 2), this.mat);
    this.mesh.position.set((MAP_SIZE / 2) * S, -RIVER_DEPTH * 0.45 * S, (-MAP_SIZE / 2) * S);
    // axe x local le long de la rivière : direction jeu (1, -1) -> monde (1, 0, 1)
    this.mesh.rotation.y = -Math.PI / 4;
    this.mesh.renderOrder = 1;
    this.mesh.name = 'water';
  }

  /** Le temps est partagé avec le décor (`decorTime`, avancé par la végétation). */
  update(_dt: number): void {}
}
