/**
 * Bus FX côté client. Propriétaire : claude-fx.
 * Le renderer appelle `fxBus.dispatch(event)` pour chaque GameEvent reçu ; les modules FX/audio s'abonnent.
 *
 * Synchronisation : les entités sont affichées avec INTERP_DELAY_MS de retard (interpolation), alors que les
 * événements arrivent avec le snapshot le plus récent. Par défaut, les abonnés reçoivent donc les événements
 * **retardés du même délai** pour que l'impact d'un sort coïncide avec la position affichée de la cible.
 * `subscribe(fn, { immediate: true })` reçoit l'événement dès réception (UI, annonces…).
 *
 * Les événements retardés sont libérés par `fxBus.update(now)`, appelé à chaque frame par `installFx`.
 */
import { INTERP_DELAY_MS, type GameEvent } from '@lov/shared';

type Fn = (e: GameEvent) => void;

export interface SubscribeOptions {
  /** Recevoir l'événement à sa réception, sans délai d'interpolation. */
  immediate?: boolean;
}

const nowMs = (): number => (typeof performance !== 'undefined' ? performance.now() : Date.now());

export class FxBus {
  /** Délai appliqué aux abonnés non immédiats (ms). 0 = livraison directe (fxlab, tests). */
  delayMs = INTERP_DELAY_MS;
  private subs = new Set<Fn>();
  private immediateSubs = new Set<Fn>();
  private queueAt: number[] = [];
  private queueEv: GameEvent[] = [];
  private head = 0;

  subscribe(fn: Fn, opts: SubscribeOptions = {}): () => void {
    const set = opts.immediate ? this.immediateSubs : this.subs;
    set.add(fn);
    return () => set.delete(fn);
  }

  dispatch(e: GameEvent, now = nowMs()): void {
    this.immediateSubs.forEach((fn) => fn(e));
    if (this.delayMs <= 0) {
      this.deliver(e);
      return;
    }
    this.queueAt.push(now + this.delayMs);
    this.queueEv.push(e);
  }

  /** Livre les événements dont le délai est écoulé (ordre de réception conservé). */
  update(now = nowMs()): void {
    while (this.head < this.queueAt.length && this.queueAt[this.head]! <= now) {
      this.deliver(this.queueEv[this.head]!);
      this.head++;
    }
    // Compacte de temps en temps plutôt qu'à chaque shift (Array.shift est O(n)).
    if (this.head > 0 && (this.head === this.queueAt.length || this.head > 256)) {
      this.queueAt.splice(0, this.head);
      this.queueEv.splice(0, this.head);
      this.head = 0;
    }
  }

  get pending(): number {
    return this.queueAt.length - this.head;
  }

  private deliver(e: GameEvent): void {
    this.subs.forEach((fn) => fn(e));
  }
}

export const fxBus = new FxBus();
