import type { Snapshot } from '@lov/shared';
import { describe, expect, it, vi } from 'vitest';
import { applySnapshot, chat, juice } from '../src/ui/game/world.ts';
import { store } from '../src/state.ts';

const snap = (events: Snapshot['events'], time = 100): Snapshot => ({
  tick: 1,
  time,
  serverTime: 0,
  entities: [{ id: 1, kind: 'champion', team: 'blue', x: 0, y: 0, facing: 0, hp: 500, maxHp: 1000, subtype: 'ilyra' }],
  events,
  self: undefined,
  score: { blue: { kills: 0, towers: 0, dragons: [], grubs: 0 }, red: { kills: 0, towers: 0, dragons: [], grubs: 0 } },
});

describe('effets et chat de l’UI', () => {
  it('réagit aux événements du joueur local', () => {
    vi.useFakeTimers();
    store.set({ game: { you: 'me', team: 'blue', entityId: 1, players: [], serverTime: 0 } });
    applySnapshot(snap([{ type: 'levelUp', target: 1, level: 6 }, { type: 'levelUp', target: 2, level: 9 }]), 'blue');
    expect(juice.state.level).toBe(6);
    expect(juice.state.levelSeq).toBe(1);

    applySnapshot(snap([{ type: 'itemBought', target: 1, itemId: 'boots' }]), 'blue');
    expect(juice.state.boughtItem).toBe('boots');

    // l'or est agrégé ; l'or passif (< 15) ne déclenche rien
    applySnapshot(snap([{ type: 'goldGain', target: 1, amount: 2 }]), 'blue');
    vi.advanceTimersByTime(300);
    expect(juice.state.golds).toHaveLength(0);
    applySnapshot(snap([{ type: 'goldGain', target: 1, amount: 300 }, { type: 'goldGain', target: 1, amount: 50 }]), 'blue');
    vi.advanceTimersByTime(300);
    expect(juice.state.golds.map((g) => g.amount)).toEqual([350]);
    vi.advanceTimersByTime(1500);
    expect(juice.state.golds).toHaveLength(0);

    applySnapshot(snap([{ type: 'chat', playerId: 'p2', text: 'salut', teamOnly: true }]), 'blue');
    expect(chat.state.lines.at(-1)?.text).toBe('salut');
    vi.useRealTimers();
  });
});
