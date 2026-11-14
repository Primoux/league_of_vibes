/**
 * Carte 3D « Faille des Vibes ». Propriétaire : claude-client.
 * Terrain peint avec relief, rivière animée, murs rocheux boisés, brousses, décor, fontaines, lumières.
 */
import * as THREE from 'three';
import { FOUNTAINS, type Vec2 } from '@lov/shared';
import { S } from './coords.ts';
import { terrainHeight } from './map/geometry.ts';
import { buildTerrain } from './map/terrain.ts';
import { Vegetation } from './map/vegetation.ts';
import { Water } from './map/water.ts';
import { assignShadowDepthTree } from './shadowDepth.ts';
import { noiseTexture } from './map/noise.ts';
import { TEAM_COLOR } from './views/DefaultViews.ts';

export class MapView {
  readonly ground: THREE.Mesh;
  readonly sun: THREE.DirectionalLight;
  readonly water: Water;
  readonly vegetation: Vegetation;
  private sunOffset = new THREE.Vector3(14, 30, 9);
  private focus = new THREE.Vector3();
  private lightFwd = new THREE.Vector3();
  private lightRight = new THREE.Vector3();
  private lightUp = new THREE.Vector3();
  private fountainCrystals: THREE.Object3D[] = [];
  private fountainGroups: THREE.Object3D[] = [];
  private fountainPools: THREE.ShaderMaterial[] = [];
  private t = 0;

  constructor(private readonly scene: THREE.Scene, _renderer: THREE.WebGLRenderer, quality: 'low' | 'high' = 'high') {
    scene.background = new THREE.Color(0x14202a);

    // ciel chaud / sol vert, soleil doré, appoint froid opposé (ombres bleutées, rendu « peint »)
    scene.add(new THREE.HemisphereLight(0xfff1dc, 0x35502e, 1.15));
    const fill = new THREE.DirectionalLight(0x9cc4ff, 0.55);
    fill.position.set(-12, 18, -10);
    scene.add(fill);
    this.sun = new THREE.DirectionalLight(0xffe0b0, 2.75);
    this.sun.castShadow = true;
    this.sun.shadow.mapSize.set(quality === 'high' ? 2048 : 1024, quality === 'high' ? 2048 : 1024);
    const sc = this.sun.shadow.camera;
    sc.left = -24;
    sc.right = 24;
    sc.top = 24;
    sc.bottom = -24;
    sc.near = 1;
    sc.far = 90;
    this.sun.shadow.bias = -0.0004;
    this.sun.shadow.normalBias = 0.02;
    scene.add(this.sun, this.sun.target);
    // base orthonormée de la lumière : sert à accrocher le cadrage de l'ombre à sa grille de texels
    this.lightFwd.copy(this.sunOffset).negate().normalize();
    this.lightRight.crossVectors(this.lightFwd, new THREE.Vector3(0, 1, 0)).normalize();
    this.lightUp.crossVectors(this.lightRight, this.lightFwd).normalize();

    this.ground = buildTerrain(quality);
    scene.add(this.ground);
    this.water = new Water();
    scene.add(this.water.mesh);
    this.vegetation = new Vegetation(quality);
    scene.add(this.vegetation.group);
    this.buildFountains();
    // décor immobile : matrices calculées une fois (sinon recomposées et remultipliées à chaque frame)
    for (const root of [this.ground, this.water.mesh, this.vegetation.group, ...this.fountainGroups]) freezeMatrices(root, this.fountainCrystals);
    assignShadowDepthTree(this.vegetation.group);
  }

  /**
   * Fontaines : socle de pierre à deux niveaux, bassin magique (shader : runes tournantes et remous aux
   * couleurs de l'équipe), piliers coiffés de gemmes, cristal flottant, lueur au sol additive.
   * Pas de PointLight : une lumière ponctuelle est évaluée dans chaque pixel de chaque matériau éclairé.
   */
  private buildFountains(): void {
    const stone = new THREE.MeshStandardMaterial({ color: 0xa79d88, roughness: 0.85, flatShading: true });
    const stoneDark = new THREE.MeshStandardMaterial({ color: 0x6f675a, roughness: 0.95, flatShading: true });
    const platGeo = new THREE.CylinderGeometry(4.2, 4.7, 0.24, 16);
    const rimGeo = new THREE.CylinderGeometry(3.3, 3.45, 0.16, 16, 1, true);
    const pillarGeo = new THREE.CylinderGeometry(0.2, 0.3, 2.2, 6);
    const capGeo = new THREE.OctahedronGeometry(0.2).scale(1, 1.6, 1);
    const crystalGeo = new THREE.OctahedronGeometry(0.9).scale(1, 1.6, 1);
    for (const team of ['blue', 'red'] as const) {
      const f = FOUNTAINS[team];
      const c = new THREE.Color(TEAM_COLOR[team]);
      const g = new THREE.Group();
      const plat = new THREE.Mesh(platGeo, stone);
      // socle bas : les unités qui réapparaissent se tiennent au niveau du sol (y = 0)
      plat.position.y = 0;
      plat.receiveShadow = true;
      const rim = new THREE.Mesh(rimGeo, stoneDark);
      rim.position.y = 0.2;
      rim.castShadow = rim.receiveShadow = true;
      const pool = new THREE.Mesh(new THREE.CircleGeometry(3.3, 48).rotateX(-Math.PI / 2), poolMaterial(c));
      pool.position.y = 0.135;
      this.fountainPools.push(pool.material as THREE.ShaderMaterial);
      g.add(plat, rim, pool);
      const gem = new THREE.MeshStandardMaterial({ color: c, emissive: c, emissiveIntensity: 3, roughness: 0.3 });
      for (let i = 0; i < 6; i++) {
        const a = (i / 6) * Math.PI * 2;
        const pillar = new THREE.Mesh(pillarGeo, stone);
        pillar.position.set(Math.cos(a) * 3.9, 1.25, Math.sin(a) * 3.9);
        pillar.castShadow = true;
        const cap = new THREE.Mesh(capGeo, gem);
        cap.position.set(Math.cos(a) * 3.9, 2.6, Math.sin(a) * 3.9);
        g.add(pillar, cap);
      }
      const crystal = new THREE.Mesh(crystalGeo, new THREE.MeshStandardMaterial({ color: c, emissive: c, emissiveIntensity: 1.35, roughness: 0.2, metalness: 0.1 }));
      crystal.position.y = 2.6;
      g.add(crystal);
      this.fountainCrystals.push(crystal);
      // lueur au sol (additive, sans éclairage) : remplace la PointLight
      const halo = new THREE.Mesh(
        new THREE.CircleGeometry(7, 32).rotateX(-Math.PI / 2),
        new THREE.ShaderMaterial({
          uniforms: { uColor: { value: c.clone().multiplyScalar(0.55) } },
          vertexShader: 'varying vec2 vUv; void main() { vUv = uv; gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0); }',
          fragmentShader: 'uniform vec3 uColor; varying vec2 vUv; void main() { float d = length(vUv - 0.5) * 2.0; float a = pow(1.0 - clamp(d, 0.0, 1.0), 2.2); gl_FragColor = vec4(uColor * a, 1.0); }',
          blending: THREE.AdditiveBlending,
          transparent: true,
          depthWrite: false,
        }),
      );
      halo.position.y = 0.125;
      halo.renderOrder = 2;
      g.add(halo);
      g.position.set(f.x * S, terrainHeight(f.x, f.y) * S, -f.y * S);
      this.scene.add(g);
      this.fountainGroups.push(g);
    }
  }

  /** Bascule de qualité à chaud : ombres et décor de fond. */
  setQuality(q: 'low' | 'high'): void {
    this.sun.castShadow = q === 'high';
    this.vegetation.setQuality(q);
  }

  /** Suit la vue avec la lumière d'ombre, anime eau/végétation, brousse transparente autour du joueur. */
  update(dt: number, camera: THREE.Camera, self: Vec2 | null): void {
    this.t += dt;
    this.focus.set(camera.position.x, 0, camera.position.z - camera.position.y * 0.67);
    // accrochage au texel d'ombre : sans lui, les bords d'ombre « grouillent » quand la caméra glisse
    const sc = this.sun.shadow.camera;
    const texel = (sc.right - sc.left) / this.sun.shadow.mapSize.x;
    const r = Math.round(this.focus.dot(this.lightRight) / texel) * texel;
    const u = Math.round(this.focus.dot(this.lightUp) / texel) * texel;
    const f = this.focus.dot(this.lightFwd);
    this.focus.set(0, 0, 0).addScaledVector(this.lightRight, r).addScaledVector(this.lightUp, u).addScaledVector(this.lightFwd, f);
    this.sun.position.copy(this.focus).add(this.sunOffset);
    this.sun.target.position.copy(this.focus);
    this.water.update(dt);
    this.vegetation.update(dt, self);
    for (const p of this.fountainPools) p.uniforms.uTime!.value = this.t;
    for (const c of this.fountainCrystals) {
      c.rotation.y += dt * 0.8;
      c.position.y = 2.6 + Math.sin(this.t * 1.5) * 0.15;
    }
  }
}

/** Fige les matrices d'un sous-arbre immobile (sauf `animated`, dont la matrice locale reste recalculée). */
function freezeMatrices(root: THREE.Object3D, animated: readonly THREE.Object3D[]): void {
  root.traverse((o) => {
    if (animated.includes(o)) return;
    o.updateMatrix();
    o.matrixAutoUpdate = false;
  });
  root.updateMatrixWorld(true);
}

/** Bassin de fontaine : remous et anneaux de runes tournants, couleur d'équipe en HDR (bloom léger). */
function poolMaterial(color: THREE.Color): THREE.ShaderMaterial {
  return new THREE.ShaderMaterial({
    uniforms: { uTime: { value: 0 }, uColor: { value: color }, uNoise: { value: noiseTexture() } },
    vertexShader: 'varying vec2 vUv; void main() { vUv = uv; gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0); }',
    fragmentShader: /* glsl */ `
      uniform float uTime;
      uniform vec3 uColor;
      uniform sampler2D uNoise;
      varying vec2 vUv;
      void main() {
        vec2 p = vUv - 0.5;
        float r = length(p) * 2.0;
        float a = atan(p.y, p.x);
        // remous : bruit en coordonnées polaires qui tourne
        float swirl = texture2D(uNoise, vec2(a / 6.2831853 * 3.0 + uTime * 0.03 + r * 0.4, r * 0.8 - uTime * 0.05)).g;
        vec3 deep = uColor * 0.18 + vec3(0.02, 0.03, 0.05);
        vec3 col = mix(deep, uColor * 0.7, smoothstep(0.35, 0.85, swirl) * (0.4 + 0.6 * r));
        // deux anneaux de runes (segments) qui tournent en sens inverse
        float ring1 = 1.0 - smoothstep(0.012, 0.03, abs(r - 0.62));
        float seg1 = step(0.45, fract((a / 6.2831853) * 24.0 + uTime * 0.08));
        float ring2 = 1.0 - smoothstep(0.01, 0.025, abs(r - 0.4));
        float seg2 = step(0.3, fract((a / 6.2831853) * 16.0 - uTime * 0.12));
        col += uColor * (ring1 * seg1 + ring2 * seg2) * 2.2;
        // bord : écume lumineuse ; centre : puits de lumière sous le cristal
        col += uColor * smoothstep(0.85, 1.0, r) * 0.8;
        col += uColor * (1.0 - smoothstep(0.0, 0.25, r)) * 1.2;
        gl_FragColor = vec4(col, 1.0);
        #include <tonemapping_fragment>
        #include <colorspace_fragment>
      }`,
  });
}
