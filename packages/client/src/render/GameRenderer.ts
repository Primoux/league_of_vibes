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
import { INHIBITORS, NEXUS, TOWERS, type EntityId, type EntitySnapshot, type Snapshot, type Vec2 } from '@lov/shared';
import { clock, flushEvents, net, renderTime, snapshots } from '../net/client.ts';
import { MovePredictor, segmentBlocked } from '../net/prediction.ts';
import type { Interpolated } from '../net/snapshots.ts';
import { store, type ClientState } from '../state.ts';
import { CameraController } from './CameraController.ts';
import { ClickMarkers } from './ClickMarkers.ts';
import { HealthBars } from './HealthBars.ts';
import { MeshBatcher, type BatchHandle } from './MeshBatcher.ts';
import { StatsOverlay } from './StatsOverlay.ts';
import { FogOfWar } from './FogOfWar.ts';
import { Indicators, computeAim, newAimState, type AimState } from './Indicators.ts';
import { S, toGameInto, toWorld, toGame } from './coords.ts';
import { getChampionFactory, type EntityView } from './EntityView.ts';
import { MapView } from './MapView.ts';
import { terrainHeight } from './map/geometry.ts';
import { rayCylinder, worldHitbox, type Ray3 } from './picking.ts';
import { defaultViewFor, getDragonElement, setDragonElement } from './views/DefaultViews.ts';
import { inhibitorModel, nexusModel, towerModel } from './models/structures.ts';

export { toWorld, toGame };
export { registerChampionFactory, type ChampionView, type EntityView } from './EntityView.ts';
export { registerIndicatorRenderer, type AimState, type IndicatorRenderer } from './Indicators.ts';

type FrameCb = (dt: number) => void;

interface Slot {
  view: EntityView;
  anim: string;
  dead: boolean;
  /** Variante visuelle avec laquelle la vue a été construite (élément des drakes). */
  variant?: string;
  /** Meshes regroupés par le MeshBatcher (vues procédurales de claude-client). */
  batch?: BatchHandle;
}


type ClientStateCameraView = ClientState['cameraView'];

let current: GameRenderer | null = null;

/** Tremblement de caméra (voir GameRenderer.shake) ; sans effet avant la création du renderer. */
export function shakeCamera(intensity: number, duration: number): void {
  current?.shake(intensity, duration);
}

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
  /** Brouillard de guerre ; `fog.canvas` = visibilité (blanc = visible) pour la minimap. */
  readonly fog = new FogOfWar();
  /** État de visée courant (mis à jour chaque frame, objet réutilisé). */
  readonly aim: AimState = newAimState();
  private indicators: Indicators;
  private slots = new Map<EntityId, Slot>();
  private positions = new Map<EntityId, Interpolated>();
  private staticStructures: { pos: Vec2; view: EntityView }[] = [];
  /** Regroupe les meshes des vues procédurales en InstancedMesh (perf). */
  readonly batcher: MeshBatcher;
  private frameCbs = new Set<FrameCb>();
  private raycaster = new THREE.Raycaster();
  private ndc = new THREE.Vector2();
  private groundPlane = new THREE.Plane(new THREE.Vector3(0, 1, 0), 0);
  private tmpV = new THREE.Vector3();
  private ray: Ray3 = { ox: 0, oy: 0, oz: 0, dx: 0, dy: 0, dz: 0 };
  private vel = { x: 0, y: 0 };
  private lastFrame = performance.now();
  private lastFocusSeq = -1;
  private lastSnapshot: Snapshot | null = null;
  private lastGame: unknown = null;
  private quality: 'low' | 'high' | null = null;
  private fogPos = (e: EntitySnapshot) => this.positions.get(e.id) ?? null;
  private readonly defaultRender = () => this.renderer.render(this.scene, this.camera);
  private renderFn: () => void = this.defaultRender;
  private stats: StatsOverlay | null = typeof location !== 'undefined' && new URLSearchParams(location.search).has('stats') ? new StatsOverlay() : null;
  private cameraInitForGame: number | null = null;

  constructor(canvas: HTMLCanvasElement) {
    current = this;
    this.renderer = new THREE.WebGLRenderer({ canvas, antialias: true, powerPreference: 'high-performance' });
    this.renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
    this.renderer.shadowMap.enabled = true;
    this.renderer.shadowMap.type = THREE.PCFShadowMap;
    this.renderer.outputColorSpace = THREE.SRGBColorSpace;
    this.renderer.toneMapping = THREE.ACESFilmicToneMapping;
    this.renderer.toneMappingExposure = 1.05;
    this.cam = new CameraController(innerWidth / innerHeight);
    this.camera = this.cam.camera;
    this.scene.add(this.camera); // permet d'attacher des objets à la caméra (FX écran)

    this.map = new MapView(this.scene, this.renderer, store.state.settings.quality);
    this.markers = new ClickMarkers(this.scene);
    this.batcher = new MeshBatcher(this.scene);
    this.bars = new HealthBars(this.scene);
    this.indicators = new Indicators(this.scene);
    this.buildStaticStructures();

    addEventListener('resize', () => this.resize());
    this.resize();
    this.applyQuality(store.state.settings.quality);
    store.subscribe((s) => {
      if (s.settings.quality !== this.quality) this.applyQuality(s.settings.quality);
      if (s.game !== this.lastGame) {
        this.lastGame = s.game;
        setDragonElement(undefined); // l'élément d'une partie précédente ne doit pas survivre
      }
      // le store notifie pour tout changement (survol, ciblage…) : n'appliquer que les nouveaux snapshots
      if (s.snapshot && s.snapshot !== this.lastSnapshot) {
        this.lastSnapshot = s.snapshot;
        this.applySnapshot(s.snapshot);
      }
    });
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
      this.batcher.adopt(view.object3d);
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
    }
  }

  private applySnapshot(snap: Snapshot): void {
    // métadonnées d'apparition AVANT de créer les vues (le store passe avant les handlers net.on)
    for (const ev of snap.events) if (ev.type === 'objectiveSpawn' && ev.monster === 'dragon') setDragonElement(ev.element);
    const element = getDragonElement() ?? '';
    for (const e of snap.entities) {
      let slot = this.slots.get(e.id);
      if (slot && e.kind === 'monster' && e.subtype === 'dragon' && slot.variant !== element) {
        // drake construit avec un élément périmé : on reconstruit sa vue
        this.removeSlot(e.id, slot);
        slot = undefined;
      }
      if (!slot) {
        const created = this.createView(e);
        if (!created) continue;
        const view = created.view;
        slot = { view, anim: '', dead: false, variant: e.subtype === 'dragon' ? element : undefined };
        if (created.batchable) slot.batch = this.batcher.adopt(view.object3d);
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
        this.removeSlot(id, slot);
        this.positions.delete(id);
        this.bars.remove(id);
      }
    }
  }

  /** Vue d'une entité ; `batchable` = vue procédurale de claude-client (les champions de claude-fx restent intacts). */
  private createView(e: EntitySnapshot): { view: EntityView; batchable: boolean } | null {
    if (e.kind === 'champion') {
      const v = getChampionFactory()?.(e);
      if (v) return { view: v, batchable: false };
    }
    const v = defaultViewFor(e);
    return v ? { view: v, batchable: true } : null;
  }

  private removeSlot(id: EntityId, slot: Slot): void {
    if (slot.batch) this.batcher.release(slot.batch);
    slot.view.dispose();
    slot.view.object3d.removeFromParent();
    this.slots.delete(id);
  }

  // ------------------------------------------------------------------ API publique

  getEntityObject(id: EntityId): THREE.Object3D | undefined {
    return this.slots.get(id)?.view.object3d;
  }

  interpolatedEntity(id: EntityId): Readonly<Interpolated> | null {
    return this.positions.get(id) ?? null;
  }

  /**
   * Qualité graphique (store.settings.quality), appliquée à chaud : résolution (pixel ratio), ombres,
   * décor de fond. Les FX suivent via setFxQuality (main.ts).
   */
  applyQuality(q: 'low' | 'high'): void {
    this.quality = q;
    this.renderer.setPixelRatio(q === 'high' ? Math.min(devicePixelRatio, 2) : 1);
    this.renderer.setSize(innerWidth, innerHeight, false);
    this.map.setQuality(q);
  }

  /**
   * Tremblement de caméra (impacts d'ultimes, explosions de structures) : `intensity` en unités de jeu
   * (≈ 15 léger, 40 fort, max 80), `duration` en secondes.
   */
  shake(intensity: number, duration: number): void {
    this.cam.shake(intensity, duration);
  }

  private static readonly VIEW_NDC: readonly [number, number][] = [[-1, -1], [1, -1], [1, 1], [-1, 1]];
  /** Emprise de la vue au sol (plan y = 0), écrite en place dans `out` (aucune allocation). */
  private updateCameraView(out: ClientStateCameraView): void {
    let minX = Infinity;
    let minY = Infinity;
    let maxX = -Infinity;
    let maxY = -Infinity;
    this.groundPlane.constant = 0;
    for (let i = 0; i < 4; i++) {
      const c = GameRenderer.VIEW_NDC[i]!;
      this.ndc.set(c[0], c[1]);
      this.raycaster.setFromCamera(this.ndc, this.camera);
      const q = out.quad[i]!;
      const hit = this.raycaster.ray.intersectPlane(this.groundPlane, this.tmpV);
      if (hit) toGameInto(q, hit);
      minX = Math.min(minX, q.x);
      minY = Math.min(minY, q.y);
      maxX = Math.max(maxX, q.x);
      maxY = Math.max(maxY, q.y);
    }
    out.minX = minX;
    out.minY = minY;
    out.maxX = maxX;
    out.maxY = maxY;
    out.w = maxX - minX;
    out.h = maxY - minY;
  }

  /** claude-fx (PostFx) : remplace le rendu final (bloom, FXAA…). `null` = rendu direct. */
  setRenderPipeline(render: (() => void) | null): void {
    this.renderFn = render ?? this.defaultRender;
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
    // cam.update() a déplacé la caméra : ses matrices monde doivent être à jour avant de lancer les rayons
    this.camera.updateMatrixWorld();
    this.updateCameraView(s.cameraView);

    if (selfPos) {
      const st = s.snapshot?.self;
      const aaRange = (st?.stats.attackRange ?? 0) + 65;
      computeAim(this.aim, s.targeting, s.selfEntity?.subtype, st?.summoners, selfPos, s.cursorGame, s.hover, aaRange);
      if (s.hover === selfId && !this.aim.active) this.aim.attackRange = aaRange;
    } else {
      this.aim.active = false;
      this.aim.attackRange = 0;
    }
    this.indicators.update(this.aim, dt);
    this.fog.setEnabled(!!s.game);
    this.fog.update(dt, snapshots.entities.values(), myTeam, this.fogPos);
    this.map.update(dt, this.camera, selfPos ?? null);
    this.markers.update(dt);
    for (const cb of this.frameCbs) cb(dt);
    this.scene.updateMatrixWorld();
    this.batcher.sync();
    this.renderFn();
    this.stats?.frame(dt, this.renderer, this.slots.size);
  }

  private resize(): void {
    this.renderer.setSize(innerWidth, innerHeight, false);
    this.cam.resize(innerWidth / innerHeight);
    this.bars.setViewport(innerWidth, innerHeight);
    this.bars.updateLabelScale(innerHeight, this.camera.fov);
  }
}
