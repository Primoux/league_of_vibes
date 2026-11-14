/**
 * Buffer de snapshots + interpolation des entités. Propriétaire : claude-client.
 *
 * - Fusionne les snapshots (complets ou delta) en une vue complète `entities`.
 *   Règle : si `snap.removed` est défini, le snapshot est un DELTA (les entités absentes sont conservées,
 *   `removed` liste celles qui disparaissent) ; sinon il est COMPLET (toute entité absente est supprimée).
 * - Garde pour chaque entité un historique court de positions horodatées (temps serveur, ms) et permet
 *   d'échantillonner la position/orientation à un instant donné (interpolation, extrapolation bornée).
 * - Les sauts (Flash, respawn, téléportation) ne sont pas interpolés : l'historique est réinitialisé.
 */
import { applyEntityPatch, type EntityId, type EntitySnapshot, type Snapshot } from '@lov/shared';

export interface Sample {
  t: number; // temps serveur (ms)
  x: number;
  y: number;
  facing: number;
}

export interface Interpolated {
  x: number;
  y: number;
  facing: number;
}

const HISTORY = 12;
/** Au-delà de cette distance entre deux échantillons consécutifs, on considère un saut (pas d'interpolation). */
export const TELEPORT_DIST = 350;
/** Extrapolation maximale au-delà du dernier échantillon (ms) en cas de paquet en retard. */
export const MAX_EXTRAPOLATE_MS = 120;

class Track {
  readonly samples: Sample[] = [];
  /** Nombre d'échantillons valides (ring buffer ordonné par insertion). */
  count = 0;
  head = 0; // index du prochain écrit

  push(t: number, x: number, y: number, facing: number): void {
    let s = this.samples[this.head];
    if (!s) {
      s = { t, x, y, facing };
      this.samples[this.head] = s;
    } else {
      s.t = t;
      s.x = x;
      s.y = y;
      s.facing = facing;
    }
    this.head = (this.head + 1) % HISTORY;
    if (this.count < HISTORY) this.count++;
  }

  /** i = 0 : plus ancien, count-1 : plus récent. */
  at(i: number): Sample {
    return this.samples[(this.head - this.count + i + HISTORY * 2) % HISTORY]!;
  }

  latest(): Sample | undefined {
    return this.count ? this.at(this.count - 1) : undefined;
  }

  clear(): void {
    this.count = 0;
  }
}

export function lerpAngle(a: number, b: number, t: number): number {
  let d = (b - a) % (Math.PI * 2);
  if (d > Math.PI) d -= Math.PI * 2;
  else if (d < -Math.PI) d += Math.PI * 2;
  return a + d * t;
}

export class SnapshotBuffer {
  /** Vue complète fusionnée, état le plus récent de chaque entité. */
  readonly entities = new Map<EntityId, EntitySnapshot>();
  private tracks = new Map<EntityId, Track>();
  lastTick = -1;
  latestServerTime = 0;

  /**
   * Intègre un snapshot. Retourne false s'il est obsolète (tick déjà vu) et a été ignoré.
   * `onRemoved` est appelé pour chaque entité qui disparaît.
   */
  push(snap: Snapshot, onRemoved?: (id: EntityId) => void): boolean {
    if (snap.tick <= this.lastTick) return false;
    this.lastTick = snap.tick;
    this.latestServerTime = snap.serverTime;
    const t = snap.serverTime;

    if (snap.removed) {
      for (const id of snap.removed) this.remove(id, onRemoved);
      for (const e of snap.entities) this.upsert(e, t);
      // Mises à jour partielles (protocole v2, claude-server) : seuls les champs modifiés.
      if (snap.patches) {
        for (const p of snap.patches) {
          const prev = this.entities.get(p.id);
          if (prev) this.upsert(applyEntityPatch(prev, p), t);
        }
      }
    } else {
      // snapshot complet : marquer les présents, supprimer les autres
      const present = new Set<EntityId>();
      for (const e of snap.entities) {
        present.add(e.id);
        this.upsert(e, t);
      }
      for (const id of [...this.entities.keys()]) if (!present.has(id)) this.remove(id, onRemoved);
    }
    return true;
  }

  private upsert(e: EntitySnapshot, t: number): void {
    const prev = this.entities.get(e.id);
    this.entities.set(e.id, e);
    let track = this.tracks.get(e.id);
    if (!track) {
      track = new Track();
      this.tracks.set(e.id, track);
    }
    const last = track.latest();
    if (last) {
      const jump = Math.hypot(e.x - last.x, e.y - last.y) > TELEPORT_DIST;
      const revived = prev?.dead && !e.dead;
      if (jump || revived) track.clear();
    }
    track.push(t, e.x, e.y, e.facing);
  }

  private remove(id: EntityId, onRemoved?: (id: EntityId) => void): void {
    if (!this.entities.delete(id)) return;
    this.tracks.delete(id);
    onRemoved?.(id);
  }

  /** Liste complète des entités (nouvelle allocation : à appeler à la fréquence des snapshots, pas par frame). */
  list(): EntitySnapshot[] {
    return [...this.entities.values()];
  }

  /**
   * Position/orientation de l'entité `id` au temps serveur `renderTime` (ms), écrite dans `out`.
   * Retourne false si l'entité est inconnue.
   */
  sample(id: EntityId, renderTime: number, out: Interpolated): boolean {
    const track = this.tracks.get(id);
    if (!track || track.count === 0) return false;
    const n = track.count;
    const newest = track.at(n - 1);
    if (n === 1 || renderTime >= newest.t) {
      if (n >= 2 && renderTime > newest.t) {
        // extrapolation bornée à partir des deux derniers échantillons
        const prev = track.at(n - 2);
        const span = newest.t - prev.t;
        const ahead = Math.min(renderTime - newest.t, MAX_EXTRAPOLATE_MS);
        if (span > 0) {
          const k = ahead / span;
          out.x = newest.x + (newest.x - prev.x) * k;
          out.y = newest.y + (newest.y - prev.y) * k;
          out.facing = newest.facing;
          return true;
        }
      }
      out.x = newest.x;
      out.y = newest.y;
      out.facing = newest.facing;
      return true;
    }
    const oldest = track.at(0);
    if (renderTime <= oldest.t) {
      out.x = oldest.x;
      out.y = oldest.y;
      out.facing = oldest.facing;
      return true;
    }
    // recherche du segment [a, b] qui encadre renderTime (du plus récent au plus ancien)
    for (let i = n - 1; i > 0; i--) {
      const a = track.at(i - 1);
      if (a.t <= renderTime) {
        const b = track.at(i);
        const k = b.t === a.t ? 1 : (renderTime - a.t) / (b.t - a.t);
        out.x = a.x + (b.x - a.x) * k;
        out.y = a.y + (b.y - a.y) * k;
        out.facing = lerpAngle(a.facing, b.facing, k);
        return true;
      }
    }
    out.x = newest.x;
    out.y = newest.y;
    out.facing = newest.facing;
    return true;
  }

  /** Vitesse serveur récente (unités de jeu / s) d'après les deux derniers échantillons. */
  velocity(id: EntityId, out: { x: number; y: number }): boolean {
    const track = this.tracks.get(id);
    if (!track || track.count < 2) {
      out.x = 0;
      out.y = 0;
      return false;
    }
    const b = track.at(track.count - 1);
    const a = track.at(track.count - 2);
    const dt = (b.t - a.t) / 1000;
    if (dt <= 0) {
      out.x = 0;
      out.y = 0;
      return false;
    }
    out.x = (b.x - a.x) / dt;
    out.y = (b.y - a.y) / dt;
    return true;
  }

  latestSample(id: EntityId): Sample | undefined {
    return this.tracks.get(id)?.latest();
  }

  clear(): void {
    this.entities.clear();
    this.tracks.clear();
    this.lastTick = -1;
  }
}
