import { expect, it } from 'vitest';
import { TICK_DT } from '@lov/shared';
import { World } from '../src/world.ts';
import { createSystems } from '../src/systems/index.ts';

it('garde le tick PvE réel sous 2 ms avec trois voies actives', () => {
  const world = new World(createSystems()); world.init();
  // Prépare de vraies vagues en combat ; aucun sbire synthétique sans IA.
  for (let i = 0; i < 1500; i++) { world.step(0.2); world.events.drain(); }
  const minions = [...world.query('minion')].length;
  expect(minions).toBeGreaterThan(60);
  const ticks = 900;
  const start = performance.now();
  for (let i = 0; i < ticks; i++) { world.step(TICK_DT); world.events.drain(); }
  const averageMs = (performance.now() - start) / ticks;
  console.log(`[perf PvE] ${averageMs.toFixed(3)} ms/tick, ${world.entities.size} entités, ${minions} sbires au départ, simulation à 30 Hz`);
  expect(world.winner).toBeNull();
  expect(averageMs).toBeLessThan(2);
}, 10000);
