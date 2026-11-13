import { deflateRawSync } from 'node:zlib';
import { describe, expect, it } from 'vitest';
import { CHAMPION_LIST, LANE_PATHS, TICK_DT, type Lane } from '@lov/shared';
import { ClientView, ScoreTracker, SnapshotFrame } from '../src/net/snapshot.ts';
import { createSystems } from '../src/systems/index.ts';
import { World, createChampion } from '../src/world.ts';
import { BASE_STATS } from './helpers.ts';

describe('performance', () => {
  it('tick < 5 ms avec 8 champions + 60 sbires qui se battent ; snapshot raisonnable', () => {
    const w = new World(createSystems());
    for (let i = 0; i < 8; i++) {
      const team = i < 4 ? 'blue' : 'red';
      createChampion(w, { playerId: `p${i}`, name: `J${i}`, team, championId: CHAMPION_LIST[i]!.id, summoners: ['flash', 'ignite'], pos: { x: 5800 + (i % 4) * 100, y: 5800 + (i < 4 ? -300 : 300) } });
    }
    const lanes: Lane[] = ['top', 'mid', 'bot'];
    for (let i = 0; i < 60; i++) {
      const team = i % 2 ? 'blue' : 'red';
      const path = LANE_PATHS[lanes[i % 3]!];
      const p = path[Math.floor(path.length / 2)]!;
      const m = w.spawn({ kind: 'minion', team, pos: { x: p.x + (i % 7) * 30, y: p.y + (i % 5) * 30 }, radius: 40, hp: 500, mana: 0, stats: { ...BASE_STATS, ad: 5, attackRange: i % 2 ? 110 : 500 } });
      m.data.keepCorpse = true;
    }
    w.init();
    // Tout le monde attaque l'ennemi le plus proche.
    for (const e of w.entities.values()) {
      let best: number | undefined;
      let bd = Infinity;
      for (const o of w.entities.values()) {
        if (o.team === e.team) continue;
        const d = Math.hypot(o.pos.x - e.pos.x, o.pos.y - e.pos.y);
        if (d < bd) (bd = d), (best = o.id);
      }
      e.attackTarget = best;
    }
    for (let i = 0; i < 30; i++) w.step(TICK_DT); // chauffe
    const ticks = 300;
    const t0 = performance.now();
    for (let i = 0; i < ticks; i++) w.step(TICK_DT);
    const perTick = (performance.now() - t0) / ticks;
    expect(perTick).toBeLessThan(5);

    const score = new ScoreTracker(w);
    const view = new ClientView();
    const full = JSON.stringify(view.build(new SnapshotFrame(w, [], score.score()), 'p0', 'blue'));
    let bytes = 0;
    let zbytes = 0;
    for (let i = 0; i < 20; i++) {
      w.step(TICK_DT);
      w.step(TICK_DT);
      const raw = JSON.stringify({ t: 'snapshot', snap: view.build(new SnapshotFrame(w, w.events.drain(), score.score()), 'p0', 'blue') });
      bytes += raw.length;
      zbytes += deflateRawSync(raw).length; // ≈ permessage-deflate sans contexte partagé
    }
    // Info utile en CI.
    console.log(`[perf] tick ${perTick.toFixed(2)} ms, snapshot complet ${full.length} o, delta ~${Math.round(bytes / 20)} o (x20/s = ${Math.round(bytes / 1024)} Ko/s brut, ${Math.round(zbytes / 1024)} Ko/s compressé)`);
    // Pire cas (68 unités visibles en combat permanent) : < 30 Ko/s une fois compressé (permessage-deflate).
    expect(zbytes).toBeLessThan(30 * 1024);
  });
});
