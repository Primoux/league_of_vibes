import { CAMPS, TIMERS, scaled } from '@lov/shared';
import type { Entity, System, World } from '../world.ts';
import { applyHeal } from '../combat/damage.ts';
import { addBuff, applyDamage, retire } from '../world-ai/runtime.ts';
import { refreshBurn, takeBurnTick } from '../world-ai/burns.ts';
import { aggroMonster, spawnMonster, updateMonster, type MonsterState } from '../world-ai/monsters.ts';
interface CampState { ids: number[]; next: number }
interface JungleBuffs { blue?: number; red?: number }
const camps = new WeakMap<World, Map<string, CampState>>();
function grant(world: World, champion: Entity, color: 'blue' | 'red', expiresAt = world.time + 120): void {
  const buffs = (champion.data.jungleBuffs ??= {}) as JungleBuffs;
  expiresAt = Math.max(expiresAt, buffs[color] ?? 0);
  buffs[color] = expiresAt;
  addBuff(world, champion, `${color}_buff`, { duration: Math.max(0, expiresAt - world.time), params: color === 'blue' ? { manaRegen: 25, abilityHaste: 10 } : { onHitSlow: 0.2, burn: 12 } });
}
export function getCampTimers(world: World): Record<string, number> {
  const result: Record<string, number> = {};
  for (const [id, state] of camps.get(world) ?? []) if (state.ids.length === 0) result[id] = state.next;
  return result;
}
export const jungleSystem: System = {
  name: 'jungle',
  init(world) {
    const states = new Map<string, CampState>(); camps.set(world, states);
    for (const camp of CAMPS) states.set(camp.id, { ids: [], next: scaled(camp.monster === 'scuttle' ? TIMERS.scuttleFirstSpawn : TIMERS.jungleFirstSpawn) });
    world.hooks.afterDamage.push((ctx, dealt) => {
      if (dealt <= 0) return;
      const target = ctx.target; const source = ctx.credit ?? ctx.source;
      if (!source) return;
      const monster = target.data.monster as MonsterState | undefined;
      if (monster?.camp) for (const id of states.get(monster.camp)?.ids ?? []) { const ally = world.get(id); if (ally) aggroMonster(world, ally, source); }
      const buffs = source.data.jungleBuffs as JungleBuffs | undefined;
      if (ctx.isAutoAttack && ctx.source?.champ && source.champ && !source.dead && (buffs?.red ?? 0) > world.time && target.team !== source.team && target.hp > 0 && (target.kind === 'champion' || target.kind === 'minion' || target.kind === 'monster')) {
        addBuff(world, target, 'red_slow', { duration: 2, source, params: { slow: 0.2 } });
        refreshBurn(world, target, 'redBurn', source);
      }
    });
    world.events.on('death', event => {
      const target = world.get(event.target); const killer = world.get(event.killer);
      if (!target) return;
      if (target.kind === 'champion') {
        const buffs = target.data.jungleBuffs as JungleBuffs | undefined;
        if (killer?.champ && buffs) for (const color of ['blue', 'red'] as const) if ((buffs[color] ?? 0) > world.time) grant(world, killer, color, buffs[color]);
        target.data.jungleBuffs = {};
      }
      const monster = target.data.monster as MonsterState | undefined;
      if (!monster?.camp || !killer?.champ) return;
      if (target.subtype === 'blue_buff' || target.subtype === 'red_buff') grant(world, killer, target.subtype === 'blue_buff' ? 'blue' : 'red');
      if (target.subtype === 'scuttle') {
        const ward = world.spawn({ kind: 'ward', subtype: 'scuttle_shrine', team: killer.team, pos: { ...monster.home }, radius: 30, hp: 1, mana: 0, stats: { ...target.stats, maxHp: 1 }, data: { jungle: { expiresAt: world.time + 90 }, sightRadius: 900 } });
        ward.data.speedRadius = 450;
      }
      if (Object.values(killer.champ.summoners).includes('smite')) {
        applyHeal(world, { source: killer, target: killer, amount: killer.stats.maxHp * 0.08 });
      }
    });
  },
  update(world, dt) {
    if (world.winner) return;
    const states = camps.get(world);
    if (!states) return;
    for (const camp of CAMPS) {
      const state = states.get(camp.id)!;
      if (state.ids.length > 0 && state.ids.every(id => !world.get(id) || world.get(id)!.dead)) {
        state.ids = []; state.next = world.time + scaled(camp.monster === 'scuttle' ? TIMERS.scuttleRespawn : camp.monster.endsWith('_buff') ? TIMERS.jungleBuffRespawn : TIMERS.jungleSmallRespawn);
      }
      if (state.ids.length === 0 && world.time >= state.next) {
        state.ids.push(spawnMonster(world, camp.monster, camp.pos, camp.leashRadius, { camp: camp.id }).id);
        for (const small of camp.smalls ?? []) state.ids.push(spawnMonster(world, small.monster, { x: camp.pos.x + small.offset.x, y: camp.pos.y + small.offset.y }, camp.leashRadius, { camp: camp.id, home: { ...camp.pos } }).id);
      }
    }
    for (const entity of world.entities.values()) {
      const monster = entity.data.monster as MonsterState | undefined;
      if (monster?.camp) { if (entity.dead) retire(world, entity); else updateMonster(world, entity, dt); }
      const ward = entity.data.jungle as { expiresAt: number } | undefined;
      if (entity.kind === 'ward' && ward) {
        if (world.time >= ward.expiresAt) { world.remove(entity.id); continue; }
        for (const champ of world.query('champion')) if (!champ.dead && champ.team === entity.team && Math.hypot(champ.pos.x - entity.pos.x, champ.pos.y - entity.pos.y) <= 450) {
          if (world.time >= Number(champ.data.nextScuttleSpeed ?? 0)) {
            champ.data.nextScuttleSpeed = world.time + 0.75;
            addBuff(world, champ, 'scuttle_speed', { duration: 1, params: { moveSpeedPercent: 0.2 } });
          }
        }
      }
      let burnSource: Entity | undefined;
      while ((burnSource = takeBurnTick(world, entity, 'redBurn'))) {
        applyDamage(world, { source: burnSource, target: entity, amount: 12, type: 'true', ability: 'red_burn' });
      }
      const buffs = entity.data.jungleBuffs as JungleBuffs | undefined;
      if (buffs && !entity.dead && (buffs.blue ?? 0) > world.time) entity.mana = Math.min(entity.stats.maxMana, entity.mana + entity.stats.maxMana * 0.01 * dt);
    }
  },
};
