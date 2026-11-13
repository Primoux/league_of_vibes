/**
 * Décal au sol : télégraphies (cercle qui se remplit, ligne, cône), zones persistantes, ondes de choc.
 * Un seul shader paramétré par forme. Les distances (bord, motif) sont en unités monde, quelle que soit l'échelle.
 */
import * as THREE from 'three';

export const DecalShape = { circle: 0, ring: 1, rect: 2, cone: 3 } as const;
export type DecalShapeName = keyof typeof DecalShape;

const VERT = /* glsl */ `
varying vec2 vP;
void main() {
  vP = vec2(position.x, -position.z);
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}
`;

const FRAG = /* glsl */ `
uniform int uShape;
uniform vec3 uColor;
uniform vec3 uColor2;
uniform vec2 uSize;      // demi-dimensions monde (x = rayon ou demi-longueur)
uniform float uOpacity;
uniform float uFill;     // opacité du remplissage de fond
uniform float uEdge;     // épaisseur du bord (monde)
uniform float uProgress; // remplissage temporel 0..1 (télégraphie), < 0 = désactivé
uniform float uAngle;    // demi-angle du cône (rad)
uniform float uTime;
uniform float uPattern;  // intensité du motif animé
varying vec2 vP;

void main() {
  float r = length(vP);
  float d;      // distance au bord (monde), > 0 à l'intérieur
  float along;  // 0..1 pour la progression
  if (uShape == 0 || uShape == 1) {
    d = (1.0 - r) * uSize.x;
    along = r;
  } else if (uShape == 2) {
    d = min((1.0 - abs(vP.x)) * uSize.x, (1.0 - abs(vP.y)) * uSize.y);
    along = vP.x * 0.5 + 0.5;
  } else {
    float a = abs(atan(vP.y, vP.x));
    d = min((1.0 - r) * uSize.x, (uAngle - a) * r * uSize.x);
    along = r;
  }
  if (d < 0.0) discard;

  float alpha;
  vec3 col = uColor;
  if (uShape == 1) {
    // onde de choc : anneau fin + halo intérieur
    float band = 1.0 - smoothstep(0.0, uEdge, d);
    float inner = (1.0 - smoothstep(0.0, uEdge * 4.0, d)) * 0.35;
    alpha = band + inner;
  } else {
    float edge = 1.0 - smoothstep(0.0, uEdge, d);
    float fill = uFill;
    float pattern = uPattern * (0.5 + 0.5 * sin(along * uSize.x * 6.0 - uTime * 4.0));
    alpha = edge + fill * (0.7 + 0.3 * pattern);
    if (uProgress >= 0.0) {
      float inside = step(along, uProgress);
      float front = 1.0 - smoothstep(0.0, 0.06, abs(along - uProgress));
      alpha += inside * 0.35 + front * 0.6 * step(0.001, uProgress);
      col = mix(uColor, uColor2, inside * 0.5);
    }
  }
  gl_FragColor = vec4(col, clamp(alpha, 0.0, 1.0) * uOpacity);
  #include <tonemapping_fragment>
  #include <colorspace_fragment>
}
`;

let sharedGeo: THREE.PlaneGeometry | null = null;
const geo = (): THREE.PlaneGeometry => {
  if (!sharedGeo) {
    sharedGeo = new THREE.PlaneGeometry(2, 2);
    sharedGeo.rotateX(-Math.PI / 2);
  }
  return sharedGeo;
};

let layerSeq = 0;

export class GroundDecal {
  readonly mesh: THREE.Mesh<THREE.PlaneGeometry, THREE.ShaderMaterial>;
  readonly u: {
    uShape: THREE.IUniform<number>;
    uColor: THREE.IUniform<THREE.Color>;
    uColor2: THREE.IUniform<THREE.Color>;
    uSize: THREE.IUniform<THREE.Vector2>;
    uOpacity: THREE.IUniform<number>;
    uFill: THREE.IUniform<number>;
    uEdge: THREE.IUniform<number>;
    uProgress: THREE.IUniform<number>;
    uAngle: THREE.IUniform<number>;
    uTime: THREE.IUniform<number>;
    uPattern: THREE.IUniform<number>;
  };
  /** Hauteur au-dessus du sol, légèrement différente par décal pour éviter le z-fighting. */
  private readonly lift = 0.03 + (layerSeq++ % 16) * 0.002;

  constructor() {
    this.u = {
      uShape: { value: 0 },
      uColor: { value: new THREE.Color(1, 1, 1) },
      uColor2: { value: new THREE.Color(1, 1, 1) },
      uSize: { value: new THREE.Vector2(1, 1) },
      uOpacity: { value: 1 },
      uFill: { value: 0.15 },
      uEdge: { value: 0.08 },
      uProgress: { value: -1 },
      uAngle: { value: 0.5 },
      uTime: { value: 0 },
      uPattern: { value: 0 },
    };
    const mat = new THREE.ShaderMaterial({
      uniforms: this.u,
      vertexShader: VERT,
      fragmentShader: FRAG,
      transparent: true,
      depthWrite: false,
      blending: THREE.AdditiveBlending,
    });
    this.mesh = new THREE.Mesh(geo(), mat);
    this.mesh.renderOrder = 5;
    this.mesh.frustumCulled = false;
    this.mesh.name = 'fx-decal';
  }

  /** Réinitialise les réglages (avant réutilisation depuis un pool). */
  reset(): this {
    const u = this.u;
    u.uOpacity.value = 1;
    u.uFill.value = 0.15;
    u.uEdge.value = 0.08;
    u.uProgress.value = -1;
    u.uPattern.value = 0;
    this.mesh.material.blending = THREE.AdditiveBlending;
    this.mesh.visible = true;
    this.mesh.rotation.set(0, 0, 0);
    return this;
  }

  setColor(c: THREE.Color, c2: THREE.Color = c): this {
    this.u.uColor.value.copy(c);
    this.u.uColor2.value.copy(c2);
    return this;
  }

  /** Cercle (ou anneau d'onde de choc) centré en `center` (monde), rayon en unités monde. */
  circle(center: THREE.Vector3, radius: number, shape: 'circle' | 'ring' = 'circle'): this {
    this.u.uShape.value = DecalShape[shape];
    this.u.uSize.value.set(radius, radius);
    this.mesh.position.set(center.x, center.y + this.lift, center.z);
    this.mesh.scale.set(radius, 1, radius);
    this.mesh.rotation.set(0, 0, 0);
    return this;
  }

  /** Rectangle de `from` à `to` (monde), largeur totale `width`. */
  rect(from: THREE.Vector3, to: THREE.Vector3, width: number): this {
    const dx = to.x - from.x;
    const dz = to.z - from.z;
    const half = Math.max(1e-3, Math.hypot(dx, dz) / 2);
    this.u.uShape.value = DecalShape.rect;
    this.u.uSize.value.set(half, width / 2);
    this.mesh.position.set((from.x + to.x) / 2, from.y + this.lift, (from.z + to.z) / 2);
    this.mesh.scale.set(half, 1, width / 2);
    this.mesh.rotation.set(0, Math.atan2(-dz, dx), 0);
    return this;
  }

  /** Cône depuis `origin` vers `toward`, portée `range`, demi-angle `halfAngle` (rad). */
  cone(origin: THREE.Vector3, toward: THREE.Vector3, range: number, halfAngle: number): this {
    this.u.uShape.value = DecalShape.cone;
    this.u.uSize.value.set(range, range);
    this.u.uAngle.value = halfAngle;
    this.mesh.position.set(origin.x, origin.y + this.lift, origin.z);
    this.mesh.scale.set(range, 1, range);
    this.mesh.rotation.set(0, Math.atan2(-(toward.z - origin.z), toward.x - origin.x), 0);
    return this;
  }

  dispose(): void {
    this.mesh.material.dispose();
  }
}
