/**
 * Prédiction locale du déplacement du joueur. Propriétaire : claude-client.
 *
 * Le champion local est affiché à une position prédite : dès un clic de déplacement, il avance en ligne
 * droite vers le point à sa vitesse, sans attendre le serveur. À chaque frame, la position prédite est
 * rappelée en douceur vers l'estimation de la position serveur « actuelle » (dernière position reçue
 * extrapolée de sa vitesse). Gros écart = recalage immédiat.
 *
 * Pas de prédiction quand la ligne droite traverse un mur (le serveur contourne, on le suit), pendant un
 * dash, mort ou si le serveur reste immobile alors qu'on prédit un mouvement (CC, attaque…).
 */
import { isInWall, type Vec2 } from '@lov/shared';

/** La ligne droite a→b traverse-t-elle un mur ? (échantillonnage tous les 50 u) */
export function segmentBlocked(a: Vec2, b: Vec2): boolean {
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

export const SNAP_DIST = 250;
/** Taux de correction (1/s) pendant un déplacement prédit et à l'arrêt. */
const CORRECT_MOVING = 2.5;
const CORRECT_IDLE = 8;
/** Vitesse serveur sous laquelle on considère l'entité immobile (u/s). */
const STILL_SPEED = 20;

export interface ServerView {
  x: number;
  y: number;
  vx: number;
  vy: number;
  /** Avance à appliquer à la position serveur pour estimer sa position actuelle (s) : âge + latence/2. */
  lead: number;
}

export class MovePredictor {
  readonly pos: Vec2 = { x: 0, y: 0 };
  private initialized = false;
  private target: Vec2 | null = null;
  private suspendedFor = 0;
  private serverStillFor = 0;
  /** Délai de grâce après un ordre avant de se fier à l'immobilité serveur (s). */
  private grace = 0;

  constructor(private readonly blocked: (a: Vec2, b: Vec2) => boolean = () => false) {}

  get moving(): boolean {
    return this.target !== null;
  }

  /** Ordre de déplacement (move / attackMove vers le sol). `latency` = RTT estimé (s). */
  command(target: Vec2, latency = 0.1): void {
    if (this.initialized && this.blocked(this.pos, target)) {
      this.target = null;
      return;
    }
    this.target = { x: target.x, y: target.y };
    this.grace = latency + 0.2;
    this.serverStillFor = 0;
  }

  /** Ordre non prédit (attaque, stop, sort) : on suit le serveur. */
  cancel(): void {
    this.target = null;
  }

  /** Suspend la prédiction (dash, temps d'incantation…) pendant `seconds`. */
  suspend(seconds: number): void {
    this.suspendedFor = Math.max(this.suspendedFor, seconds);
    this.target = null;
  }

  reset(x: number, y: number): void {
    this.pos.x = x;
    this.pos.y = y;
    this.target = null;
    this.initialized = true;
  }

  update(dt: number, server: ServerView, speed: number, canMove: boolean): Vec2 {
    if (!this.initialized) this.reset(server.x, server.y);
    const lead = Math.min(server.lead, 0.25);
    const ex = server.x + server.vx * lead;
    const ey = server.y + server.vy * lead;

    if (this.suspendedFor > 0) this.suspendedFor -= dt;
    if (!canMove) this.target = null;

    const serverSpeed = Math.hypot(server.vx, server.vy);
    if (this.grace > 0) this.grace -= dt;
    else if (serverSpeed < STILL_SPEED) this.serverStillFor += dt;
    else this.serverStillFor = 0;
    if (this.target && this.grace <= 0 && this.serverStillFor > 0.15) this.target = null;

    if (this.target && this.suspendedFor <= 0) {
      const dx = this.target.x - this.pos.x;
      const dy = this.target.y - this.pos.y;
      const d = Math.hypot(dx, dy);
      const step = speed * dt;
      if (d <= step) {
        this.pos.x = this.target.x;
        this.pos.y = this.target.y;
        this.target = null;
      } else {
        this.pos.x += (dx / d) * step;
        this.pos.y += (dy / d) * step;
      }
    }

    const errX = ex - this.pos.x;
    const errY = ey - this.pos.y;
    const err = Math.hypot(errX, errY);
    if (err > SNAP_DIST || this.suspendedFor > 0) {
      this.pos.x = ex;
      this.pos.y = ey;
    } else {
      const k = Math.min(1, dt * (this.target ? CORRECT_MOVING : CORRECT_IDLE));
      this.pos.x += errX * k;
      this.pos.y += errY * k;
    }
    return this.pos;
  }
}
