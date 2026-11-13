/**
 * Poses et animateur procédural des champions (logique pure, sans Three, testée).
 *
 * Conventions (contrat claude-client : le modèle regarde vers +x local) :
 *   avant = +x, haut = +y, droite = +z.
 *   Un membre pendant tourné de +θ autour de z part VERS L'AVANT ; le torse tourné de +θ autour de z part EN ARRIÈRE.
 *   Écarter un membre vers l'extérieur : rotation x de −θ côté droit (+z), +θ côté gauche.
 */

export const J = {
  body: 0, // racine du corps (inclinaison globale)
  hips: 1,
  spine: 2,
  head: 3,
  armL: 4,
  foreL: 5,
  armR: 6,
  foreR: 7,
  legL: 8,
  shinL: 9,
  legR: 10,
  shinR: 11,
  prop: 12, // arme / accessoire tenu
} as const;
export type JointName = keyof typeof J;
export const JOINT_COUNT = 13;
/** Indice du décalage de position du corps (x, y, z) dans une pose. */
export const BODY_OFFSET = JOINT_COUNT * 3;
export const POSE_SIZE = BODY_OFFSET + 3;

export type Pose = Float32Array;
export const createPose = (): Pose => new Float32Array(POSE_SIZE);

export function setRot(p: Pose, j: number, x: number, y: number, z: number): void {
  p[j * 3] = x;
  p[j * 3 + 1] = y;
  p[j * 3 + 2] = z;
}
export function addRot(p: Pose, j: number, x: number, y: number, z: number): void {
  p[j * 3]! += x;
  p[j * 3 + 1]! += y;
  p[j * 3 + 2]! += z;
}
export function setOffset(p: Pose, x: number, y: number, z: number): void {
  p[BODY_OFFSET] = x;
  p[BODY_OFFSET + 1] = y;
  p[BODY_OFFSET + 2] = z;
}

/** out = a + (b − a)·k */
export function blendPose(a: Pose, b: Pose, k: number, out: Pose): void {
  for (let i = 0; i < POSE_SIZE; i++) out[i] = a[i]! + (b[i]! - a[i]!) * k;
}

/** Contexte passé aux clips (paramètres propres au champion). */
export interface ClipCtx {
  /** Bras qui frappe : 1 = droit, −1 = gauche. */
  hand: 1 | -1;
  /** Amplitude générale (tanks lourds < 1, assassins > 1). */
  energy: number;
}

export interface Clip {
  /** Boucle (idle, run…) ou one-shot (strike, cast…). */
  loop: boolean;
  /** Durée (s) pour un one-shot ; période indicative pour une boucle. */
  duration: number;
  /** Écrit la pose à l'instant `t` (s depuis le début du clip). La pose arrive remise à zéro. */
  sample(t: number, out: Pose, ctx: ClipCtx): void;
}

export type ClipSet = Record<string, Clip>;

/** Chaîne de repli : cast_Q → cast → idle ; strike_crit → strike → idle… */
export function resolveClip(clips: ClipSet, name: string): string {
  if (clips[name]) return name;
  const i = name.indexOf('_');
  if (i > 0 && clips[name.slice(0, i)]) return name.slice(0, i);
  return 'idle';
}

/**
 * Animateur : un clip courant + fondu depuis la pose précédente.
 * Les one-shots reviennent automatiquement au clip de base (idle/run/attackReady…) choisi par `setBase`.
 */
export class Animator {
  readonly pose = createPose();
  /** Multiplicateur de vitesse des boucles (cadence de course selon la vitesse réelle). */
  rate = 1;
  private readonly from = createPose();
  private readonly target = createPose();
  private name = 'idle';
  private base = 'idle';
  private time = 0;
  private blend = 1;
  private blendDur = 0.15;
  private oneShotSpeed = 1;

  constructor(
    private readonly clips: ClipSet,
    private readonly ctx: ClipCtx,
  ) {}

  get current(): string {
    return this.name;
  }
  get baseClip(): string {
    return this.base;
  }

  /** Clip de fond (boucle) : appliqué tout de suite si aucun one-shot n'est en cours. */
  setBase(name: string): void {
    const n = resolveClip(this.clips, name);
    if (n === this.base) return;
    this.base = n;
    if (this.clips[this.name]?.loop !== false) this.start(n, 0.18, 1);
  }

  /** Joue un clip ; un one-shot redémarre toujours, une boucle déjà active continue. */
  play(name: string, opts: { fade?: number; speed?: number } = {}): void {
    const n = resolveClip(this.clips, name);
    const clip = this.clips[n]!;
    if (clip.loop && n === this.name) return;
    this.start(n, opts.fade ?? 0.1, opts.speed ?? 1);
  }

  private start(name: string, fade: number, speed: number): void {
    this.from.set(this.pose);
    this.name = name;
    this.time = 0;
    this.blend = fade > 0 ? 0 : 1;
    this.blendDur = fade;
    this.oneShotSpeed = speed;
  }

  update(dt: number): Pose {
    const clip = this.clips[this.name]!;
    this.time += dt * (clip.loop ? this.rate : this.oneShotSpeed);
    if (!clip.loop && this.time >= clip.duration && this.name !== this.base) {
      // fin du one-shot : on ne revient pas au fond si c'est un état final (mort)
      if (this.name !== 'death') this.start(this.base, 0.2, 1);
    }
    const c = this.clips[this.name]!;
    this.target.fill(0);
    c.sample(c.loop ? this.time : Math.min(this.time, c.duration), this.target, this.ctx);
    if (this.blend < 1) {
      this.blend = Math.min(1, this.blend + dt / Math.max(1e-3, this.blendDur));
      const k = this.blend * this.blend * (3 - 2 * this.blend); // smoothstep
      blendPose(this.from, this.target, k, this.pose);
    } else this.pose.set(this.target);
    return this.pose;
  }
}
