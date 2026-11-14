import { dist, getChampion, type AbilitySlot, type ClientMsg, type Vec2 } from '@lov/shared';
import { breakInvisibility, canCast, hasCC } from '../combat/buffs.ts';
import { cooldownWithHaste } from '../systems/leveling.ts';
import type { Entity, World } from '../world.ts';
import { atRank, type CastContext } from './context.ts';
import { cancelAttack } from '../systems/combat.ts';
import { endpoint, isUnit, isVisibleTarget, validPoint, updatePrimitives } from './primitives.ts';
import { CHAMPION_ABILITIES } from './registry.ts';
import { initPassives, updatePassives } from './passives.ts';

export type CastCommand = Extract<ClientMsg, { t: 'cast' }>;
interface Recast { remaining: number; expires: number; index: number; readyAt: number }
interface Windup { until: number; context: CastContext }
export interface AbilityState {
  pending?: CastCommand;
  windup?: Windup;
  recasts: Partial<Record<AbilitySlot, Recast>>;
  casts: number;
}
export function abilityState(e: Entity): AbilityState {
  return (e.data.abilities ??= { recasts: {}, casts: 0 }) as AbilityState;
}
const isSlot = (slot: string): slot is AbilitySlot => ['Q', 'W', 'E', 'R'].includes(slot);
export const isMobility = (targeting: string): boolean => targeting === 'dash' || targeting === 'targetedDash';
function resolveTarget(world: World, source: Entity, slot: AbilitySlot, targetId?: number): Entity | undefined {
  const id = source.champ!.championId;
  const ally = (id === 'ilyra' && slot === 'E') || (id === 'solenne' && (slot === 'W' || slot === 'E'));
  const target = targetId === undefined && ally ? source : world.get(targetId);
  if (!target || target.dead || target.data.stasis || !isVisibleTarget(world, source, target) || !world.allow(world.hooks.isTargetable, target, source)) return;
  if (ally) return target.team === source.team && target.kind === 'champion' ? target : undefined;
  if (id === 'nyx' && slot === 'E') return (isUnit(target) || target.kind === 'ward') && target.id !== source.id ? target : undefined;
  if (target.team === source.team || !isUnit(target)) return;
  if (slot === 'R' && ['kaelen', 'vrenna', 'nyx', 'touilleur'].includes(id) && !target.champ) return;
  return target;
}
export function interruptCast(e: Entity): void {
  const state = abilityState(e);
  state.windup = undefined;
  delete e.data.casting;
  if (!e.dead && e.anim?.startsWith('cast_')) e.anim = 'idle';
}
function execute(ctx: CastContext): void {
  CHAMPION_ABILITIES[ctx.source.champ!.championId]?.[ctx.def.slot as AbilitySlot]?.(ctx);
  ctx.source.data.castAnimUntil = ctx.world.time + .2;
}
/** Validates and spends resources once. An out-of-range targeted cast becomes a cancellable move order. */
export function castAbility(world: World, source: Entity, command: CastCommand, allowQueue = true): boolean {
  initPassives(world);
  const c = source.champ;
  if (!c || !isSlot(command.slot) || !canCast(source) || source.data.dash) return false;
  const slot = command.slot, def = getChampion(c.championId).abilities[slot];
  if (!CHAMPION_ABILITIES[c.championId]?.[slot] || c.abilityLevels[slot] < 1) return false;
  if (isMobility(def.targeting) && (hasCC(source, 'root') || hasCC(source, 'grounded'))) return false;
  const state = abilityState(source);
  let recast = state.recasts[slot];
  if (recast && world.time > recast.expires + 1e-8) { delete state.recasts[slot]; recast = undefined; }
  if ((!recast && c.cooldowns[slot] > 1e-8) || (recast && world.time < recast.readyAt - 1e-8)) return false;
  const rank = c.abilityLevels[slot];
  const cost = recast ? 0 : atRank(def.cost, rank);
  if (getChampion(c.championId).resource !== 'none' && source.mana + 1e-8 < cost) return false;
  const targeted = def.targeting === 'targeted' || def.targeting === 'targetedDash';
  const target = targeted ? resolveTarget(world, source, slot, command.targetId) : undefined;
  if (targeted && !target) return false;
  if (target && dist(source.pos, target.pos) > def.range + source.radius + target.radius) {
    if (allowQueue) { state.pending = { ...command }; source.moveTarget = { ...target.pos }; source.path = undefined; }
    return false;
  }
  if (!targeted && def.targeting !== 'self' && !validPoint(command.targetPos)) return false;
  const desired = target?.pos ?? command.targetPos ?? source.pos;
  const point: Vec2 = def.targeting === 'skillshot' || def.targeting === 'cone' ? { ...desired } : endpoint(source.pos, desired, def.range);
  state.pending = undefined;
  source.moveTarget = undefined; source.path = undefined; source.attackTarget = undefined; cancelAttack(source);
  breakInvisibility(world, source);
  if (getChampion(c.championId).resource !== 'none') source.mana -= cost;
  if (!recast) c.cooldowns[slot] = cooldownWithHaste(atRank(def.cooldown, rank), source.stats.abilityHaste);
  const castIndex = recast?.index ?? 0;
  if (recast) {
    recast.remaining--; recast.index++; recast.readyAt = world.time + atRank(def.params?.recastDelay, rank, .3);
    if (recast.remaining <= 0) delete state.recasts[slot];
  } else {
    const remaining = atRank(def.params?.recasts, rank, Math.max(0, atRank(def.params?.charges, rank, 1) - 1));
    if (remaining > 0) state.recasts[slot] = { remaining, expires: world.time + atRank(def.params?.window, rank, 4), index: 1, readyAt: world.time + atRank(def.params?.recastDelay, rank, .3) };
  }
  const passive = getChampion(c.championId).passive;
  const empowered = c.championId === 'zephyr' && state.casts > 0 && state.casts % (Number(passive.params?.every) + 1) === Number(passive.params?.every);
  state.casts++;
  if (empowered) source.mana = Math.min(source.stats.maxMana, source.mana + Number(passive.params?.mana ?? 30));
  const ctx: CastContext = { world, source, def, rank, point, target, castIndex, amplification: empowered ? 1 + Number(passive.params?.amp ?? .25) : 1 };
  source.anim = `cast_${slot}`;
  source.facing = Math.atan2(point.y - source.pos.y, point.x - source.pos.x);
  world.events.emit({ type: 'abilityCast', source: source.id, championId: c.championId, slot, abilityId: def.id, targetPos: { ...point }, targetId: target?.id, fxKey: def.fxKey });
  if ((def.castTime ?? 0) > 0) { source.data.casting = true; state.windup = { until: world.time + def.castTime!, context: ctx }; }
  else execute(ctx);
  return true;
}
export function updateAbilities(world: World, dt: number): void {
  initPassives(world);
  for (const source of world.query('champion')) {
    const state = abilityState(source);
    for (const slot of ['Q', 'W', 'E', 'R'] as const) {
      source.champ!.cooldowns[slot] = Math.max(0, source.champ!.cooldowns[slot] - dt);
      if (state.recasts[slot] && world.time > state.recasts[slot]!.expires) delete state.recasts[slot];
    }
    if (source.dead) { interruptCast(source); state.pending = undefined; state.recasts = {}; continue; }
    if (state.windup) {
      source.moveTarget = undefined; source.path = undefined;
      const { context: ctx } = state.windup;
      if (['stun', 'silence', 'knockup', 'suppress', 'taunt', 'fear'].some((cc) => hasCC(source, cc as 'stun'))
        || (isMobility(ctx.def.targeting) && (hasCC(source, 'root') || hasCC(source, 'grounded')))
        || (ctx.target && (ctx.target.dead || !world.get(ctx.target.id)))) interruptCast(source);
      else if (world.time + 1e-8 >= state.windup.until) { state.windup = undefined; delete source.data.casting; execute(ctx); }
    } else if (source.anim?.startsWith('cast_') && Number(source.data.castAnimUntil ?? 0) < world.time) source.anim = 'idle';
  }
  for (const { playerId, msg } of world.commands) {
    const e = world.championOf(playerId);
    if (!e) continue;
    if (['move', 'attackMove', 'attack', 'stop', 'recall'].includes(msg.t)) abilityState(e).pending = undefined;
    if (msg.t === 'cast') { abilityState(e).pending = undefined; castAbility(world, e, msg); }
  }
  for (const e of world.query('champion')) {
    const pending = abilityState(e).pending;
    if (!pending) continue;
    const target = world.get(pending.targetId);
    if (!target || target.dead || !isVisibleTarget(world, e, target)) { abilityState(e).pending = undefined; e.moveTarget = undefined; continue; }
    if (!castAbility(world, e, pending, false) && canCast(e)) e.moveTarget = { ...target.pos };
  }
  updatePrimitives(world, dt);
  updatePassives(world, dt);
}
