/**
 * Rendu Three.js de la partie. Propriétaire : claude-client.
 *
 * API publique (contrats pour claude-fx / mistral-ui) :
 *  - `scene`, `camera`, `renderer`
 *  - `toWorld(p, h)` / `toGame(v)` (réexportés depuis ./coords.ts)
 *  - `getEntityObject(id)` : Object3D de l'entité (racine au sol), ou undefined
 *  - `pickEntity(clientX, clientY)` : id de l'entité sous le curseur (hitbox simplifiées), ou null
 *  - `pickGround(clientX, clientY)` : position jeu au sol sous le curseur, ou null
 *  - `onFrame(cb(dt))` : hook appelé à chaque frame avant le rendu ; retourne une fonction de désabonnement
 *  - `interpolatedEntity(id)` : position/orientation affichées (interpolées ; prédites pour le joueur local)
 *  - `registerChampionFactory` : voir ./EntityView.ts
 */
import * as THREE from 'three';
import { INHIBITORS, NEXUS, TOWERS, isInWall, type EntityId, type EntitySnapshot, type Snapshot, type Vec2 } from '@lov/shared';
import { clock, flushEvents, net, renderTime, snapshots } from '../net/client.ts';
import { MovePredictor } from '../net/prediction.ts';
import type { Interpolated } from '../net/snapshots.ts';
import { store } from '../state.ts';
import { CameraController } from './CameraController.ts';
import { ClickMarkers } from './ClickMarkers.ts';
import { HealthBars } from './HealthBars.ts';
import { S, toGameInto, toWorld, toGame } from './coords.ts';
import { getChampionFactory, type EntityView } from './EntityView.ts';
import { MapView } from './MapView.ts';
import { terrainHeight } from './map/geometry.ts';
import { rayCylinder, worldHitbox, type Ray3 } from './picking.ts';
import { defaultViewFor, setDragonElement } from './views/DefaultViews.ts';
import { inhibitorModel, nexusModel, towerModel } from './models/structures.ts';

export { toWorld, toGame };
export { registerChampionFactory, type ChampionView, type EntityView } from './EntityView.ts';

type FrameCb = (dt: number) => void;

interface Slot {
  view: EntityView;
  anim: string;
  dead: boolean;
}

/** La ligne droite a→b traverse-t-elle un mur ? (échantillonnage tous les 50 u) */
function segmentBlocked(a: Vec2, b: Vec2): boolean {
  const d = Math.hypot(b.x - a.x, b.y - a.y);
  const n = Math.ceil(d / 50);
  const p = { x: 0, y: 0 };
  for (let i = 1; i <= n; i++) {
    p.x = a.x + ((b.x - a.x) * i) / n;
    p.y = a.y + ((b.y - a.y) * i) / n;
    if (isInWall(p)) return true;
  }
  return false;
}

let current: GameRenderer | null = null;

/** Instance active du renderer (pour claude-fx / mistral-ui), null avant création. */
export function getRenderer(): GameRenderer | null {
  return current;
}

/** Position/orientation affichées d'une entité (null si inconnue). L'objet retourné est réutilisé : copier si besoin. */
export function interpolatedEntity(id: EntityId): Readonly<Interpolated> | null {
  return current?.interpolatedEntity(id) ?? null;
}

export class GameRenderer {
  readonly renderer: THREE.WebGLRenderer;
  readonly scene = new THREE.Scene();
  readonly cam: CameraController;
  readonly camera: THREE.PerspectiveCamera;
  readonly predictor = new MovePredictor(segmentBlocked);
  readonly map: MapView;
  readonly markers: ClickMarkers;
  readonly bars: HealthBars;
  private slots = new Map<EntityId, Slot>();
  private positions = new Map<EntityId, Interpolated>();
  private staticStructures: { pos: Vec2; view: EntityView }[] = [];
  private frameCbs = new Set<FrameCb>();
  private raycaster = new THREE.Raycaster();
  private ndc = new THREE.Vector2();
  private groundPlane = new THREE.Plane(new THREE.Vector3(0, 1, 0), 0);
  private tmpV = new THREE.Vector3();
  private ray: Ray3 = { ox: 0, oy: 0, oz: 0, dx: 0, dy: 0, dz: 0 };
  private vel = { x: 0, y: 0 };
  private lastFrame = performance.now();
  private lastFocusSeq = -1;
  private cameraInitForGame: number | null = null;

  constructor(canvas: HTMLCanvasElement) {
    current = this;
    this.renderer = new THREE.WebGLRenderer({ canvas, antialias: true, powerPreference: 'high-performance' });
    this.renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
    this.renderer.shadowMap.enabled = true;
    this.renderer.shadowMap.type = THREE.PCFSoftShadowMap;
    this.renderer.outputColorSpace = THREE.SRGBColorSpace;
    this.renderer.toneMapping = THREE.ACESFilmicToneMapping;
    this.renderer.toneMappingExposure = 1.05;
    this.cam = new CameraController(innerWidth / innerHeight);
    this.camera = this.cam.camera;
    this.scene.add(this.camera); // permet d'attacher des objets à la caméra (FX écran)

    this.map = new MapView(this.scene, this.renderer, store.state.settings.quality);
    this.markers = new ClickMarkers(this.scene);
    this.bars = new HealthBars(this.scene);
    this.buildStaticStructures();

    addEventListener('resize', () => this.resize());
    this.resize();
    store.subscribe((s) => s.snapshot && this.applySnapshot(s.snapshot));
    net.on((msg) => {
      if (msg.t === 'snapshot') this.onRawSnapshot(msg.snap);
    });
    this.renderer.setAnimationLoop(() => this.frame());
  }

  /** Structures issues de map.ts, visibles tant que le serveur n'envoie pas l'entité correspondante. */
  private buildStaticStructures(): void {
    const add = (pos: Vec2, view: EntityView) => {
      view.object3d.position.copy(toWorld(pos, terrainHeight(pos.x, pos.y) * S));
      this.scene.add(view.object3d);
      this.staticStructures.push({ pos, view });
    };
    for (const t of TOWERS) add(t.pos, towerModel(t.team, t.tier));
    for (const i of INHIBITORS) add(i.pos, inhibitorModel(i.team));
    for (const team of ['blue', 'red'] as const) add(NEXUS[team], nexusModel(team));
  }

  private hideStaticNear(e: EntitySnapshot): void {
    for (const s of this.staticStructures) {
      if (s.view.object3d.visible && Math.abs(s.pos.x - e.x) < 200 && Math.abs(s.pos.y - e.y) < 200) s.view.object3d.visible = false;
    }
  }

  // ------------------------------------------------------------------ snapshots

  private onRawSnapshot(snap: Snapshot): void {
    const selfId = store.state.game?.entityId;
    for (const ev of snap.events) {
      if (ev.type === 'dash' && ev.source === selfId) this.predictor.suspend(ev.duration + 0.1);
      else if (ev.type === 'objectiveSpawn' && ev.monster === 'dragon') setDragonElement(ev.element);
    }
  }

  private applySnapshot(snap: Snapshot): void {
    for (const e of snap.entities) {
      let slot = this.slots.get(e.id);
      if (!slot) {
        const view = this.createView(e);
        if (!view) continue;
        slot = { view, anim: '', dead: false };
        this.slots.set(e.id, slot);
        this.scene.add(view.object3d);
        if (e.kind === 'tower' || e.kind === 'inhibitor' || e.kind === 'nexus') this.hideStaticNear(e);
      }
      const anim = e.dead ? 'death' : (e.anim ?? 'idle');
      if (anim !== slot.anim) {
        slot.anim = anim;
        slot.view.playAnim(anim);
      }
      slot.dead = !!e.dead;
    }
    // entités disparues (snapshot delta ou complet : le buffer fait foi)
    for (const [id, slot] of this.slots) {
      if (!snapshots.entities.has(id)) {
        slot.view.dispose();
        slot.view.object3d.removeFromParent();
        this.slots.delete(id);
        this.positions.delete(id);
        this.bars.remove(id);
      }
    }
  }

  private createView(e: EntitySnapshot): EntityView | null {
    if (e.kind === 'champion') {
      const f = getChampionFactory();
      const v = f?.(e);
      if (v) return v;
    }
    return defaultViewFor(e);
  }

  // ------------------------------------------------------------------ API publique

  getEntityObject(id: EntityId): THREE.Object3D | undefined {
    return this.slots.get(id)?.view.object3d;
  }

  interpolatedEntity(id: EntityId): Readonly<Interpolated> | null {
    return this.positions.get(id) ?? null;
  }

  onFrame(cb: FrameCb): () => void {
    this.frameCbs.add(cb);
    return () => this.frameCbs.delete(cb);
  }

  private setRay(clientX: number, clientY: number): void {
    const rect = this.renderer.domElement.getBoundingClientRect();
    this.ndc.set(((clientX - rect.left) / rect.width) * 2 - 1, -((clientY - rect.top) / rect.height) * 2 + 1);
    this.raycaster.setFromCamera(this.ndc, this.camera);
  }

  /** Position au sol sous le curseur (coordonnées jeu), ou null. */
  pickGround(clientX: number, clientY: number, out: Vec2 = { x: 0, y: 0 }): Vec2 | null {
    this.setRay(clientX, clientY);
    // intersection avec le plan y = 0 puis 2 raffinements sur la hauteur réelle du terrain (rivière creusée)
    this.groundPlane.constant = 0;
    let hit = this.raycaster.ray.intersectPlane(this.groundPlane, this.tmpV);
    for (let i = 0; hit && i < 2; i++) {
      this.groundPlane.constant = -terrainHeight(hit.x / S, -hit.z / S) * S;
      hit = this.raycaster.ray.intersectPlane(this.groundPlane, this.tmpV);
    }
    return hit ? toGameInto(out, hit) : null;
  }

  /**
   * Entité sous le curseur (la plus proche de la caméra), ou null.
   * `filter` permet d'exclure des entités (ex. alliés pour une attaque).
   */
  pickEntity(clientX: number, clientY: number, filter?: (e: EntitySnapshot) => boolean): EntityId | null {
    this.setRay(clientX, clientY);
    const r = this.raycaster.ray;
    const ray = this.ray;
    ray.ox = r.origin.x;
    ray.oy = r.origin.y;
    ray.oz = r.origin.z;
    ray.dx = r.direction.x;
    ray.dy = r.direction.y;
    ray.dz = r.direction.z;
    let best: EntityId | null = null;
    let bestT = Infinity;
    for (const [id, slot] of this.slots) {
      if (slot.dead || !slot.view.object3d.visible) continue;
      const e = snapshots.entities.get(id);
      if (!e || (filter && !filter(e))) continue;
      const hb = worldHitbox(e);
      const p = this.positions.get(id);
      if (!hb || !p) continue;
      const t = rayCylinder(ray, p.x * S, -p.y * S, hb[0], hb[1]);
      if (t >= 0 && t < bestT) {
        bestT = t;
        best = id;
      }
    }
    return best;
  }

  /** Ordre de déplacement local (prédiction + marqueur). Appelé par l'input. */
  localMoveCommand(pos: Vec2, attack = false): void {
    this.predictor.command(pos, (store.state.rtt ?? 80) / 1000);
    this.markers.spawn(pos, attack ? 0xff4040 : 0x5cff8a);
  }

  // ------------------------------------------------------------------ boucle

  private updatePositions(dt: number): void {
    const now = Date.now();
    clock.update(dt * 1000);
    const rt = renderTime(now);
    const selfId = store.state.game?.entityId;
    for (const id of this.slots.keys()) {
      let p = this.positions.get(id);
      if (!p) {
        p = { x: 0, y: 0, facing: 0 };
        this.positions.set(id, p);
      }
      if (id === selfId) this.predictSelf(id, p, dt, now);
      else snapshots.sample(id, rt, p);
    }
    flushEvents(rt);
  }

  private predictSelf(id: EntityId, out: Interpolated, dt: number, now: number): void {
    const e = snapshots.entities.get(id);
    const last = snapshots.latestSample(id);
    if (!e || !last) return;
    snapshots.velocity(id, this.vel);
    const lead = (clock.serverNow(now) - last.t) / 1000 + (store.state.rtt ?? 80) / 2000;
    const speed = store.state.snapshot?.self?.stats.moveSpeed ?? 335;
    const px = out.x;
    const py = out.y;
    const pos = this.predictor.update(dt, { x: last.x, y: last.y, vx: this.vel.x, vy: this.vel.y, lead }, speed, !e.dead);
    out.x = pos.x;
    out.y = pos.y;
    const mx = pos.x - px;
    const my = pos.y - py;
    if (this.predictor.moving && mx * mx + my * my > 1) out.facing = Math.atan2(my, mx);
    else out.facing = e.facing;
  }

  private frame(): void {
    const nowMs = performance.now();
    const dt = Math.min(0.1, (nowMs - this.lastFrame) / 1000);
    this.lastFrame = nowMs;
    const s = store.state;

    this.updatePositions(dt);

    const myTeam = s.game?.team;
    const selfId = s.game?.entityId;
    this.bars.begin();
    for (const [id, slot] of this.slots) {
      const p = this.positions.get(id);
      const e = snapshots.entities.get(id);
      if (!p || !e) continue;
      const o = slot.view.object3d;
      o.position.set(p.x * S, terrainHeight(p.x, p.y) * S, -p.y * S);
      o.rotation.y = p.facing;
      slot.view.update(e, dt);
      if (o.visible) this.bars.push(e, o.position.x, o.position.y, o.position.z, myTeam, id === selfId);
    }
    this.bars.end();

    // caméra
    const selfPos = s.game ? this.positions.get(s.game.entityId) : undefined;
    if (s.cameraFocus && s.cameraFocus.seq !== this.lastFocusSeq) {
      this.lastFocusSeq = s.cameraFocus.seq;
      this.cam.jumpTo(s.cameraFocus.pos);
      if (s.game) this.cameraInitForGame = s.game.entityId; // un focus explicite remplace le centrage initial
      if (s.cameraLocked) store.set({ cameraLocked: false });
    }
    if (s.game && this.cameraInitForGame !== s.game.entityId && selfPos && (selfPos.x || selfPos.y)) {
      this.cameraInitForGame = s.game.entityId;
      this.cam.jumpTo(selfPos);
    }
    this.cam.update(dt, selfPos ?? null, s.cameraLocked, s.settings.edgePan, s.settings.cameraSpeed, innerWidth, innerHeight);
    s.cameraCenter.x = this.cam.center.x;
    s.cameraCenter.y = this.cam.center.y;

    this.map.update(dt, this.camera, selfPos ?? null);
    this.markers.update(dt);
    for (const cb of this.frameCbs) cb(dt);
    this.renderer.render(this.scene, this.camera);
  }

  private resize(): void {
    this.renderer.setSize(innerWidth, innerHeight, false);
    this.cam.resize(innerWidth / innerHeight);
    this.bars.setViewport(innerWidth, innerHeight);
    this.bars.updateLabelScale(innerHeight, this.camera.fov);
  }
}
