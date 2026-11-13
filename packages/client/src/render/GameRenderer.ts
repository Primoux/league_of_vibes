/**
 * Rendu Three.js de la partie. Propriétaire : claude-client.
 * BOOTSTRAP : sol, structures depuis shared/map, entités en formes simples, caméra iso qui suit le joueur,
 * interpolation naïve. TODO(claude-client) : interpolation bufferisée, terrain, murs 3D, brouillard, barres de vie.
 */
import * as THREE from 'three';
import {
  CAMPS,
  INHIBITORS,
  MAP_SIZE,
  NEXUS,
  PITS,
  RENDER_SCALE,
  TOWERS,
  WALLS,
  type EntitySnapshot,
  type Snapshot,
  type Vec2,
} from '@lov/shared';
import { fxBus } from '../fx/FxBus.ts';
import { store } from '../state.ts';

const S = RENDER_SCALE;
/** Coordonnées jeu (x, y) -> monde Three.js (x, 0, -y) : y du jeu = -z. */
export const toWorld = (p: Vec2, h = 0): THREE.Vector3 => new THREE.Vector3(p.x * S, h, -p.y * S);
export const toGame = (v: THREE.Vector3): Vec2 => ({ x: v.x / S, y: -v.z / S });

const TEAM_COLOR = { blue: 0x3a7bff, red: 0xff4a4a, neutral: 0xc9a227 } as const;

export class GameRenderer {
  readonly renderer: THREE.WebGLRenderer;
  readonly scene = new THREE.Scene();
  readonly camera: THREE.PerspectiveCamera;
  private ground: THREE.Mesh;
  private meshes = new Map<number, THREE.Object3D>();
  private targets = new Map<number, THREE.Vector3>();
  private raycaster = new THREE.Raycaster();
  private camTarget = new THREE.Vector3();
  private lastTick = -1;

  constructor(canvas: HTMLCanvasElement) {
    this.renderer = new THREE.WebGLRenderer({ canvas, antialias: true });
    this.renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
    this.renderer.shadowMap.enabled = true;
    this.scene.background = new THREE.Color(0x0b1020);
    this.camera = new THREE.PerspectiveCamera(40, 1, 0.1, 500);

    this.scene.add(new THREE.HemisphereLight(0xbfd8ff, 0x203020, 0.9));
    const sun = new THREE.DirectionalLight(0xffffff, 1.6);
    sun.position.set(40, 80, 20);
    this.scene.add(sun);

    const size = MAP_SIZE * S;
    this.ground = new THREE.Mesh(new THREE.PlaneGeometry(size, size), new THREE.MeshStandardMaterial({ color: 0x2f4f2a }));
    this.ground.rotation.x = -Math.PI / 2;
    this.ground.position.set(size / 2, 0, -size / 2);
    this.scene.add(this.ground);
    this.buildMap();

    addEventListener('resize', () => this.resize());
    this.resize();
    store.subscribe((s) => s.snapshot && this.applySnapshot(s.snapshot));
    this.renderer.setAnimationLoop(() => this.frame());
  }

  private buildMap(): void {
    const river = new THREE.Mesh(new THREE.PlaneGeometry(2.2, MAP_SIZE * S * 1.3), new THREE.MeshStandardMaterial({ color: 0x2a5d8f }));
    river.rotation.set(-Math.PI / 2, 0, -Math.PI / 4);
    river.position.copy(toWorld({ x: MAP_SIZE / 2, y: MAP_SIZE / 2 }, 0.01));
    this.scene.add(river);
    const wallMat = new THREE.MeshStandardMaterial({ color: 0x4a4038 });
    for (const poly of WALLS) {
      const shape = new THREE.Shape(poly.map((p) => new THREE.Vector2(p.x * S, p.y * S)));
      const geo = new THREE.ExtrudeGeometry(shape, { depth: 1.2, bevelEnabled: false });
      const m = new THREE.Mesh(geo, wallMat);
      m.rotation.x = -Math.PI / 2;
      this.scene.add(m);
    }
    for (const t of TOWERS) this.addStatic(new THREE.CylinderGeometry(0.5, 0.7, 3), TEAM_COLOR[t.team], t.pos, 1.5);
    for (const i of INHIBITORS) this.addStatic(new THREE.TorusGeometry(0.6, 0.2), TEAM_COLOR[i.team], i.pos, 0.8);
    for (const team of ['blue', 'red'] as const) this.addStatic(new THREE.OctahedronGeometry(1.4), TEAM_COLOR[team], NEXUS[team], 1.6);
    for (const c of CAMPS) this.addStatic(new THREE.CircleGeometry(0.5), 0x8a7a40, c.pos, 0.02, true);
    for (const pit of Object.values(PITS)) this.addStatic(new THREE.CircleGeometry(pit.radius * S), 0x1d3d5f, pit.pos, 0.015, true);
  }

  private addStatic(geo: THREE.BufferGeometry, color: number, pos: Vec2, h: number, flat = false): void {
    const m = new THREE.Mesh(geo, new THREE.MeshStandardMaterial({ color }));
    m.position.copy(toWorld(pos, h));
    if (flat) m.rotation.x = -Math.PI / 2;
    this.scene.add(m);
  }

  private meshFor(e: EntitySnapshot): THREE.Object3D {
    let m = this.meshes.get(e.id);
    if (m) return m;
    const color = TEAM_COLOR[e.team];
    const geo = e.kind === 'champion' ? new THREE.CapsuleGeometry(0.45, 0.9) : new THREE.BoxGeometry(0.5, 0.5, 0.5);
    m = new THREE.Mesh(geo, new THREE.MeshStandardMaterial({ color }));
    m.position.copy(toWorld(e, 0.9));
    this.scene.add(m);
    this.meshes.set(e.id, m);
    return m;
  }

  applySnapshot(snap: Snapshot): void {
    if (snap.tick === this.lastTick) return;
    this.lastTick = snap.tick;
    const seen = new Set<number>();
    for (const e of snap.entities) {
      seen.add(e.id);
      const m = this.meshFor(e);
      m.visible = !e.dead;
      this.targets.set(e.id, toWorld(e, 0.9));
    }
    for (const [id, m] of this.meshes) {
      if (!seen.has(id)) {
        this.scene.remove(m);
        this.meshes.delete(id);
        this.targets.delete(id);
      }
    }
    for (const ev of snap.events) fxBus.dispatch(ev);
  }

  /** Position au sol sous le curseur (coordonnées jeu), ou null. */
  pickGround(clientX: number, clientY: number): Vec2 | null {
    const ndc = new THREE.Vector2((clientX / innerWidth) * 2 - 1, -(clientY / innerHeight) * 2 + 1);
    this.raycaster.setFromCamera(ndc, this.camera);
    const hit = this.raycaster.intersectObject(this.ground)[0];
    return hit ? toGame(hit.point) : null;
  }

  private frame(): void {
    for (const [id, m] of this.meshes) {
      const t = this.targets.get(id);
      if (t) m.position.lerp(t, 0.25);
    }
    const s = store.state;
    const me = s.game ? this.meshes.get(s.game.entityId) : undefined;
    if (me) this.camTarget.lerp(me.position, 0.15);
    else if (s.game) this.camTarget.copy(toWorld({ x: 1500, y: 1500 }));
    this.camera.position.set(this.camTarget.x, this.camTarget.y + 22, this.camTarget.z + 14);
    this.camera.lookAt(this.camTarget);
    this.renderer.render(this.scene, this.camera);
  }

  private resize(): void {
    this.renderer.setSize(innerWidth, innerHeight, false);
    this.camera.aspect = innerWidth / innerHeight;
    this.camera.updateProjectionMatrix();
  }
}
