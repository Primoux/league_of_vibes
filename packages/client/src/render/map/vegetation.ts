/**
 * Murs (falaises rocheuses + forêt), bordure de forêt, brousses (herbes hautes animées), rochers, touffes et
 * fleurs au sol. Propriétaire : claude-client (refonte graphique claude-fx).
 *
 * Tout est instancié ; les couleurs sont dans des couleurs de sommets (dégradés d'occlusion : base sombre,
 * pointes claires) et la couleur d'instance n'est qu'une légère teinte, ce qui permet de fusionner tronc et
 * feuillage dans un seul mesh (1 draw call par variante et par tuile).
 */
import * as THREE from 'three';
import { BRUSHES, CAMPS, FOUNTAINS, LANE_HALF_WIDTH, MAP_SIZE, PITS, RIVER_HALF_WIDTH, WALLS, isInBrush, isInWall, pointInPolygon, type Polygon } from '@lov/shared';
import { S } from '../coords.ts';
import { distToLane, distToRiver, terrainHeight } from './geometry.ts';
import { rng } from './rng.ts';
import { GROUND_MARGIN } from './terrain.ts';
import { applyFog } from '../FogOfWar.ts';
import { decorTime, noiseTexture } from './noise.ts';

/** Uniform de temps partagé par les shaders de végétation (vent) — le même que le reste du décor. */
export const windTime = decorTime;

/**
 * Vent (balancement proportionnel au carré de la hauteur locale) + liseré lumineux de feuillage
 * (`rim` > 0 : les bords tournés à l'opposé de la caméra s'éclaircissent, effet de translucidité bon marché).
 */
function addWind(mat: THREE.MeshStandardMaterial, strength: number, rim = 0): void {
  mat.customProgramCacheKey = () => `wind${strength}|${rim}`;
  mat.onBeforeCompile = (sh) => {
    sh.uniforms.uTime = windTime;
    sh.vertexShader = sh.vertexShader
      .replace('#include <common>', '#include <common>\nuniform float uTime;')
      .replace(
        '#include <begin_vertex>',
        `#include <begin_vertex>
        #ifdef USE_INSTANCING
          vec3 ip = vec3(instanceMatrix[3][0], instanceMatrix[3][1], instanceMatrix[3][2]);
        #else
          vec3 ip = vec3(0.0);
        #endif
        float sway = max(position.y, 0.0);
        transformed.x += sin(uTime * 1.7 + ip.x * 1.3 + ip.z * 0.7) * ${strength.toFixed(3)} * sway * sway;
        transformed.z += cos(uTime * 1.3 + ip.z * 1.1) * ${(strength * 0.6).toFixed(3)} * sway * sway;`,
      );
    if (rim > 0)
      sh.fragmentShader = sh.fragmentShader.replace(
        '#include <emissivemap_fragment>',
        `#include <emissivemap_fragment>
        float rimF = 1.0 - clamp(dot(normal, normalize(vViewPosition)), 0.0, 1.0);
        totalEmissiveRadiance += diffuseColor.rgb * rimF * rimF * rimF * ${rim.toFixed(3)};`,
      );
  };
}

// ---------------------------------------------------------------- outils de géométrie

/** Bruit 3D déterministe (fonction de la position seule : les sommets dupliqués bougent ensemble). */
function hash3(x: number, y: number, z: number): number {
  let h = (Math.round(x * 997) * 374761393 + Math.round(y * 997) * 668265263 + Math.round(z * 997) * 1442695041) | 0;
  h = Math.imul(h ^ (h >>> 13), 1274126177);
  return ((h ^ (h >>> 16)) >>> 0) / 4294967296;
}

/** Fusion de géométries (position, normal, color ; couleur blanche si absente). */
function mergeGeometries(geos: THREE.BufferGeometry[]): THREE.BufferGeometry {
  const parts = geos.map((g) => (g.index ? g.toNonIndexed() : g));
  const out = new THREE.BufferGeometry();
  for (const name of ['position', 'normal', 'color'] as const) {
    const total = parts.reduce((n, g) => n + g.getAttribute('position').count * 3, 0);
    const arr = new Float32Array(total);
    let off = 0;
    for (const g of parts) {
      const a = g.getAttribute(name);
      if (a) arr.set(a.array as Float32Array, off);
      else arr.fill(1, off, off + g.getAttribute('position').count * 3);
      off += g.getAttribute('position').count * 3;
    }
    out.setAttribute(name, new THREE.BufferAttribute(arr, 3));
  }
  return out;
}

/** Géométrie non indexée, sommets décalés de façon déterministe, normales de facettes. */
function facet(g: THREE.BufferGeometry, jitter: number): THREE.BufferGeometry {
  const ng = g.index ? g.toNonIndexed() : g;
  ng.deleteAttribute('uv');
  const p = ng.getAttribute('position') as THREE.BufferAttribute;
  for (let i = 0; i < p.count; i++) {
    const x = p.getX(i);
    const y = p.getY(i);
    const z = p.getZ(i);
    p.setXYZ(i, x + (hash3(x, y, z) - 0.5) * jitter, y + (hash3(z, x, y) - 0.5) * jitter * 0.6, z + (hash3(y, z, x) - 0.5) * jitter);
  }
  ng.computeVertexNormals();
  return ng;
}

/** Couleurs de sommets : `fn(x, y, z, ny)` → couleur (linéaire). */
function paint(g: THREE.BufferGeometry, fn: (x: number, y: number, z: number, ny: number, out: THREE.Color) => void): THREE.BufferGeometry {
  const p = g.getAttribute('position') as THREE.BufferAttribute;
  const n = g.getAttribute('normal') as THREE.BufferAttribute;
  const col = new Float32Array(p.count * 3);
  const c = new THREE.Color();
  for (let i = 0; i < p.count; i++) {
    fn(p.getX(i), p.getY(i), p.getZ(i), n.getY(i), c);
    col[i * 3] = c.r;
    col[i * 3 + 1] = c.g;
    col[i * 3 + 2] = c.b;
  }
  g.setAttribute('color', new THREE.BufferAttribute(col, 3));
  return g;
}

const smooth = (a: number, b: number, x: number): number => {
  const t = Math.min(1, Math.max(0, (x - a) / (b - a)));
  return t * t * (3 - 2 * t);
};

// ---------------------------------------------------------------- arbres

const TRUNK = new THREE.Color(0x5a3f2a);
const BARK_DARK = new THREE.Color(0x2e2018);

function trunkGeo(h: number, r: number): THREE.BufferGeometry {
  const g = facet(new THREE.CylinderGeometry(r * 0.65, r, h, 5).translate(0, h / 2, 0), 0.02);
  return paint(g, (_x, y, _z, _ny, out) => out.copy(BARK_DARK).lerp(TRUNK, smooth(0, h * 0.8, y)));
}

/** Sapin : étages de cônes, chaque étage sombre à la base (ombre de l'étage supérieur) et clair à la pointe. */
function pineGeo(layers: [r: number, h: number, y: number][], green: number): THREE.BufferGeometry {
  const base = new THREE.Color(green);
  const dark = base.clone().multiplyScalar(0.42);
  const tip = base.clone().lerp(new THREE.Color(0xd8e88a), 0.25).multiplyScalar(1.25);
  const parts = [trunkGeo(0.6, 0.11)];
  for (const [r, h, y] of layers) {
    const g = facet(new THREE.ConeGeometry(r, h, 8, 2).translate(0, y + h / 2, 0), 0.09);
    parts.push(paint(g, (_x, py, _z, ny, out) => {
      const t = smooth(y, y + h, py);
      out.copy(dark).lerp(base, smooth(0, 0.45, t)).lerp(tip, smooth(0.55, 1, t) * 0.8);
      if (ny < -0.3) out.multiplyScalar(0.6);
    }));
  }
  return mergeGeometries(parts);
}

/** Feuillu : touffes sphériques facettées, ombre douce en dessous, lumière sur le dessus. */
function broadleafGeo(): THREE.BufferGeometry {
  const base = new THREE.Color(0x467f36);
  const dark = base.clone().multiplyScalar(0.4);
  const top = base.clone().lerp(new THREE.Color(0xd8e08a), 0.2).multiplyScalar(1.05);
  const parts = [trunkGeo(0.95, 0.12)];
  const blobs: [number, number, number, number][] = [
    [0, 1.45, 0, 0.62],
    [0.38, 1.2, 0.15, 0.45],
    [-0.32, 1.22, -0.2, 0.47],
    [0.05, 1.18, -0.38, 0.42],
    [-0.1, 1.85, 0.08, 0.42],
  ];
  for (const [x, y, z, r] of blobs) {
    const g = facet(new THREE.IcosahedronGeometry(r, 1).translate(x, y, z), 0.1);
    parts.push(paint(g, (_px, py, _pz, ny, out) => {
      out.copy(dark).lerp(base, smooth(0.8, 1.45, py)).lerp(top, smooth(1.4, 2.2, py) * 0.85);
      out.multiplyScalar(0.75 + 0.35 * Math.max(0, ny));
    }));
  }
  return mergeGeometries(parts);
}

/** Variantes d'arbres (sapin élancé, sapin trapu, feuillu). */
function treeVariants(): THREE.BufferGeometry[] {
  return [
    pineGeo([[0.66, 1.05, 0.45], [0.52, 0.9, 0.95], [0.36, 0.75, 1.42], [0.2, 0.5, 1.85]], 0x3c7f3e),
    pineGeo([[0.78, 0.9, 0.4], [0.58, 0.8, 0.85], [0.36, 0.7, 1.25]], 0x4a8a3a),
    broadleafGeo(),
  ];
}

interface TreeSpot {
  x: number;
  y: number;
  s: number;
}

function insetInside(p: { x: number; y: number }, poly: Polygon, inset: number): boolean {
  if (!pointInPolygon(p, poly)) return false;
  for (let a = 0; a < 6; a++) {
    if (!pointInPolygon({ x: p.x + Math.cos(a) * inset, y: p.y + Math.sin(a) * inset }, poly)) return false;
  }
  return true;
}

function wallTreeSpots(r: () => number): TreeSpot[] {
  const out: TreeSpot[] = [];
  for (const w of WALLS) {
    let minX = Infinity;
    let minY = Infinity;
    let maxX = -Infinity;
    let maxY = -Infinity;
    for (const p of w) {
      minX = Math.min(minX, p.x);
      minY = Math.min(minY, p.y);
      maxX = Math.max(maxX, p.x);
      maxY = Math.max(maxY, p.y);
    }
    const step = 125;
    for (let y = minY + 40; y < maxY; y += step) {
      for (let x = minX + 40; x < maxX; x += step) {
        const p = { x: x + (r() - 0.5) * 90, y: y + (r() - 0.5) * 90 };
        if (insetInside(p, w, 55)) out.push({ ...p, s: 0.85 + r() * 0.5 });
      }
    }
  }
  return out;
}

function borderTreeSpots(r: () => number): TreeSpot[] {
  const out: TreeSpot[] = [];
  const step = 210;
  for (let y = -GROUND_MARGIN + 100; y < MAP_SIZE + GROUND_MARGIN; y += step) {
    for (let x = -GROUND_MARGIN + 100; x < MAP_SIZE + GROUND_MARGIN; x += step) {
      const px = x + (r() - 0.5) * 140;
      const py = y + (r() - 0.5) * 140;
      const inside = px > 0 && py > 0 && px < MAP_SIZE && py < MAP_SIZE;
      const edge = Math.min(px, py, MAP_SIZE - px, MAP_SIZE - py);
      if (inside) {
        if (edge > 750) continue;
        if (distToLane(px, py) < LANE_HALF_WIDTH + 170) continue;
        if (isInBrush({ x: px, y: py })) continue;
        if (Math.hypot(px - FOUNTAINS.blue.x, py - FOUNTAINS.blue.y) < 2400) continue;
        if (Math.hypot(px - FOUNTAINS.red.x, py - FOUNTAINS.red.y) < 2400) continue;
        if (distToRiver(px, py) < 700 && Math.abs(px - py) < 9400) continue;
      } else if (edge < -1300) continue;
      out.push({ x: px, y: py, s: 1 + r() * 0.7 });
    }
  }
  return out;
}

/** Taille des tuiles de végétation (unités de jeu) : une InstancedMesh par tuile, cullée par la caméra. */
export const TREE_TILE = 2000;

/** Teintes d'instance (proches du blanc) : variété de verts sans casser les dégradés des sommets. */
const TREE_TINTS = [0xffffff, 0xe0f0d0, 0xfff2c8, 0xd2e6e0, 0xf0ffd8, 0xc8dcc0];

/** Regroupe des éléments par tuile (clé `tx,ty`). */
function byTile<T extends { x: number; y: number }>(items: T[], tile: number): Map<string, T[]> {
  const tiles = new Map<string, T[]>();
  for (const it of items) {
    const key = `${Math.floor(it.x / tile)},${Math.floor(it.y / tile)}`;
    let arr = tiles.get(key);
    if (!arr) tiles.set(key, (arr = []));
    arr.push(it);
  }
  return tiles;
}

function buildTrees(spots: TreeSpot[], r: () => number): THREE.Group {
  const variants = treeVariants();
  const mat = new THREE.MeshStandardMaterial({ vertexColors: true, roughness: 0.88 });
  addWind(mat, 0.025, 0.55);
  applyFog(mat);
  // tirages aléatoires d'abord (ordre stable quel que soit le découpage en tuiles)
  const placed = spots.map((t) => ({
    x: t.x,
    y: t.y,
    t,
    rot: r() * Math.PI * 2,
    sy: 0.9 + r() * 0.35,
    tint: TREE_TINTS[Math.floor(r() * TREE_TINTS.length)]!,
    light: (r() - 0.5) * 0.08,
    // les feuillus restent en lisière (bordure, bas des murs) : les murs sont surtout des sapins
    variant: r() < (isInWall(t) ? 0.18 : 0.4) ? 2 : r() < 0.55 ? 0 : 1,
  }));
  const m = new THREE.Matrix4();
  const q = new THREE.Quaternion();
  const up = new THREE.Vector3(0, 1, 0);
  const pos = new THREE.Vector3();
  const scl = new THREE.Vector3();
  const col = new THREE.Color();
  const g = new THREE.Group();
  for (const arr of byTile(placed, TREE_TILE).values()) {
    // une tuile entièrement hors de la carte n'est qu'un fond : pas d'ombre portée
    const inside = arr.some((p) => p.x > 0 && p.y > 0 && p.x < MAP_SIZE && p.y < MAP_SIZE);
    variants.forEach((geo, vi) => {
      const list = arr.filter((p) => p.variant === vi);
      if (!list.length) return;
      const im = new THREE.InstancedMesh(geo, mat, list.length);
      list.forEach((p, i) => {
        const t = p.t;
        q.setFromAxisAngle(up, p.rot);
        pos.set(t.x * S, terrainHeight(t.x, t.y) * S + (isInWall(t) ? 0.95 : 0), -t.y * S);
        scl.set(t.s, t.s * p.sy, t.s);
        m.compose(pos, q, scl);
        im.setMatrixAt(i, m);
        im.setColorAt(i, col.setHex(p.tint).offsetHSL(0, 0, p.light));
      });
      im.userData.backdrop = !inside;
      im.castShadow = inside;
      im.receiveShadow = true;
      im.computeBoundingSphere();
      g.add(im);
    });
  }
  return g;
}

// ---------------------------------------------------------------- murs (falaises rocheuses)

/** Insère des points le long des arêtes (pas ≈ `step` unités monde) pour pouvoir bosseler les parois. */
function subdivide(poly: Polygon, step: number): THREE.Vector2[] {
  const out: THREE.Vector2[] = [];
  for (let i = 0; i < poly.length; i++) {
    const a = poly[i]!;
    const b = poly[(i + 1) % poly.length]!;
    const n = Math.max(1, Math.round((Math.hypot(b.x - a.x, b.y - a.y) * S) / step));
    for (let k = 0; k < n; k++) out.push(new THREE.Vector2((a.x + ((b.x - a.x) * k) / n) * S, (a.y + ((b.y - a.y) * k) / n) * S));
  }
  return out;
}

/** Hauteur du dessus des murs (profondeur + biseau − enfoncement). */
const WALL_TOP = 0.8 + 0.3 - 0.08;

function buildWallBases(): THREE.Mesh {
  const rock = new THREE.Color(0x8d806a);
  const rockDark = new THREE.Color(0x4f463b);
  const rockLight = new THREE.Color(0xb3a589);
  const moss = new THREE.Color(0x2c5428);
  const mossLight = new THREE.Color(0x4a7a36);
  const geos: THREE.BufferGeometry[] = [];
  for (const poly of WALLS) {
    const shape = new THREE.Shape(subdivide(poly, 0.55));
    const g = new THREE.ExtrudeGeometry(shape, { depth: 0.8, steps: 2, bevelEnabled: true, bevelSize: 0.4, bevelThickness: 0.3, bevelSegments: 2, curveSegments: 1 });
    g.rotateX(-Math.PI / 2);
    g.translate(0, -0.08, 0);
    geos.push(g);
  }
  const merged = mergeGeometries(geos);
  // parois bosselées : décalage horizontal fort, vertical faible (le sommet reste praticable visuellement)
  const p = merged.getAttribute('position') as THREE.BufferAttribute;
  for (let i = 0; i < p.count; i++) {
    const x = p.getX(i);
    const y = p.getY(i);
    const z = p.getZ(i);
    const k = smooth(-0.1, 0.4, y);
    const n1 = hash3(Math.round(x * 2.2) / 2.2, Math.round(y * 3) / 3, Math.round(z * 2.2) / 2.2);
    // le dessus (capot triangulé en longues lamelles) reste plan : seul son contour bouge
    const ky = y > WALL_TOP - 0.01 ? 0 : k;
    p.setXYZ(i, x + (hash3(x, y, z) - 0.5) * 0.32 * k, y + (n1 - 0.5) * 0.14 * ky, z + (hash3(z, y, x) - 0.5) * 0.32 * k);
  }
  merged.computeVertexNormals();
  // couleurs : mousse sur le dessus, strates sur les parois, pied assombri (occlusion)
  paint(merged, (x, y, z, ny, out) => {
    if (ny > 0.72) {
      out.copy(moss).lerp(mossLight, hash3(Math.floor(x * 1.5), 0, Math.floor(z * 1.5)) * 0.8);
      return;
    }
    const strata = Math.sin(y * 13 + Math.sin(x * 0.7 + z * 0.5) * 1.6) * 0.5 + 0.5;
    out.copy(rockDark).lerp(rock, 0.55 + strata * 0.3).lerp(rockLight, hash3(Math.round(x * 4), Math.round(y * 6), Math.round(z * 4)) * 0.3);
    out.multiplyScalar(0.55 + 0.45 * smooth(-0.1, 0.55, y));
    if (ny > 0.35) out.lerp(moss, (ny - 0.35) * 1.2);
  });
  const mat = new THREE.MeshStandardMaterial({ vertexColors: true, roughness: 0.95 });
  // dessus : herbe/mousse calculée par pixel (bruit monde), comme le sol — pas d'aplat par triangle
  mat.onBeforeCompile = (sh) => {
    sh.uniforms.uNoise = { value: noiseTexture() };
    sh.uniforms.uMoss = { value: moss };
    sh.uniforms.uMossLight = { value: mossLight };
    sh.vertexShader = sh.vertexShader
      .replace('#include <common>', '#include <common>\nvarying vec3 vWallW;\nvarying float vWallNy;')
      .replace('#include <worldpos_vertex>', '#include <worldpos_vertex>\nvWallW = (modelMatrix * vec4(transformed, 1.0)).xyz;\nvWallNy = objectNormal.y;');
    sh.fragmentShader = sh.fragmentShader
      .replace('#include <common>', '#include <common>\nuniform sampler2D uNoise;\nuniform vec3 uMoss, uMossLight;\nvarying vec3 vWallW;\nvarying float vWallNy;')
      .replace(
        '#include <color_fragment>',
        `#include <color_fragment>
        if (vWallNy > 0.9) {
          vec2 wp = vWallW.xz;
          float n = texture2D(uNoise, wp * 0.011).r * 0.6 + texture2D(uNoise, wp * 0.05).g * 0.4;
          diffuseColor.rgb = mix(uMoss, uMossLight, smoothstep(0.25, 0.75, n)) * (0.84 + 0.3 * texture2D(uNoise, wp * 0.31).b);
        }`,
      );
  };
  mat.customProgramCacheKey = () => 'cliff';
  applyFog(mat);
  const mesh = new THREE.Mesh(merged, mat);
  mesh.castShadow = true;
  mesh.receiveShadow = true;
  mesh.name = 'walls';
  return mesh;
}

// ---------------------------------------------------------------- rochers

function rockGeo(): THREE.BufferGeometry {
  const g = facet(new THREE.IcosahedronGeometry(0.22, 0), 0.08);
  const base = new THREE.Color(0x8f8676);
  const moss = new THREE.Color(0x4d7a38);
  return paint(g, (_x, y, _z, ny, out) => {
    out.copy(base).multiplyScalar(0.6 + 0.5 * smooth(-0.2, 0.2, y));
    if (ny > 0.55) out.lerp(moss, 0.65);
  });
}

function buildRocks(r: () => number): THREE.InstancedMesh {
  const spots: { x: number; y: number }[] = [];
  for (let i = 0; i < 1400 && spots.length < 220; i++) {
    const x = 600 + r() * (MAP_SIZE - 1200);
    const y = 600 + r() * (MAP_SIZE - 1200);
    const p = { x, y };
    if (isInWall(p) || isInBrush(p)) continue;
    if (distToLane(x, y) < LANE_HALF_WIDTH + 80) continue;
    if (CAMPS.some((c) => Math.hypot(c.pos.x - x, c.pos.y - y) < 450)) continue;
    if (Object.values(PITS).some((pit) => Math.hypot(pit.pos.x - x, pit.pos.y - y) < pit.radius + 100)) continue;
    if (Math.hypot(x - FOUNTAINS.blue.x, y - FOUNTAINS.blue.y) < 2500 || Math.hypot(x - FOUNTAINS.red.x, y - FOUNTAINS.red.y) < 2500) continue;
    // les rochers longent les murs : on garde surtout ceux proches d'un mur
    let nearWall = false;
    for (let a = 0; a < 8 && !nearWall; a++) nearWall = isInWall({ x: x + Math.cos(a) * 220, y: y + Math.sin(a) * 220 });
    if (!nearWall && r() < 0.8) continue;
    spots.push(p);
  }
  const mat = new THREE.MeshStandardMaterial({ vertexColors: true, roughness: 1 });
  applyFog(mat);
  const im = new THREE.InstancedMesh(rockGeo(), mat, spots.length);
  const m = new THREE.Matrix4();
  const e = new THREE.Euler();
  const q = new THREE.Quaternion();
  const pos = new THREE.Vector3();
  const scl = new THREE.Vector3();
  const col = new THREE.Color();
  spots.forEach((p, i) => {
    // rotation surtout autour de la verticale : la mousse reste sur le dessus
    e.set((r() - 0.5) * 0.5, r() * Math.PI * 2, (r() - 0.5) * 0.5);
    q.setFromEuler(e);
    const s = 0.6 + r() * 1.3;
    scl.set(s, s * (0.5 + r() * 0.4), s * (0.8 + r() * 0.4));
    pos.set(p.x * S, terrainHeight(p.x, p.y) * S + 0.02, -p.y * S);
    m.compose(pos, q, scl);
    im.setMatrixAt(i, m);
    im.setColorAt(i, col.setRGB(1, 1, 1).offsetHSL(0, 0, (r() - 0.5) * 0.12));
  });
  im.castShadow = true;
  im.receiveShadow = true;
  im.computeBoundingSphere();
  return im;
}

// ---------------------------------------------------------------- brins d'herbe

/**
 * Touffe de brins courbés (2 triangles par brin) ; couleurs de sommets racine sombre → pointe claire.
 * Normales vers le haut : l'herbe est éclairée comme le sol, sans facettes noires selon l'orientation.
 */
function bladeTuft(n: number, h: number, spread: number, w: number, root: THREE.Color, tip: THREE.Color): THREE.BufferGeometry {
  const pos: number[] = [];
  const col: number[] = [];
  const c = new THREE.Color();
  const push = (x: number, y: number, z: number, t: number) => {
    pos.push(x, y, z);
    c.copy(root).lerp(tip, t);
    col.push(c.r, c.g, c.b);
  };
  for (let i = 0; i < n; i++) {
    const a = (i / n) * Math.PI * 2 + i * 1.7;
    const bh = h * (0.75 + ((i * 7) % 5) * 0.08);
    const ox = Math.cos(a) * spread;
    const oz = Math.sin(a) * spread;
    // plan du brin perpendiculaire à sa direction d'inclinaison
    const px = -Math.sin(a) * w;
    const pz = Math.cos(a) * w;
    const lean = 0.35 + (i % 3) * 0.12;
    const mx = ox + Math.cos(a) * bh * lean * 0.45;
    const mz = oz + Math.sin(a) * bh * lean * 0.45;
    const tx = ox + Math.cos(a) * bh * lean;
    const tz = oz + Math.sin(a) * bh * lean;
    // base → milieu (quad), milieu → pointe (triangle)
    push(ox - px, 0, oz - pz, 0);
    push(ox + px, 0, oz + pz, 0);
    push(mx + px * 0.6, bh * 0.55, mz + pz * 0.6, 0.55);
    push(ox - px, 0, oz - pz, 0);
    push(mx + px * 0.6, bh * 0.55, mz + pz * 0.6, 0.55);
    push(mx - px * 0.6, bh * 0.55, mz - pz * 0.6, 0.55);
    push(mx - px * 0.6, bh * 0.55, mz - pz * 0.6, 0.55);
    push(mx + px * 0.6, bh * 0.55, mz + pz * 0.6, 0.55);
    push(tx, bh, tz, 1);
  }
  const g = new THREE.BufferGeometry();
  g.setAttribute('position', new THREE.Float32BufferAttribute(pos, 3));
  g.setAttribute('color', new THREE.Float32BufferAttribute(col, 3));
  const nrm = new Float32Array(pos.length);
  for (let i = 1; i < nrm.length; i += 3) nrm[i] = 1;
  g.setAttribute('normal', new THREE.BufferAttribute(nrm, 3));
  return g;
}

// ---------------------------------------------------------------- brousses

export interface BrushView {
  poly: Polygon;
  mesh: THREE.InstancedMesh;
  mat: THREE.MeshStandardMaterial;
  opacity: number;
}

function buildBrushes(r: () => number): BrushView[] {
  const geo = bladeTuft(7, 0.85, 0.07, 0.05, new THREE.Color(0x173d1c), new THREE.Color(0x9ccf55));
  const out: BrushView[] = [];
  for (const poly of BRUSHES) {
    let minX = Infinity;
    let minY = Infinity;
    let maxX = -Infinity;
    let maxY = -Infinity;
    for (const p of poly) {
      minX = Math.min(minX, p.x);
      minY = Math.min(minY, p.y);
      maxX = Math.max(maxX, p.x);
      maxY = Math.max(maxY, p.y);
    }
    const pts: { x: number; y: number }[] = [];
    const step = 34;
    for (let y = minY + 10; y < maxY; y += step)
      for (let x = minX + 10; x < maxX; x += step) {
        const p = { x: x + (r() - 0.5) * 26, y: y + (r() - 0.5) * 26 };
        if (pointInPolygon(p, poly)) pts.push(p);
      }
    // opaque tant que le joueur n'est pas dedans ; une seule passe même transparente (sinon three dessine
    // faces arrière puis avant en changeant de programme à chaque fois, pour chaque brousse)
    const mat = new THREE.MeshStandardMaterial({ vertexColors: true, roughness: 0.8, side: THREE.DoubleSide, transparent: false, opacity: 1, forceSinglePass: true });
    addWind(mat, 0.09);
    applyFog(mat);
    const im = new THREE.InstancedMesh(geo, mat, pts.length);
    const m = new THREE.Matrix4();
    const q = new THREE.Quaternion();
    const up = new THREE.Vector3(0, 1, 0);
    const pos = new THREE.Vector3();
    const scl = new THREE.Vector3();
    const col = new THREE.Color();
    pts.forEach((p, i) => {
      q.setFromAxisAngle(up, r() * Math.PI * 2);
      const s = 0.9 + r() * 0.5;
      scl.set(s, s * (0.85 + r() * 0.4), s);
      pos.set(p.x * S, terrainHeight(p.x, p.y) * S, -p.y * S);
      m.compose(pos, q, scl);
      im.setMatrixAt(i, m);
      im.setColorAt(i, col.setHex(r() < 0.5 ? 0xffffff : 0xe2f0c8).offsetHSL(0, 0, (r() - 0.5) * 0.08));
    });
    im.castShadow = true;
    im.receiveShadow = true;
    im.computeBoundingSphere();
    out.push({ poly, mesh: im, mat, opacity: 1 });
  }
  return out;
}

// ---------------------------------------------------------------- touffes et fleurs au sol (détail)

/** Fleur : disque à 5 pétales (blanc, teinté par l'instance), cœur jaune. */
function flowerGeo(): THREE.BufferGeometry {
  const pos: number[] = [];
  const col: number[] = [];
  const h = 0.1;
  for (let i = 0; i < 5; i++) {
    const a0 = (i / 5) * Math.PI * 2;
    const a1 = a0 + (Math.PI * 2) / 5;
    const am = (a0 + a1) / 2;
    pos.push(0, h, 0, Math.cos(a1) * 0.03, h, Math.sin(a1) * 0.03, Math.cos(am) * 0.075, h + 0.01, Math.sin(am) * 0.075);
    pos.push(0, h, 0, Math.cos(am) * 0.075, h + 0.01, Math.sin(am) * 0.075, Math.cos(a0) * 0.03, h, Math.sin(a0) * 0.03);
    for (let k = 0; k < 2; k++) col.push(1, 0.75, 0.15, 0.95, 0.95, 0.95, 0.9, 0.9, 0.9);
  }
  // tige
  pos.push(-0.006, 0, 0, 0.006, 0, 0, 0, h, 0);
  col.push(0.1, 0.25, 0.08, 0.1, 0.25, 0.08, 0.15, 0.35, 0.1);
  const g = new THREE.BufferGeometry();
  g.setAttribute('position', new THREE.Float32BufferAttribute(pos, 3));
  g.setAttribute('color', new THREE.Float32BufferAttribute(col, 3));
  const nrm = new Float32Array(pos.length);
  for (let i = 1; i < nrm.length; i += 3) nrm[i] = 1;
  g.setAttribute('normal', new THREE.BufferAttribute(nrm, 3));
  return g;
}

const FLOWER_TINTS = [0xffffff, 0xfff27a, 0xffa6d2, 0xc9a6ff, 0x9ad4ff, 0xffffff];

/** Emplacement libre pour du détail au sol (herbe de la jungle, bords des voies). */
function freeGround(x: number, y: number): boolean {
  if (x < 150 || y < 150 || x > MAP_SIZE - 150 || y > MAP_SIZE - 150) return false;
  if (distToLane(x, y) < LANE_HALF_WIDTH - 30) return false;
  if (distToRiver(x, y) < RIVER_HALF_WIDTH + 60 && Math.abs(x - y) * Math.SQRT1_2 < 6600) return false;
  if (Math.hypot(x - FOUNTAINS.blue.x, y - FOUNTAINS.blue.y) < 2380 || Math.hypot(x - FOUNTAINS.red.x, y - FOUNTAINS.red.y) < 2380) return false;
  const p = { x, y };
  if (isInWall(p) || isInBrush(p)) return false;
  if (Object.values(PITS).some((pit) => Math.hypot(pit.pos.x - x, pit.pos.y - y) < pit.radius + 60)) return false;
  return true;
}

function buildGroundDetail(r: () => number): THREE.Group {
  const g = new THREE.Group();
  const tuft = bladeTuft(6, 0.36, 0.05, 0.035, new THREE.Color(0x3b6e30), new THREE.Color(0xb6d870));
  const flower = flowerGeo();
  const tuftMat = new THREE.MeshStandardMaterial({ vertexColors: true, roughness: 0.85, side: THREE.DoubleSide });
  addWind(tuftMat, 0.6);
  applyFog(tuftMat);
  const flowerMat = new THREE.MeshStandardMaterial({ vertexColors: true, roughness: 0.7, side: THREE.DoubleSide });
  applyFog(flowerMat);
  const tufts: { x: number; y: number; s: number; rot: number; c: number }[] = [];
  const flowers: { x: number; y: number; s: number; rot: number; c: number }[] = [];
  const step = 62;
  for (let y = 100; y < MAP_SIZE; y += step)
    for (let x = 100; x < MAP_SIZE; x += step) {
      const px = x + (r() - 0.5) * step;
      const py = y + (r() - 0.5) * step;
      // densité en grappes (somme de sinus : pas de motif visible à l'échelle du jeu)
      const clump = Math.sin(px * 0.0031) * Math.cos(py * 0.0027) + Math.sin((px + py) * 0.0013) * 0.6;
      const edge = distToLane(px, py) - LANE_HALF_WIDTH;
      const nearLane = edge > -30 && edge < 120;
      const pKeep = nearLane ? 0.85 : 0.18 + Math.max(0, clump) * 0.5;
      const roll = r();
      const rot = r() * Math.PI * 2;
      const s = 0.75 + r() * 0.6;
      const c = r();
      if (roll > pKeep || !freeGround(px, py)) continue;
      if (!nearLane && c < 0.09 && clump > 0.2) flowers.push({ x: px, y: py, s, rot, c });
      else tufts.push({ x: px, y: py, s, rot, c });
    }
  const m = new THREE.Matrix4();
  const q = new THREE.Quaternion();
  const up = new THREE.Vector3(0, 1, 0);
  const pos = new THREE.Vector3();
  const scl = new THREE.Vector3();
  const col = new THREE.Color();
  const fill = (geo: THREE.BufferGeometry, mat: THREE.Material, items: typeof tufts, tint: (c: number) => number) => {
    for (const arr of byTile(items, TREE_TILE).values()) {
      const im = new THREE.InstancedMesh(geo, mat, arr.length);
      arr.forEach((p, i) => {
        q.setFromAxisAngle(up, p.rot);
        pos.set(p.x * S, terrainHeight(p.x, p.y) * S, -p.y * S);
        scl.set(p.s, p.s, p.s);
        m.compose(pos, q, scl);
        im.setMatrixAt(i, m);
        im.setColorAt(i, col.setHex(tint(p.c)));
      });
      im.receiveShadow = true;
      im.computeBoundingSphere();
      g.add(im);
    }
  };
  fill(tuft, tuftMat, tufts, (c) => (c < 0.3 ? 0xfff0c0 : c < 0.6 ? 0xe8ffe0 : 0xffffff));
  fill(flower, flowerMat, flowers, (c) => FLOWER_TINTS[Math.floor(c * 66) % FLOWER_TINTS.length]!);
  g.userData.detail = true;
  return g;
}

export class Vegetation {
  readonly group = new THREE.Group();
  readonly brushes: BrushView[];
  private readonly detail: THREE.Group;

  constructor(quality: 'low' | 'high') {
    const r = rng(4242);
    this.group.add(buildWallBases());
    const spots = wallTreeSpots(r);
    if (quality === 'high') spots.push(...borderTreeSpots(r));
    else spots.push(...borderTreeSpots(r).filter((_, i) => i % 2 === 0));
    this.group.add(buildTrees(spots, r));
    this.group.add(buildRocks(r));
    this.brushes = buildBrushes(r);
    for (const b of this.brushes) this.group.add(b.mesh);
    this.detail = buildGroundDetail(rng(99));
    this.group.add(this.detail);
    this.setQuality(quality);
  }

  /** Qualité basse : les tuiles d'arbres hors carte (simple fond) et le détail au sol sont masqués. */
  setQuality(q: 'low' | 'high'): void {
    this.group.traverse((o) => {
      if (o.userData.backdrop) o.visible = q === 'high';
    });
    this.detail.visible = q === 'high';
  }

  /** Rend semi-transparente la brousse où se trouve le joueur local. */
  update(dt: number, self: { x: number; y: number } | null): void {
    windTime.value += dt;
    for (const b of this.brushes) {
      const inside = !!self && pointInPolygon(self, b.poly);
      const target = inside ? 0.4 : 1;
      if (b.opacity !== target) {
        b.opacity += (target - b.opacity) * Math.min(1, dt * 8);
        if (Math.abs(b.opacity - target) < 0.01) b.opacity = target;
        b.mat.opacity = b.opacity;
        const opaque = b.opacity > 0.99;
        b.mat.depthWrite = opaque;
        if (b.mat.transparent === opaque) {
          b.mat.transparent = !opaque;
          b.mat.needsUpdate = true;
        }
      }
    }
  }
}
