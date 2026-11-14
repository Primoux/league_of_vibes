import { dist2, type DragonElement } from '@lov/shared';
import { removeBuff } from '../combat/buffs.ts';
import { applyHeal, applyShield, timeSinceCombat } from '../combat/damage.ts';
import type { Entity, World } from '../world.ts';
import { addBuff, applyDamage } from './runtime.ts';
import type { ObjectiveState } from '../systems/objectives.ts';
interface SoulRuntime { nextBurst: number; oceanUntil: number; nextShield: number }
const runtime = (entity: Entity): SoulRuntime => (entity.data.objectiveSoul ??= { nextBurst: 0, oceanUntil: 0, nextShield: 0 }) as SoulRuntime;
function dragonStacks(dragons: readonly DragonElement[], element: DragonElement): number {
  let count = 0; for (const dragon of dragons) if (dragon === element) count++; return count;
}
/** Effets des âmes indépendants du recalcul des stats et de l'implémentation externe des buffs. */
export function installObjectiveEffects(world: World, state: ObjectiveState): void {
  world.hooks.beforeDamage.push(ctx => {
    const source = ctx.credit;
    if (source?.champ && source.team !== 'neutral' && state.teams[source.team].soul === 'chemtech' && source.hp < source.stats.maxHp * 0.5) ctx.multiplier *= 1.11;
    if (ctx.target.champ && ctx.target.team !== 'neutral' && state.teams[ctx.target.team].soul === 'chemtech' && ctx.target.hp < ctx.target.stats.maxHp * 0.5 && ctx.type !== 'true') ctx.multiplier *= 0.89;
  });
  world.hooks.beforeHeal.push(ctx => {
    if (ctx.source?.team === 'neutral' || !ctx.source?.champ) return;
    const stacks = dragonStacks(state.teams[ctx.source.team].dragons, 'chemtech');
    ctx.amount *= 1 + stacks * 0.06;
  });
  world.events.on('abilityCast', event => {
    const source = world.get(event.source);
    if (event.slot === 'R' && source && source.team !== 'neutral' && state.teams[source.team].soul === 'cloud') addBuff(world, source, 'soul_cloud_speed', { duration: 6, params: { moveSpeedPercent: 0.5 } });
  });
  world.hooks.afterDamage.push((ctx, dealt) => {
    const source = ctx.credit ?? ctx.source; const target = ctx.target;
    if (dealt <= 0 || !source?.champ || source.dead || source.team === 'neutral' || !target || target.dead || target.team === source.team || target.kind !== 'champion' || ctx.ability?.startsWith('soul_')) return;
    const soul = state.teams[source.team].soul; const active = runtime(source);
    if (soul === 'ocean') { active.oceanUntil = world.time + 3; return; }
    if (world.time < active.nextBurst) return;
    if (soul === 'infernal') {
      active.nextBurst = world.time + 3;
      const amount = 80 + source.stats.ad * 0.2 + source.stats.ap * 0.12;
      world.events.emit({ type: 'areaEffect', source: source.id, fxKey: 'soul_infernal', pos: { ...target.pos }, radius: 250, duration: 0.4 });
      for (const victim of world.query('champion')) if (!victim.dead && victim.hp > 0 && victim.team !== source.team && dist2(victim.pos, target.pos) <= 250 ** 2) applyDamage(world, { source, target: victim, amount, type: 'magic', ability: 'soul_infernal' });
    } else if (soul === 'hextech') {
      active.nextBurst = world.time + 8;
      let chained = 0;
      for (const victim of world.query('champion')) if (!victim.dead && victim.hp > 0 && victim.team !== source.team && dist2(victim.pos, target.pos) <= 500 ** 2) {
        addBuff(world, victim, 'soul_hextech_slow', { duration: 2, source, params: { slow: 0.4 } });
        applyDamage(world, { source, target: victim, amount: 80, type: 'true', ability: 'soul_hextech' });
        if (++chained === 4) break;
      }
    }
  });
}
export function updateObjectiveEffects(world: World, state: ObjectiveState, dt: number): void {
  for (const champion of world.query('champion')) {
    if (champion.dead || champion.team === 'neutral') continue;
    const progress = state.teams[champion.team];
    const oceans = dragonStacks(progress.dragons, 'ocean');
    if (oceans && champion.hp < champion.stats.maxHp) applyHeal(world, { source: champion, target: champion, amount: (champion.stats.maxHp - champion.hp) * 0.025 * oceans * dt / 5, silent: true });
    const clouds = dragonStacks(progress.dragons, 'cloud');
    const outOfCombat = clouds > 0 && timeSinceCombat(world, champion) > 5;
    if (outOfCombat && champion.data.cloudOocStacks !== clouds) {
      champion.data.cloudOocStacks = clouds;
      addBuff(world, champion, 'dragon_cloud_ooc', { duration: 1e9, params: { moveSpeedPercent: 0.04 * clouds } });
    } else if (!outOfCombat && champion.data.cloudOocStacks) {
      delete champion.data.cloudOocStacks; removeBuff(world, champion, 'dragon_cloud_ooc');
    }
    const soul = progress.soul; if (!soul) continue;
    const active = runtime(champion);
    if (soul === 'ocean' && active.oceanUntil > world.time) {
      applyHeal(world, { source: champion, target: champion, amount: (130 + champion.stats.ap * 0.26 + champion.stats.maxHp * 0.03) * dt / 3, silent: true });
      champion.mana = Math.min(champion.stats.maxMana, champion.mana + 30 * dt / 3);
    }
    if (soul === 'mountain' && timeSinceCombat(world, champion) >= 5 && world.time >= active.nextShield) {
      active.nextShield = world.time + 5;
      applyShield(world, { source: champion, target: champion, amount: 180 + champion.stats.maxHp * 0.05 + champion.stats.ad * 0.16 + champion.stats.ap * 0.13, id: 'soul_mountain' });
    }
  }
}
