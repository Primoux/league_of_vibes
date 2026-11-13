import { describe, expect, it } from 'vitest';
import type { SummonerId } from '@lov/shared';
import { addBuff, getBuff, hasCC } from '../src/combat/buffs.ts';
import { applyDamage, applyHeal } from '../src/combat/damage.ts';
import { castSummoner } from '../src/systems/summoners.ts';
import { isWalkable } from '../src/abilities/primitives.ts';
import { advance, damage, dummy, game } from './champion-helpers.ts';
const use = (g: ReturnType<typeof game>, id: SummonerId, target = g.target, point = { x: 1400, y: 1000 }) => {
  g.source.champ!.summoners.D = id;
  return castSummoner(g.world, g.source, { t: 'summoner', slot: 'D', targetId: target.id, targetPos: point });
};
describe('Summoner spells', () => {
  it('Flash clamps to 400 and finds a radius-safe landing near walls', () => {
    const g = game('ilyra');
    expect(use(g, 'flash', g.target, { x: 10000, y: 1000 })).toBe(true);
    expect(g.source.pos.x).toBe(1400); expect(g.source.champ!.cooldowns.D).toBe(300);
    expect(use(g, 'flash')).toBe(false);
    g.source.champ!.cooldowns.D = 0; g.source.pos = { x: 1900, y: 3900 };
    expect(use(g, 'flash', g.target, { x: 2300, y: 3900 })).toBe(true);
    expect(isWalkable(g.source.pos, g.source.radius)).toBe(true);
    expect(Math.hypot(g.source.pos.x - 1900, g.source.pos.y - 3900)).toBeLessThanOrEqual(400.001);
  });
  it('Ignite deals all five true-damage ticks and reduces healing', () => {
    const g = game('ilyra'); use(g, 'ignite');
    g.target.hp -= 100;
    expect(applyHeal(g.world, { source: g.target, target: g.target, amount: 100 })).toBe(60);
    advance(g.world, 5.1); expect(damage(g.world, 'sum_ignite')).toBeCloseTo(170);
  });
  it('Heal restores health and grants speed; Barrier expires', () => {
    const g = game('ilyra'); g.source.hp -= 300; use(g, 'heal');
    expect(g.source.hp).toBeCloseTo(g.source.stats.maxHp - 130);
    expect(g.source.stats.moveSpeed).toBeCloseTo(330 * 1.3);
    g.source.champ!.cooldowns.D = 0; use(g, 'barrier');
    expect(g.source.shield).toBe(225); advance(g.world, 2.6); expect(g.source.shield).toBe(0);
  });
  it('Exhaust reduces outgoing damage and slows its victim', () => {
    const g = game('ilyra'); use(g, 'exhaust');
    const reduced = applyDamage(g.world, { source: g.target, target: g.source, amount: 100, type: 'physical' });
    expect(reduced).toBeCloseTo(60 * 100 / (100 + g.source.stats.armor));
    expect(hasCC(g.target, 'slow')).toBe(true);
  });
  it('Smite accepts only minions/monsters, deals 600, and Ghost grants speed', () => {
    const g = game('ilyra'); expect(use(g, 'smite')).toBe(false);
    const minion = dummy(g.world, g.source); expect(use(g, 'smite', minion)).toBe(true);
    expect(damage(g.world, 'sum_smite')).toBe(600);
    g.source.champ!.cooldowns.D = 0; use(g, 'ghost'); expect(g.source.data.ghosted).toBe(true);
    expect(g.source.stats.moveSpeed).toBeGreaterThan(330);
  });
  it('Cleanse removes eligible CC, keeps knockups and grants 65% tenacity', () => {
    const g = game('ilyra'); addBuff(g.world, g.source, 'stun', { duration: 3 }); addBuff(g.world, g.source, 'knockup', { duration: 3 });
    expect(use(g, 'cleanse')).toBe(true);
    expect(hasCC(g.source, 'stun')).toBe(false); expect(hasCC(g.source, 'knockup')).toBe(true);
    addBuff(g.world, g.source, 'root', { duration: 2 }); expect(getBuff(g.source, 'root')!.remaining).toBeCloseTo(.7);
  });
  it('Teleport channels four seconds to a living ally and cancels on damage', () => {
    const g = game('ilyra');
    const tower = dummy(g.world, g.source, { x: 8000, y: 8000 }); tower.kind = 'tower'; tower.team = 'blue';
    expect(use(g, 'teleport', tower)).toBe(true); advance(g.world, 3.9); expect(g.source.pos.x).toBe(1000);
    advance(g.world, .2); expect(g.source.pos.x).toBeGreaterThan(7900);
    g.source.champ!.cooldowns.D = 0; use(g, 'teleport', tower);
    applyDamage(g.world, { source: g.target, target: g.source, amount: 1, type: 'true' }); advance(g.world, .1);
    expect(g.source.data.channeling).toBeUndefined();
  });
});
