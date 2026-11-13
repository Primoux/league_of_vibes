import { afterAll, describe, expect, it } from 'vitest';
import { CHAMPION_LIST, TICK_DT, dist, getChampion, type AbilitySlot, type ChampionClass } from '@lov/shared';
import { World, createChampion, type Entity } from '../src/world.ts';
import { createSystems } from '../src/systems/index.ts';
import { canAttack, canMove, hasCC, recalculateStats } from '../src/combat/buffs.ts';
import { castAbility } from '../src/abilities/engine.ts';
import { grantXP, xpForLevel } from '../src/systems/leveling.ts';
import { cast, game, advance } from './champion-helpers.ts';

const LOADOUTS: Record<ChampionClass, string[]> = {
  tank: ['ruby_crystal', 'ruby_crystal', 'boots'],
  fighter: ['long_sword', 'ruby_crystal', 'boots'],
  mage: ['amplifying_tome', 'amplifying_tome', 'boots'],
  marksman: ['long_sword', 'long_sword', 'boots'],
  assassin: ['long_sword', 'long_sword', 'boots'],
  support: ['amplifying_tome', 'ruby_crystal', 'boots'],
};
interface DuelResult { winner: string | null; seconds: number; damage: [number, number]; hp: [number, number] }
/** Deterministic integration harness using the real combat, movement, vision and champion systems.
 * Standard class loadouts have no random crit chance; both AIs alternate initiative each tick. */
export function simulateDuel(a: string, b: string, level: number): DuelResult {
  const world = new World(createSystems());
  const champions = [a, b].map((id, i) => {
    const e = createChampion(world, { playerId: `p${i}`, name: id, championId: id, team: i ? 'red' : 'blue', summoners: ['flash', 'ignite'], pos: { x: 6000 + i * 400, y: 1000 } });
    grantXP(world, e, xpForLevel(level));
    e.champ!.abilityLevels = level === 6 ? { Q: 3, W: 1, E: 1, R: 1 } : { Q: 5, W: 3, E: 1, R: 2 };
    e.champ!.skillPoints = 0; e.champ!.items = [...LOADOUTS[getChampion(id).class], null, null, null];
    recalculateStats(world, e); e.hp = e.stats.maxHp; e.mana = e.stats.maxMana;
    return e;
  }) as [Entity, Entity];
  world.init();
  for (let tick = 0; tick < 30 * 35 && champions.every((e) => !e.dead); tick++) {
    // Alternate initiative per tick to avoid giving one side a permanent lethal-hit priority.
    for (const index of tick % 2 ? [1, 0] : [0, 1]) {
      const source = champions[index]!, target = champions[1 - index]!;
      if (source.dead || target.dead) continue;
      for (const slot of ['R', 'Q', 'W', 'E'] as AbilitySlot[]) {
        const def = getChampion(source.champ!.championId).abilities[slot];
        const ally = (source.champ!.championId === 'ilyra' && slot === 'E') || (source.champ!.championId === 'solenne' && ['W', 'E'].includes(slot));
        if (castAbility(world, source, { t: 'cast', slot, targetId: ally ? source.id : target.id, targetPos: { ...target.pos } }, false)) break;
        if (def.targeting === 'self') continue;
      }
      if (canAttack(source) && !source.data.dash) source.attackTarget = target.id;
      else if (canMove(source) && !source.data.dash && dist(source.pos, target.pos) > source.stats.attackRange + source.radius + target.radius) source.moveTarget = { ...target.pos };
    }
    world.step(TICK_DT);
    world.events.drain(); // Simulate snapshots; do not retain unbounded events in long duels.
    for (const e of champions) {
      if (!Number.isFinite(e.hp + e.mana + e.stats.moveSpeed + e.shield)) throw new Error(`Non-finite state: ${a}/${b}`);
      if (e.hp < 0 || e.mana < -1e-6 || e.shield < 0 || e.hp > e.stats.maxHp + 1e-6) throw new Error(`Invalid resources: ${a}/${b}`);
    }
  }
  return { winner: champions[0].dead ? b : champions[1].dead ? a : null, seconds: world.time, damage: [champions[0].champ!.damageToChampions, champions[1].champ!.damageToChampions], hp: [champions[0].hp, champions[1].hp] };
}
const reports: { a: string; b: string; level: number; result: DuelResult }[] = [];
afterAll(() => {
  if (process.env.LOV_BALANCE_REPORT !== '1') return;
  const report = CHAMPION_LIST.map((champion) => {
    const games = reports.filter((r) => r.a === champion.id || r.b === champion.id);
    return { champion: champion.id, wins: games.filter((r) => r.result.winner === champion.id).length, draws: games.filter((r) => !r.result.winner).length, games: games.length, maxDps: Math.round(Math.max(...games.map((r) => r.result.damage[r.a === champion.id ? 0 : 1] / r.result.seconds))) };
  });
  console.info('BALANCE_REPORT', JSON.stringify(report));
});
const pairs = CHAMPION_LIST.flatMap((a, i) => CHAMPION_LIST.slice(i + 1).flatMap((b) => [6, 11].map((level) => [a.id, b.id, level] as const)));
describe('Champion balance guardrails', () => {
  it.each(pairs)('%s vs %s at level %i with class loadouts', (a, b, level) => {
    const duel = simulateDuel(a, b, level);
    reports.push({ a, b, level, result: duel });
    expect(duel.seconds).toBeGreaterThan(1);
    expect(duel.damage[0] + duel.damage[1]).toBeGreaterThan(0);
    expect(duel.damage[0] / duel.seconds).toBeLessThan(1500);
    expect(duel.damage[1] / duel.seconds).toBeLessThan(1500);
    expect(duel.hp.every(Number.isFinite)).toBe(true);
  });
  it.each(CHAMPION_LIST.flatMap((c) => (['Q', 'W', 'E', 'R'] as const).map((slot) => [c.id, slot] as const)))('%s %s consumes its data-defined cost and cooldown once', (id, slot) => {
    const { world, source, target } = game(id);
    const def = getChampion(id).abilities[slot], mana = source.mana;
    const ally = (id === 'ilyra' && slot === 'E') || (id === 'solenne' && ['W', 'E'].includes(slot));
    expect(cast(world, source, slot, ally ? source : target)).toBe(true);
    expect(source.champ!.cooldowns[slot]).toBe(def.cooldown[0]);
    expect(source.mana).toBeCloseTo(mana - def.cost[0]!);
    advance(world, .1);
    expect(source.champ!.cooldowns[slot]).toBeCloseTo(def.cooldown[0]! - .1);
  });
  it('repeats the same duel deterministically', () => { expect(simulateDuel('nyx', 'ilyra', 11)).toEqual(simulateDuel('nyx', 'ilyra', 11)); });
});
