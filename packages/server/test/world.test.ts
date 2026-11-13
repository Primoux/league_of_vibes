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

  it('spawn copie la position (pas de Vec2 partagé avec les données de la carte)', () => {
    const world = new World(createSystems());
    const shared = { x: 100, y: 100 };
    const e = world.spawn({ kind: 'minion', team: 'blue', pos: shared, radius: 40, hp: 1, mana: 0, stats: createChampion(world, { playerId: 'x', name: 'x', team: 'blue', championId: 'nyx', summoners: ['flash', 'ignite'], pos: shared }).stats });
    e.moveTarget = { x: 500, y: 100 };
    world.step();
    expect(shared).toEqual({ x: 100, y: 100 });
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
