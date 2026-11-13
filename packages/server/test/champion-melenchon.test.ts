import { describe, expect, it } from 'vitest';
import { getChampion } from '@lov/shared';
import { addBuff, getBuff, hasCC, removeBuff } from '../src/combat/buffs.ts';
import { castAbility } from '../src/abilities/engine.ts';
import { applyDamage } from '../src/combat/damage.ts';
import { advance, cast, damage, dummy, game } from './champion-helpers.ts';

describe('Mélenchon, le Tribun indigné', () => {
  it('is an eleventh playable champion with the requested ultimate', () => {
    expect(getChampion('melenchon').abilities.R.name).toBe('La République, c’est moi !');
    expect(getChampion('melenchon').lore).toContain('satirique');
  });
  it('Q asks the first enemy a forceful question, slowing only that target', () => {
    const { world, source, target } = game('melenchon');
    const behind = dummy(world, source, { x: 1600, y: 1000 });
    expect(cast(world, source, 'Q', target)).toBe(true); advance(world, .6);
    expect(damage(world, 'melenchon_q', target)).toBeGreaterThan(0);
    expect(damage(world, 'melenchon_q', behind)).toBe(0); expect(hasCC(target, 'slow')).toBe(true);
    expect(source.champ!.cooldowns.Q).toBeGreaterThan(6);
  });
  it('W announces its motion before damaging and silencing enemies', () => {
    const { world, source, target } = game('melenchon'); cast(world, source, 'W', target);
    advance(world, .7); expect(damage(world, 'melenchon_w')).toBe(0);
    expect(world.events.pending.some((e) => e.type === 'areaEffect' && e.fxKey === 'melenchon_w_telegraph')).toBe(true);
    advance(world, .1); expect(damage(world, 'melenchon_w')).toBeGreaterThan(0); expect(hasCC(target, 'silence')).toBe(true);
  });
  it('E leaves a delayed hologram at departure and shields the relocated caster', () => {
    const { world, source, target } = game('melenchon'); target.pos.x = 1100;
    cast(world, source, 'E', undefined, { x: 1400, y: 1000 }); expect(source.pos.x).toBe(1400); expect(source.shield).toBe(45);
    advance(world, 1.1); expect(damage(world, 'melenchon_e')).toBe(0);
    advance(world, .2); expect(damage(world, 'melenchon_e')).toBeGreaterThan(0);
    advance(world, 1); expect(source.shield).toBe(0);
  });
  it('R delivers its declaration after windup, taunts enemies and grants a four-second shield', () => {
    const { world, source, target } = game('melenchon'); const expected = source.stats.maxHp * .2;
    cast(world, source, 'R'); advance(world, .5); expect(damage(world, 'melenchon_r')).toBe(0);
    advance(world, .2); expect(damage(world, 'melenchon_r')).toBeGreaterThan(0); expect(hasCC(target, 'taunt')).toBe(true);
    expect(source.shield).toBeCloseTo(expected); expect(getBuff(source, 'melenchon_r')).toBeDefined();
    advance(world, 4.1); expect(source.shield).toBe(0);
  });
  it('R windup can be interrupted: being the Republic does not bypass CC', () => {
    const { world, source, target } = game('melenchon'); cast(world, source, 'R'); addBuff(world, source, 'stun', { duration: 1 });
    advance(world, 1); expect(damage(world, 'melenchon_r')).toBe(0); expect(source.shield).toBe(0); expect(hasCC(target, 'taunt')).toBe(false);
  });
  it('requires three spaced champion spell hits to trigger Indignation', () => {
    const { world, source, target } = game('melenchon');
    for (let i = 0; i < 3; i++) {
      applyDamage(world, { source, target, amount: 1, type: 'true', ability: 'melenchon_q' });
      if (i < 2) advance(world, .55);
    }
    expect(getBuff(source, 'melenchon_indignation')).toBeUndefined();
    expect(source.shield).toBeCloseTo(35 + source.champ!.level * 5);
    expect(source.stats.moveSpeed).toBeCloseTo(325 * 1.18);
  });
  it('does not stack its passive from one AoE, autos or minions', () => {
    const { world, source, target } = game('melenchon');
    for (let i = 0; i < 5; i++) applyDamage(world, { source, target, amount: 1, type: 'true', ability: 'melenchon_r' });
    expect(getBuff(source, 'melenchon_indignation')?.stacks).toBe(1);
    advance(world, .55);
    applyDamage(world, { source, target, amount: 1, type: 'true', isAutoAttack: true });
    applyDamage(world, { source, target: dummy(world, source), amount: 1, type: 'true', ability: 'melenchon_q' });
    expect(getBuff(source, 'melenchon_indignation')?.stacks).toBe(1);
  });
  it('rejects a rooted hologram and a locked ultimate', () => {
    const { world, source } = game('melenchon', 1);
    expect(cast(world, source, 'R')).toBe(false);
    addBuff(world, source, 'root', { duration: 1 }); expect(cast(world, source, 'E')).toBe(false);
    removeBuff(world, source, 'root'); expect(castAbility(world, source, { t: 'cast', slot: 'E', targetPos: { x: NaN, y: 1000 } })).toBe(false);
  });
});
