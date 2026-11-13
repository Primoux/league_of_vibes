import { describe, expect, it } from 'vitest';
import { getBuff, hasCC } from '../src/combat/buffs.ts';
import { game, advance, cast, damage, auto } from './champion-helpers.ts';
describe('Tharox', () => {
  it('Q hits, knocks back and stops on the first unit', () => {
    const { world, source, target } = game('tharox');
    cast(world, source, 'Q', target); advance(world, .6);
    expect(damage(world, 'tharox_q')).toBeGreaterThan(0); expect(target.pos.x).toBeGreaterThan(1250);
  });
  it('W grants resistances and deals four aura ticks', () => {
    const { world, source, target } = game('tharox'); const armor = source.stats.armor;
    cast(world, source, 'W'); expect(source.stats.armor).toBeCloseTo(armor + 15); advance(world, 4.1);
    expect(world.events.pending.filter((e) => e.type === 'damage' && e.ability === 'tharox_w' && e.target === target.id)).toHaveLength(4);
    expect(source.stats.armor).toBeCloseTo(armor);
  });
  it('E damages and slows nearby enemies', () => {
    const { world, source, target } = game('tharox'); cast(world, source, 'E');
    expect(damage(world, 'tharox_e')).toBeGreaterThan(0); expect(hasCC(target, 'slow')).toBe(true);
  });
  it('R winds up then collides with and stuns champions', () => {
    const { world, source, target } = game('tharox'); cast(world, source, 'R', target); advance(world, .9);
    expect(damage(world, 'tharox_r')).toBe(0); advance(world, .4);
    expect(damage(world, 'tharox_r')).toBeGreaterThan(0); expect(hasCC(target, 'stun')).toBe(true);
  });
  it('charges momentum from movement and consumes it on an attack', () => {
    const { world, source, target } = game('tharox'); world.step();
    source.pos.x += 3400; world.step(); expect(source.data.tharoxMomentum).toBe(100);
    auto(world, source, target);
    expect(damage(world, 'tharox_p')).toBeGreaterThan(0); expect(getBuff(target, 'slow')).toBeDefined();
    const amount = damage(world, 'tharox_p'); auto(world, source, target); expect(damage(world, 'tharox_p')).toBe(amount);
  });
});
