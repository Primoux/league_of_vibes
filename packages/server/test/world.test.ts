import { describe, expect, it } from 'vitest';
import { TICK_RATE } from '@lov/shared';
import { createSystems } from '../src/systems/index.ts';
import { World, createChampion } from '../src/world.ts';

describe('World', () => {
  it('déplace un champion vers la cible demandée', () => {
    const world = new World(createSystems());
    const ent = createChampion(world, { playerId: 'p1', name: 'Test', team: 'blue', championId: 'grondak', summoners: ['flash', 'ignite'], pos: { x: 1000, y: 1000 } });
    world.init();
    world.commands.push({ playerId: 'p1', msg: { t: 'move', pos: { x: 2000, y: 1000 } } });
    for (let i = 0; i < TICK_RATE; i++) world.step();
    expect(ent.pos.x).toBeCloseTo(1000 + ent.stats.moveSpeed, 0);
    for (let i = 0; i < TICK_RATE * 5; i++) world.step();
    expect(ent.pos).toEqual({ x: 2000, y: 1000 });
    expect(ent.moveTarget).toBeUndefined();
  });

  it("relaie les événements via l'EventBus", () => {
    const world = new World([]);
    let seen = 0;
    world.events.on('levelUp', () => seen++);
    world.events.emit({ type: 'levelUp', target: 1, level: 2 });
    expect(seen).toBe(1);
    expect(world.events.drain()).toHaveLength(1);
    expect(world.events.drain()).toHaveLength(0);
  });
});
