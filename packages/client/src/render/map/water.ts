/**
 * Rivière animée (shader eau stylisé). Propriétaire : claude-client.
 * Bande le long de x + y = MAP_SIZE, courant vers le bas-droite, reflets et écume sur les berges.
 */
import * as THREE from 'three';
import { MAP_SIZE, RIVER_HALF_WIDTH } from '@lov/shared';
import { S } from '../coords.ts';
import { RIVER_DEPTH } from './geometry.ts';

const vert = /* glsl */ `
varying vec2 vUv;
varying vec3 vWorld;
void main() {
  vUv = uv;
  vec4 w = modelMatrix * vec4(position, 1.0);
  vWorld = w.xyz;
  gl_Position = projectionMatrix * viewMatrix * w;
}`;

const frag = /* glsl */ `
uniform float uTime;
uniform vec2 uSize; // longueur, largeur (monde)
uniform vec3 uDeep;
uniform vec3 uShallow;
uniform vec3 uFoam;
varying vec2 vUv;
varying vec3 vWorld;

float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
float noise(vec2 p) {
  vec2 i = floor(p), f = fract(p);
  vec2 u = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash(i), hash(i + vec2(1, 0)), u.x), mix(hash(i + vec2(0, 1)), hash(i + vec2(1, 1)), u.x), u.y);
}

void main() {
  vec2 p = vUv * uSize;
  float across = abs(vUv.y - 0.5) * 2.0; // 0 au centre, 1 aux berges
  float ends = min(vUv.x, 1.0 - vUv.x) * uSize.x; // distance aux extrémités
  float n1 = noise(p * 0.9 + vec2(uTime * 0.6, 0.0));
  float n2 = noise(p * 2.3 - vec2(uTime * 0.9, uTime * 0.2));
  float waves = n1 * 0.6 + n2 * 0.4;
  vec3 col = mix(uDeep, uShallow, smoothstep(0.2, 1.0, across) * 0.7 + waves * 0.25);
  // reflets scintillants
  float sparkle = smoothstep(0.78, 0.95, noise(p * 4.0 + vec2(uTime * 1.3, -uTime * 0.4)) * waves);
  col += sparkle * 0.35;
  // écume des berges
  float foamLine = smoothstep(0.80, 0.97, across + (n2 - 0.5) * 0.12);
  col = mix(col, uFoam, foamLine * 0.6);
  float alpha = 0.78 * (1.0 - smoothstep(0.96, 1.0, across)) * smoothstep(0.0, 4.0, ends);
  gl_FragColor = vec4(col, alpha);
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
        uTime: { value: 0 },
        uSize: { value: new THREE.Vector2(len * S, width * S) },
        uDeep: { value: new THREE.Color(0x1d5a86) },
        uShallow: { value: new THREE.Color(0x4fa8c4) },
        uFoam: { value: new THREE.Color(0xe6f6ff) },
      },
    });
    this.mesh = new THREE.Mesh(new THREE.PlaneGeometry(len * S, width * S).rotateX(-Math.PI / 2), this.mat);
    this.mesh.position.set((MAP_SIZE / 2) * S, -RIVER_DEPTH * 0.45 * S, (-MAP_SIZE / 2) * S);
    // axe x local le long de la rivière : direction jeu (1, -1) -> monde (1, 0, 1)
    this.mesh.rotation.y = -Math.PI / 4;
    this.mesh.renderOrder = 1;
    this.mesh.name = 'water';
  }

  update(dt: number): void {
    this.mat.uniforms.uTime!.value += dt;
  }
}
