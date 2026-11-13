import { describe, expect, it } from 'vitest';
import { FOUNTAINS, RECALL_SECONDS, TICK_RATE, dist, type ClientMsg, type GameEvent, type Vec2 } from '@lov/shared';
import { applyDamage } from '../src/combat/damage.ts';
import { econ } from '../src/items/index.ts';
import { RECALL_LEVIATHAN_SECONDS } from '../src/systems/economy.ts';
import { createSystems } from '../src/systems/index.ts';
import { World, createChampion, type Entity } from '../src/world.ts';

let n = 0;
function makeWorld(): World {
  const w = new World(createSystems());
  w.init();
  return w;
}
function addChamp(world: World, pos: Vec2): Entity {
  return createChampion(world, { playerId: `p${++n}`, name: `T${n}`, team: 'blue', championId: 'grondak', summoners: ['flash', 'ignite'], pos });
}
function run(world: World, seconds: number): void {
  for (let i = 0; i < Math.round(seconds * TICK_RATE); i++) world.step();
}
function cmd(world: World, ent: Entity, msg: ClientMsg): void {
  world.commands.push({ playerId: ent.champ!.playerId, msg });
  world.step();
}

describe('rappel', () => {
  it('canalise 8 s puis se téléporte à la fontaine', () => {
    const w = makeWorld();
    const c = addChamp(w, { x: 6000, y: 6000 });
    const events: GameEvent[] = [];
    w.events.on('recallStart', (e) => events.push(e));
    w.events.on('recallEnd', (e) => events.push(e));
    cmd(w, c, { t: 'recall' });
    expect(econ(c).recall).not.toBeNull();
    expect(econ(c).recall!.total).toBe(RECALL_SECONDS);
    run(w, RECALL_SECONDS - 1);
    expect(econ(c).recall).not.toBeNull(); // pas encore terminé
    expect(dist(c.pos, FOUNTAINS.blue)).toBeGreaterThan(1000);
    run(w, 1.5);
    expect(econ(c).recall).toBeNull();
    expect(dist(c.pos, FOUNTAINS.blue)).toBeLessThan(200);
    expect(events.map((e) => e.type)).toEqual(['recallStart', 'recallEnd']);
    expect(events[1]).toMatchObject({ type: 'recallEnd', completed: true });
  });

  it('est interrompu par les dégâts subis', () => {
    const w = makeWorld();
    const c = addChamp(w, { x: 6000, y: 6000 });
    const enemy = createChampion(w, { playerId: `e${++n}`, name: 'E', team: 'red', championId: 'sable', summoners: ['flash', 'ignite'], pos: { x: 6100, y: 6000 } });
    const ends: GameEvent[] = [];
    w.events.on('recallEnd', (e) => ends.push(e));
    cmd(w, c, { t: 'recall' });
    run(w, 2);
    applyDamage(w, { source: enemy, target: c, amount: 10, type: 'physical' });
    expect(econ(c).recall).toBeNull();
    expect(ends).toHaveLength(1);
    expect(ends[0]).toMatchObject({ type: 'recallEnd', completed: false });
    run(w, RECALL_SECONDS);
    expect(dist(c.pos, FOUNTAINS.blue)).toBeGreaterThan(1000); // pas téléporté
  });

  it('est interrompu par un ordre de déplacement', () => {
    const w = makeWorld();
    const c = addChamp(w, { x: 6000, y: 6000 });
    cmd(w, c, { t: 'recall' });
    run(w, 1);
    cmd(w, c, { t: 'move', pos: { x: 6100, y: 6000 } });
    expect(econ(c).recall).toBeNull();
  });

  it(' dure 4 s avec le buff du Léviathan', () => {
    const w = makeWorld();
    const c = addChamp(w, { x: 6000, y: 6000 });
    c.data.leviathanBuff = true; // posé par codex-objectives
    cmd(w, c, { t: 'recall' });
    expect(econ(c).recall!.total).toBe(RECALL_LEVIATHAN_SECONDS);
    run(w, RECALL_LEVIATHAN_SECONDS + 0.5);
    expect(econ(c).recall).toBeNull();
    expect(dist(c.pos, FOUNTAINS.blue)).toBeLessThan(200);
  });

  it('est annulé à la mort et impossible quand mort', () => {
    const w = makeWorld();
    const c = addChamp(w, { x: 6000, y: 6000 });
    const enemy = createChampion(w, { playerId: `e${++n}`, name: 'E', team: 'red', championId: 'sable', summoners: ['flash', 'ignite'], pos: { x: 6100, y: 6000 } });
    c.dead = true;
    c.champ!.respawnIn = 9999; // empêche le respawn auto du système combat pendant le test
    cmd(w, c, { t: 'recall' });
    expect(econ(c).recall).toBeNull();
    c.dead = false;
    cmd(w, c, { t: 'recall' });
    expect(econ(c).recall).not.toBeNull();
    applyDamage(w, { source: enemy, target: c, amount: 100000, type: 'true' });
    expect(c.dead).toBe(true);
    expect(econ(c).recall).toBeNull();
  });
});
