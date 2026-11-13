import { describe, expect, it } from 'vitest';
import { World, createChampion } from '../src/world.ts';
import { levelingSystem, grantXP, xpForLevel, levelAbility, cooldownWithHaste } from '../src/systems/leveling.ts';
import { applyDamage } from '../src/combat/damage.ts';
const setup = () => {
  const world = new World([levelingSystem]);
  const e = createChampion(world, { playerId: 'p', name: 'Test', team: 'blue', championId: 'grondak', summoners: ['flash', 'ignite'], pos: { x: 1000, y: 1000 } });
  world.init();
  return { world, e };
};
describe('Leveling', () => {
  it('uses the complete XP curve and unlocks R only at 6/11/16', () => {
    const { world, e } = setup();
    expect(levelAbility(e, 'R')).toBe(false);
    grantXP(world, e, xpForLevel(6));
    expect(e.champ!.level).toBe(6);
    expect(e.champ!.skillPoints).toBe(6);
    expect(levelAbility(e, 'R')).toBe(true);
    expect(levelAbility(e, 'R')).toBe(false);
    grantXP(world, e, xpForLevel(18));
    expect(e.champ!.level).toBe(18);
    expect(e.champ!.xp).toBe(0);
    expect(world.events.pending.filter((ev) => ev.type === 'levelUp')).toHaveLength(17);
  });
  it('enforces basic ability ranks and reduces cooldown with haste', () => {
    const { world, e } = setup();
    expect(levelAbility(e, 'Q')).toBe(true);
    grantXP(world, e, 280);
    expect(levelAbility(e, 'Q')).toBe(false);
    grantXP(world, e, 380);
    expect(levelAbility(e, 'Q')).toBe(true);
    expect(cooldownWithHaste(10, 100)).toBe(5);
  });
  it('shares nearby minion XP once and excludes dead/out-of-range champions', () => {
    const { world, e } = setup();
    const ally = createChampion(world, { playerId: 'a', name: 'Ally', team: 'blue', championId: 'ilyra', summoners: ['flash', 'ignite'], pos: { x: 1200, y: 1000 } });
    const enemy = world.spawn({ kind: 'minion', team: 'red', pos: { x: 1100, y: 1000 }, hp: 1, mana: 0, radius: 30, stats: { ...e.stats }, data: { xpReward: 100 } });
    applyDamage(world, { source: e, target: enemy, amount: 1, type: 'true' });
    world.events.emit({ type: 'death', target: enemy.id, killer: e.id });
    expect(e.champ!.xp).toBe(50);
    expect(ally.champ!.xp).toBe(50);
  });
});
