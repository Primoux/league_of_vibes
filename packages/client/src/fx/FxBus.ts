/**
 * Bus FX côté client. Propriétaire : claude-fx.
 * Le renderer appelle fxBus.dispatch(event) pour chaque GameEvent reçu ; les modules FX/audio s'abonnent.
 */
import type { GameEvent } from '@lov/shared';

type Fn = (e: GameEvent) => void;

class FxBus {
  private subs = new Set<Fn>();
  subscribe(fn: Fn): () => void {
    this.subs.add(fn);
    return () => this.subs.delete(fn);
  }
  dispatch(e: GameEvent): void {
    this.subs.forEach((fn) => fn(e));
  }
}

export const fxBus = new FxBus();
