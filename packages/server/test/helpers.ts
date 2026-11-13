import type { Stats, Team, Vec2 } from '@lov/shared';
import { combatSystem } from '../src/systems/combat.ts';
import { inputSystem } from '../src/systems/input.ts';
import { movementSystem } from '../src/systems/movement.ts';
import { visionSystem } from '../src/systems/vision.ts';
import { World, createChampion, type Entity } from '../src/world.ts';

/** Monde avec uniquement les systèmes de claude-server (input, movement, combat, vision). */
export function coreWorld(): World {
  const w = new World([inputSystem, movementSystem, combatSystem, visionSystem]);
  w.init();
  return w;
}

export const BASE_STATS: Stats = {
  maxHp: 1000, hpRegen: 0, maxMana: 0, manaRegen: 0, ad: 100, ap: 0, armor: 0, mr: 0, attackSpeed: 1,
  attackRange: 150, moveSpeed: 300, critChance: 0, abilityHaste: 0, lifesteal: 0, armorPen: 0, magicPen: 0,
};

let n = 0;
export function champ(world: World, team: Team, pos: Vec2, championId = 'grondak'): Entity {
  return createChampion(world, { playerId: `p${++n}`, name: 'T', team, championId, summoners: ['flash', 'ignite'], pos });
}

export function dummy(world: World, team: Team | 'neutral', pos: Vec2, stats: Partial<Stats> = {}, kind: Entity['kind'] = 'minion'): Entity {
  const s = { ...BASE_STATS, ...stats };
  return world.spawn({ kind, team, pos: { ...pos }, radius: 40, hp: s.maxHp, mana: 0, stats: s });
}

export function run(world: World, seconds: number): void {
  const ticks = Math.round(seconds * 30);
  for (let i = 0; i < ticks; i++) world.step();
}
