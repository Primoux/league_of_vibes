/**
 * Carte 3D « Faille des Vibes ». Propriétaire : claude-client.
 * Version M1 : sol, voies, rivière, murs extrudés, brousses, lumières. Le terrain détaillé arrive en M2.
 */
import * as THREE from 'three';
import { BRUSHES, LANE_PATHS, MAP_SIZE, WALLS, type Polygon, type Vec2 } from '@lov/shared';
import { S } from './coords.ts';

export class MapView {
  readonly ground: THREE.Mesh;
  readonly sun: THREE.DirectionalLight;
  private sunOffset = new THREE.Vector3(18, 40, 12);
  private focus = new THREE.Vector3();

  constructor(private readonly scene: THREE.Scene, _renderer: THREE.WebGLRenderer) {
    scene.background = new THREE.Color(0x0b1020);
    scene.fog = new THREE.Fog(0x0b1020, 60, 140);

    scene.add(new THREE.HemisphereLight(0xffe8c8, 0x2a3a28, 1.1));
    this.sun = new THREE.DirectionalLight(0xfff0d8, 2.2);
    this.sun.castShadow = true;
    this.sun.shadow.mapSize.set(2048, 2048);
    const sc = this.sun.shadow.camera;
    sc.left = -22;
    sc.right = 22;
    sc.top = 22;
    sc.bottom = -22;
    sc.near = 1;
    sc.far = 120;
    this.sun.shadow.bias = -0.0005;
    scene.add(this.sun, this.sun.target);

    const size = MAP_SIZE * S;
    this.ground = new THREE.Mesh(new THREE.PlaneGeometry(size + 20, size + 20).rotateX(-Math.PI / 2), new THREE.MeshStandardMaterial({ color: 0x35552e, roughness: 1 }));
    this.ground.position.set(size / 2, 0, -size / 2);
    this.ground.receiveShadow = true;
    scene.add(this.ground);

    this.buildLanes();
    this.buildRiver();
    this.buildWalls();
    this.buildBrushes();
  }

  private flatPolygon(poly: Polygon, color: number, y: number, opacity = 1): THREE.Mesh {
    const shape = new THREE.Shape(poly.map((p) => new THREE.Vector2(p.x * S, p.y * S)));
    const geo = new THREE.ShapeGeometry(shape).rotateX(-Math.PI / 2);
    const m = new THREE.Mesh(geo, new THREE.MeshStandardMaterial({ color, roughness: 1, transparent: opacity < 1, opacity }));
    m.position.y = y;
    m.receiveShadow = true;
    return m;
  }

  private buildLanes(): void {
    const mat = new THREE.MeshStandardMaterial({ color: 0x8a7a58, roughness: 1 });
    const width = 6.5; // ≈ 650 u
    for (const path of Object.values(LANE_PATHS)) {
      for (let i = 0; i < path.length - 1; i++) this.scene.add(this.strip(path[i]!, path[i + 1]!, width, mat, 0.01));
    }
  }

  private strip(a: Vec2, b: Vec2, width: number, mat: THREE.Material, y: number): THREE.Mesh {
    const len = Math.hypot(b.x - a.x, b.y - a.y) * S;
    const m = new THREE.Mesh(new THREE.PlaneGeometry(len + width, width).rotateX(-Math.PI / 2), mat);
    m.position.set(((a.x + b.x) / 2) * S, y, (-(a.y + b.y) / 2) * S);
    m.rotation.y = Math.atan2(b.y - a.y, b.x - a.x);
    m.receiveShadow = true;
    return m;
  }

  private buildRiver(): void {
    const mat = new THREE.MeshStandardMaterial({ color: 0x2c6a9a, roughness: 0.25, metalness: 0.1 });
    this.scene.add(this.strip({ x: 1500, y: 10500 }, { x: 10500, y: 1500 }, 10, mat, 0.005));
  }

  private buildWalls(): void {
    const mat = new THREE.MeshStandardMaterial({ color: 0x5b4f43, roughness: 0.95, flatShading: true });
    for (const poly of WALLS) {
      const shape = new THREE.Shape(poly.map((p) => new THREE.Vector2(p.x * S, p.y * S)));
      const geo = new THREE.ExtrudeGeometry(shape, { depth: 1.4, bevelEnabled: true, bevelSize: 0.3, bevelThickness: 0.3, bevelSegments: 1 });
      const m = new THREE.Mesh(geo, mat);
      m.rotation.x = -Math.PI / 2;
      m.castShadow = true;
      m.receiveShadow = true;
      this.scene.add(m);
    }
  }

  private buildBrushes(): void {
    for (const poly of BRUSHES) this.scene.add(this.flatPolygon(poly, 0x2f7a3a, 0.02));
  }

  /** Suit la vue avec la lumière d'ombre (ombres nettes autour de la caméra). */
  update(_dt: number, camera: THREE.Camera): void {
    // point du sol visé par la caméra ≈ projection de sa position
    this.focus.set(camera.position.x, 0, camera.position.z - camera.position.y * 0.67);
    this.sun.position.copy(this.focus).add(this.sunOffset);
    this.sun.target.position.copy(this.focus);
  }
}
