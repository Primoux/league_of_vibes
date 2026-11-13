import { SUMMONERS, dist2, type ClientMsg, type SummonerId, type SummonerSlot } from '@lov/shared';
import { addBuff, canAct, cleanse, hasCC } from '../combat/buffs.ts';
import { applyDamage, applyHeal, applyShield } from '../abilities/effects.ts';
import { blink, isVisibleTarget, schedule, validLanding, validPoint } from '../abilities/primitives.ts';
import type { Entity, System, World } from '../world.ts';
interface TeleportChannel { until: number; target: Entity; from: { x: number; y: number }; interrupted: boolean }
interface SummonerState { channel?: TeleportChannel }
const state = (e: Entity): SummonerState => (e.data.summoners ??= {}) as SummonerState;
const initialized = new WeakSet<World>();
export function initSummoners(world: World): void {
  if (initialized.has(world)) return;
  initialized.add(world);
  world.events.on('damage', ({ target }) => { const e = world.get(target); if (e && state(e).channel) state(e).channel!.interrupted = true; });
}
function canUse(e: Entity, id: SummonerId): boolean {
  if (e.dead || e.data.stasis) return false;
  if (id === 'cleanse') return !hasCC(e, 'suppress');
  if (!canAct(e) || e.data.channeling) return false;
  if ((id === 'flash' || id === 'teleport') && (hasCC(e, 'root') || hasCC(e, 'grounded') || hasCC(e, 'silence') || e.data.casting || e.data.dash)) return false;
  return true;
}
export function castSummoner(world: World, e: Entity, command: Extract<ClientMsg, { t: 'summoner' }>): boolean {
  initSummoners(world);
  const c = e.champ;
  if (!c || !(['D', 'F'] as string[]).includes(command.slot)) return false;
  const slot = command.slot, id = c.summoners[slot], def = SUMMONERS[id];
  if (!def || c.cooldowns[slot] > 1e-8 || !canUse(e, id)) return false;
  const target = world.get(command.targetId);
  if (def.targeting === 'targeted') {
    if (!target || target.dead || target.data.stasis || dist2(e.pos, target.pos) > (def.range + e.radius + target.radius) ** 2) return false;
    if (id === 'teleport') {
      if (target.team !== e.team || !['tower', 'minion', 'ward', 'inhibitor', 'nexus'].includes(target.kind)) return false;
    } else {
      if (target.team === e.team || !isVisibleTarget(world, e, target)) return false;
      if (id === 'smite' ? !['monster', 'minion'].includes(target.kind) : target.kind !== 'champion') return false;
    }
  }
  if (id === 'flash' && !validPoint(command.targetPos)) return false;
  const p = def.params ?? {};
  const from = { ...e.pos };
  c.cooldowns[slot] = def.cooldown; // Summoner spells are not reduced by ability haste.
  switch (id) {
    case 'flash': blink(world, e, command.targetPos!, def.range, def.fxKey); break;
    case 'ignite': {
      const victim = target!;
      addBuff(world, victim, 'ignite', { duration: p.duration!, source: e, params: { grievous: p.grievous! } });
      const perTick = (p.baseDamage! + p.damagePerLevel! * c.level) / p.duration!;
      for (let i = 1; i <= p.duration!; i++) schedule(world, i, () => {
        if (!victim.dead && world.get(victim.id)) applyDamage(world, { source: e, target: victim, amount: perTick, type: 'true', ability: 'sum_ignite' });
      });
      break;
    }
    case 'heal': {
      const allies = [...world.query('champion')].filter((a) => a.id !== e.id && a.team === e.team && !a.dead && dist2(a.pos, e.pos) <= def.range ** 2);
      allies.sort((a, b) => a.hp / a.stats.maxHp - b.hp / b.stats.maxHp || a.id - b.id);
      for (const ally of [e, ...(allies[0] ? [allies[0]] : [])]) {
        applyHeal(world, { source: e, target: ally, amount: p.baseHeal! + c.level * p.healPerLevel! });
        addBuff(world, ally, 'sum_heal_speed', { duration: p.speedDuration!, source: e, params: { statPercentMods: { moveSpeed: p.speed! } }, definition: { fxKey: def.fxKey } });
      }
      break;
    }
    case 'barrier': applyShield(world, { source: e, target: e, amount: p.baseShield! + p.shieldPerLevel! * c.level, duration: p.duration!, buffId: 'sum_barrier', fxKey: def.fxKey }); break;
    case 'exhaust': addBuff(world, target!, 'exhaust', { duration: p.duration!, source: e, params: { amount: p.slow!, damageDealtReduction: p.damageReduction! } }); break;
    case 'teleport':
      state(e).channel = { until: world.time + p.channelDuration!, target: target!, from: { ...e.pos }, interrupted: false };
      e.data.channeling = true;
      e.moveTarget = undefined; e.path = undefined; e.attackTarget = undefined;
      e.anim = 'channel';
      world.events.emit({ type: 'areaEffect', source: e.id, fxKey: 'sum_teleport_channel', pos: { ...target!.pos }, radius: 150, duration: p.channelDuration! });
      break;
    case 'smite':
      applyDamage(world, { source: e, target: target!, amount: p.damage!, type: 'true', ability: 'sum_smite' });
      if (target!.kind === 'monster') applyHeal(world, { source: e, target: e, amount: e.stats.maxHp * p.healMaxHp! });
      break;
    case 'ghost': addBuff(world, e, 'sum_ghost', { duration: p.duration!, source: e, params: { ghosted: true, statPercentMods: { moveSpeed: p.speedMin! + (p.speedMax! - p.speedMin!) * (c.level - 1) / 17 } }, definition: { fxKey: def.fxKey } }); break;
    case 'cleanse': cleanse(world, e); addBuff(world, e, 'cleanse', { duration: p.duration!, source: e, params: { tenacity: p.tenacity! } }); break;
  }
  world.events.emit({ type: 'summonerCast', source: e.id, spell: id, from, to: id === 'flash' ? { ...e.pos } : target ? { ...target.pos } : undefined, targetId: target?.id, fxKey: def.fxKey });
  return true;
}
export const summonersSystem: System = {
  name: 'summoners', init: initSummoners,
  update(world, dt) {
    initSummoners(world);
    for (const e of world.query('champion')) {
      for (const slot of ['D', 'F'] as SummonerSlot[]) e.champ!.cooldowns[slot] = Math.max(0, e.champ!.cooldowns[slot] - dt);
      const channel = state(e).channel;
      if (!channel) continue;
      const movedOrActed = world.commands.some(({ playerId, msg }) => playerId === e.champ?.playerId && ['move', 'attackMove', 'attack', 'stop', 'cast', 'recall', 'summoner'].includes(msg.t));
      const interrupted = channel.interrupted || e.dead || !canAct(e) || hasCC(e, 'silence') || hasCC(e, 'root') || channel.target.dead || !world.get(channel.target.id) || channel.target.team !== e.team || movedOrActed || dist2(e.pos, channel.from) > 1;
      if (interrupted || world.time + 1e-8 >= channel.until) {
        if (!interrupted) {
          const from = { ...e.pos };
          e.pos = validLanding(channel.target.pos, { x: channel.target.pos.x + channel.target.radius + e.radius + 10, y: channel.target.pos.y }, 350, e.radius);
          world.events.emit({ type: 'dash', source: e.id, from, to: { ...e.pos }, duration: 0, fxKey: 'sum_teleport' });
        }
        state(e).channel = undefined; delete e.data.channeling; if (!e.dead) e.anim = 'idle';
        world.events.emit({ type: 'areaEffect', source: e.id, fxKey: interrupted ? 'sum_teleport_cancel' : 'sum_teleport_arrive', pos: { ...e.pos }, radius: 150, duration: .3 });
      } else { e.moveTarget = undefined; e.path = undefined; }
    }
    for (const { playerId, msg } of world.commands) if (msg.t === 'summoner') {
      const e = world.championOf(playerId); if (e) castSummoner(world, e, msg);
    }
  },
};
