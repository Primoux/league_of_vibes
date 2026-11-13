import { TICK_DT, type AbilitySlot, type Stats } from '@lov/shared';
import { World, createChampion, type Entity } from '../src/world.ts';
import { buffsSystem } from '../src/systems/buffs.ts';
import { summonersSystem } from '../src/systems/summoners.ts';
import { abilitiesSystem } from '../src/systems/abilities.ts';
import { movementSystem } from '../src/systems/movement.ts';
import { grantXP, xpForLevel } from '../src/systems/leveling.ts';
import { castAbility } from '../src/abilities/engine.ts';
import { applyDamage } from '../src/combat/damage.ts';
const hitsByWorld = new WeakMap<World, { ability?: string; target: number; amount: number }[]>();
export function game(id: string, level = 6) {
  const world = new World([buffsSystem, summonersSystem, abilitiesSystem, movementSystem]);
  const source = createChampion(world, { playerId: 'p1', name: id, championId: id, team: 'blue', summoners: ['flash', 'ignite'], pos: { x: 1000, y: 1000 } });
  const target = createChampion(world, { playerId: 'p2', name: 'Target', championId: 'sable', team: 'red', summoners: ['flash', 'ignite'], pos: { x: 1250, y: 1000 } });
  grantXP(world, source, xpForLevel(level)); grantXP(world, target, xpForLevel(level));
  source.champ!.abilityLevels = { Q: 1, W: 1, E: 1, R: level >= 6 ? 1 : 0 };
  world.init(); world.events.drain();
  const hits: { ability?: string; target: number; amount: number }[] = [];
  hitsByWorld.set(world, hits);
  world.hooks.afterDamage.push((ctx, amount) => { hits.push({ ability: ctx.ability, target: ctx.target.id, amount }); });
  return { world, source, target };
}
export function advance(world: World, seconds: number): void {
  const until = world.time + seconds;
  while (world.time + 1e-8 < until) world.step(Math.min(TICK_DT, until - world.time));
}
export function cast(world: World, source: Entity, slot: AbilitySlot, target?: Entity, point = target?.pos ?? { x: 1400, y: 1000 }): boolean {
  return castAbility(world, source, { t: 'cast', slot, targetId: target?.id, targetPos: { ...point } });
}
export function auto(world: World, source: Entity, target: Entity, crit = false): number {
  world.events.emit({ type: 'autoAttack', source: source.id, target: target.id, ranged: false, crit });
  return applyDamage(world, { source, target, amount: source.stats.ad * (crit ? 1.75 : 1), type: 'physical', crit, isAutoAttack: true });
}
export function damage(world: World, ability?: string, target?: Entity): number {
  return (hitsByWorld.get(world) ?? []).filter((ev) => (!ability || ev.ability === ability) && (!target || ev.target === target.id)).reduce((sum, ev) => sum + ev.amount, 0);
}
export function dummy(world: World, source: Entity, pos = { x: 1400, y: 1000 }, stats: Partial<Stats> = {}): Entity {
  return world.spawn({ kind: 'minion', team: 'red', pos, radius: 30, hp: 10000, mana: 0, stats: { ...source.stats, maxHp: 10000, armor: 0, mr: 0, ...stats } });
}
