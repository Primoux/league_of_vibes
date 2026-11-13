/**
 * Bus FX côté client. Propriétaire : claude-fx.
 * Le renderer appelle `fxBus.dispatch(event)` pour chaque GameEvent reçu ; les modules FX/audio s'abonnent.
 *
 * Synchronisation : `net/client.ts` (claude-client) publie déjà chaque événement au moment où le rendu
 * interpolé atteint son snapshot (horloge serveur synchronisée) : `delayMs` vaut donc 0 par défaut.
 * Si une source publiait des événements « bruts » (dès réception), `delayMs = INTERP_DELAY_MS` les retarde
 * pour les abonnés normaux ; `subscribe(fn, { immediate: true })` les reçoit toujours sans délai.
 * Les événements retardés sont libérés par `fxBus.update(now)`, appelé à chaque frame par `installFx`.
 */
import type { GameEvent } from '@lov/shared';

type Fn = (e: GameEvent) => void;

export interface SubscribeOptions {
  /** Recevoir l'événement à sa réception, sans délai d'interpolation. */
  immediate?: boolean;
}

const nowMs = (): number => (typeof performance !== 'undefined' ? performance.now() : Date.now());

export class FxBus {
  /** Délai appliqué aux abonnés non immédiats (ms). 0 = livraison directe (cas du jeu : déjà synchronisé par net/client). */
  delayMs = 0;
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
