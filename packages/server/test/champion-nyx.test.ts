import { describe, expect, it } from 'vitest';
import { getBuff, hasCC } from '../src/combat/buffs.ts';
import { applyDamage } from '../src/combat/damage.ts';
import { game, advance, cast, damage, dummy, auto } from './champion-helpers.ts';
describe('Nyx', () => {
  it('Q uses energy and immediately resets on a lethal hit', () => {
    const { world, source, target } = game('nyx'); target.hp = 1; cast(world, source, 'Q', target);
    expect(target.dead).toBe(true); expect(source.champ!.cooldowns.Q).toBe(0); expect(source.mana).toBe(160);
  });
  it('Q also resets when another source kills its victim in the window', () => {
    const { world, source, target } = game('nyx'); cast(world, source, 'Q', target);
    applyDamage(world, { source: target, target, amount: 10000, type: 'true' }); expect(source.champ!.cooldowns.Q).toBe(0);
  });
  it('W invisibility ends on attacking or casting', () => {
    const { world, source, target } = game('nyx'); cast(world, source, 'W'); expect(source.data.invisible).toBe(true);
    auto(world, source, target); expect(source.data.invisible).toBe(false);
    source.champ!.cooldowns.W = 0; source.mana = 200; cast(world, source, 'W'); cast(world, source, 'Q', target); expect(source.data.invisible).toBe(false);
  });
  it('E blinks behind enemies and slows them, without hurting allies', () => {
    const { world, source, target } = game('nyx'); cast(world, source, 'E', target);
    expect(source.pos.x).toBeLessThan(target.pos.x); expect(damage(world, 'nyx_e')).toBeGreaterThan(0); expect(hasCC(target, 'slow')).toBe(true);
    const ally = dummy(world, source, { x: 1500, y: 1000 }); ally.team = 'blue'; source.champ!.cooldowns.E = 0; source.mana = 200;
    expect(cast(world, source, 'E', ally)).toBe(true); expect(damage(world, 'nyx_e', ally)).toBe(0);
  });
  it('R records actual damage and detonates after three seconds', () => {
    const { world, source, target } = game('nyx'); cast(world, source, 'R', target);
    applyDamage(world, { source: target, target, amount: 100, type: 'true', ability: 'test' });
    expect(getBuff(target, 'nyx_r', source.id)?.params.stored).toBe(100); advance(world, 2.9); expect(damage(world, 'nyx_r')).toBe(0);
    advance(world, .2); expect(damage(world, 'nyx_r')).toBeCloseTo((100 + 100 * .3) * 1.2 * 100 / (100 + target.stats.armor));
    expect(getBuff(target, 'nyx_r')).toBeUndefined();
  });
  it('passive amplifies damage only against isolated targets', () => {
    const isolated = game('nyx'); const bonus = auto(isolated.world, isolated.source, isolated.target);
    const grouped = game('nyx'); dummy(grouped.world, grouped.source, { x: 1300, y: 1000 }); const normal = auto(grouped.world, grouped.source, grouped.target);
    expect(bonus).toBeCloseTo(normal * 1.2);
  });
});
