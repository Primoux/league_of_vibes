import { describe, expect, it } from 'vitest';
import { getBuff, hasCC } from '../src/combat/buffs.ts';
import { game, advance, cast, damage, auto, dummy } from './champion-helpers.ts';
describe('Corvin', () => {
  it('Q pierces units and applies a mark to each', () => {
    const { world, source, target } = game('corvin'); const extra = dummy(world, source, { x: 1600, y: 1000 });
    cast(world, source, 'Q', target); advance(world, .8);
    expect(damage(world, 'corvin_q', extra)).toBeGreaterThan(0); expect(getBuff(target, 'corvin_marks', source.id)?.stacks).toBe(1);
  });
  it('W roots and reveals the first enemy, then removes the trap', () => {
    const { world, source, target } = game('corvin'); cast(world, source, 'W', target);
    expect([...world.query('zone')][0]?.data.invisible).toBe(true); advance(world, .1);
    expect(hasCC(target, 'root')).toBe(true); expect(target.data.revealed).toBe(true); expect([...world.query('zone')]).toHaveLength(0);
  });
  it('keeps only the three newest traps', () => {
    const { world, source } = game('corvin');
    for (let i = 0; i < 4; i++) { source.champ!.cooldowns.W = 0; cast(world, source, 'W', undefined, { x: 1000 + i * 50, y: 1500 }); }
    expect([...world.query('zone')]).toHaveLength(3);
  });
  it('E jumps backward from the aim direction and grants speed', () => {
    const { world, source, target } = game('corvin'); cast(world, source, 'E', target); advance(world, .5);
    expect(source.pos.x).toBe(575); expect(source.stats.moveSpeed).toBeCloseTo(330 * 1.3);
  });
  it('R deals three ticks, reveals and reduces healing', () => {
    const { world, source, target } = game('corvin'); cast(world, source, 'R', target); advance(world, 3.3);
    expect(world.events.pending.filter((e) => e.type === 'damage' && e.ability === 'corvin_r')).toHaveLength(3);
    expect(getBuff(target, 'grievous')).toBeDefined(); expect(target.data.revealed).toBe(true);
  });
  it('consumes three marks to deal six percent max HP as magic damage', () => {
    const { world, source, target } = game('corvin'); for (let i = 0; i < 3; i++) auto(world, source, target);
    expect(damage(world, 'corvin_p')).toBeCloseTo(target.stats.maxHp * .06 * 100 / (100 + target.stats.mr));
    expect(getBuff(target, 'corvin_marks')).toBeUndefined();
  });
});
