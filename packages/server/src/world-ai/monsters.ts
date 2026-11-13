import { dist2, type MonsterType, type Vec2 } from '@lov/shared';
import { MONSTERS } from '../../../shared/src/data/monsters.ts';
import type { Entity, World } from '../world.ts';
import { attack, stats } from './runtime.ts';
export interface MonsterState { home: Vec2; leash: number; lastCombat: number; resetting: boolean; camp?: string; epic?: boolean }
const installed = new WeakSet<World>();
export function installMonsterHooks(world: World): void {
  if (installed.has(world)) return;
  installed.add(world);
  world.hooks.canAttack.push(entity => {
    const state = entity.data.monster as MonsterState | undefined;
    return !state || (!state.resetting && entity.subtype !== 'scuttle');
  });
}
export function spawnMonster(world: World, type: MonsterType, pos: Vec2, leash: number, extra: Partial<MonsterState> = {}): Entity {
  installMonsterHooks(world);
  const def = MONSTERS[type];
  const growth = 1 + Math.floor(world.time / 300) * 0.08;
  const hp = Math.round(def.hp * growth);
  return world.spawn({ kind: 'monster', subtype: type, team: 'neutral', pos: { ...pos }, radius: extra.epic ? 100 : 55, hp, mana: 0,
    stats: stats(hp, def.ad * growth, def.armor, def.range, type === 'scuttle' ? 200 : 300, 0.75),
    data: { pveCombat: true, xpReward: def.xp, xpGlobal: !!extra.epic, monster: { home: { ...pos }, leash, lastCombat: world.time, resetting: false, ...extra } satisfies MonsterState, rewards: { ...def, smiteRequired: !extra.epic, xpRadius: 1400 } } });
}
export function aggroMonster(world: World, target: Entity, source: Entity): void {
  const state = target.data.monster as MonsterState | undefined;
  if (!state || target.dead || state.resetting || source.dead || source.team === 'neutral') return;
  target.attackTarget = source.id; state.lastCombat = world.time;
}
export function updateMonster(world: World, entity: Entity, dt: number): boolean {
  const state = entity.data.monster as MonsterState;
  if (entity.dead) return false;
  let target = world.get(entity.attackTarget);
  if (target?.dead || (target && dist2(target.pos, state.home) > (state.leash + 250) ** 2)) target = undefined;
  const tooFar = dist2(entity.pos, state.home) > state.leash ** 2;
  if (tooFar || (!target && entity.hp < entity.stats.maxHp && world.time - state.lastCombat >= 5)) state.resetting = true;
  if (state.resetting) {
    entity.attackTarget = undefined; entity.moveTarget = state.home;
    entity.hp = Math.min(entity.stats.maxHp, entity.hp + entity.stats.maxHp * dt * 0.25);
    if (dist2(entity.pos, state.home) < 60 ** 2 && entity.hp === entity.stats.maxHp) { state.resetting = false; entity.pos = { ...state.home }; entity.moveTarget = undefined; }
    return false;
  }
  if (!target) { entity.attackTarget = undefined; entity.moveTarget = undefined; return false; }
  state.lastCombat = world.time;
  return attack(world, entity, target);
}
