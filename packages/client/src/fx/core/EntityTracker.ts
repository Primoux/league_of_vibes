/**
 * Positions et infos des entités pour les FX (suivre un champion, viser une cible…).
 * Source : objet 3D du renderer si disponible (`host.getEntityObject`), sinon derniers snapshots du store
 * lissés localement (pour ne pas sautiller à 20 Hz).
 */
import * as THREE from 'three';
import { RENDER_SCALE, type EntitySnapshot, type Snapshot, type Team } from '@lov/shared';
import { store } from '../../state.ts';
import type { FxHost } from './host.ts';

interface Tracked {
  snap: EntitySnapshot;
  x: number; // position affichée (monde)
  z: number;
  fresh: boolean;
}

export class EntityTracker {
  private readonly map = new Map<number, Tracked>();
  private lastSnap: Snapshot | null = null;
  private readonly unsub: () => void;

  constructor(private readonly host: FxHost) {
    this.unsub = store.subscribe((s) => s.snapshot && this.ingest(s.snapshot));
    if (store.state.snapshot) this.ingest(store.state.snapshot);
  }

  private ingest(snap: Snapshot): void {
    if (snap === this.lastSnap) return;
    this.lastSnap = snap;
    for (const e of snap.entities) {
      const t = this.map.get(e.id);
      if (t) t.snap = e;
      else this.map.set(e.id, { snap: e, x: e.x * RENDER_SCALE, z: -e.y * RENDER_SCALE, fresh: true });
    }
    if (snap.removed) for (const id of snap.removed) this.map.delete(id);
  }

  /** Lissage exponentiel vers la dernière position connue. */
  update(dt: number): void {
    const k = 1 - Math.exp(-dt * 14);
    for (const t of this.map.values()) {
      const tx = t.snap.x * RENDER_SCALE;
      const tz = -t.snap.y * RENDER_SCALE;
      if (t.fresh) {
        t.x = tx;
        t.z = tz;
        t.fresh = false;
      } else {
        t.x += (tx - t.x) * k;
        t.z += (tz - t.z) * k;
      }
    }
  }

  /** Position monde au sol (+ hauteur `h`) de l'entité ; false si inconnue. */
  pos(id: number, out: THREE.Vector3, h = 0): boolean {
    const obj = this.host.getEntityObject?.(id);
    if (obj) {
      out.set(obj.position.x, h, obj.position.z);
      return true;
    }
    const t = this.map.get(id);
    if (!t) return false;
    out.set(t.x, h, t.z);
    return true;
  }

  info(id: number): EntitySnapshot | undefined {
    return this.map.get(id)?.snap;
  }

  /** Objet 3D de l'entité (modèle du champion…) si le renderer l'expose. */
  object(id: number): THREE.Object3D | undefined {
    return this.host.getEntityObject?.(id);
  }

  localTeam(): Team | null {
    return store.state.game?.team ?? null;
  }

  /** L'entité est-elle ennemie du joueur local ? (teinte rouge des FX ennemis, M4) */
  isEnemy(id: number): boolean {
    const me = this.localTeam();
    const team = this.info(id)?.team;
    return !!me && !!team && team !== 'neutral' && team !== me;
  }

  dispose(): void {
    this.unsub();
  }
}
