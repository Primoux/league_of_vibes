import { describe, expect, it } from 'vitest';
import { getBuff, hasCC } from '../src/combat/buffs.ts';
import { game, advance, cast, damage, auto, dummy } from './champion-helpers.ts';
describe('Sable', () => {
  it('Q empowers one attack with splash damage', () => {
    const { world, source, target } = game('sable'); const nearby = dummy(world, source, { x: 1300, y: 1000 });
    cast(world, source, 'Q'); auto(world, source, target);
    expect(damage(world, 'sable_q', target)).toBeGreaterThan(0); expect(damage(world, 'sable_q', nearby)).toBeGreaterThan(0);
    const dealt = damage(world, 'sable_q'); auto(world, source, target); expect(damage(world, 'sable_q')).toBe(dealt);
  });
  it('W fires seven projectiles but only damages each enemy once', () => {
    const { world, source, target } = game('sable'); cast(world, source, 'W', target); advance(world, .8);
    expect(world.events.pending.filter((e) => e.type === 'projectileSpawn' && e.fxKey === 'sable_w')).toHaveLength(7);
    expect(world.events.pending.filter((e) => e.type === 'damage' && e.ability === 'sable_w')).toHaveLength(1); expect(hasCC(target, 'slow')).toBe(true);
  });
  it('E rolls and makes the next attack a strengthened guaranteed crit', () => {
    const { world, source, target } = game('sable'); cast(world, source, 'E', undefined, { x: 900, y: 1000 }); advance(world, .2);
    const amount = auto(world, source, target);
    expect(amount).toBeCloseTo(source.stats.ad * 1.75 * 1.1 * 100 / (100 + target.stats.armor));
    expect(getBuff(source, 'sable_e')).toBeUndefined();
    expect(world.events.pending.some((e) => e.type === 'damage' && e.crit)).toBe(true);
  });
  it('R pierces minions but stops at the first champion after aiming', () => {
    const { world, source, target } = game('sable'); target.pos.x = 1600;
    const minion = dummy(world, source, { x: 1300, y: 1000 });
    cast(world, source, 'R', target); advance(world, .9); expect(damage(world, 'sable_r')).toBe(0);
    advance(world, .5); expect(damage(world, 'sable_r', minion)).toBeGreaterThan(0); expect(damage(world, 'sable_r', target)).toBeGreaterThan(0);
    expect([...world.query('projectile')]).toHaveLength(0);
  });
  it('passive grants temporary speed on champion hits', () => {
    const { world, source, target } = game('sable'); auto(world, source, target);
    expect(source.stats.moveSpeed).toBeCloseTo(330 * 1.15); advance(world, 1.1); expect(source.stats.moveSpeed).toBe(330);
  });
});
