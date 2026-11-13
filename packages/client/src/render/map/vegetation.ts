/**
 * Murs (socle rocheux + forêt), bordure de forêt, brousses (herbes hautes animées), rochers. Propriétaire : claude-client.
 * Tout est instancié : quelques draw calls pour des milliers d'éléments.
 */
import * as THREE from 'three';
import { BRUSHES, CAMPS, FOUNTAINS, LANE_HALF_WIDTH, MAP_SIZE, PITS, WALLS, isInBrush, isInWall, pointInPolygon, type Polygon } from '@lov/shared';
import { S } from '../coords.ts';
import { distToLane, distToRiver, terrainHeight } from './geometry.ts';
import { rng } from './rng.ts';
import { GROUND_MARGIN } from './terrain.ts';

/** Uniform de temps partagé par les shaders de végétation (vent). */
export const windTime = { value: 0 };

function addWind(mat: THREE.MeshStandardMaterial, strength: number): void {
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
  };
}

// ---------------------------------------------------------------- arbres

function treeGeometries(): { trunk: THREE.BufferGeometry; leaves: THREE.BufferGeometry } {
  const trunk = new THREE.CylinderGeometry(0.07, 0.11, 0.6, 5).translate(0, 0.3, 0);
  const a = new THREE.ConeGeometry(0.62, 1.1, 7).translate(0, 1.0, 0);
  const b = new THREE.ConeGeometry(0.48, 0.9, 7).translate(0, 1.5, 0);
  const c = new THREE.ConeGeometry(0.32, 0.7, 7).translate(0, 1.95, 0);
  return { trunk, leaves: mergeGeometries([a, b, c]) };
}

/** Fusion minimale de géométries non indexées/indexées ayant les mêmes attributs (position, normal, uv). */
function mergeGeometries(geos: THREE.BufferGeometry[]): THREE.BufferGeometry {
  const parts = geos.map((g) => (g.index ? g.toNonIndexed() : g));
  const out = new THREE.BufferGeometry();
  for (const name of ['position', 'normal', 'uv'] as const) {
    const itemSize = parts[0]!.getAttribute(name).itemSize;
    const total = parts.reduce((n, g) => n + g.getAttribute(name).array.length, 0);
    const arr = new Float32Array(total);
    let off = 0;
    for (const g of parts) {
      arr.set(g.getAttribute(name).array as Float32Array, off);
      off += g.getAttribute(name).array.length;
    }
    out.setAttribute(name, new THREE.BufferAttribute(arr, itemSize));
  }
  return out;
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

function buildTrees(spots: TreeSpot[], r: () => number): THREE.Group {
  const { trunk, leaves } = treeGeometries();
  const trunkMat = new THREE.MeshStandardMaterial({ color: 0x5a3f2a, roughness: 1, flatShading: true });
  const leafMat = new THREE.MeshStandardMaterial({ color: 0xffffff, roughness: 0.9, flatShading: true });
  addWind(leafMat, 0.025);
  const tm = new THREE.InstancedMesh(trunk, trunkMat, spots.length);
  const lm = new THREE.InstancedMesh(leaves, leafMat, spots.length);
  const m = new THREE.Matrix4();
  const q = new THREE.Quaternion();
  const up = new THREE.Vector3(0, 1, 0);
  const pos = new THREE.Vector3();
  const scl = new THREE.Vector3();
  const col = new THREE.Color();
  const palette = [0x2f6b3a, 0x3a7a3c, 0x2a5f36, 0x47843f, 0x356f45, 0x5b8a3a];
  spots.forEach((t, i) => {
    q.setFromAxisAngle(up, r() * Math.PI * 2);
    pos.set(t.x * S, terrainHeight(t.x, t.y) * S + (isInWall(t) ? 0.85 : 0), -t.y * S);
    scl.set(t.s, t.s * (0.9 + r() * 0.35), t.s);
    m.compose(pos, q, scl);
    tm.setMatrixAt(i, m);
    lm.setMatrixAt(i, m);
    col.setHex(palette[Math.floor(r() * palette.length)]!).offsetHSL(0, 0, (r() - 0.5) * 0.06);
    lm.setColorAt(i, col);
  });
  for (const im of [tm, lm]) {
    im.castShadow = true;
    im.receiveShadow = true;
    im.computeBoundingSphere();
  }
  const g = new THREE.Group();
  g.add(tm, lm);
  return g;
}

// ---------------------------------------------------------------- murs (socle rocheux)

function buildWallBases(): THREE.Mesh {
  const mat = new THREE.MeshStandardMaterial({ color: 0x7d705c, roughness: 0.95, flatShading: true });
  const geos: THREE.BufferGeometry[] = [];
  for (const poly of WALLS) {
    const shape = new THREE.Shape(poly.map((p) => new THREE.Vector2(p.x * S, p.y * S)));
    const g = new THREE.ExtrudeGeometry(shape, { depth: 0.75, bevelEnabled: true, bevelSize: 0.35, bevelThickness: 0.25, bevelSegments: 2, curveSegments: 1 });
    g.rotateX(-Math.PI / 2);
    g.translate(0, -0.05, 0);
    geos.push(g.index ? g.toNonIndexed() : g);
  }
  const merged = mergeGeometries(geos);
  merged.computeVertexNormals();
  const mesh = new THREE.Mesh(merged, mat);
  mesh.castShadow = true;
  mesh.receiveShadow = true;
  mesh.name = 'walls';
  return mesh;
}

// ---------------------------------------------------------------- rochers

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
  const geo = new THREE.DodecahedronGeometry(0.22, 0);
  const mat = new THREE.MeshStandardMaterial({ color: 0x8a8174, roughness: 1, flatShading: true });
  const im = new THREE.InstancedMesh(geo, mat, spots.length);
  const m = new THREE.Matrix4();
  const e = new THREE.Euler();
  const q = new THREE.Quaternion();
  const pos = new THREE.Vector3();
  const scl = new THREE.Vector3();
  const col = new THREE.Color();
  spots.forEach((p, i) => {
    e.set(r() * 3, r() * 3, r() * 3);
    q.setFromEuler(e);
    const s = 0.6 + r() * 1.3;
    scl.set(s, s * (0.5 + r() * 0.4), s * (0.8 + r() * 0.4));
    pos.set(p.x * S, terrainHeight(p.x, p.y) * S + 0.05, -p.y * S);
    m.compose(pos, q, scl);
    im.setMatrixAt(i, m);
    im.setColorAt(i, col.setHex(0x8a8174).offsetHSL(0, 0, (r() - 0.5) * 0.12));
  });
  im.castShadow = true;
  im.receiveShadow = true;
  im.computeBoundingSphere();
  return im;
}

// ---------------------------------------------------------------- brousses

/** Une touffe = 5 brins inclinés (géométrie partagée). */
function tuftGeometry(): THREE.BufferGeometry {
  const blades: THREE.BufferGeometry[] = [];
  for (let i = 0; i < 5; i++) {
    const g = new THREE.ConeGeometry(0.05, 0.75 + (i % 3) * 0.12, 3, 1, true);
    g.translate(0, 0.38, 0);
    g.rotateZ((i - 2) * 0.18);
    g.rotateY((i / 5) * Math.PI * 2);
    g.translate(Math.cos(i * 1.3) * 0.08, 0, Math.sin(i * 1.3) * 0.08);
    blades.push(g);
  }
  return mergeGeometries(blades);
}

export interface BrushView {
  poly: Polygon;
  mesh: THREE.InstancedMesh;
  mat: THREE.MeshStandardMaterial;
  opacity: number;
}

function buildBrushes(r: () => number): BrushView[] {
  const geo = tuftGeometry();
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
    const mat = new THREE.MeshStandardMaterial({ color: 0xffffff, roughness: 0.85, side: THREE.DoubleSide, transparent: true, opacity: 1 });
    addWind(mat, 0.09);
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
      im.setColorAt(i, col.setHex(r() < 0.5 ? 0x3f8f3a : 0x4f9e3c).offsetHSL(0, 0, (r() - 0.5) * 0.08));
    });
    im.castShadow = true;
    im.receiveShadow = true;
    im.computeBoundingSphere();
    out.push({ poly, mesh: im, mat, opacity: 1 });
  }
  return out;
}

export class Vegetation {
  readonly group = new THREE.Group();
  readonly brushes: BrushView[];

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
        b.mat.depthWrite = b.opacity > 0.99;
      }
    }
  }
}
