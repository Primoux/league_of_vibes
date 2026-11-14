/**
 * Sol de la carte : géométrie avec relief + matériaux procéduraux. Propriétaire : claude-client
 * (refonte graphique claude-fx).
 *
 * Un canvas basse résolution ne porte plus des couleurs mais des **masques** (voies, sable/lit de rivière,
 * dallage des bases, occlusion) ; le shader compose herbe, terre, cailloux, sable humide, pavés et caustiques
 * à partir d'une texture de bruit échantillonnée en coordonnées monde. Résultat : détail net à tout zoom
 * (l'ancienne texture couleur faisait ~6 unités de jeu par pixel) et bords de chemins organiques.
 */
import * as THREE from 'three';
import { BRUSHES, FOUNTAINS, LANE_HALF_WIDTH, LANE_PATHS, MAP_SIZE, PITS, RIVER_HALF_WIDTH, WALLS, type Polygon } from '@lov/shared';
import { S } from '../coords.ts';
import { terrainHeight } from './geometry.ts';
import { applyFog } from '../FogOfWar.ts';
import { decorTime, noiseTexture } from './noise.ts';

const TEX = 1024;
/** Le sol déborde de la carte pour remplir la vue aux bords. */
export const GROUND_MARGIN = 2000;

/** Couleurs sRGB (converties en linéaire par THREE.Color). */
const PALETTE = {
  grass: 0x5a8c3e,
  grassDark: 0x356a31,
  grassDry: 0x9aa64a,
  dirt: 0xb4935f,
  dirtDark: 0x8a6c47,
  pebble: 0xd8ccb0,
  sand: 0xcdb98a,
  sandWet: 0x6d6a52,
  stoneA: 0x9d9584,
  stoneB: 0xb8ae97,
  mortar: 0x5a5244,
  caustic: 0xbff4ff,
  teamBlue: 0x3d8bff,
  teamRed: 0xff4a3d,
};

const lin = (hex: number) => new THREE.Color(hex);

export function buildTerrain(quality: 'low' | 'high'): THREE.Mesh {
  const size = MAP_SIZE + GROUND_MARGIN * 2;
  const seg = quality === 'high' ? 200 : 120;
  const geo = new THREE.PlaneGeometry(size * S, size * S, seg, seg).rotateX(-Math.PI / 2);
  const pos = geo.attributes.position as THREE.BufferAttribute;
  for (let i = 0; i < pos.count; i++) {
    const gx = pos.getX(i) / S + MAP_SIZE / 2;
    const gy = -pos.getZ(i) / S + MAP_SIZE / 2;
    pos.setY(i, terrainHeight(gx, gy) * S);
  }
  geo.computeVertexNormals();
  // UV : les masques couvrent exactement la carte (0..MAP_SIZE), le bord déborde en ClampToEdge
  const uv = geo.attributes.uv as THREE.BufferAttribute;
  for (let i = 0; i < uv.count; i++) {
    const gx = pos.getX(i) / S + MAP_SIZE / 2;
    const gy = -pos.getZ(i) / S + MAP_SIZE / 2;
    uv.setXY(i, gx / MAP_SIZE, gy / MAP_SIZE);
  }
  const splat = paintMasks();
  // `map` n'est pas une couleur : il active simplement vMapUv dans le shader standard (masques en linéaire)
  const mat = new THREE.MeshStandardMaterial({ map: splat, roughness: 0.93, metalness: 0 });
  const u = {
    uNoise: { value: noiseTexture() },
    uTime: decorTime,
    uFountB: { value: new THREE.Vector2(FOUNTAINS.blue.x * S, -FOUNTAINS.blue.y * S) },
    uFountR: { value: new THREE.Vector2(FOUNTAINS.red.x * S, -FOUNTAINS.red.y * S) },
    uGrass: { value: lin(PALETTE.grass) },
    uGrassDark: { value: lin(PALETTE.grassDark) },
    uGrassDry: { value: lin(PALETTE.grassDry) },
    uDirt: { value: lin(PALETTE.dirt) },
    uDirtDark: { value: lin(PALETTE.dirtDark) },
    uPebble: { value: lin(PALETTE.pebble) },
    uSand: { value: lin(PALETTE.sand) },
    uSandWet: { value: lin(PALETTE.sandWet) },
    uStoneA: { value: lin(PALETTE.stoneA) },
    uStoneB: { value: lin(PALETTE.stoneB) },
    uMortar: { value: lin(PALETTE.mortar) },
    uCaustic: { value: lin(PALETTE.caustic) },
    uTeamB: { value: lin(PALETTE.teamBlue) },
    uTeamR: { value: lin(PALETTE.teamRed) },
  };
  mat.onBeforeCompile = (sh) => {
    Object.assign(sh.uniforms, u);
    sh.vertexShader = sh.vertexShader
      .replace('#include <common>', '#include <common>\nvarying vec3 vTerrW;\nvarying float vTerrNy;')
      .replace('#include <worldpos_vertex>', '#include <worldpos_vertex>\nvTerrW = (modelMatrix * vec4(transformed, 1.0)).xyz;\nvTerrNy = objectNormal.y;');
    sh.fragmentShader = sh.fragmentShader
      .replace('#include <common>', `#include <common>\n${TERRAIN_DECL}`)
      .replace('#include <map_fragment>', TERRAIN_MAP)
      .replace('#include <emissivemap_fragment>', '#include <emissivemap_fragment>\ntotalEmissiveRadiance += terrGlow;');
  };
  mat.customProgramCacheKey = () => 'terrain2';
  applyFog(mat);
  const mesh = new THREE.Mesh(geo, mat);
  mesh.position.set((MAP_SIZE / 2) * S, 0, (-MAP_SIZE / 2) * S);
  mesh.receiveShadow = true;
  mesh.name = 'terrain';
  return mesh;
}

const TERRAIN_DECL = /* glsl */ `
uniform sampler2D uNoise;
uniform float uTime;
uniform vec2 uFountB, uFountR;
uniform vec3 uGrass, uGrassDark, uGrassDry, uDirt, uDirtDark, uPebble, uSand, uSandWet;
uniform vec3 uStoneA, uStoneB, uMortar, uCaustic, uTeamB, uTeamR;
varying vec3 vTerrW;
varying float vTerrNy;
float terrHash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
`;

/**
 * Composition des matériaux (remplace map_fragment). Masques : r = voie, g = sable/rivière, b = dallage,
 * a = occlusion (pied des murs, sol des brousses). Unités monde : 1 = 100 unités de jeu.
 */
const TERRAIN_MAP = /* glsl */ `
vec4 sp = texture2D(map, vMapUv);
vec2 wp = vTerrW.xz;
float nL = texture2D(uNoise, wp * 0.011).r;
float nL2 = texture2D(uNoise, wp * 0.0041 + 0.37).r;
float nM = texture2D(uNoise, wp * 0.045).g;
float nH = texture2D(uNoise, wp * 0.31).b;
float nC = texture2D(uNoise, wp * 0.23).a;
vec3 terrGlow = vec3(0.0);

// herbe : deux tons + zones plus sèches, grain fin
float tone = nL * 0.6 + nL2 * 0.4;
vec3 col = mix(uGrassDark, uGrass, smoothstep(0.28, 0.72, tone));
col = mix(col, uGrassDry, smoothstep(0.55, 0.85, nM) * smoothstep(0.45, 0.8, nL2) * 0.5);
col *= 0.84 + 0.3 * nH;

// terre des voies : bords bruités, ornières sombres, cailloux
float laneV = sp.r + (nM - 0.5) * 0.34 + (nH - 0.5) * 0.12;
float laneM = smoothstep(0.43, 0.55, laneV);
vec3 dirt = mix(uDirtDark, uDirt, smoothstep(0.2, 0.85, nL + (nH - 0.5) * 0.4));
dirt *= 1.0 - (1.0 - smoothstep(0.5, 0.72, laneV)) * 0.22;
float pebble = (1.0 - smoothstep(0.07, 0.15, nC)) * step(0.52, texture2D(uNoise, wp * 0.06 + 0.5).b);
dirt = mix(dirt, uPebble * (0.75 + 0.4 * nH), pebble * 0.85);
col = mix(col, dirt, laneM);
// liseré d'herbe foncée au bord du chemin (herbe piétinée)
col *= 1.0 - (smoothstep(0.3, 0.43, laneV) - laneM) * 0.18;

// sable des berges et lit de la rivière (humide sous l'eau), plus de sable dans les pentes
float slope = 1.0 - clamp(vTerrNy, 0.0, 1.0);
float sandM = smoothstep(0.38, 0.6, sp.g + (nM - 0.5) * 0.3 + slope * 1.5);
float wet = 1.0 - smoothstep(-0.2, -0.06, vTerrW.y);
vec3 sand = mix(uSand, uSandWet, wet) * (0.86 + 0.26 * nH);
col = mix(col, sand, sandM);

// dallage circulaire des bases (calculé : joints nets à tout zoom)
if (sp.b > 0.02) {
  vec2 fb = wp - uFountB;
  vec2 fr = wp - uFountR;
  bool blue = dot(fb, fb) < dot(fr, fr);
  vec2 d = blue ? fb : fr;
  float r = length(d);
  float ringW = 1.9;
  float ri = floor(r / ringW);
  float fri = fract(r / ringW);
  float circ = 6.2831853 * max(ri + 0.5, 0.5) * ringW;
  float nst = max(5.0, floor(circ / 2.3));
  float seg = (atan(d.y, d.x) / 6.2831853 + 0.5) * nst + ri * 0.5;
  float fs = fract(seg);
  float cell = terrHash(vec2(ri, floor(seg)) + (blue ? 0.0 : 17.0));
  float edge = min(min(fri, 1.0 - fri) * ringW, min(fs, 1.0 - fs) * circ / nst);
  float joint = 1.0 - smoothstep(0.022, 0.055, edge + (nH - 0.5) * 0.03);
  vec3 stone = mix(uStoneA, uStoneB, cell) * (0.8 + 0.32 * nH) * (0.92 + 0.16 * terrHash(vec2(floor(seg), ri) + 3.1));
  // pierres légèrement bombées : bords plus sombres, mousse dans certains joints
  stone *= 0.8 + 0.2 * smoothstep(0.03, 0.35, edge);
  stone = mix(stone, mix(uMortar, uGrassDark * 0.8, step(0.6, nM) * 0.7), joint);
  // incrustations lumineuses aux couleurs de l'équipe
  vec3 team = blue ? uTeamB : uTeamR;
  float inlay = (1.0 - smoothstep(0.05, 0.13, abs(r - 6.5))) + (1.0 - smoothstep(0.06, 0.16, abs(r - 22.6)));
  stone = mix(stone, team * 0.6, inlay * 0.8);
  float pm = smoothstep(0.45, 0.6, sp.b + (nH - 0.5) * 0.1);
  terrGlow = team * inlay * pm * 0.55;
  col = mix(col, stone, pm);
}

// occlusion : pied des murs, sol des brousses
col *= 1.0 - sp.a * 0.6;

// caustiques animées sur le lit de la rivière
if (wet > 0.0) {
  float c1 = texture2D(uNoise, wp * 0.33 + vec2(uTime * 0.045, uTime * 0.03)).a;
  float c2 = texture2D(uNoise, wp * 0.27 - vec2(uTime * 0.035, -uTime * 0.025)).a;
  float ca = smoothstep(0.42, 0.8, c1) * smoothstep(0.42, 0.8, c2);
  col += uCaustic * ca * wet * 0.45;
}
diffuseColor.rgb *= col;
`;

const K = TEX / MAP_SIZE;
const cx = (x: number) => x * K;
const cy = (y: number) => (MAP_SIZE - y) * K;

function pathPoly(ctx: CanvasRenderingContext2D, p: Polygon): void {
  ctx.beginPath();
  p.forEach((q, i) => (i ? ctx.lineTo(cx(q.x), cy(q.y)) : ctx.moveTo(cx(q.x), cy(q.y))));
  ctx.closePath();
}

/** Un masque en niveaux de gris : `draw` peint en blanc sur fond noir. */
function mask(draw: (ctx: CanvasRenderingContext2D) => void): Uint8ClampedArray {
  const c = document.createElement('canvas');
  c.width = c.height = TEX;
  const ctx = c.getContext('2d', { willReadFrequently: true })!;
  ctx.fillStyle = '#000';
  ctx.fillRect(0, 0, TEX, TEX);
  ctx.fillStyle = ctx.strokeStyle = '#fff';
  draw(ctx);
  return ctx.getImageData(0, 0, TEX, TEX).data;
}

/**
 * Masques RGBA (r voie, g sable, b dallage, a occlusion). Les masques sont flous : le seuil 0,5 du shader
 * tombe sur le bord réel, et le bruit qui le décale donne des contours organiques.
 */
function paintMasks(): THREE.DataTexture {
  const lane = mask((ctx) => {
    ctx.filter = 'blur(5px)';
    ctx.lineJoin = ctx.lineCap = 'round';
    ctx.lineWidth = LANE_HALF_WIDTH * K * 2;
    for (const p of Object.values(LANE_PATHS)) {
      ctx.beginPath();
      p.forEach((q, i) => (i ? ctx.lineTo(cx(q.x), cy(q.y)) : ctx.moveTo(cx(q.x), cy(q.y))));
      ctx.stroke();
    }
  });
  const sand = mask((ctx) => {
    ctx.filter = 'blur(7px)';
    ctx.lineCap = 'butt';
    ctx.lineWidth = (RIVER_HALF_WIDTH + 90) * K * 2;
    ctx.beginPath();
    ctx.moveTo(cx(1350), cy(10650));
    ctx.lineTo(cx(10650), cy(1350));
    ctx.stroke();
    for (const pit of Object.values(PITS)) {
      ctx.beginPath();
      ctx.arc(cx(pit.pos.x), cy(pit.pos.y), pit.radius * 0.9 * K, 0, Math.PI * 2);
      ctx.fill();
    }
  });
  const paving = mask((ctx) => {
    ctx.filter = 'blur(2px)';
    for (const team of ['blue', 'red'] as const) {
      const f = FOUNTAINS[team];
      ctx.beginPath();
      ctx.arc(cx(f.x), cy(f.y), 2330 * K, 0, Math.PI * 2);
      ctx.fill();
    }
  });
  const occl = mask((ctx) => {
    ctx.filter = 'blur(6px)';
    ctx.globalAlpha = 0.75;
    for (const w of WALLS) {
      pathPoly(ctx, w);
      ctx.fill();
    }
    ctx.filter = 'blur(3px)';
    ctx.globalAlpha = 0.4;
    for (const b of BRUSHES) {
      pathPoly(ctx, b);
      ctx.fill();
    }
  });
  const data = new Uint8Array(TEX * TEX * 4);
  // le canvas a l'origine en haut : on retourne les lignes pour une DataTexture (origine en bas, flipY inopérant)
  for (let y = 0; y < TEX; y++) {
    const src = (TEX - 1 - y) * TEX * 4;
    const dst = y * TEX * 4;
    for (let x = 0; x < TEX * 4; x += 4) {
      data[dst + x] = lane[src + x]!;
      data[dst + x + 1] = sand[src + x]!;
      data[dst + x + 2] = paving[src + x]!;
      data[dst + x + 3] = occl[src + x]!;
    }
  }
  const tex = new THREE.DataTexture(data, TEX, TEX, THREE.RGBAFormat);
  tex.colorSpace = THREE.NoColorSpace;
  tex.wrapS = tex.wrapT = THREE.ClampToEdgeWrapping;
  tex.magFilter = THREE.LinearFilter;
  tex.minFilter = THREE.LinearMipmapLinearFilter;
  tex.generateMipmaps = true;
  tex.anisotropy = 4;
  tex.needsUpdate = true;
  return tex;
}
