import { describe, expect, it } from 'vitest';
import { TIMERS, scaled } from '@lov/shared';
import { World, createChampion } from '../src/world.ts';
import { structuresSystem, isStructureAttackable, structureState } from '../src/systems/structures.ts';
import { applyDamage, stats } from '../src/world-ai/runtime.ts';
function setup() {
  const world = new World([structuresSystem]); world.init();
  const champion = createChampion(world, { playerId: 'p', name: 'Test', team: 'red', championId: 'grondak', summoners: ['flash', 'smite'], pos: { x: 4900, y: 4900 } });
  const tower = [...world.query('tower')].find(e => e.team === 'blue' && e.subtype === 'outer' && structureState(e).lane === 'mid')!;
  return { world, champion, tower };
}
const hit = (world: World, source: ReturnType<typeof createChampion>, target: ReturnType<typeof createChampion>, amount: number) => applyDamage(world, { source, target, amount, type: 'true' });
describe('structures', () => {
  it('initialise 22 tours, six inhibiteurs et deux nexus', () => {
    const { world } = setup(); expect([...world.query('tower')]).toHaveLength(22); expect([...world.query('inhibitor')]).toHaveLength(6); expect([...world.query('nexus')]).toHaveLength(2);
  });
  it('bloque les dégâts hors ordre et fortifie les tours externes', () => {
    const { world, champion, tower } = setup();
    hit(world, champion, tower, 100); expect(tower.hp).toBe(4950);
    const inner = [...world.query('tower')].find(e => e.team === 'blue' && e.subtype === 'inner' && structureState(e).lane === 'mid')!;
    hit(world, champion, inner, 10000); expect(inner.hp).toBe(inner.stats.maxHp);
    world.time = scaled(180); hit(world, champion, tower, 10000); expect(isStructureAttackable(world, inner)).toBe(true);
  });
  it('ne donne une plaque qu’au seuil, y compris plusieurs sur un même coup', () => {
    const { world, champion, tower } = setup(); world.time = scaled(180);
    hit(world, champion, tower, 1); expect(structureState(tower).plates).toBe(5);
    hit(world, champion, tower, 999); expect(structureState(tower).plates).toBe(4);
    hit(world, champion, tower, 2000); expect(structureState(tower).plates).toBe(2);
    expect(world.events.pending.filter(e => e.type === 'structureDestroyed' && e.plate)).toHaveLength(3);
    world.time = scaled(TIMERS.towerPlatesUntil); world.step(); expect(structureState(tower).plates).toBe(0);
  });
  it('ajoute puis retire la résistance anti-backdoor', () => {
    const { world, tower } = setup(); world.step(); expect(tower.stats.armor).toBe(240);
    world.spawn({ kind: 'minion', team: 'red', pos: { ...tower.pos }, radius: 30, hp: 500, mana: 0, stats: stats(500, 10) });
    world.step(); expect(tower.stats.armor).toBe(40);
  });
  it('privilégie un sbire, puis le champion qui agresse un allié', () => {
    const { world, champion, tower } = setup();
    const minion = world.spawn({ kind: 'minion', subtype: 'melee', team: 'red', pos: { x: tower.pos.x + 200, y: tower.pos.y }, radius: 30, hp: 1000, mana: 0, stats: stats(1000, 10) });
    world.step(); expect(tower.attackTarget).toBe(minion.id);
    const ally = createChampion(world, { playerId: 'a', name: 'Allié', team: 'blue', championId: 'grondak', summoners: ['flash', 'smite'], pos: { ...tower.pos } });
    applyDamage(world, { source: champion, target: ally, amount: 1, type: 'true' }); world.step();
    expect(tower.attackTarget).toBe(champion.id);
  });
  it('augmente les tirs successifs contre un champion et réinitialise à la perte de cible', () => {
    const { world, champion, tower } = setup(); champion.hp = champion.stats.maxHp = 10000;
    world.step(); const first = 10000 - champion.hp;
    world.step(1.3); const second = 10000 - champion.hp - first;
    expect(second / first).toBeCloseTo(1.4);
    champion.pos = { x: 6000, y: 6000 }; world.step(); expect(structureState(tower).ramp).toBe(0);
  });
  it('réanime un inhibiteur après cinq minutes scalées et valide la victoire seulement au nexus exposé', () => {
    const { world, champion } = setup(); world.time = 300;
    for (const e of world.query('tower')) if (e.team === 'blue' && structureState(e).lane === 'mid') hit(world, champion, e, 10000);
    const inhibitor = [...world.query('inhibitor')].find(e => e.team === 'blue' && structureState(e).lane === 'mid')!;
    hit(world, champion, inhibitor, 10000); expect(inhibitor.dead).toBe(true);
    world.step(scaled(TIMERS.inhibitorRespawn) - 0.1); expect(inhibitor.dead).toBe(true);
    world.step(0.1); expect(inhibitor.dead).toBe(false);
    hit(world, champion, inhibitor, 10000);
    const nexus = [...world.query('nexus')].find(e => e.team === 'blue')!;
    hit(world, champion, nexus, 10000); expect(world.winner).toBeNull();
    for (const e of world.query('tower')) if (e.team === 'blue' && e.subtype === 'nexus') hit(world, champion, e, 10000);
    hit(world, champion, nexus, 10000); expect(world.winner).toBe('red');
  });
});
