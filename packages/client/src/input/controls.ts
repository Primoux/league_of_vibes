/**
 * Contrôles clavier/souris. Propriétaire : claude-client.
 * BOOTSTRAP : clic droit = déplacement, Q/W/E/R = sort vers le curseur, D/F = sorts d'invocateur, S = stop.
 * TODO(claude-client) : attaque sur clic droit d'un ennemi (picking d'entités), A+clic, B rappel, Ctrl+QWER, quick-cast.
 */
import type { AbilitySlot, SummonerSlot, Vec2 } from '@lov/shared';
import { net } from '../net/client.ts';
import type { GameRenderer } from '../render/GameRenderer.ts';
import { store } from '../state.ts';

export function installControls(renderer: GameRenderer): void {
  const canvas = renderer.renderer.domElement;
  let mouse: Vec2 | null = null;
  canvas.addEventListener('contextmenu', (e) => e.preventDefault());
  canvas.addEventListener('pointermove', (e) => (mouse = renderer.pickGround(e.clientX, e.clientY)));
  canvas.addEventListener('pointerdown', (e) => {
    if (e.button !== 2 || !store.state.game) return;
    const pos = renderer.pickGround(e.clientX, e.clientY);
    if (pos) net.send({ t: 'move', pos });
  });
  addEventListener('keydown', (e) => {
    if (!store.state.game || (e.target as HTMLElement).tagName === 'INPUT') return;
    const key = e.key.toUpperCase();
    if ('QWER'.includes(key) && key.length === 1) {
      if (e.ctrlKey) net.send({ t: 'levelAbility', slot: key as AbilitySlot });
      else net.send({ t: 'cast', slot: key as AbilitySlot, targetPos: mouse ?? undefined });
      e.preventDefault();
    } else if (key === 'D' || key === 'F') net.send({ t: 'summoner', slot: key as SummonerSlot, targetPos: mouse ?? undefined });
    else if (key === 'S') net.send({ t: 'stop' });
    else if (key === 'B') net.send({ t: 'recall' });
  });
}
