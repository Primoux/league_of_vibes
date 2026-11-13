import { dist, dist2, getChampion } from '@lov/shared';
import { addBuff, breakInvisibility, getBuff, getBuffs, removeBuff } from '../combat/buffs.ts';
import { applyDamage, applyHeal } from '../combat/damage.ts';
import { registerDamageModifier } from './combatHooks.ts';
import { atRank, scaling } from './context.ts';
import { enemiesInRadius, isUnit } from './primitives.ts';
import type { Entity, World } from '../world.ts';
import { updateGranite } from './champions/grondak.ts';
interface PassiveState { attacks: number; momentum: number; lastPos: { x: number; y: number } }
interface Bleed { source: number; stacks: number; expires: number; nextTick: number }
const state = (e: Entity): PassiveState => (e.data.championPassive ??= { attacks: 0, momentum: 0, lastPos: { ...e.pos } }) as PassiveState;
const bleeds = (e: Entity): Record<string, Bleed> => (e.data.vrennaBleeds ??= {}) as Record<string, Bleed>;
const initialized = new WeakSet<World>();
function markCorvin(world: World, source: Entity, target: Entity): void {
  const params = getChampion('corvin').passive.params!;
  const mark = addBuff(world, target, 'corvin_marks', { source, duration: Number(params.markDuration), definition: { kind: 'debuff', maxStacks: Number(params.marks), fxKey: 'corvin_p' } });
  if (mark && mark.stacks >= Number(params.marks)) {
    removeBuff(world, target, mark.id, source.id);
    applyDamage(world, { source, target, amount: target.stats.maxHp * Number(params.targetMaxHp), type: 'magic', ability: 'corvin_p' });
  }
}
function addBleed(world: World, source: Entity, target: Entity): void {
  const params = getChampion('vrenna').passive.params!, existing = bleeds(target)[source.id];
  const dot: Bleed = existing ?? { source: source.id, stacks: 0, expires: 0, nextTick: world.time + 1 };
  dot.stacks = Math.min(Number(params.maxStacks), dot.stacks + 1);
  dot.expires = world.time + Number(params.duration);
  bleeds(target)[source.id] = dot;
  addBuff(world, target, 'vrenna_bleed', { source, duration: Number(params.duration), definition: { kind: 'debuff', maxStacks: Number(params.maxStacks), fxKey: 'vrenna_p' } });
  if (dot.stacks >= Number(params.maxStacks)) addBuff(world, source, 'vrenna_p_speed', { source, duration: Number(params.duration), params: { statPercentMods: { attackSpeed: Number(params.attackSpeedAtMax) } }, definition: { fxKey: 'vrenna_p' } });
}
/** Installs per-world hooks once. `damage` without `ability` means a confirmed auto-attack HIT.
 * `autoAttack` is the windup/launch event and only breaks invisibility; it never procs on-hits.
 */
export function initPassives(world: World): void {
  if (initialized.has(world)) return;
  initialized.add(world);
  registerDamageModifier(world, (context) => {
    const { source, target } = context;
    const frostShield = getBuff(target, 'ilyra_e');
    if (frostShield && (frostShield.shieldRemaining ?? 0) > 0 && source.team !== target.team)
      addBuff(world, source, 'slow', { duration: 1, source: frostShield.source, params: { amount: .2 } });
    if (!source.champ || source.team === target.team) return;
    const id = source.champ.championId, passive = getChampion(id).passive.params!;
    if (id === 'nyx' && (context.ability === undefined || context.ability.startsWith('nyx_'))) {
      let isolated = true;
      for (const ally of world.entities.values()) if (ally.id !== target.id && !ally.dead && ally.team === target.team && isUnit(ally) && dist2(ally.pos, target.pos) <= Number(passive.isolationRadius) ** 2) { isolated = false; break; }
      if (isolated) context.amount *= 1 + Number(passive.amp);
    }
    if (context.ability !== undefined) return;
    if (id === 'kaelen' && (state(source).attacks + 1) % Number(passive.everyN) === 0) context.amount *= 1 + Number(passive.bonus);
    if (id === 'sable') {
      if (getBuff(source, 'sable_e', source.id)) {
        if (!context.crit) context.amount *= 1.75;
        context.crit = true;
        removeBuff(world, source, 'sable_e', source.id);
      }
      if (context.crit) context.amount *= 1 + Number(passive.critBonus);
    }
  });
  world.events.on('autoAttack', ({ source }) => { const e = world.get(source); if (e) breakInvisibility(world, e); });
  world.events.on('damage', (event) => {
    const source = world.get(event.source), target = world.get(event.target);
    if (!source || !target) return;
    // Nyx records actual health damage, including damage from allies, never shield absorption/overkill.
    if (event.ability !== 'nyx_r') for (const mark of getBuffs(target)) if (mark.id === 'nyx_r') mark.params.stored = Number(mark.params.stored ?? 0) + event.amount;
    if (!source.champ || target.dead || source.team === target.team) return;
    const id = source.champ.championId, passive = getChampion(id).passive.params!;
    if (event.ability === undefined) {
      breakInvisibility(world, source);
      state(source).attacks++;
      if (id === 'kaelen' && state(source).attacks % Number(passive.everyN) === 0) applyHeal(world, { source, target: source, amount: event.amount * Number(passive.heal) });
      if (id === 'corvin') markCorvin(world, source, target);
      if (id === 'tharox' && state(source).momentum >= 100) {
        state(source).momentum = 0;
        applyDamage(world, { source, target, amount: target.stats.maxHp * Number(passive.bonusTargetMaxHp), type: 'physical', ability: 'tharox_p' });
        addBuff(world, target, 'slow', { duration: Number(passive.slowDuration ?? 1.5), source, params: { amount: Number(passive.slow) }, definition: { fxKey: 'tharox_p' } });
      }
      if (id === 'sable') {
        const empowered = getBuff(source, 'sable_q', source.id);
        if (empowered) {
          const rank = Number(empowered.params.rank), def = getChampion('sable').abilities.Q;
          removeBuff(world, source, 'sable_q', source.id);
          world.events.emit({ type: 'areaEffect', source: source.id, pos: { ...target.pos }, radius: def.radius!, duration: .3, fxKey: 'sable_q_explosion' });
          for (const enemy of enemiesInRadius(world, source, target.pos, def.radius!)) applyDamage(world, { source, target: enemy, amount: scaling(source, def.damage!, rank, enemy), type: 'physical', ability: def.id });
        }
      }
    }
    if (event.amount <= 0) return;
    if (id === 'sable' && target.champ) addBuff(world, source, 'sable_p_speed', { source, duration: Number(passive.duration), params: { statPercentMods: { moveSpeed: Number(passive.moveSpeed) } }, definition: { fxKey: 'sable_p' } });
    if (id === 'corvin' && event.ability === 'corvin_q') markCorvin(world, source, target);
    if (id === 'vrenna' && (event.ability === undefined || (event.ability.startsWith('vrenna_') && event.ability !== 'vrenna_p'))) addBleed(world, source, target);
    if (id === 'ilyra' && event.ability?.startsWith('ilyra_') && !getBuff(target, 'ilyra_frost_immunity', source.id)) {
      const buff = addBuff(world, target, 'ilyra_frost', { duration: Number(passive.markDuration ?? 4), source, definition: { kind: 'debuff', maxStacks: Number(passive.stacks), fxKey: 'ilyra_p' } });
      if (buff && buff.stacks >= Number(passive.stacks)) {
        removeBuff(world, target, buff.id, source.id);
        addBuff(world, target, 'stun', { duration: Number(passive.stun), source, definition: { fxKey: 'ilyra_p' } });
        addBuff(world, target, 'ilyra_frost_immunity', { duration: Number(passive.perTargetCooldown), source });
      }
    }
  });
  world.events.on('death', ({ target }) => {
    const victim = world.get(target);
    if (!victim) return;
    for (const mark of getBuffs(victim)) if (mark.id === 'nyx_q_reset') {
      const owner = world.get(mark.source);
      if (owner?.champ) owner.champ.cooldowns.Q = 0;
    }
  });
}
export function updatePassives(world: World, _dt: number): void {
  for (const e of world.entities.values()) {
    if (e.dead) { if (e.champ) state(e).momentum = 0; delete e.data.vrennaBleeds; continue; }
    if (e.champ?.championId === 'grondak') updateGranite({ world, source: e });
    if (e.champ?.championId === 'tharox') {
      const s = state(e);
      s.momentum = Math.min(100, s.momentum + dist(s.lastPos, e.pos) * Number(getChampion('tharox').passive.params!.perUnit));
      s.lastPos = { ...e.pos };
      e.data.tharoxMomentum = s.momentum;
    }
    if (!e.data.vrennaBleeds) continue;
    for (const [key, dot] of Object.entries(bleeds(e))) {
      const owner = world.get(dot.source);
      if (!owner) { delete bleeds(e)[key]; continue; }
      while (dot.nextTick <= Math.min(world.time, dot.expires) + 1e-8 && !e.dead) {
        applyDamage(world, { source: owner, target: e, amount: dot.stacks * Number(getChampion('vrenna').passive.params!.dps), type: 'physical', ability: 'vrenna_p' });
        dot.nextTick += 1;
      }
      if (world.time + 1e-8 >= dot.expires || e.dead) delete bleeds(e)[key];
    }
  }
}
