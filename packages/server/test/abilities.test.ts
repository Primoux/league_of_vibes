import { describe, expect, it } from 'vitest';
import { registerStatSource, addBuff } from '../src/combat/buffs.ts';
import { abilityState } from '../src/abilities/engine.ts';
import { spawnProjectile, spawnZone, tether } from '../src/abilities/primitives.ts';
import { game, advance, cast, dummy } from './champion-helpers.ts';
describe('Ability engine', () => {
  it('validates ranks, resources, CC, finite positions and cooldowns', () => {
    const { world, source, target } = game('ilyra');
    source.champ!.abilityLevels.Q = 0;
    expect(cast(world, source, 'Q', target)).toBe(false);
    source.champ!.abilityLevels.Q = 1; source.mana = 0;
    expect(cast(world, source, 'Q', target)).toBe(false);
    source.mana = 100;
    addBuff(world, source, 'silence', { duration: .1 });
    expect(cast(world, source, 'Q', target)).toBe(false);
    advance(world, .1);
    expect(cast(world, source, 'Q', undefined, { x: NaN, y: 0 })).toBe(false);
    expect(cast(world, source, 'Q', target)).toBe(true);
    expect(source.mana).toBe(50);
    expect(cast(world, source, 'Q', target)).toBe(false);
  });
  it('uses swept collision and the closest enemy regardless of insertion order', () => {
    const { world, source, target } = game('ilyra');
    target.pos = { x: 1600, y: 1000 };
    const close = dummy(world, source, { x: 1300, y: 1000 });
    const seen: number[] = [];
    spawnProjectile(world, { source, to: { x: 2000, y: 1000 }, speed: 100000, width: 10, fxKey: 'test', onHit: (e) => seen.push(e.id) });
    world.step(.1);
    expect(seen).toEqual([close.id]);
    expect([...world.query('projectile')]).toHaveLength(0);
  });
  it('pierces once per target and misses enemies outside the width', () => {
    const { world, source, target } = game('ilyra');
    const far = dummy(world, source, { x: 1600, y: 1000 });
    dummy(world, source, { x: 1200, y: 1200 });
    const seen: number[] = [];
    spawnProjectile(world, { source, to: { x: 2000, y: 1000 }, speed: 500, width: 20, fxKey: 'test', pierce: true, onHit: (e) => seen.push(e.id) });
    advance(world, 2.1);
    expect(seen).toEqual([target.id, far.id]);
  });
  it('queues a targeted cast until in range and cancels on a new move order', () => {
    const { world, source, target } = game('grondak');
    target.pos.x = 2200;
    expect(cast(world, source, 'E', target)).toBe(false);
    expect(abilityState(source).pending).toBeDefined();
    expect(source.mana).toBeCloseTo(source.stats.maxMana);
    advance(world, 2);
    expect(source.champ!.cooldowns.E).toBeGreaterThan(0);
    source.champ!.cooldowns.E = 0; target.pos.x = 4000;
    cast(world, source, 'E', target);
    world.commands.push({ playerId: 'p1', msg: { t: 'move', pos: { x: 1000, y: 1000 } } }); world.step();
    expect(abilityState(source).pending).toBeUndefined();
  });
  it('interrupts windups, applies haste once and prevents rooted dashes', () => {
    const { world, source, target } = game('ilyra');
    const off = registerStatSource((_w, e) => e === source ? { flat: { abilityHaste: 100 } } : undefined);
    try {
      world.step();
      expect(cast(world, source, 'Q', target)).toBe(true);
      expect(source.champ!.cooldowns.Q).toBe(3);
      addBuff(world, source, 'stun', { duration: 1 }); advance(world, .4);
      expect([...world.query('projectile')]).toHaveLength(0);
      expect(source.data.casting).toBeUndefined();
    } finally { off(); }
  });
  it('ticks zones the exact number of times and breaks tethers on distance', () => {
    const { world, source, target } = game('ilyra');
    let ticks = 0, complete = 0;
    spawnZone(world, { source, pos: target.pos, radius: 200, duration: 3, interval: 1, fxKey: 'test', onTick: () => ticks++ });
    tether(world, source, target, 1, 500, () => complete++);
    advance(world, .5); target.pos.x = 3000; advance(world, 3);
    expect(ticks).toBe(3); expect(complete).toBe(0);
    expect([...world.query('zone')]).toHaveLength(0);
  });
});
