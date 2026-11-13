import { describe, expect, it } from 'vitest';
import { getChampion } from '@lov/shared';
import { hasCC } from '../src/combat/buffs.ts';
import { game, advance, cast, damage } from './champion-helpers.ts';
describe('Zephyr', () => {
  it('Q hits outbound and returns for true damage', () => {
    const { world, source, target } = game('zephyr'); cast(world, source, 'Q', target); advance(world, 1.7);
    const hits = world.events.pending.filter((e) => e.type === 'damage' && e.ability === 'zephyr_q');
    expect(hits).toHaveLength(2); expect(hits[0]).toMatchObject({ damageType: 'magic' }); expect(hits[1]).toMatchObject({ damageType: 'true', amount: 60 });
  });
  it('W blinks and explodes at the departure position', () => {
    const { world, source, target } = game('zephyr'); target.pos.x = 1100;
    cast(world, source, 'W', undefined, { x: 1450, y: 1000 }); expect(source.pos.x).toBe(1450);
    advance(world, .9); expect(damage(world, 'zephyr_w')).toBe(0);
    advance(world, .2); expect(damage(world, 'zephyr_w')).toBeGreaterThan(0);
  });
  it('E roots only when the tether remains in range', () => {
    const g = game('zephyr'); cast(g.world, g.source, 'E', g.target); advance(g.world, 1.6);
    expect(hasCC(g.target, 'root')).toBe(true); expect(damage(g.world, 'zephyr_e')).toBeGreaterThan(0);
    const broken = game('zephyr'); cast(broken.world, broken.source, 'E', broken.target); broken.target.pos.x = 4000; advance(broken.world, 2);
    expect(damage(broken.world, 'zephyr_e')).toBe(0);
  });
  it('R provides exactly three rays for one resource cost', () => {
    const { world, source, target } = game('zephyr'); const mana = source.mana;
    for (let i = 0; i < 3; i++) { expect(cast(world, source, 'R', target)).toBe(true); advance(world, .8); }
    expect(cast(world, source, 'R', target)).toBe(false); expect(source.mana).toBeCloseTo(mana - 100);
    expect(world.events.pending.filter((e) => e.type === 'damage' && e.ability === 'zephyr_r')).toHaveLength(3);
  });
  it('amplifies the spell following four casts and refunds mana', () => {
    const { world, source, target } = game('zephyr');
    for (let i = 0; i < 4; i++) { source.champ!.cooldowns.W = 0; source.mana = source.stats.maxMana; cast(world, source, 'W', undefined, source.pos); }
    source.mana = 100; cast(world, source, 'Q', target); advance(world, .5);
    expect(source.mana).toBe(100 - getChampion('zephyr').abilities.Q.cost[0]! + 30);
    expect(damage(world, 'zephyr_q')).toBeCloseTo(60 * 1.25 * 100 / (100 + target.stats.mr));
  });
});
