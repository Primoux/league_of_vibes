import { describe, expect, it } from 'vitest';
import { getBuff, hasCC } from '../src/combat/buffs.ts';
import { game, advance, cast, damage, auto } from './champion-helpers.ts';
describe('Vrenna', () => {
  it('Q pierces and pulls targets hit by the tip', () => {
    const { world, source, target } = game('vrenna'); target.pos.x = 1550;
    cast(world, source, 'Q', target); advance(world, .9);
    expect(damage(world, 'vrenna_q')).toBeGreaterThan(0); expect(target.pos.x).toBeLessThan(1550);
  });
  it('W grants attack speed and heals eight percent of missing health', () => {
    const { world, source } = game('vrenna'); const speed = source.stats.attackSpeed; source.hp -= 500;
    cast(world, source, 'W'); expect(source.hp).toBeCloseTo(source.stats.maxHp - 460); expect(source.stats.attackSpeed).toBeCloseTo(speed * 1.3);
  });
  it('E damages and slows at landing', () => {
    const { world, source, target } = game('vrenna'); cast(world, source, 'E', target); expect(damage(world, 'vrenna_e')).toBe(0);
    advance(world, .5); expect(damage(world, 'vrenna_e')).toBeGreaterThan(0); expect(hasCC(target, 'slow')).toBe(true);
  });
  it('R requires a champion and knocks up at impact', () => {
    const { world, source, target } = game('vrenna'); cast(world, source, 'R', target); advance(world, .3);
    expect(damage(world, 'vrenna_r')).toBeGreaterThan(0); expect(hasCC(target, 'knockup')).toBe(true);
  });
  it('stacks bleed to five, grants AS and ticks without recursively stacking', () => {
    const { world, source, target } = game('vrenna');
    for (let i = 0; i < 5; i++) auto(world, source, target);
    expect(getBuff(target, 'vrenna_bleed', source.id)?.stacks).toBe(5);
    expect(getBuff(source, 'vrenna_p_speed')).toBeDefined();
    advance(world, 3.1);
    expect(world.events.pending.filter((e) => e.type === 'damage' && e.ability === 'vrenna_p')).toHaveLength(3);
    expect(getBuff(target, 'vrenna_bleed')).toBeUndefined();
  });
});
