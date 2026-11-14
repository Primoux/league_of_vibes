import { TIMERS, dist2, lanePathFor, otherTeam, scaled, type Lane, type MinionType, type Vec2 } from '@lov/shared';
import { MINIONS } from '../../../shared/src/data/minions.ts';
import type { Entity, System, World } from '../world.ts';
import { attack, inRange, isStructure, retire, stats } from '../world-ai/runtime.ts';
import { isStructureAttackable, structureState } from './structures.ts';
interface MinionState { lane: Lane; path: readonly Vec2[]; waypoint: number; aggro?: number; aggroUntil: number }
const waves = new WeakMap<World, { next: number; count: number }>();
export function spawnWave(world: World, count: number): void {
  for (const team of ['blue', 'red'] as const) for (const lane of ['top', 'mid', 'bot'] as const) {
    const composition: MinionType[] = ['melee', 'melee', 'melee', 'caster', 'caster', 'caster'];
    const every = world.time >= scaled(TIMERS.cannonLateAt) ? TIMERS.cannonEveryNWavesLate : TIMERS.cannonEveryNWaves;
    if (count % every === 0) composition.push('cannon');
    if ([...world.query('inhibitor')].some(e => e.team === otherTeam(team) && e.dead && structureState(e)?.lane === lane)) composition.push('super');
    const path = lanePathFor(lane, team);
    const start = path[0]!; const next = path[1]!;
    const length = Math.hypot(next.x - start.x, next.y - start.y);
    const growth = Math.floor(world.time / scaled(90));
    composition.forEach((type, index) => {
      const def = MINIONS[type]; const hp = def.hp + growth * (type === 'cannon' ? 45 : 20);
      world.spawn({ kind: 'minion', subtype: type, team, radius: 35, pos: { x: start.x - (next.x - start.x) / length * index * 75, y: start.y - (next.y - start.y) / length * index * 75 }, hp, mana: 0,
        stats: stats(hp, def.ad + growth * 1.5, def.armor + growth, def.range, def.speed, def.attackSpeed),
        data: { pveCombat: true, xpReward: def.xp, minion: { lane, path, waypoint: 1, aggroUntil: 0 } satisfies MinionState, rewards: { gold: def.gold + (type === 'cannon' ? Math.min(30, growth * 3) : growth * 0.2), xp: def.xp, lastHitOnly: true, xpRadius: 1400 } } });
    });
  }
}
function validTarget(world: World, source: Entity, target: Entity): boolean {
  return !target.dead && target.team !== source.team && target.team !== 'neutral' && (target.kind === 'champion' || target.kind === 'minion' || (isStructure(target) && isStructureAttackable(world, target)));
}
/** Une seule passe, distance avant les règles de structure (qui sont plus coûteuses). */
function acquireTarget(world: World, source: Entity): Entity | undefined {
  let best: Entity | undefined;
  let priority = 3; let distance = 650 ** 2;
  for (const candidate of world.entities.values()) {
    if (candidate.dead || candidate.team === source.team || candidate.team === 'neutral') continue;
    const d = dist2(source.pos, candidate.pos);
    if (d > 650 ** 2 || !validTarget(world, source, candidate)) continue;
    const attackingAlly = world.get(candidate.attackTarget)?.team === source.team;
    const rank = attackingAlly && candidate.kind === 'minion' ? 0 : attackingAlly && candidate.kind === 'champion' ? 1 : 2;
    if (rank < priority || (rank === priority && (d < distance || (d === distance && (!best || candidate.id < best.id))))) {
      best = candidate; priority = rank; distance = d;
    }
  }
  return best;
}
export const minionsSystem: System = {
  name: 'minions',
  init(world) {
    waves.set(world, { next: scaled(TIMERS.minionFirstWave), count: 0 });
    world.hooks.afterDamage.push((ctx, dealt) => {
      if (dealt <= 0) return;
      const source = ctx.credit ?? ctx.source; const target = ctx.target;
      if (!source || !target || source.team === target.team || source.kind !== 'champion' || target.kind !== 'champion') return;
      for (const e of world.query('minion')) {
        if (e.dead || !e.data.minion || e.team !== target.team || !inRange(e, source, 600)) continue;
        const state = e.data.minion as MinionState; state.aggro = source.id; state.aggroUntil = world.time + 3;
      }
    });
  },
  update(world) {
    if (world.winner) return;
    const state = waves.get(world);
    if (state && world.time >= state.next) { state.count++; spawnWave(world, state.count); state.next += scaled(TIMERS.minionWaveInterval); }
    for (const e of world.query('minion')) {
      if (!e.data.minion) continue;
      if (e.dead) { retire(world, e); continue; }
      const minion = e.data.minion as MinionState;
      let target = minion.aggroUntil > world.time ? world.get(minion.aggro) : undefined;
      if (target && (!validTarget(world, e, target) || !inRange(e, target, 800))) target = undefined;
      target ??= acquireTarget(world, e);
      if (target) { attack(world, e, target); continue; }
      e.attackTarget = undefined;
      const path = minion.path;
      let waypoint = path[minion.waypoint];
      if (waypoint && Math.hypot(e.pos.x - waypoint.x, e.pos.y - waypoint.y) < 50) waypoint = path[++minion.waypoint];
      e.moveTarget = waypoint;
    }
  },
};
