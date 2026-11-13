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
import { TEAM_COLOR } from './views/DefaultViews.ts';

export class MapView {
  readonly ground: THREE.Mesh;
  readonly sun: THREE.DirectionalLight;
  readonly water: Water;
  readonly vegetation: Vegetation;
  private sunOffset = new THREE.Vector3(14, 30, 9);
  private focus = new THREE.Vector3();
  private fountainCrystals: THREE.Object3D[] = [];
  private t = 0;

  constructor(private readonly scene: THREE.Scene, _renderer: THREE.WebGLRenderer, quality: 'low' | 'high' = 'high') {
    scene.background = new THREE.Color(0x14202a);

    scene.add(new THREE.HemisphereLight(0xfff1dc, 0x3a4a30, 1.25));
    this.sun = new THREE.DirectionalLight(0xffe2b8, 2.6);
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

    this.ground = buildTerrain(quality);
    scene.add(this.ground);
    this.water = new Water();
    scene.add(this.water.mesh);
    this.vegetation = new Vegetation(quality);
    scene.add(this.vegetation.group);
    this.buildFountains();
  }

  private buildFountains(): void {
    for (const team of ['blue', 'red'] as const) {
      const f = FOUNTAINS[team];
      const c = TEAM_COLOR[team];
      const g = new THREE.Group();
      const stone = new THREE.MeshStandardMaterial({ color: 0x9a927f, roughness: 0.9, flatShading: true });
      const plat = new THREE.Mesh(new THREE.CylinderGeometry(4.2, 4.6, 0.3, 12), stone);
      plat.position.y = 0.15;
      plat.receiveShadow = true;
      g.add(plat);
      const glow = new THREE.MeshStandardMaterial({ color: c, emissive: c, emissiveIntensity: 1.4 });
      const ring = new THREE.Mesh(new THREE.TorusGeometry(3.6, 0.08, 6, 48).rotateX(Math.PI / 2), glow);
      ring.position.y = 0.32;
      g.add(ring);
      for (let i = 0; i < 6; i++) {
        const a = (i / 6) * Math.PI * 2;
        const pillar = new THREE.Mesh(new THREE.CylinderGeometry(0.22, 0.3, 2.2, 6), stone);
        pillar.position.set(Math.cos(a) * 3.2, 1.25, Math.sin(a) * 3.2);
        pillar.castShadow = true;
        g.add(pillar);
      }
      const crystal = new THREE.Mesh(new THREE.OctahedronGeometry(0.9), glow);
      crystal.position.y = 2.6;
      crystal.scale.set(1, 1.6, 1);
      g.add(crystal);
      this.fountainCrystals.push(crystal);
      const light = new THREE.PointLight(c, 6, 14, 1.6);
      light.position.y = 2.6;
      g.add(light);
      g.position.set(f.x * S, terrainHeight(f.x, f.y) * S, -f.y * S);
      this.scene.add(g);
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
    this.sun.position.copy(this.focus).add(this.sunOffset);
    this.sun.target.position.copy(this.focus);
    this.water.update(dt);
    this.vegetation.update(dt, self);
    for (const c of this.fountainCrystals) {
      c.rotation.y += dt * 0.8;
      c.position.y = 2.6 + Math.sin(this.t * 1.5) * 0.15;
    }
  }
}
