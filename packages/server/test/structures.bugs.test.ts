import { expect, it } from 'vitest';
import { World, createChampion } from '../src/world.ts';
import { applyDamage } from '../src/combat/damage.ts';
import { structuresSystem, structureState } from '../src/systems/structures.ts';
import { stats } from '../src/world-ai/runtime.ts';

function setup() {
  const world = new World([structuresSystem]); world.init(); world.time = 200;
  const tower = [...world.query('tower')].find(e => e.team === 'blue' && e.subtype === 'outer')!;
  const attacker = createChampion(world, { playerId: 'attacker', name: 'Ennemi', team: 'red', championId: 'grondak', summoners: ['flash', 'smite'], pos: { x: tower.pos.x + 300, y: tower.pos.y } });
  return { world, tower, attacker };
}
it('donne une plaque franchie par des dégâts positifs arrondis à zéro', () => {
  const { world, tower, attacker } = setup();
  tower.hp = 4000.05;
  applyDamage(world, { source: attacker, target: tower, amount: 0.1, type: 'true' });
  expect(structureState(tower).plates).toBe(4);
  expect(world.events.pending.filter(e => e.type === 'structureDestroyed' && e.plate)).toHaveLength(1);
});
it('protège un allié agressé même si les dégâts sont inférieurs à une unité', () => {
  const { world, tower, attacker } = setup();
  const ally = createChampion(world, { playerId: 'ally', name: 'Allié', team: 'blue', championId: 'grondak', summoners: ['flash', 'smite'], pos: { x: tower.pos.x + 100, y: tower.pos.y } });
  const minion = world.spawn({ kind: 'minion', subtype: 'melee', team: 'red', pos: { x: tower.pos.x + 150, y: tower.pos.y }, radius: 30, hp: 1000, mana: 0, stats: stats(1000, 10) });
  world.step(); expect(tower.attackTarget).toBe(minion.id);
  applyDamage(world, { source: attacker, target: ally, amount: 0.1, type: 'true' }); world.step();
  expect(tower.attackTarget).toBe(attacker.id);
});
