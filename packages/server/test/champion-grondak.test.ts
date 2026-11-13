import { describe, expect, it } from 'vitest';
import { hasCC } from '../src/combat/buffs.ts';
import { damageMultiplier } from '@lov/shared';
import { advance, cast, damage, game } from './champion-helpers.ts';
import { applyDamage } from '../src/combat/damage.ts';
describe('Grondak', () => {
  it('Q damages and slows on its line and starts cooldown', () => {
    const { world, source, target } = game('grondak');
    expect(cast(world, source, 'Q', target)).toBe(true); advance(world, .3);
    expect(damage(world, 'grondak_q')).toBeCloseTo(70 * damageMultiplier(target.stats.armor));
    expect(hasCC(target, 'slow')).toBe(true); expect(source.champ!.cooldowns.Q).toBeGreaterThan(8);
  });
  it('W taunts and reduces incoming damage by 30%', () => {
    const { world, source, target } = game('grondak');
    cast(world, source, 'W');
    expect(hasCC(target, 'taunt')).toBe(true);
    const value = applyDamage(world, { source: target, target: source, amount: 100, type: 'magic' });
    expect(value).toBeCloseTo(70 * damageMultiplier(source.stats.mr));
  });
  it('E damages and stuns only after arriving', () => {
    const { world, source, target } = game('grondak');
    target.pos.x = 1500;
    cast(world, source, 'E', target); expect(hasCC(target, 'stun')).toBe(false);
    advance(world, .5);
    expect(damage(world, 'grondak_e')).toBeGreaterThan(0); expect(hasCC(target, 'stun')).toBe(true);
  });
  it('R telegraphs before applying damage and unreduced knockup', () => {
    const { world, source, target } = game('grondak');
    cast(world, source, 'R', target); advance(world, .9);
    expect(damage(world, 'grondak_r')).toBe(0);
    expect(world.events.pending.some((e) => e.type === 'areaEffect' && e.fxKey === 'grondak_r_telegraph')).toBe(true);
    advance(world, .2);
    expect(damage(world, 'grondak_r')).toBeGreaterThan(0); expect(hasCC(target, 'knockup')).toBe(true);
  });
  it('regrows its passive shield after eight seconds out of combat', () => {
    const { world, source, target } = game('grondak');
    advance(world, 8.1); expect(source.shield).toBeCloseTo(source.stats.maxHp * .1);
    applyDamage(world, { source: target, target: source, amount: 10000, type: 'physical' });
    expect(source.dead).toBe(true);
    advance(world, 9); expect(source.shield).toBe(0);
  });
});
