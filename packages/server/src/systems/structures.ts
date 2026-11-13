import { INHIBITORS, NEXUS, TEAMS, TIMERS, TOWERS, dist2, otherTeam, scaled, type Lane, type Team, type TowerTier } from '@lov/shared';
import { STRUCTURES } from '../../../shared/src/data/structures.ts';
import type { Entity, System, World } from '../world.ts';
import { attack, inRange, isStructure, stats, usesCoreCombat, type DamageRequest } from '../world-ai/runtime.ts';

export interface StructureState { lane: Lane | 'base'; tier?: TowerTier; plates: number; respawnAt?: number; destroyed: boolean; rampTarget?: number; ramp: number; bulwarkUntil: number; bulwark: number; attackableTick?: number; attackableRevision?: number; attackable?: boolean }
const structureRevisions = new WeakMap<World, number>();
const invalidateStructures = (world: World): void => { structureRevisions.set(world, (structureRevisions.get(world) ?? 0) + 1); };
const structureLists = new WeakMap<World, { all: Entity[]; inhibitors: Entity[] }>();
export const structureState = (e: Entity): StructureState => e.data.structure as StructureState;
export function isStructureAttackable(world: World, target: Entity): boolean {
  if (target.dead) return false;
  const state = structureState(target);
  if (!state) return true;
  const revision = structureRevisions.get(world) ?? 0;
  if (state.attackableTick === world.tick && state.attackableRevision === revision) return state.attackable!;
  state.attackable = computeAttackable(world, target, state);
  state.attackableTick = world.tick; state.attackableRevision = revision;
  return state.attackable;
}
function computeAttackable(world: World, target: Entity, state: StructureState): boolean {
  if (target.kind === 'tower' && state.tier === 'outer') return true;
  for (const e of structureLists.get(world)?.all ?? world.entities.values()) {
    if (e.team !== target.team || e.dead || e.id === target.id) continue;
    const previous = structureState(e);
    if (!previous) continue;
    if (state.lane !== 'base' && previous.lane === state.lane && e.kind === 'tower') {
      if (target.kind === 'inhibitor' || (state.tier === 'inner' && previous.tier === 'outer') || (state.tier === 'inhib' && previous.tier !== 'nexus' && previous.tier !== 'inhib')) return false;
    }
    if (state.lane === 'base') {
      if (target.kind === 'nexus' && e.kind === 'tower' && previous.tier === 'nexus') return false;
    }
  }
  if (state.lane === 'base') {
    for (const inhibitor of structureLists.get(world)?.inhibitors ?? world.query('inhibitor')) if (inhibitor.team === target.team && inhibitor.dead) return true;
    return false;
  }
  return true;
}
/** À appeler par l'API centrale avant réduction des résistances. */
export function structureDamageMultiplier(world: World, request: DamageRequest): number {
  const { target } = request;
  if (!isStructureAttackable(world, target)) return 0;
  return target.kind === 'tower' && target.subtype === 'outer' && world.time < scaled(STRUCTURES.fortificationUntil) ? 0.5 : 1;
}
function spawnStructures(world: World): void {
  const lists = { all: [] as Entity[], inhibitors: [] as Entity[] }; structureLists.set(world, lists);
  const spawn = (kind: 'tower' | 'inhibitor' | 'nexus', team: Team, lane: Lane | 'base', pos: { x: number; y: number }, tier?: TowerTier) => {
    const def = tier ? STRUCTURES.towers[tier] : undefined;
    const hp = def?.hp ?? (kind === 'inhibitor' ? STRUCTURES.inhibitorHp : STRUCTURES.nexusHp);
    const e = world.spawn({ kind, team, pos: { ...pos }, subtype: tier ?? kind, radius: kind === 'nexus' ? 220 : 100, hp, mana: 0,
      stats: stats(hp, def?.ad ?? 0, def?.armor ?? 20, kind === 'tower' ? STRUCTURES.range : 0, 0, 1.1),
      data: { pveCombat: true, structure: { lane, tier, plates: tier === 'outer' ? 5 : 0, destroyed: false, ramp: 0, bulwark: 0, bulwarkUntil: 0 } satisfies StructureState,
        rewards: { localGold: def?.localGold ?? 0, teamGold: def?.teamGold ?? 0 } } });
    lists.all.push(e); if (kind === 'inhibitor') lists.inhibitors.push(e);
    return e;
  };
  for (const t of TOWERS) spawn('tower', t.team, t.lane, t.pos, t.tier);
  for (const i of INHIBITORS) spawn('inhibitor', i.team, i.lane, i.pos);
  for (const team of TEAMS) spawn('nexus', team, 'base', NEXUS[team]);
}
function towerDamage(tower: Entity, target: Entity): number {
  return target.kind === 'minion' ? target.stats.maxHp * (target.subtype === 'cannon' || target.subtype === 'super' ? 0.14 : target.subtype === 'caster' ? 0.7 : 0.45) : STRUCTURES.towers[structureState(tower).tier!].ad * (1 + structureState(tower).ramp * 0.4);
}
function acquireTowerTarget(world: World, tower: Entity): Entity | undefined {
  let best: Entity | undefined; let minion = false; let distance = STRUCTURES.range ** 2;
  for (const candidate of world.entities.values()) {
    if (candidate.dead || candidate.team === tower.team || candidate.team === 'neutral' || (candidate.kind !== 'minion' && candidate.kind !== 'champion')) continue;
    const d = dist2(tower.pos, candidate.pos);
    if (d > STRUCTURES.range ** 2) continue;
    const preferred = candidate.kind === 'minion';
    if ((!minion && preferred) || (preferred === minion && (d < distance || (d === distance && (!best || candidate.id < best.id))))) {
      best = candidate; minion = preferred; distance = d;
    }
  }
  return best;
}
export const structuresSystem: System = {
  name: 'structures',
  init(world) {
    spawnStructures(world);
    world.hooks.beforeDamage.push(ctx => {
      if (isStructure(ctx.target)) {
        if (ctx.source?.kind === 'minion') ctx.multiplier *= ctx.source.subtype === 'super' ? 0.6 : 0.35;
        if (!isStructureAttackable(world, ctx.target)) ctx.multiplier = 0;
        else if (ctx.target.kind === 'tower' && ctx.target.subtype === 'outer' && world.time < scaled(STRUCTURES.fortificationUntil)) ctx.multiplier *= 0.5;
      }
    });
    world.hooks.isTargetable.push(target => !isStructure(target) || isStructureAttackable(world, target));
    world.hooks.beforeAttackHit.push(ctx => {
      if (ctx.attacker.kind !== 'tower' || !ctx.attacker.data.structure) return;
      const state = structureState(ctx.attacker);
      if (state.rampTarget !== ctx.target.id) { state.rampTarget = ctx.target.id; state.ramp = 0; }
      ctx.amount = towerDamage(ctx.attacker, ctx.target);
      if (ctx.target.kind === 'minion') ctx.damageType = 'true';
    });
    world.hooks.afterAttackHit.push(ctx => {
      if (ctx.attacker.kind === 'tower' && ctx.attacker.data.structure && ctx.target.kind === 'champion') {
        const state = structureState(ctx.attacker); state.ramp = Math.min(3, state.ramp + 1);
      }
    });
    world.events.on('damage', event => {
      const source = world.get(event.source); const target = world.get(event.target);
      if (!source || !target || event.amount <= 0) return;
      if (source.kind === 'champion' && target.kind === 'champion' && source.team !== target.team) {
        for (const tower of world.query('tower')) if (!tower.dead && tower.team === target.team && inRange(tower, source, STRUCTURES.range)) tower.data.towerAggressor = { id: source.id, until: world.time + 3 };
      }
      if (target.kind !== 'tower' || target.subtype !== 'outer' || world.time >= scaled(TIMERS.towerPlatesUntil)) return;
      const state = structureState(target);
      const remaining = Math.max(0, Math.ceil(target.hp / (target.stats.maxHp / 5)));
      while (state.plates > remaining) {
        state.plates--;
        target.data.plateReward = { gold: STRUCTURES.plateGold, radius: STRUCTURES.plateRadius, source: source.id };
        world.events.emit({ type: 'structureDestroyed', target: target.id, kind: 'tower', team: target.team as Team, plate: true });
        let nearby = 0;
        for (const champ of world.query('champion')) if (!champ.dead && champ.team !== target.team && dist2(champ.pos, target.pos) <= STRUCTURES.plateRadius ** 2) nearby++;
        state.bulwark = Math.max(0, nearby - 1) * 35;
        state.bulwarkUntil = world.time + 30;
        target.data.plateReward = { gold: STRUCTURES.plateGold, radius: STRUCTURES.plateRadius, source: source.id };
      }
    });
    world.events.on('death', event => {
      const target = world.get(event.target);
      if (!target || !isStructure(target)) return;
      const state = structureState(target);
      if (!state || state.destroyed) return;
      invalidateStructures(world);
      state.destroyed = true;
      if (target.kind === 'inhibitor') state.respawnAt = world.time + scaled(TIMERS.inhibitorRespawn);
      world.events.emit({ type: 'structureDestroyed', target: target.id, kind: target.kind as 'tower' | 'inhibitor' | 'nexus', team: target.team as Team });
      if (target.kind === 'nexus' && !world.winner) {
        world.winner = otherTeam(target.team as Team);
        world.events.emit({ type: 'announcement', key: 'victory', team: world.winner });
      }
    });
  },
  update(world) {
    if (world.winner) return;
    for (const e of structureLists.get(world)?.all ?? world.entities.values()) {
      const state = structureState(e);
      if (!state) continue;
      if (e.dead) {
        if (e.kind === 'inhibitor' && state.respawnAt !== undefined && world.time >= state.respawnAt) {
          e.dead = false; e.hp = e.stats.maxHp; e.anim = 'idle'; state.destroyed = false; state.respawnAt = undefined;
          invalidateStructures(world);
          world.events.emit({ type: 'announcement', key: 'inhibitor_respawn', team: e.team as Team });
        }
        continue;
      }
      if (e.kind !== 'tower') continue;
      const def = STRUCTURES.towers[state.tier!];
      let backdoor = true;
      for (const minion of world.query('minion')) if (!minion.dead && minion.team !== e.team && dist2(e.pos, minion.pos) <= STRUCTURES.backdoorRadius ** 2) { backdoor = false; break; }
      e.stats.armor = e.stats.mr = def.armor + (backdoor ? STRUCTURES.backdoorResistance : 0) + (world.time < state.bulwarkUntil ? state.bulwark : 0);
      if (world.time >= scaled(TIMERS.towerPlatesUntil)) state.plates = 0;
      const aggressor = e.data.towerAggressor as { id: number; until: number } | undefined;
      let target = aggressor && aggressor.until > world.time ? world.get(aggressor.id) : undefined;
      if (!target || target.dead || !inRange(e, target, STRUCTURES.range)) target = undefined;
      const current = world.get(e.attackTarget);
      if (!target && current && !current.dead && inRange(e, current, STRUCTURES.range)) target = current;
      target ??= acquireTowerTarget(world, e);
      if (!target) { e.attackTarget = undefined; state.ramp = 0; state.rampTarget = undefined; continue; }
      if (state.rampTarget !== target.id) { state.rampTarget = target.id; state.ramp = 0; }
      const damage = towerDamage(e, target);
      if (attack(world, e, target, damage) && !usesCoreCombat(world) && target.kind === 'champion') state.ramp = Math.min(3, state.ramp + 1);
    }
  },
};
