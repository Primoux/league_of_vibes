import { describe, expect, it } from 'vitest';
import { hasCC } from '../src/combat/buffs.ts';
import { game, advance, cast, damage } from './champion-helpers.ts';
import { createChampion } from '../src/world.ts';
describe('Solenne', () => {
  it('Q damages and roots its first target', () => {
    const { world, source, target } = game('solenne'); cast(world, source, 'Q', target); advance(world, .6);
    expect(damage(world, 'solenne_q')).toBeGreaterThan(0); expect(hasCC(target, 'root')).toBe(true);
  });
  it('W heals an ally and the caster with missing-health amplification', () => {
    const { world, source } = game('solenne');
    const ally = createChampion(world, { playerId: 'ally', name: 'Ally', team: 'blue', championId: 'ilyra', summoners: ['flash', 'heal'], pos: { x: 1100, y: 1000 } });
    ally.hp = ally.stats.maxHp / 2; source.hp = source.stats.maxHp / 2;
    const before = ally.hp, selfBefore = source.hp; cast(world, source, 'W', ally);
    expect(ally.hp - before).toBeCloseTo(70 * 1.25); expect(source.hp - selfBefore).toBeCloseTo(70 * .6 * 1.25);
  });
  it('self-targeted W applies a single heal', () => {
    const { world, source } = game('solenne'); source.hp *= .5; const before = source.hp;
    cast(world, source, 'W', source); expect(source.hp - before).toBeCloseTo(70 * 1.25);
  });
  it('E shields, grants attack speed and expires', () => {
    const { world, source } = game('solenne'); const speed = source.stats.attackSpeed; cast(world, source, 'E', source);
    expect(source.shield).toBe(60); expect(source.stats.attackSpeed).toBeCloseTo(speed * 1.25); advance(world, 2.6); expect(source.shield).toBe(0); expect(source.stats.attackSpeed).toBe(speed);
  });
  it('R heals and shields allies only after the delay', () => {
    const { world, source, target } = game('solenne'); source.hp -= 500; target.hp -= 500;
    const hp = source.hp, enemyHp = target.hp; cast(world, source, 'R', undefined, source.pos); advance(world, .7);
    expect(source.hp).toBe(hp); advance(world, .2);
    expect(source.hp).toBeGreaterThan(hp); expect(source.shield).toBeGreaterThan(100); expect(target.hp).toBe(enemyHp); expect(target.shield).toBe(0);
  });
});
