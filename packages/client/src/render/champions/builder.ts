/**
 * Construction des modèles low-poly : les pièces attachées à une même articulation sont fusionnées en UN mesh
 * à couleurs de sommets (1 draw call par articulation), les parties lumineuses (yeux, cristaux) forment un
 * second mesh émissif, et un contour par coque inversée est ajouté à chaque mesh opaque.
 */
import * as THREE from 'three';
import { mergeGeometries, mergeVertices } from 'three/examples/jsm/utils/BufferGeometryUtils.js';
import { J, type JointName } from './anim/pose.ts';
import type { Rig } from './rig.ts';

export interface PartOpts {
  pos?: [number, number, number];
  rot?: [number, number, number];
  scale?: [number, number, number] | number;
}

// ---------------------------------------------------------------- primitives
const hash3 = (x: number, y: number, z: number): number => {
  const h = Math.sin(x * 127.1 + y * 311.7 + z * 74.7) * 43758.5453;
  return h - Math.floor(h);
};

/** Déforme chaque sommet selon sa position (les sommets confondus bougent pareil : pas de fissures). */
function jitter(g: THREE.BufferGeometry, amount: number, seed: number): THREE.BufferGeometry {
  const p = g.getAttribute('position') as THREE.BufferAttribute;
  for (let i = 0; i < p.count; i++) {
    const x = p.getX(i);
    const y = p.getY(i);
    const z = p.getZ(i);
    const k = (hash3(Math.round(x * 1000) + seed, Math.round(y * 1000), Math.round(z * 1000)) - 0.5) * 2 * amount;
    const len = Math.hypot(x, y, z) || 1;
    p.setXYZ(i, x + (x / len) * k, y + (y / len) * k, z + (z / len) * k);
  }
  g.computeVertexNormals();
  return g;
}

export const geo = {
  box: (w: number, h: number, d: number): THREE.BufferGeometry => new THREE.BoxGeometry(w, h, d),
  /** Pavé biseauté façon « pierre taillée ». */
  slab: (w: number, h: number, d: number, seed = 1): THREE.BufferGeometry => jitter(new THREE.BoxGeometry(w, h, d, 2, 2, 2), Math.min(w, h, d) * 0.12, seed),
  rock: (r: number, seed = 1, detail = 0): THREE.BufferGeometry => jitter(new THREE.DodecahedronGeometry(r, detail), r * 0.22, seed),
  ball: (r: number, detail = 1): THREE.BufferGeometry => new THREE.IcosahedronGeometry(r, detail),
  cyl: (rTop: number, rBottom: number, h: number, seg = 6): THREE.BufferGeometry => new THREE.CylinderGeometry(rTop, rBottom, h, seg),
  cone: (r: number, h: number, seg = 5): THREE.BufferGeometry => new THREE.ConeGeometry(r, h, seg),
  crystal: (r: number, h: number): THREE.BufferGeometry => {
    const g = new THREE.OctahedronGeometry(r, 0);
    g.scale(1, h / r / 2, 1);
    return g;
  },
  torus: (r: number, tube: number, seg = 10): THREE.BufferGeometry => new THREE.TorusGeometry(r, tube, 4, seg),
};

// ---------------------------------------------------------------- contour (coque inversée, skinnée)
const OUTLINE_VERT = /* glsl */ `
#include <common>
#include <skinning_pars_vertex>
uniform float uThickness;
void main() {
  #include <skinbase_vertex>
  #include <begin_vertex>
  #include <beginnormal_vertex>
  #include <skinnormal_vertex>
  #include <skinning_vertex>
  vec4 mv = modelViewMatrix * vec4(transformed, 1.0);
  vec3 n = normalize(normalMatrix * objectNormal);
  // épaisseur ~ constante à l'écran : proportionnelle à la distance
  mv.xyz += n * uThickness * max(1.0, -mv.z) * 0.01;
  gl_Position = projectionMatrix * mv;
}
`;
const OUTLINE_FRAG = /* glsl */ `
uniform vec3 uColor;
void main() {
  gl_FragColor = vec4(uColor, 1.0);
  #include <colorspace_fragment>
}
`;
let outlineMat: THREE.ShaderMaterial | null = null;
export function getOutlineMaterial(): THREE.ShaderMaterial {
  outlineMat ??= new THREE.ShaderMaterial({
    uniforms: { uThickness: { value: 0.14 }, uColor: { value: new THREE.Color(0.03, 0.025, 0.04) } },
    vertexShader: OUTLINE_VERT,
    fragmentShader: OUTLINE_FRAG,
    side: THREE.BackSide,
  });
  return outlineMat;
}

/** Coque du contour : normales lissées (sommets fusionnés) pour que l'extrusion ne se fende pas aux arêtes. */
function hullGeometry(g: THREE.BufferGeometry): THREE.BufferGeometry {
  const h = new THREE.BufferGeometry();
  h.setAttribute('position', g.getAttribute('position').clone());
  h.setAttribute('skinIndex', g.getAttribute('skinIndex').clone());
  h.setAttribute('skinWeight', g.getAttribute('skinWeight').clone());
  const merged = mergeVertices(h, 1e-3);
  merged.computeVertexNormals();
  return merged;
}

// ---------------------------------------------------------------- builder
interface Part {
  geo: THREE.BufferGeometry; // en repère local de l'articulation
  joint: number;
  color: THREE.Color;
}

const _m = new THREE.Matrix4();
const _q = new THREE.Quaternion();
const _e = new THREE.Euler();
const _s = new THREE.Vector3();
const _p = new THREE.Vector3();

/** Géométrie non indexée, position seule, transformée par les options de la pièce. */
function prepare(src: THREE.BufferGeometry, o: PartOpts): THREE.BufferGeometry {
  const g = src.index ? src.toNonIndexed() : src.clone();
  for (const name of Object.keys(g.attributes)) if (name !== 'position') g.deleteAttribute(name);
  const sc = o.scale ?? 1;
  _s.set(...(typeof sc === 'number' ? ([sc, sc, sc] as [number, number, number]) : sc));
  _q.setFromEuler(_e.set(...(o.rot ?? [0, 0, 0])));
  _p.set(...(o.pos ?? [0, 0, 0]));
  g.applyMatrix4(_m.compose(_p, _q, _s));
  src.dispose();
  return g;
}

/** Passe les pièces en repère modèle (pose de repos), couleur + os par sommet, et fusionne. */
function mergeParts(parts: Part[], rig: Rig): THREE.BufferGeometry {
  const geos = parts.map(({ geo: g, joint, color }) => {
    g.applyMatrix4(rig.joints[joint]!.matrixWorld);
    const n = g.getAttribute('position').count;
    const col = new Float32Array(n * 3);
    const idx = new Uint16Array(n * 4);
    const w = new Float32Array(n * 4);
    for (let i = 0; i < n; i++) {
      col[i * 3] = color.r;
      col[i * 3 + 1] = color.g;
      col[i * 3 + 2] = color.b;
      idx[i * 4] = joint;
      w[i * 4] = 1; // skinning rigide : 100 % sur l'os de la pièce
    }
    g.setAttribute('color', new THREE.BufferAttribute(col, 3));
    g.setAttribute('skinIndex', new THREE.Uint16BufferAttribute(idx, 4));
    g.setAttribute('skinWeight', new THREE.BufferAttribute(w, 4));
    g.computeVertexNormals();
    return g;
  });
  const merged = mergeGeometries(geos, false)!;
  geos.forEach((g) => g.dispose());
  return merged;
}

export interface BuiltModel {
  /** Matériau principal (un par instance : la dissolution est propre à chaque champion). */
  material: THREE.MeshStandardMaterial;
  /** Matériau des parties lumineuses (couleurs de sommets HDR ; `color` module l'intensité). */
  glowMaterial: THREE.MeshBasicMaterial | null;
  outlines: THREE.Mesh[];
  glows: THREE.Mesh[];
  meshes: THREE.Mesh[];
}

export class ModelBuilder {
  private readonly solids: Part[] = [];
  private readonly glowParts: Part[] = [];

  /** Pièce opaque de couleur `color` (hex/CSS, sRGB) attachée à `joint`. */
  add(joint: JointName, g: THREE.BufferGeometry, color: THREE.ColorRepresentation, o: PartOpts = {}): this {
    this.solids.push({ geo: prepare(g, o), joint: J[joint], color: new THREE.Color(color) });
    return this;
  }

  /** Pièce lumineuse (non éclairée, peut être HDR pour le bloom). */
  glow(joint: JointName, g: THREE.BufferGeometry, color: THREE.Color, o: PartOpts = {}): this {
    this.glowParts.push({ geo: prepare(g, o), joint: J[joint], color: color.clone() });
    return this;
  }

  /**
   * Fusionne tout en 3 SkinnedMesh (corps, contour, lumières) liés au squelette du rig :
   * 3 draw calls par champion au lieu d'un mesh par articulation.
   */
  build(rig: Rig, opts: { outline?: boolean; roughness?: number } = {}): BuiltModel {
    rig.root.updateMatrixWorld(true);
    const material = new THREE.MeshStandardMaterial({ vertexColors: true, flatShading: true, roughness: opts.roughness ?? 0.78, metalness: 0.05 });
    const out: BuiltModel = { material, glowMaterial: null, outlines: [], glows: [], meshes: [] };
    const skinned = (g: THREE.BufferGeometry, m: THREE.Material): THREE.SkinnedMesh => {
      const mesh = new THREE.SkinnedMesh(g, m);
      mesh.frustumCulled = false; // la boîte de la pose de repos ne couvre pas les bras levés
      rig.root.add(mesh);
      mesh.bind(rig.skeleton);
      return mesh;
    };
    if (this.solids.length) {
      const g = mergeParts(this.solids, rig);
      const body = skinned(g, material);
      body.castShadow = true;
      body.receiveShadow = true;
      out.meshes.push(body);
      if (opts.outline !== false) {
        const hull = skinned(hullGeometry(g), getOutlineMaterial());
        hull.userData.noDissolve = true;
        out.outlines.push(hull);
      }
    }
    if (this.glowParts.length) {
      out.glowMaterial = new THREE.MeshBasicMaterial({ vertexColors: true, toneMapped: false });
      out.glows.push(skinned(mergeParts(this.glowParts, rig), out.glowMaterial));
    }
    return out;
  }
}
