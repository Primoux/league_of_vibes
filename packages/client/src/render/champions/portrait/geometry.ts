/** Géométrie originale des modèles inspirés des portraits : volumes, mèches, étoffes et ornements. */
import * as THREE from 'three';
import type { JointName } from '../anim/pose.ts';
import { ModelBuilder, type PartOpts } from '../builder.ts';

export type Point = [number, number, number];
export type Ring = [y: number, depth: number, width: number, x?: number];

export function loft(rings: Ring[], segments = 12, caps = true): THREE.BufferGeometry {
  const vertices: number[] = [], indices: number[] = [];
  for (const [y, depth, width, x = 0] of rings) {
    for (let i = 0; i <= segments; i++) {
      const a = i / segments * Math.PI * 2;
      vertices.push(x + Math.cos(a) * depth, y, Math.sin(a) * width);
    }
  }
  for (let j = 0; j < rings.length - 1; j++) for (let i = 0; i < segments; i++) {
    const a = j * (segments + 1) + i, b = a + segments + 1;
    indices.push(a, b, a + 1, b, b + 1, a + 1);
  }
  for (const top of caps ? [false, true] : []) {
    const row = top ? rings.length - 1 : 0, ring = rings[row]!;
    const center = vertices.length / 3; vertices.push(ring[3] ?? 0, ring[0], 0);
    for (let i = 0; i < segments; i++) {
      const a = row * (segments + 1) + i;
      indices.push(center, top ? a + 1 : a, top ? a : a + 1);
    }
  }
  const g = new THREE.BufferGeometry();
  g.setAttribute('position', new THREE.Float32BufferAttribute(vertices, 3)); g.setIndex(indices); g.computeVertexNormals();
  return g;
}

export function strand(points: Point[], radii: number[], sides = 6, steps = 10): THREE.BufferGeometry {
  const curve = new THREE.CatmullRomCurve3(points.map(p => new THREE.Vector3(...p)));
  const g = new THREE.TubeGeometry(curve, steps, 1, sides, false), position = g.getAttribute('position');
  for (let i = 0; i <= steps; i++) {
    const t = i / steps, center = curve.getPointAt(t), f = t * (radii.length - 1), k = Math.min(radii.length - 2, Math.floor(f));
    const radius = Math.max(.002, THREE.MathUtils.lerp(radii[k]!, radii[k + 1]!, f - k));
    for (let j = 0; j <= sides; j++) {
      const index = i * (sides + 1) + j;
      position.setXYZ(index, center.x + (position.getX(index) - center.x) * radius, center.y + (position.getY(index) - center.y) * radius, center.z + (position.getZ(index) - center.z) * radius);
    }
  }
  g.computeVertexNormals(); return g;
}

/** Polygone sculpté dans le plan yz, face vers +x. */
export function plate(points: [number, number][], depth = .025, bevel = .008): THREE.BufferGeometry {
  const shape = new THREE.Shape();
  points.forEach(([z, y], i) => i ? shape.lineTo(-z, y) : shape.moveTo(-z, y)); shape.closePath();
  const g = new THREE.ExtrudeGeometry(shape, { depth, bevelEnabled: bevel > 0, bevelSize: bevel, bevelThickness: bevel, bevelSegments: 2, steps: 1 });
  g.rotateY(Math.PI / 2); g.translate(-depth / 2, 0, 0); return g;
}

/** Cape épaisse à ourlet ondulé ; chaque couche possède ses normales propres. */
export function cloth(height: number, width: number, topWidth: number, back = -.18, wave = .025): THREE.BufferGeometry {
  const vertices: number[] = [], indices: number[] = [], nx = 14, ny = 8, layerSize = (nx + 1) * (ny + 1);
  for (let side = 0; side < 2; side++) for (let j = 0; j <= ny; j++) for (let i = 0; i <= nx; i++) {
    const t = j / ny, u = i / nx * 2 - 1;
    const half = THREE.MathUtils.lerp(topWidth, width, t);
    vertices.push(back - .12 * t - .08 * (1 - u * u) + Math.sin(u * Math.PI * 4) * wave * t + side * .014,
      .48 - height * t + Math.cos(u * Math.PI * 5) * .025 * t * t, u * half);
  }
  for (let side = 0; side < 2; side++) for (let j = 0; j < ny; j++) for (let i = 0; i < nx; i++) {
    const a = side * layerSize + j * (nx + 1) + i, b = a + nx + 1;
    if (side === 0) indices.push(a, b, a + 1, b, b + 1, a + 1);
    else indices.push(a, a + 1, b, b, a + 1, b + 1);
  }
  const g = new THREE.BufferGeometry(); g.setAttribute('position', new THREE.Float32BufferAttribute(vertices, 3)); g.setIndex(indices); g.computeVertexNormals(); return g;
}

export class Sculpt {
  constructor(readonly b: ModelBuilder) { b.sculptedSurfaces(); }
  ell(j: JointName, color: THREE.ColorRepresentation, pos: Point, size: Point, detail = 10, metal = false): void {
    const g = new THREE.SphereGeometry(1, detail, Math.max(6, Math.floor(detail * .65)));
    this.part(j, g, color, { pos, scale: size, smooth: true }, metal);
  }
  part(j: JointName, g: THREE.BufferGeometry, color: THREE.ColorRepresentation, opts: PartOpts = {}, metal = false): void {
    if (metal) this.b.metal(j, g, color, opts); else this.b.add(j, g, color, opts);
  }
  tube(j: JointName, color: THREE.ColorRepresentation, points: Point[], radii: number[], metal = false, sides = 6): void {
    this.part(j, strand(points, radii, sides, points.length >= 4 ? 10 : 8), color, { smooth: true }, metal);
  }
  panel(j: JointName, color: THREE.ColorRepresentation, points: [number, number][], pos: Point, metal = false, depth = .025): void {
    this.part(j, plate(points, depth), color, { pos }, metal);
  }
  form(j: JointName, color: THREE.ColorRepresentation, rings: Ring[], metal = false, opts: PartOpts = {}): void {
    this.part(j, loft(rings), color, { ...opts, smooth: true }, metal);
  }
  ring(j: JointName, color: THREE.ColorRepresentation, radius: number, tube: number, pos: Point, normal: 'x' | 'y' = 'x', metal = false): void {
    this.part(j, new THREE.TorusGeometry(radius, tube, 5, 18), color, { pos, rot: normal === 'x' ? [0, Math.PI / 2, 0] : [Math.PI / 2, 0, 0], smooth: true }, metal);
  }
  gem(j: JointName, color: THREE.Color, pos: Point, scale: Point): void {
    this.b.glow(j, new THREE.OctahedronGeometry(1), color, { pos, scale });
  }
}
