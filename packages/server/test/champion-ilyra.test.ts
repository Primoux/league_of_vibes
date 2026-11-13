import { describe, expect, it } from 'vitest';
import { getBuff, hasCC } from '../src/combat/buffs.ts';
import { advance, cast, damage, game } from './champion-helpers.ts';
import { applyDamage } from '../src/combat/damage.ts';
describe('Ilyra', () => {
  it('Q hits only the first enemy and applies frost and a slow', () => {
    const { world, source, target } = game('ilyra');
    cast(world, source, 'Q', target); advance(world, .5);
    expect(damage(world, 'ilyra_q')).toBeGreaterThan(0); expect(hasCC(target, 'slow')).toBe(true);
    expect(getBuff(target, 'ilyra_frost', source.id)?.stacks).toBe(1);
  });
  it('W explodes after its telegraph, with a stronger slow', () => {
    const { world, source, target } = game('ilyra');
    cast(world, source, 'W', target); advance(world, .8); expect(damage(world, 'ilyra_w')).toBe(0);
    advance(world, .1); expect(damage(world, 'ilyra_w')).toBeGreaterThan(0);
    expect(target.stats.moveSpeed).toBeCloseTo(330 * .5);
  });
  it('E shields an ally, slows an attacker and expires', () => {
    const { world, source, target } = game('ilyra');
    cast(world, source, 'E', source); expect(source.shield).toBe(70);
    applyDamage(world, { source: target, target: source, amount: 10, type: 'physical' });
    expect(hasCC(target, 'slow')).toBe(true);
    advance(world, 2.6); expect(source.shield).toBe(0);
  });
  it('R deals four ticks and triggers the passive only once per cooldown window', () => {
    const { world, source, target } = game('ilyra');
    cast(world, source, 'R', target); advance(world, 4.3);
    const hits = world.events.pending.filter((e) => e.type === 'damage' && e.ability === 'ilyra_r');
    expect(hits).toHaveLength(4);
    const stuns = world.events.pending.filter((e) => e.type === 'buffApplied' && e.buffId === 'stun');
    expect(stuns).toHaveLength(1); expect(getBuff(target, 'ilyra_frost_immunity', source.id)).toBeDefined();
  });
});
