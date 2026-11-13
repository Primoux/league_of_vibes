import { describe, expect, it } from 'vitest';
import { getBuff, hasCC, registerStatSource } from '../src/combat/buffs.ts';
import { applyDamage } from '../src/combat/damage.ts';
import { game, advance, cast, damage, auto } from './champion-helpers.ts';
describe('Kaelen', () => {
  it('Q allows exactly three casts and knocks up on the third only', () => {
    const { world, source, target } = game('kaelen');
    for (let i = 0; i < 3; i++) { expect(cast(world, source, 'Q', target)).toBe(true); advance(world, .35); if (i < 2) expect(hasCC(target, 'knockup')).toBe(false); }
    expect(hasCC(target, 'knockup')).toBe(true); expect(cast(world, source, 'Q', target)).toBe(false);
    expect(world.events.pending.filter((e) => e.type === 'damage' && e.ability === 'kaelen_q')).toHaveLength(3);
    expect(source.mana).toBe(0);
  });
  it('W blocks one champion hit and stuns a melee attacker', () => {
    const { world, source, target } = game('kaelen'); cast(world, source, 'W');
    expect(applyDamage(world, { source: target, target: source, amount: 100, type: 'physical' })).toBe(0);
    expect(hasCC(target, 'stun')).toBe(true); expect(getBuff(source, 'kaelen_w')).toBeUndefined();
    expect(applyDamage(world, { source: target, target: source, amount: 100, type: 'physical' })).toBeGreaterThan(0);
  });
  it('E dashes and scales its shield from bonus AD', () => {
    const { world, source, target } = game('kaelen');
    const off = registerStatSource((_w, e) => e === source ? { flat: { ad: 25 } } : undefined);
    try { world.step(); cast(world, source, 'E', target); expect(source.shield).toBeCloseTo(85); advance(world, .4); expect(source.pos.x).toBeGreaterThan(1100); expect(source.pos.x).toBeLessThanOrEqual(1250); advance(world, 2); expect(source.shield).toBe(0); } finally { off(); }
  });
  it('R deals more damage to a wounded champion', () => {
    const full = game('kaelen'); cast(full.world, full.source, 'R', full.target); advance(full.world, .5);
    const wounded = game('kaelen'); wounded.target.hp *= .5; cast(wounded.world, wounded.source, 'R', wounded.target); advance(wounded.world, .5);
    expect(damage(wounded.world, 'kaelen_r')).toBeCloseTo(damage(full.world, 'kaelen_r') * 1.5);
  });
  it('the third auto-attack deals 40% more damage and heals', () => {
    const { world, source, target } = game('kaelen'); source.hp -= 300;
    const first = auto(world, source, target); auto(world, source, target); const before = source.hp;
    const third = auto(world, source, target);
    expect(third).toBeCloseTo(first * 1.4); expect(source.hp - before).toBeCloseTo(third * .3);
  });
});
