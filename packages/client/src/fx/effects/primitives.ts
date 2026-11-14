/**
 * Primitives d'effets composables : télégraphie, onde de choc, éclat lumineux, zone persistante,
 * suivi d'entité, projectile, traînée de dash. Les FX des sorts sont des assemblages de ces briques.
 * Toutes les positions sont en coordonnées monde (utiliser `rt.world(vec2)` depuis les événements).
 * Les primitives copient les positions qu'elles conservent : on peut leur passer des vecteurs temporaires.
 */
import * as THREE from 'three';
import { ease } from '../core/curves.ts';
import type { Effect, FxRuntime } from '../core/FxRuntime.ts';
import type { ParticleStyle } from '../particles/ParticleSystem.ts';
import type { GroundDecal } from '../shaders/decal.ts';
import type { Ribbon } from '../shaders/ribbon.ts';

const _a = new THREE.Vector3();

/** Effet générique piloté par une fonction de progression t ∈ 0..1. */
export function timed(rt: FxRuntime, duration: number, step: (t: number, dt: number) => void, done?: () => void): void {
  let elapsed = 0;
  rt.add({
    update(dt) {
      elapsed += dt;
      const t = Math.min(1, elapsed / Math.max(1e-3, duration));
      step(t, dt);
      return t < 1;
    },
    end: done,
  });
}

export interface TelegraphOptions {
  shape?: 'circle' | 'rect' | 'cone';
  /** Pour rect/cone : point visé et largeur/demi-angle. */
  to?: THREE.Vector3;
  width?: number;
  halfAngle?: number;
  fill?: number;
  /** Couleur du remplissage progressif (défaut = couleur). */
  color2?: THREE.Color;
  onDone?: () => void;
}

/** Télégraphie au sol qui se remplit pendant `duration`, puis disparaît rapidement. */
export function telegraph(rt: FxRuntime, center: THREE.Vector3, radius: number, duration: number, color: THREE.Color, o: TelegraphOptions = {}): GroundDecal {
  const d = rt.acquireDecal().setColor(rt.groundColor(color), rt.groundColor(o.color2 ?? color));
  if (o.shape === 'rect' && o.to) d.rect(center, o.to, o.width ?? radius);
  else if (o.shape === 'cone' && o.to) d.cone(center, o.to, radius, o.halfAngle ?? 0.5);
  else d.circle(center, radius);
  d.u.uFill.value = o.fill ?? 0.12;
  d.u.uProgress.value = 0;
  d.u.uEdge.value = 0.06;
  d.u.uOpacity.value = 0;
  timed(
    rt,
    duration,
    (t) => {
      d.u.uProgress.value = t;
      d.u.uOpacity.value = Math.min(1, t * 6);
    },
    () => {
      o.onDone?.();
      d.u.uProgress.value = -1;
      timed(rt, 0.15, (t) => (d.u.uOpacity.value = 1 - t), () => rt.releaseDecal(d));
    },
  );
  return d;
}

/** Anneau au sol qui s'étend de 0 à `radius` en s'estompant. */
export function shockwave(rt: FxRuntime, at: THREE.Vector3, radius: number, duration: number, color: THREE.Color, edge = 0.12): void {
  const center = at.clone(); // conservé entre les frames : on copie (l'appelant peut passer un vecteur temporaire)
  const d = rt.acquireDecal().setColor(color);
  d.u.uEdge.value = edge;
  timed(
    rt,
    duration,
    (t) => {
      d.circle(center, Math.max(0.01, radius * ease.outCubic(t)), 'ring');
      d.u.uOpacity.value = 1 - t * t;
    },
    () => rt.releaseDecal(d),
  );
}

/** Éclat lumineux (sprite additif) qui grossit puis s'éteint. */
export function flash(rt: FxRuntime, pos: THREE.Vector3, size: number, color: THREE.Color, duration = 0.25): void {
  const sp = rt.acquireSprite(color, size * 0.3);
  sp.position.copy(pos);
  timed(
    rt,
    duration,
    (t) => {
      const s = size * (0.3 + 0.7 * ease.outCubic(t));
      sp.scale.set(s, s, 1);
      sp.material.opacity = 1 - t;
    },
    () => rt.releaseSprite(sp),
  );
}

export interface ZoneOptions {
  fill?: number;
  pattern?: number;
  /** Émission continue de particules dans la zone. */
  emit?: { style: ParticleStyle; rate: number; size: [number, number]; life: [number, number]; up?: [number, number]; tint?: THREE.Color; intensity?: number };
  normalBlend?: boolean;
}

/** Zone persistante (blizzard, nuée, aura) : fondu d'entrée/sortie + motif animé + particules. */
export function zone(rt: FxRuntime, at: THREE.Vector3, radius: number, duration: number, color: THREE.Color, o: ZoneOptions = {}): void {
  const center = at.clone();
  const d = rt.acquireDecal().setColor(rt.groundColor(color)).circle(center, radius);
  d.u.uFill.value = o.fill ?? 0.18;
  d.u.uPattern.value = o.pattern ?? 0.6;
  d.u.uEdge.value = 0.1;
  if (o.normalBlend) d.mesh.material.blending = THREE.NormalBlending;
  let acc = 0;
  timed(
    rt,
    duration,
    (t, dt) => {
      const fadeIn = Math.min(1, t * duration * 5);
      const fadeOut = Math.min(1, (1 - t) * duration * 4);
      d.u.uOpacity.value = Math.min(fadeIn, fadeOut);
      if (o.emit) {
        acc += o.emit.rate * dt;
        const n = Math.floor(acc);
        if (n > 0) {
          acc -= n;
          rt.ps.emit(o.emit.style, center, {
            count: n,
            shape: 'disc',
            radius: radius * 0.95,
            life: o.emit.life,
            size: o.emit.size,
            up: o.emit.up,
            tint: o.emit.tint ?? color,
            intensity: o.emit.intensity,
          });
        }
      }
    },
    () => rt.releaseDecal(d),
  );
}

/** Suit une entité pendant `duration` : `step(pos, t, dt)` reçoit sa position monde au sol. */
export function follow(rt: FxRuntime, entityId: number, duration: number, step: (pos: THREE.Vector3, t: number, dt: number) => void, done?: () => void): void {
  const pos = new THREE.Vector3();
  timed(
    rt,
    duration,
    (t, dt) => {
      if (rt.entities.pos(entityId, pos)) step(pos, t, dt);
    },
    done,
  );
}

// ---------------------------------------------------------------- Projectile
export interface ProjectileLook {
  color: THREE.Color;
  core?: THREE.Color;
  /** Taille du halo de tête (monde). */
  size?: number;
  height?: number;
  /** Particules de traînée par unité monde parcourue. */
  trailDensity?: number;
  trailStyle?: ParticleStyle;
  trailSize?: [number, number];
  trailLife?: [number, number];
  ribbonWidth?: number;
  ribbonLife?: number;
  /** Appelé à chaque frame avec la position de tête (FX spécifiques : rotation d'un mesh, étincelles…). */
  onStep?: (head: THREE.Vector3, dir: THREE.Vector3, dt: number) => void;
  onEnd?: (head: THREE.Vector3) => void;
  /** Tête chercheuse : suit la position courante de cette entité (auto-attaques, sorts ciblés). */
  targetId?: number;
}

/** Projectile visuel : se déplace de `from` à `to` à `speed` (monde/s), ou jusqu'à `finish()`. */
export class ProjectileFx implements Effect {
  readonly head = new THREE.Vector3();
  readonly dir = new THREE.Vector3();
  private readonly from = new THREE.Vector3();
  private dist = 0;
  private traveled = 0;
  private flying = true;
  private acc = 0;
  private readonly sprite: THREE.Sprite;
  private readonly ribbon: Ribbon | null;
  private readonly prev = new THREE.Vector3();
  private readonly aim = new THREE.Vector3();

  constructor(
    private readonly rt: FxRuntime,
    from: THREE.Vector3,
    to: THREE.Vector3,
    private readonly speed: number,
    private readonly look: ProjectileLook,
  ) {
    const h = look.height ?? 1;
    this.from.set(from.x, h, from.z);
    this.dir.set(to.x - from.x, 0, to.z - from.z);
    this.dist = this.dir.length();
    if (this.dist > 1e-4) this.dir.divideScalar(this.dist);
    else this.dir.set(1, 0, 0);
    this.head.copy(this.from);
    this.prev.copy(this.from);
    this.sprite = rt.acquireSprite(look.core ?? look.color, look.size ?? 0.6);
    this.sprite.position.copy(this.head);
    if (look.size === 0) this.sprite.visible = false;
    if ((look.ribbonWidth ?? 0.25) > 0) {
      this.ribbon = rt.acquireRibbon();
      this.ribbon.width = look.ribbonWidth ?? 0.25;
      this.ribbon.life = look.ribbonLife ?? 0.25;
      this.ribbon.setColor(look.color, look.core);
      this.ribbon.push(this.head);
    } else this.ribbon = null;
  }

  /** Termine le vol (impact serveur reçu) en plaçant la tête sur `at` si fourni. */
  finish(at?: THREE.Vector3): void {
    if (!this.flying) return;
    if (at) this.head.set(at.x, this.head.y, at.z);
    this.land();
  }

  private land(): void {
    this.flying = false;
    this.sprite.visible = false;
    this.ribbon?.push(this.head);
    this.look.onEnd?.(this.head);
  }

  update(dt: number): boolean {
    if (this.flying) {
      this.prev.copy(this.head);
      if (this.look.targetId !== undefined && this.rt.entities.pos(this.look.targetId, this.aim, this.head.y)) {
        // tête chercheuse : avance vers la position actuelle de la cible
        this.aim.sub(this.head);
        const d = this.aim.length();
        const step = this.speed * dt;
        if (d > 1e-4) this.dir.copy(this.aim).divideScalar(d);
        this.head.addScaledVector(this.dir, Math.min(step, d));
        // arrivé quand le pas couvre la distance restante ; sinon on reste « en vol » même si la cible s'éloigne
        this.traveled = step >= d ? this.dist : Math.min(this.dist - 1e-3, this.traveled + step);
      } else {
        this.traveled = Math.min(this.dist, this.traveled + this.speed * dt);
        this.head.copy(this.from).addScaledVector(this.dir, this.traveled);
      }
      this.sprite.position.copy(this.head);
      this.ribbon?.push(this.head);
      this.look.onStep?.(this.head, this.dir, dt);
      const style = this.look.trailStyle;
      if (style) {
        this.acc += this.prev.distanceTo(this.head) * (this.look.trailDensity ?? 6);
        const n = Math.floor(this.acc);
        if (n > 0) {
          this.acc -= n;
          _a.copy(this.dir).multiplyScalar(-this.speed * 0.08);
          this.rt.ps.emit(style, this.prev, {
            count: n,
            shape: 'line',
            to: this.head,
            radius: 0.1,
            speed: [0.1, 0.5],
            inherit: _a,
            life: this.look.trailLife ?? [0.25, 0.45],
            size: this.look.trailSize ?? [0.15, 0.3],
            tint: this.look.color,
          });
        }
      }
      if (this.traveled >= this.dist) this.land();
      return true;
    }
    return (this.ribbon?.alive ?? 0) > 0;
  }

  end(): void {
    this.rt.releaseSprite(this.sprite);
    if (this.ribbon) this.rt.releaseRibbon(this.ribbon);
  }
}

/** Traînée au sol + poussière pendant un dash (suit l'entité si connue, sinon interpole from → to). */
export function dashTrail(rt: FxRuntime, entityId: number, fromAt: THREE.Vector3, toAt: THREE.Vector3, duration: number, color: THREE.Color, dust = true): void {
  const from = fromAt.clone();
  const to = toAt.clone();
  const r = rt.acquireRibbon();
  r.mode = 'flat';
  r.width = 0.7;
  r.life = 0.4;
  r.setColor(color);
  const p = new THREE.Vector3();
  const prev = from.clone();
  let flying = true;
  let elapsedDash = 0;
  rt.add({
    update(dt) {
      if (flying) {
        elapsedDash += dt;
        const t = Math.min(1, elapsedDash / Math.max(0.05, duration));
        if (!rt.entities.pos(entityId, p)) p.lerpVectors(from, to, t);
        p.y = 0.25;
        r.push(p);
        if (dust && prev.distanceToSquared(p) > 0.04) {
          rt.ps.emit(rt.s.dust, p, { count: 2, shape: 'disc', radius: 0.25, life: [0.4, 0.7], size: [0.4, 0.8], up: [0.2, 0.6] });
          prev.copy(p);
        }
        if (t >= 1) flying = false;
        return true;
      }
      return r.alive > 0;
    },
    end: () => rt.releaseRibbon(r),
  });
}

/** Éclair en zigzag (ruban) de `from` à `to`, qui s'efface en `life` s. */
export function lightning(rt: FxRuntime, from: THREE.Vector3, to: THREE.Vector3, color: THREE.Color, opts: { width?: number; life?: number; jag?: number; segments?: number } = {}): void {
  const r = rt.acquireRibbon();
  r.width = opts.width ?? 0.18;
  r.life = opts.life ?? 0.25;
  r.minDist = 0.001;
  r.setColor(color, new THREE.Color(3, 3, 3));
  const n = opts.segments ?? 10;
  const jag = opts.jag ?? 0.25;
  const p = new THREE.Vector3();
  for (let i = 0; i <= n; i++) {
    const k = i / n;
    p.lerpVectors(from, to, k);
    if (i > 0 && i < n) p.set(p.x + (Math.random() - 0.5) * jag * 2, p.y + (Math.random() - 0.5) * jag, p.z + (Math.random() - 0.5) * jag * 2);
    r.push(p);
  }
  rt.add({
    update: () => r.alive > 0,
    end: () => rt.releaseRibbon(r),
  });
}
