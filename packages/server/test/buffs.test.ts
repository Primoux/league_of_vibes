import { describe, expect, it } from 'vitest';
import { World, createChampion } from '../src/world.ts';
import { buffsSystem } from '../src/systems/buffs.ts';
import { addBuff, getBuff, hasCC, recalculateStats, registerStatSource } from '../src/combat/buffs.ts';
import { applyDamage, applyHeal, applyShield } from '../src/combat/damage.ts';
const setup = () => {
  const world = new World([buffsSystem]);
  const e = createChampion(world, { playerId: 'p', name: 'Test', team: 'blue', championId: 'grondak', summoners: ['flash', 'ignite'], pos: { x: 1000, y: 1000 } });
  return { world, e };
};
describe('Buffs and damage contract', () => {
  it('only applies the strongest slow and restores base stats on expiry', () => {
    const { world, e } = setup();
    addBuff(world, e, 'slow', { duration: 1, source: 1, params: { amount: .2 } });
    addBuff(world, e, 'slow', { duration: .5, source: 2, params: { amount: .5 } });
    expect(e.stats.moveSpeed).toBe(335 * .5);
    world.step(.5);
    expect(e.stats.moveSpeed).toBe(335 * .8);
    world.step(.5);
    expect(e.stats.moveSpeed).toBe(335);
  });
  it('reduces CC with tenacity except knockups and suppressions', () => {
    const { world, e } = setup();
    addBuff(world, e, 'cleanse', { duration: 3, params: { tenacity: .65 } });
    addBuff(world, e, 'stun', { duration: 2 });
    addBuff(world, e, 'knockup', { duration: 2 });
    expect(getBuff(e, 'stun')!.remaining).toBeCloseTo(.7);
    expect(getBuff(e, 'knockup')!.remaining).toBe(2);
    world.step(1);
    expect(hasCC(e, 'stun')).toBe(false);
    expect(hasCC(e, 'knockup')).toBe(true);
  });
  it('consumes independent shields by expiry without removing other shields', () => {
    const { world, e } = setup();
    applyShield(world, { source: e, target: e, amount: 100, duration: 1, buffId: 'short' });
    applyShield(world, { source: e, target: e, amount: 200, duration: 3, buffId: 'long' });
    applyDamage(world, { source: e, target: e, amount: 150, type: 'true' });
    expect(e.shield).toBe(150);
    world.step(1);
    expect(e.shield).toBe(150);
    world.step(2);
    expect(e.shield).toBe(0);
    expect(e.hp).toBe(680);
  });
  it('applies grievous wounds, mitigation and emits a single death', () => {
    const { world, e } = setup();
    e.hp = 200;
    addBuff(world, e, 'grievous', { duration: 5, params: { grievous: .4 } });
    expect(applyHeal(world, { source: e, target: e, amount: 100 })).toBe(60);
    expect(applyDamage(world, { source: e, target: e, amount: 138, type: 'physical' })).toBeCloseTo(100);
    applyDamage(world, { source: e, target: e, amount: 10000, type: 'true' });
    applyDamage(world, { source: e, target: e, amount: 10000, type: 'true' });
    expect(world.events.pending.filter((ev) => ev.type === 'death')).toHaveLength(1);
  });
  it('recalculates items and hooked stats without accumulating each tick', () => {
    const { world, e } = setup();
    e.champ!.items[0] = 'long_sword';
    const unsubscribe = registerStatSource(() => ({ flat: { ad: 20 }, percent: { ad: .1 } }));
    try {
      recalculateStats(world, e);
      const expected = (64 + 10 + 20) * 1.1;
      expect(e.stats.ad).toBe(expected);
      world.step(); world.step();
      expect(e.stats.ad).toBe(expected);
    } finally { unsubscribe(); }
  });
});
