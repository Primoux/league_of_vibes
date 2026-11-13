/**
 * Vue 3D d'un champion (contrat `ChampionView` de claude-client) : modèle procédural, animations, mort en dissolve.
 *
 * Pilotage des animations :
 *  - `playAnim(snap.anim)` (serveur) : idle / run / attack / dash / death / cast_* ;
 *  - événements du fxBus routés par `index.ts` : autoAttack → frappe synchronisée sur l'impact serveur,
 *    abilityCast → cast_<slot>, recallStart/End ;
 *  - vitesse mesurée à l'écran : cadence de course, et course immédiate pour le joueur local (prédiction).
 */
import * as THREE from 'three';
import { CHAMPIONS, type ChampionDef, type EntitySnapshot } from '@lov/shared';
import { applyDissolve, createDissolveUniforms, type DissolveUniforms } from '../../fx/shaders/materials.ts';
import type { ChampionView } from '../EntityView.ts';
import { GENERIC_CLIPS, DEFAULT_CTX } from './anim/clips.ts';
import { Animator, type ClipCtx, type ClipSet } from './anim/pose.ts';
import { ModelBuilder, type BuiltModel } from './builder.ts';
import { Rig, type RigSpec } from './rig.ts';

export const TEAM_HEX = { blue: '#3a7bff', red: '#ff4a4a', neutral: '#c9a227' } as const;

export interface ModelColors {
  primary: string;
  secondary: string;
  accent: string;
  team: string;
  /** Accent surexposé pour les parties lumineuses (bloom). */
  glow: THREE.Color;
  def: ChampionDef;
}

/** Ce que les « extras » d'un modèle (cristaux en orbite, pulsations…) peuvent lire. */
export interface ExtrasState {
  readonly time: number;
  readonly dead: boolean;
  readonly moving: boolean;
  readonly anim: string;
}

export interface ChampionModelDef {
  rig: RigSpec;
  ctx?: Partial<ClipCtx>;
  clips?: Partial<ClipSet>;
  /** Échelle globale du modèle. */
  scale?: number;
  build(b: ModelBuilder, c: ModelColors): void;
  extras?(rig: Rig, built: BuiltModel, c: ModelColors): (dt: number, s: ExtrasState) => void;
}

/** Accent surexposé, d'autant moins que l'accent est déjà clair (un blanc × 2 sature le bloom). */
export function glowColor(hex: string): THREE.Color {
  const c = new THREE.Color(hex);
  const lum = 0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b;
  return c.multiplyScalar(1.3 + (1 - Math.min(1, lum)) * 1.4);
}

const STUN_BUFFS = new Set(['stun', 'knockup', 'suppress', 'taunt', 'fear']);
const NOMINAL_SPEED = 3.4; // monde/s (≈ 340 u/s)

export class ProceduralChampionView implements ChampionView {
  readonly object3d = new THREE.Group();
  readonly rig: Rig;
  readonly animator: Animator;
  readonly colors: ModelColors;
  private readonly model = new THREE.Group();
  private readonly built: BuiltModel;
  private readonly dissolve: DissolveUniforms;
  private readonly extras: ((dt: number, s: ExtrasState) => void) | null;
  private readonly state = { time: 0, dead: false, moving: false, anim: 'idle' };
  private serverAnim = 'idle';
  private recalling = false;
  private stunned = false;
  private deathT = -1;
  private readonly lastPos = new THREE.Vector3();
  private hasLastPos = false;
  private speed = 0;

  constructor(
    readonly entityId: number,
    championId: string,
    team: keyof typeof TEAM_HEX,
    def: ChampionModelDef,
  ) {
    const champ = CHAMPIONS[championId]!;
    const p = champ.palette;
    this.colors = {
      primary: p.primary,
      secondary: p.secondary,
      accent: p.accent,
      team: TEAM_HEX[team],
      glow: glowColor(p.accent),
      def: champ,
    };
    this.rig = new Rig(def.rig);
    const b = new ModelBuilder();
    def.build(b, this.colors);
    this.built = b.build(this.rig);
    this.dissolve = createDissolveUniforms(new THREE.Color(p.accent).multiplyScalar(3));
    applyDissolve(this.rig.root, this.dissolve);
    this.model.add(this.rig.root);
    this.model.scale.setScalar(def.scale ?? 1);
    this.object3d.add(this.model);
    this.object3d.name = `champion-${championId}-${entityId}`;
    const ctx: ClipCtx = { ...DEFAULT_CTX, ...def.ctx };
    this.animator = new Animator({ ...GENERIC_CLIPS, ...(def.clips as ClipSet) }, ctx);
    this.extras = def.extras?.(this.rig, this.built, this.colors) ?? null;
  }

  /** Hauteur du modèle (monde), pour placer des FX au-dessus de la tête. */
  get height(): number {
    return this.rig.height * this.model.scale.y;
  }

  // ------------------------------------------------------------ contrat ChampionView
  playAnim(name: string): void {
    if (name === 'death') return this.die();
    if (this.state.dead) this.respawn();
    if (name.startsWith('cast_')) {
      this.animator.play(name);
      return;
    }
    this.serverAnim = name;
    if (name === 'run' || name === 'dash') this.recalling = false;
    this.refreshBase();
  }

  update(snap: EntitySnapshot, dt: number): void {
    this.state.time += dt;
    if (snap.dead && !this.state.dead) this.die();
    else if (!snap.dead && this.state.dead) this.respawn();

    // vitesse affichée (la position est posée par le renderer)
    const pos = this.object3d.position;
    if (this.hasLastPos && dt > 0) {
      const v = Math.hypot(pos.x - this.lastPos.x, pos.z - this.lastPos.z) / dt;
      this.speed += (v - this.speed) * Math.min(1, dt * 12);
    }
    this.lastPos.copy(pos);
    this.hasLastPos = true;
    const moving = this.state.moving ? this.speed > 0.35 : this.speed > 0.8;
    let stunned = false;
    if (snap.buffs) for (const b of snap.buffs) if (STUN_BUFFS.has(b.id)) stunned = true;
    if (moving !== this.state.moving || stunned !== this.stunned) {
      this.state.moving = moving;
      this.stunned = stunned;
      this.refreshBase();
    }
    this.animator.rate = this.animator.baseClip === 'run' ? THREE.MathUtils.clamp(this.speed / NOMINAL_SPEED, 0.6, 1.7) : 1;

    this.tickDeath(dt);
    this.rig.apply(this.animator.update(dt));
    this.state.anim = this.animator.current;
    this.extras?.(dt, this.state);
  }

  dispose(): void {
    this.object3d.traverse((o) => {
      if (o instanceof THREE.Mesh) {
        o.geometry.dispose();
        if (!o.userData.noDissolve) (Array.isArray(o.material) ? o.material : [o.material]).forEach((m: THREE.Material) => m.dispose());
      }
    });
    this.object3d.removeFromParent();
  }

  // ------------------------------------------------------------ événements
  strike(): void {
    if (!this.state.dead) this.animator.play('strike', { fade: 0.04 });
  }
  cast(slot: string): void {
    if (!this.state.dead) this.animator.play(`cast_${slot}`, { fade: 0.08 });
  }
  setRecalling(on: boolean): void {
    this.recalling = on;
    this.refreshBase();
  }

  // ------------------------------------------------------------ interne
  private refreshBase(): void {
    if (this.state.dead) return;
    let base: string;
    if (this.stunned) base = 'stunned';
    else if (this.recalling) base = 'recall';
    else if (this.serverAnim === 'dash') base = 'dash';
    else if (this.state.moving) base = 'run';
    else if (this.serverAnim === 'attack') base = 'attackReady';
    else base = this.serverAnim === 'run' ? 'run' : 'idle';
    this.animator.setBase(base);
  }

  private die(): void {
    if (this.state.dead) return;
    this.state.dead = true;
    this.recalling = false;
    this.deathT = 0;
    this.animator.play('death', { fade: 0.08 });
  }

  private respawn(): void {
    this.state.dead = false;
    this.deathT = -1;
    this.dissolve.uDissolve.value = 0;
    this.model.visible = true;
    for (const o of this.built.outlines) o.visible = true;
    this.animator.setBase('idle');
    this.animator.play('idle', { fade: 0 });
  }

  /** Chute (0,8 s) puis dissolution (1,2 s), puis invisible jusqu'à la réapparition. */
  private tickDeath(dt: number): void {
    if (this.deathT < 0) return;
    this.deathT += dt;
    const k = (this.deathT - 0.8) / 1.2;
    if (k > 0) {
      for (const o of this.built.outlines) o.visible = false;
      this.dissolve.uDissolve.value = Math.min(1.05, k * 1.05);
    }
    if (k >= 1) this.model.visible = false;
  }
}
