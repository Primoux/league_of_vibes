/**
 * Runtime FX : possède la scène FX, les particules, les pools (décals, rubans, sprites), les effets actifs
 * et le minuteur. Les handlers de sorts (`registerFx`) reçoivent ce runtime et composent ses primitives.
 */
import * as THREE from 'three';
import { RENDER_SCALE, type Vec2 } from '@lov/shared';
import { ParticleSystem } from '../particles/ParticleSystem.ts';
import { createStdStyles, type StdStyles } from '../particles/styles.ts';
import { atlasCell } from '../particles/textures.ts';
import { GroundDecal } from '../shaders/decal.ts';
import { Ribbon } from '../shaders/ribbon.ts';
import { createIceMaterial } from '../shaders/materials.ts';
import { MeshBurstSystem, rockGeometry, shardGeometry, spikeGeometry } from '../effects/meshBursts.ts';
import { EntityTracker } from './EntityTracker.ts';
import type { FxHost } from './host.ts';
import { paletteForKey } from './palette.ts';
import { Pool } from './pool.ts';
import { Scheduler } from './scheduler.ts';

/** Effet actif : `update` renvoie false quand il est terminé (le runtime appelle alors `end`). */
export interface Effect {
  update(dt: number): boolean;
  end?(): void;
}

/** Couleurs prêtes à l'emploi (linéaires) d'une palette ; `*Hdr` = surexposées pour le bloom. */
export interface FxColors {
  primary: THREE.Color;
  secondary: THREE.Color;
  accent: THREE.Color;
  primaryHdr: THREE.Color;
  accentHdr: THREE.Color;
  /**
   * Couleur « signature » lisible : la plus saturée/lumineuse entre primaire et accent, éclaircie si besoin.
   * Indispensable pour les décals additifs : une palette sombre (Nyx, Grondak) serait invisible sur l'herbe.
   */
  vivid: THREE.Color;
  vividHdr: THREE.Color;
}

const _hsl = { h: 0, s: 0, l: 0 };
/** Score de « lisibilité » d'une couleur : saturée et de luminosité moyenne-haute. */
const vividScore = (c: THREE.Color): number => {
  c.getHSL(_hsl);
  return _hsl.s * (1 - Math.abs(_hsl.l - 0.6));
};

export function makeVivid(primary: THREE.Color, accent: THREE.Color): THREE.Color {
  const base = vividScore(accent) > vividScore(primary) ? accent : primary;
  base.getHSL(_hsl);
  return new THREE.Color().setHSL(_hsl.h, Math.max(_hsl.s, 0.35), Math.max(_hsl.l, 0.58));
}

/** Systèmes de meshes instanciés partagés (créés à la première utilisation). */
export interface BurstSystems {
  rockSpikes: MeshBurstSystem;
  rockDebris: MeshBurstSystem;
  iceSpikes: MeshBurstSystem;
  iceDebris: MeshBurstSystem;
}

export class FxRuntime {
  readonly root = new THREE.Group();
  readonly ps: ParticleSystem;
  readonly s: StdStyles;
  readonly entities: EntityTracker;
  readonly scheduler = new Scheduler();
  time = 0;

  private readonly effects: Effect[] = [];
  private readonly colorCache = new Map<string, FxColors>();
  private readonly decals: Pool<GroundDecal>;
  private readonly ribbons: Pool<Ribbon>;
  private readonly sprites: Pool<THREE.Sprite>;
  private readonly activeRibbons = new Set<Ribbon>();
  private readonly activeDecals = new Set<GroundDecal>();
  private burstSystems: BurstSystems | null = null;
  private readonly auras = new Map<string, () => void>();

  constructor(readonly host: FxHost) {
    this.root.name = 'fx-root';
    host.scene.add(this.root);
    this.ps = new ParticleSystem();
    this.s = createStdStyles(this.ps);
    this.root.add(this.ps.group);
    this.entities = new EntityTracker(host);
    this.decals = new Pool(
      () => new GroundDecal(),
      (d) => {
        d.mesh.visible = false;
        this.activeDecals.delete(d);
      },
    );
    this.ribbons = new Pool(
      () => new Ribbon(),
      (r) => {
        r.reset();
        r.mesh.visible = false;
        this.activeRibbons.delete(r);
      },
    );
    const glowTex = atlasCell('glow');
    this.sprites = new Pool(
      () => {
        const sp = new THREE.Sprite(
          // depthTest off : un grand éclat traverserait le sol et serait coupé net à l'horizon
          new THREE.SpriteMaterial({ map: glowTex, blending: THREE.AdditiveBlending, depthWrite: false, depthTest: false, transparent: true }),
        );
        sp.renderOrder = 13;
        return sp;
      },
      (sp) => {
        sp.visible = false;
        sp.material.rotation = 0;
      },
    );
  }

  get camera(): THREE.Camera {
    return this.host.camera;
  }

  // ------------------------------------------------------------ boucle
  update(dt: number): void {
    this.time += dt;
    this.entities.update(dt);
    this.scheduler.update(dt);
    for (let i = this.effects.length - 1; i >= 0; i--) {
      const e = this.effects[i]!;
      if (e.update(dt)) continue;
      const last = this.effects.pop()!;
      if (i < this.effects.length) this.effects[i] = last;
      e.end?.();
    }
    for (const r of this.activeRibbons) r.update(dt, this.host.camera);
    for (const d of this.activeDecals) d.u.uTime.value = this.time;
    if (this.burstSystems) for (const k in this.burstSystems) this.burstSystems[k as keyof BurstSystems].update(dt);
    this.ps.update(dt);
  }

  get bursts(): BurstSystems {
    if (!this.burstSystems) {
      const stone = new THREE.MeshStandardMaterial({ color: 0xffffff, flatShading: true, roughness: 0.9 });
      const ice = createIceMaterial();
      this.burstSystems = {
        rockSpikes: new MeshBurstSystem(spikeGeometry(5), stone, 96),
        rockDebris: new MeshBurstSystem(rockGeometry(), stone, 160),
        iceSpikes: new MeshBurstSystem(shardGeometry(), ice, 96),
        iceDebris: new MeshBurstSystem(shardGeometry(), ice, 160),
      };
      for (const k in this.burstSystems) this.root.add(this.burstSystems[k as keyof BurstSystems].mesh);
    }
    return this.burstSystems;
  }

  /** Enregistre l'annulation d'un effet persistant (bouclier, aura) ; remplace celui de même clé. */
  setAura(key: string, cancel: () => void): void {
    this.auras.get(key)?.();
    this.auras.set(key, cancel);
  }

  /** Termine l'effet persistant `key` (ex. sur buffRemoved). */
  endAura(key: string): void {
    const c = this.auras.get(key);
    if (!c) return;
    this.auras.delete(key);
    c();
  }

  /** À appeler par l'effet lui-même quand il se termine seul. */
  forgetAura(key: string, cancel: () => void): void {
    if (this.auras.get(key) === cancel) this.auras.delete(key);
  }

  add(e: Effect): void {
    this.effects.push(e);
  }

  after(delay: number, fn: () => void): void {
    this.scheduler.after(delay, fn);
  }

  get activeEffects(): number {
    return this.effects.length;
  }

  // ------------------------------------------------------------ utilitaires
  /** Couleurs de la palette associée à une fxKey (mises en cache). */
  colors(key: string): FxColors {
    let c = this.colorCache.get(key);
    if (!c) {
      const pal = paletteForKey(key);
      const primary = new THREE.Color(pal.primary);
      const accent = new THREE.Color(pal.accent);
      const vivid = makeVivid(primary, accent);
      c = {
        primary,
        secondary: new THREE.Color(pal.secondary),
        accent,
        primaryHdr: primary.clone().multiplyScalar(2.6),
        accentHdr: accent.clone().multiplyScalar(2.2),
        vivid,
        vividHdr: vivid.clone().multiplyScalar(2.4),
      };
      this.colorCache.set(key, c);
    }
    return c;
  }

  /**
   * Tremblement de caméra pour un impact en `at` (monde), atténué avec la distance à la caméra :
   * une explosion hors écran ne secoue pas la vue. `intensity` en unités de jeu (15 léger, 40 fort).
   */
  shake(at: THREE.Vector3, intensity: number, duration: number): void {
    if (!this.host.shake) return;
    const cam = this.host.camera.position;
    // point visé ≈ projection de la caméra au sol (vue plongeante)
    const d = Math.hypot(at.x - cam.x, at.z - (cam.z - cam.y * 0.6));
    const k = 1 - d / 30;
    if (k > 0) this.host.shake(intensity * k, duration);
  }

  /** Coordonnées de jeu → monde (dans `out`). */
  world(p: Vec2, h = 0, out = new THREE.Vector3()): THREE.Vector3 {
    return out.set(p.x * RENDER_SCALE, h, -p.y * RENDER_SCALE);
  }

  /** Distance de jeu → monde. */
  readonly scale = (gameUnits: number): number => gameUnits * RENDER_SCALE;

  // ------------------------------------------------------------ pools
  acquireDecal(): GroundDecal {
    const d = this.decals.acquire()!.reset();
    if (!d.mesh.parent) this.root.add(d.mesh);
    this.activeDecals.add(d);
    return d;
  }
  releaseDecal(d: GroundDecal): void {
    this.decals.release(d);
  }

  acquireRibbon(): Ribbon {
    const r = this.ribbons.acquire()!;
    r.reset();
    r.mesh.visible = true;
    r.width = 0.3;
    r.life = 0.35;
    r.minDist = 0.08;
    r.mode = 'billboard';
    if (!r.mesh.parent) this.root.add(r.mesh);
    this.activeRibbons.add(r);
    return r;
  }
  releaseRibbon(r: Ribbon): void {
    this.ribbons.release(r);
  }

  acquireSprite(color: THREE.Color, size: number): THREE.Sprite {
    const sp = this.sprites.acquire()!;
    sp.material.color.copy(color);
    sp.material.opacity = 1;
    sp.scale.set(size, size, 1);
    sp.visible = true;
    if (!sp.parent) this.root.add(sp);
    return sp;
  }
  releaseSprite(sp: THREE.Sprite): void {
    this.sprites.release(sp);
  }
}
