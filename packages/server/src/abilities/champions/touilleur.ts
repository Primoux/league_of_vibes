import { TICK_DT, dist, dist2, getChampion } from '@lov/shared';
import { addBuff, canAct, getBuff, hasCC, removeBuff, type BuffInstance } from '../../combat/buffs.ts';
import type { Entity, World } from '../../world.ts';
import { cc, hit, line, param, type AbilityHandler, type CastContext } from '../context.ts';
import { applyDamage, applyShield } from '../effects.ts';
import { blink, delayedArea, enemiesInRadius, isEnemy, isVisibleTarget, schedule, validLanding } from '../primitives.ts';

interface Channel { target: number; stun: BuffInstance; end(): void }
const channels = new WeakMap<World, Map<number, Channel>>();

/** Canal annulable, jamais de dégâts périodiques survivant à la mort ou à une purification. */
function channel(ctx: CastContext, target: Entity, stun: BuffInstance): void {
  const { world, source } = ctx, active = channels.get(world)!;
  const origin = { ...source.pos }, until = world.time + stun.duration;
  let finished = false;
  const state: Channel = { target: target.id, stun, end() {
    if (finished) return;
    finished = true; active.delete(source.id);
    if (source.data.touilleurChannel === state) { delete source.data.touilleurChannel; delete source.data.channeling; }
    if (getBuff(target, stun.id, source.id) === stun) removeBuff(world, target, stun.id, source.id);
    removeBuff(world, source, 'touilleur_r_channel', source.id);
    if (!source.dead) source.anim = 'idle';
  } };
  active.set(source.id, state); source.data.touilleurChannel = state; source.data.channeling = true;
  addBuff(world, source, 'touilleur_r_channel', { source, duration: stun.duration, definition: { fxKey: 'touilleur_r_channel' } });
  const tick = (): void => {
    if (finished) return;
    const cancelled = world.commands.some(c => c.playerId === source.champ!.playerId && ['move', 'attackMove', 'attack', 'stop', 'recall'].includes(c.msg.t));
    if (world.time >= until - 1e-8 || source.dead || target.dead || !world.get(source.id) || !world.get(target.id)
      || !canAct(source) || hasCC(source, 'silence') || source.data.stasis || target.data.stasis || cancelled
      || getBuff(target, stun.id, source.id) !== stun || dist2(source.pos, origin) > 25 ** 2 || dist2(source.pos, target.pos) > param(ctx, 'breakRange') ** 2) { state.end(); return; }
    source.moveTarget = undefined; source.path = undefined; source.attackTarget = undefined;
    source.anim = 'cast_R'; source.data.castAnimUntil = world.time + TICK_DT * 2;
    source.facing = Math.atan2(target.pos.y - source.pos.y, target.pos.x - source.pos.x);
    schedule(world, Math.min(TICK_DT, until - world.time), tick);
  };
  source.anim = 'cast_R';
  schedule(world, Math.min(TICK_DT, stun.duration), tick);
}

export const touilleur: Record<'Q' | 'W' | 'E' | 'R', AbilityHandler> = {
  Q(ctx) { line(ctx, target => { hit(ctx, target); cc(ctx, target, 'slow', param(ctx, 'slowDuration'), param(ctx, 'slow')); }); },
  W(ctx) {
    const point = { ...ctx.point };
    delayedArea(ctx.world, ctx.source, point, ctx.def.radius!, param(ctx, 'delay'), ctx.def.fxKey, () => {
      for (const target of enemiesInRadius(ctx.world, ctx.source, point, ctx.def.radius!)) { hit(ctx, target); cc(ctx, target, 'slow', param(ctx, 'slowDuration'), param(ctx, 'slow')); }
    });
  },
  E(ctx) {
    blink(ctx.world, ctx.source, ctx.point, ctx.def.range, ctx.def.fxKey);
    applyShield(ctx.world, { source: ctx.source, target: ctx.source, amount: param(ctx, 'shield') + ctx.source.stats.ap * param(ctx, 'shieldAp'), duration: param(ctx, 'duration'), buffId: ctx.def.id });
  },
  R(ctx) {
    const { world, source, target } = ctx;
    if (!target?.champ || !isEnemy(source, target) || !isVisibleTarget(world, source, target) || !world.allow(world.hooks.isTargetable, target, source)
      || dist(source.pos, target.pos) > ctx.def.range + source.radius + target.radius) return;
    const offset = source.radius + target.radius + param(ctx, 'offset'), dx = Math.cos(target.facing), dy = Math.sin(target.facing);
    const travel = ctx.def.range + source.radius + target.radius + offset;
    const behind = { x: target.pos.x - dx * offset, y: target.pos.y - dy * offset };
    const landing = validLanding(source.pos, behind, travel, source.radius);
    const distance2 = dist2(landing, target.pos);
    // Un mur ne doit pas transformer l'ultime en téléportation devant la cible ou dans son collider.
    if ((landing.x - target.pos.x) * dx + (landing.y - target.pos.y) * dy >= -target.radius * .5
      || distance2 > param(ctx, 'breakRange') ** 2 || distance2 < (source.radius + target.radius) ** 2) return;
    blink(world, source, landing, travel, ctx.def.fxKey);
    source.facing = Math.atan2(target.pos.y - source.pos.y, target.pos.x - source.pos.x);
    hit(ctx, target);
    if (target.dead || source.dead) return;
    const stun = addBuff(world, target, 'stun', { source, duration: param(ctx, 'duration'), definition: { kind: 'cc', cc: 'stun', fxKey: 'touilleur_r_stun' } });
    if (stun) channel(ctx, target, stun);
  },
};

const initialized = new WeakSet<World>();
export function installTouilleurPassive(world: World): void {
  if (initialized.has(world)) return;
  initialized.add(world); const active = new Map<number, Channel>(); channels.set(world, active);
  const lastHit = new Map<number, number>();
  world.hooks.afterDamage.push((ctx, dealt) => {
    const source = ctx.credit ?? ctx.source;
    if (!source?.champ || source.dead || source.champ.championId !== 'touilleur' || !ctx.target.champ || ctx.target.team === source.team || dealt <= 0
      || !ctx.ability?.startsWith('touilleur_') || ctx.ability === 'touilleur_p') return;
    const p = getChampion('touilleur').passive.params!;
    if (world.time - (lastHit.get(source.id) ?? -Infinity) < Number(p.lockout) - 1e-8) return;
    lastHit.set(source.id, world.time);
    const count = addBuff(world, source, 'touilleur_count', { source, duration: Number(p.duration), definition: { maxStacks: Number(p.stacks), fxKey: 'touilleur_p' } });
    if (!count || count.stacks < Number(p.stacks)) return;
    removeBuff(world, source, count.id, source.id);
    addBuff(world, source, 'touilleur_p_speed', { source, duration: Number(p.speedDuration), params: { statPercentMods: { moveSpeed: Number(p.speed) } }, definition: { fxKey: 'touilleur_p' } });
    if (ctx.target.hp > 0 && !ctx.target.dead) applyDamage(world, { source, target: ctx.target, amount: Number(p.damage) + source.stats.ap * Number(p.ap), type: 'magic', ability: 'touilleur_p' });
  });
  world.events.on('buffRemoved', e => {
    if (e.buffId === 'stun') {
      const target = world.get(e.target);
      for (const [id, state] of active) if (state.target === e.target && (!target || getBuff(target, e.buffId, id) !== state.stun)) state.end();
    }
    else if (e.buffId === 'touilleur_r_channel') active.get(e.target)?.end();
  });
  world.events.on('death', e => {
    lastHit.delete(e.target);
    for (const [id, state] of active) if (id === e.target || state.target === e.target) state.end();
  });
  world.events.on('respawn', e => lastHit.delete(e.target));
}
