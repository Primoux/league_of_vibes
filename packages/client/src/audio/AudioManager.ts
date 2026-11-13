/**
 * Audio du jeu. Propriétaire : mistral-economy.
 * S'abonne au fxBus (GameEvent) et joue les sons correspondants. BOOTSTRAP : stub.
 */
import { fxBus } from '../fx/FxBus.ts';

export function installAudio(): void {
  fxBus.subscribe(() => {
    /* TODO(mistral-economy) : sons par type d'événement / fxKey */
  });
}
