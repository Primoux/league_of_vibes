/**
 * Brouillard de guerre côté client. Propriétaire : claude-client.
 *
 * Texture de visibilité (canvas 2D basse résolution couvrant la carte) recalculée ~12 fois/s à partir des
 * entités alliées (rayons PLAN §9). La vision révélée apparaît immédiatement et s'estompe en douceur quand
 * elle est perdue. Elle assombrit/désature le décor via `applyFog(material)` (injection de shader).
 * Le serveur ne transmet pas les ennemis invisibles : le brouillard est purement visuel.
 *
 * `fog.canvas` (blanc = visible) est réutilisable par la minimap (mistral-ui).
 */
import * as THREE from 'three';
import { MAP_SIZE, type EntitySnapshot, type Team } from '@lov/shared';
import { S } from './coords.ts';

const RES = 192;
const K = RES / MAP_SIZE;
/** Luminosité des zones non visibles. */
const DARK = 0.42;

export function visionRadius(e: EntitySnapshot): number {
  switch (e.kind) {
    case 'champion':
      return 1350;
    case 'minion':
      return 1100;
    case 'tower':
      return 1350;
    case 'inhibitor':
    case 'nexus':
      return 1000;
    case 'ward':
      return 900;
    default:
      return 0;
  }
}

/** Uniforms partagés par tous les matériaux brouillés. */
export const fogUniforms = {
  uFow: { value: null as THREE.Texture | null },
  uFowDark: { value: 1 }, // 1 = brouillard désactivé (hors partie)
  uFowMap: { value: MAP_SIZE * S },
};

/**
 * Injecte le brouillard dans un MeshStandardMaterial (ou tout matériau « lit » de three) : la couleur finale
 * est multipliée selon la visibilité au point monde. Compatible instancing. Chaînable avec un onBeforeCompile existant.
 */
export function applyFog(mat: THREE.Material): void {
  const prev = mat.onBeforeCompile.bind(mat);
  const prevKey = mat.customProgramCacheKey.bind(mat);
  mat.customProgramCacheKey = () => `${prevKey()}|fow`;
  mat.onBeforeCompile = (sh, r) => {
    prev(sh, r);
    Object.assign(sh.uniforms, fogUniforms);
    sh.vertexShader = sh.vertexShader
      .replace('#include <common>', '#include <common>\nuniform float uFowMap;\nvarying vec2 vFowUv;')
      .replace(
        '#include <project_vertex>',
        `#include <project_vertex>
        #ifdef USE_INSTANCING
          vec4 fowW = modelMatrix * instanceMatrix * vec4(transformed, 1.0);
        #else
          vec4 fowW = modelMatrix * vec4(transformed, 1.0);
        #endif
        vFowUv = vec2(fowW.x / uFowMap, -fowW.z / uFowMap);`,
      );
    sh.fragmentShader = sh.fragmentShader
      .replace('#include <common>', '#include <common>\nuniform sampler2D uFow;\nuniform float uFowDark;\nvarying vec2 vFowUv;')
      .replace(
        '#include <opaque_fragment>',
        `#include <opaque_fragment>
        float fowV = uFowDark >= 1.0 ? 1.0 : texture2D(uFow, vFowUv).r;
        vec3 fowGrey = vec3(dot(gl_FragColor.rgb, vec3(0.299, 0.587, 0.114)));
        gl_FragColor.rgb = mix(mix(fowGrey, gl_FragColor.rgb, 0.45) * uFowDark * vec3(0.85, 0.9, 1.05), gl_FragColor.rgb, fowV);`,
      );
  };
  mat.needsUpdate = true;
}

/** Version pour les ShaderMaterial maison : à appeler dans le fragment shader. */
export const FOG_GLSL = {
  vertexDecl: 'uniform float uFowMap;\nvarying vec2 vFowUv;',
  vertexMain: (worldVar: string) => `vFowUv = vec2(${worldVar}.x / uFowMap, -${worldVar}.z / uFowMap);`,
  fragmentDecl: 'uniform sampler2D uFow;\nuniform float uFowDark;\nvarying vec2 vFowUv;',
  fragmentApply: (colorVar: string) => `{ float fowV = uFowDark >= 1.0 ? 1.0 : texture2D(uFow, vFowUv).r; ${colorVar} = mix(${colorVar} * uFowDark, ${colorVar}, fowV); }`,
};

export class FogOfWar {
  readonly canvas: HTMLCanvasElement;
  readonly texture: THREE.CanvasTexture;
  private ctx: CanvasRenderingContext2D;
  private acc = 0;
  private brush: HTMLCanvasElement;

  constructor() {
    this.canvas = document.createElement('canvas');
    this.canvas.width = this.canvas.height = RES;
    this.ctx = this.canvas.getContext('2d')!;
    this.ctx.fillStyle = '#000';
    this.ctx.fillRect(0, 0, RES, RES);
    this.texture = new THREE.CanvasTexture(this.canvas);
    this.texture.magFilter = THREE.LinearFilter;
    this.texture.minFilter = THREE.LinearFilter;
    this.texture.generateMipmaps = false;
    this.texture.flipY = true;
    fogUniforms.uFow.value = this.texture;
    // tampon : disque à bord doux réutilisé pour chaque source de vision
    this.brush = document.createElement('canvas');
    this.brush.width = this.brush.height = 64;
    const b = this.brush.getContext('2d')!;
    const g = b.createRadialGradient(32, 32, 0, 32, 32, 32);
    g.addColorStop(0, 'rgba(255,255,255,1)');
    g.addColorStop(0.82, 'rgba(255,255,255,1)');
    g.addColorStop(1, 'rgba(255,255,255,0)');
    b.fillStyle = g;
    b.fillRect(0, 0, 64, 64);
  }

  setEnabled(on: boolean): void {
    fogUniforms.uFowDark.value = on ? DARK : 1;
  }

  /** Recalcule la visibilité (≈ 12 Hz). `positions(id)` donne la position affichée d'une entité. */
  update(dt: number, entities: Iterable<EntitySnapshot>, myTeam: Team | undefined, pos: (e: EntitySnapshot) => { x: number; y: number } | null): void {
    this.acc += dt;
    if (this.acc < 1 / 12 || !myTeam) return;
    this.acc = 0;
    const ctx = this.ctx;
    // estompe l'ancienne vision (≈ 0,5 s pour disparaître)
    ctx.globalCompositeOperation = 'source-over';
    ctx.globalAlpha = 0.3;
    ctx.fillStyle = '#000';
    ctx.fillRect(0, 0, RES, RES);
    ctx.globalAlpha = 1;
    ctx.globalCompositeOperation = 'lighter';
    for (const e of entities) {
      if (e.team !== myTeam || e.dead) continue;
      const r = visionRadius(e);
      if (!r) continue;
      const p = pos(e) ?? e;
      const rr = r * K;
      ctx.drawImage(this.brush, p.x * K - rr, (MAP_SIZE - p.y) * K - rr, rr * 2, rr * 2);
    }
    ctx.globalCompositeOperation = 'source-over';
    this.texture.needsUpdate = true;
  }
}
