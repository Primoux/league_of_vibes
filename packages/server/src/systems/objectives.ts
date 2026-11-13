import { PITS, TEAMS, TIMERS, dist2, scaled, type DragonElement, type MonsterType, type Team } from '@lov/shared';
import { DRAGON_ELEMENTS } from '../../../shared/src/data/monsters.ts';
import type { Entity, System, World } from '../world.ts';
import { addBuff, applyDamage, nearest, retire, type BuffOptions } from '../world-ai/runtime.ts';
import { aggroMonster, spawnMonster, updateMonster, type MonsterState } from '../world-ai/monsters.ts';
import { getCampTimers } from './jungle.ts';
import { isStructureAttackable } from './structures.ts';

interface Slot { next: number; ids: number[]; done: boolean }
export interface TeamObjectives { dragons: DragonElement[]; grubs: number; soul?: DragonElement; leviathanUntil: number; elderUntil: number }
export interface ObjectiveState {
  dragon: Slot; grubs: Slot; herald: Slot; leviathan: Slot;
  dragonCount: number; rift?: DragonElement; elder: boolean;
  teams: Record<Team, TeamObjectives>;
}
const states = new WeakMap<World, ObjectiveState>();
export function getObjectiveState(world: World): ObjectiveState | undefined { return states.get(world); }
export function getObjectiveTimers(world: World, _team: Team): Record<string, number> {
  const state = states.get(world); const timers = getCampTimers(world);
  if (!state) return timers;
  for (const name of ['dragon', 'grubs', 'herald', 'leviathan'] as const) {
    const slot = state[name];
    if (!slot.done && slot.ids.length === 0) timers[name === 'dragon' && state.elder ? 'elder_dragon' : name] = slot.next;
  }
  return timers;
}
const dragonParams: Record<DragonElement, Record<string, number>> = {
  infernal: { adPercent: 0.05, apPercent: 0.05 }, ocean: { missingHpRegen: 0.025 }, mountain: { armorPercent: 0.08, mrPercent: 0.08 },
  cloud: { outOfCombatMoveSpeed: 0.04 }, hextech: { abilityHaste: 5, attackSpeed: 0.05 }, chemtech: { tenacity: 0.06, healPower: 0.06 },
};
function grant(world: World, champion: Entity, id: string, options: BuffOptions): void { addBuff(world, champion, id, options); }
function grantDragon(world: World, team: Team, element: DragonElement, killer?: Entity): void {
  const state = states.get(world)!; const progress = state.teams[team];
  progress.dragons.push(element);
  const stacks = progress.dragons.filter(e => e === element).length;
  for (const champ of world.query('champion')) if (champ.team === team) grant(world, champ, `dragon_${element}`, { duration: 1e9, source: killer, stacks, params: dragonParams[element] });
  if (progress.dragons.length === 4 && !progress.soul) {
    progress.soul = state.rift ?? element;
    for (const champ of world.query('champion')) if (champ.team === team) grant(world, champ, `soul_${progress.soul}`, { duration: 1e9, source: killer, params: { soul: 1 } });
    state.elder = true;
    world.events.emit({ type: 'announcement', key: 'dragon_soul', team, params: { element: progress.soul } });
  }
}
interface HeraldEye { expiresAt: number }
/** Point de raccordement de l'objet actif pour economy. Ne consomme l'œil qu'après validation. */
export function useHeraldEye(world: World, owner: Entity): boolean {
  const eye = owner.data.heraldEye as HeraldEye | undefined;
  if (!eye || owner.dead || owner.team === 'neutral' || world.time >= eye.expiresAt || world.winner) return false;
  const herald = spawnMonster(world, 'herald', owner.pos, 30000);
  herald.team = owner.team; herald.subtype = 'summoned_herald'; herald.data.summonedHerald = { owner: owner.id, expiresAt: world.time + 180, nextCharge: 0 };
  delete herald.data.monster; delete herald.data.rewards; delete owner.data.heraldEye;
  world.events.emit({ type: 'announcement', key: 'herald_summoned', team: owner.team });
  return true;
}
function spawnEpic(world: World, type: MonsterType, x = 0, y = 0): Entity {
  const pit = type === 'dragon' || type === 'elder_dragon' ? PITS.dragon : PITS.leviathan;
  const entity = spawnMonster(world, type, { x: pit.pos.x + x, y: pit.pos.y + y }, pit.radius, { epic: true, home: { ...pit.pos } });
  world.events.emit({ type: 'objectiveSpawn', monster: type, pos: { ...entity.pos } });
  return entity;
}
export const objectivesSystem: System = {
  name: 'objectives',
  init(world) {
    const empty = (): TeamObjectives => ({ dragons: [], grubs: 0, leviathanUntil: 0, elderUntil: 0 });
    const slot = (timer: number): Slot => ({ next: scaled(timer), ids: [], done: false });
    const state: ObjectiveState = { dragon: slot(TIMERS.dragonFirstSpawn), grubs: slot(TIMERS.grubsSpawn), herald: slot(TIMERS.heraldSpawn), leviathan: slot(TIMERS.leviathanSpawn), dragonCount: 0, elder: false, teams: { blue: empty(), red: empty() } };
    states.set(world, state);
    world.events.on('damage', event => {
      if (event.amount <= 0) return;
      const source = world.get(event.source); const target = world.get(event.target);
      if (!source || !target) return;
      source.data.pveLastCombat = target.data.pveLastCombat = world.time;
      const monster = target.data.monster as MonsterState | undefined;
      if (monster?.epic) aggroMonster(world, target, source);
      if (source.champ && source.team !== 'neutral' && target.team !== source.team) {
        const progress = state.teams[source.team];
        if (target.kind === 'tower' && progress.grubs > 0 && event.ability !== 'grub_siege') {
          applyDamage(world, { source, target, amount: 8 * progress.grubs, type: 'true', ability: 'grub_siege' });
          if (progress.grubs >= 3 && world.time >= Number(source.data.nextGrubling ?? 0)) {
            source.data.nextGrubling = world.time + 15;
            const grub = spawnMonster(world, 'grub', source.pos, 30000); grub.kind = 'minion'; grub.team = source.team; grub.subtype = 'grubling';
            delete grub.data.monster; delete grub.data.rewards;
            grub.hp = grub.stats.maxHp = 250; grub.stats.ad = 25; grub.stats.moveSpeed = 400;
            grub.data.grubling = { target: target.id, expiresAt: world.time + 15 };
          }
        }
        if (target.kind === 'champion' && progress.elderUntil > world.time && source.data.elderBuff && !target.dead && !event.ability?.startsWith('elder_')) {
          target.data.elderBurn = { source: source.id, until: world.time + 3, next: world.time + 1 };
          if (target.hp / target.stats.maxHp < 0.2) applyDamage(world, { source, target, amount: target.hp + target.shield + 1, type: 'true', ability: 'elder_execute' });
        }
      }
    });
    world.events.on('death', event => {
      const target = world.get(event.target); const killer = world.get(event.killer);
      if (!target) return;
      if (target.champ) { target.data.leviathanBuff = undefined; target.data.elderBuff = undefined; return; }
      const monster = target.data.monster as MonsterState | undefined;
      if (!monster?.epic || target.data.objectiveClaimed) return;
      target.data.objectiveClaimed = true;
      const type = target.subtype as MonsterType;
      const name = type === 'dragon' || type === 'elder_dragon' ? 'dragon' : type === 'grub' ? 'grubs' : type === 'herald' ? 'herald' : 'leviathan';
      const slot = state[name]; slot.ids = slot.ids.filter(id => id !== target.id);
      if (name === 'grubs' || name === 'herald') { if (slot.ids.length === 0) slot.done = true; }
      else slot.next = world.time + scaled(name === 'dragon' ? (state.elder || type === 'elder_dragon' ? TIMERS.elderRespawn : TIMERS.dragonRespawn) : TIMERS.leviathanRespawn);
      if (!killer || killer.team === 'neutral') return;
      const team = killer.team; const progress = state.teams[team];
      const element = target.data.dragonElement as DragonElement | undefined;
      if (type === 'dragon' && element) { grantDragon(world, team, element, killer); if (state.elder) slot.next = world.time + scaled(TIMERS.elderRespawn); }
      if (type === 'grub') {
        progress.grubs++;
        for (const champion of world.query('champion')) if (champion.team === team) grant(world, champion, 'grub_siege', { duration: 1e9, stacks: progress.grubs, params: { structureDamage: 8 } });
      }
      if (type === 'herald' && killer.champ) killer.data.heraldEye = { expiresAt: world.time + 240 } satisfies HeraldEye;
      if (type === 'leviathan' || type === 'elder_dragon') {
        const leviathan = type === 'leviathan'; const duration = scaled(leviathan ? TIMERS.leviathanBuffDuration : TIMERS.elderBuffDuration);
        if (leviathan) progress.leviathanUntil = world.time + duration; else progress.elderUntil = world.time + duration;
        for (const champion of world.query('champion')) if (!champion.dead && champion.team === team) {
          champion.data[leviathan ? 'leviathanBuff' : 'elderBuff'] = { expiresAt: world.time + duration, ...(leviathan ? { recallSeconds: 4 } : {}) };
          grant(world, champion, leviathan ? 'leviathan' : 'elder', { duration, source: killer, params: leviathan ? { ad: 35, ap: 50, recallSeconds: 4 } : { executeThreshold: 0.2 } });
        }
      }
      world.events.emit({ type: 'objectiveTaken', monster: type, team, killer: killer.id, element });
      world.events.emit({ type: 'announcement', key: 'objective_taken', team, params: { monster: type } });
    });
  },
  update(world, dt) {
    if (world.winner) return;
    const state = states.get(world); if (!state) return;
    const spawn = (name: 'dragon' | 'leviathan' | 'herald' | 'grubs', fn: () => number[]) => {
      const slot = state[name];
      if (!slot.done && slot.ids.length === 0 && world.time >= slot.next) slot.ids = fn();
    };
    spawn('dragon', () => {
      const entity = spawnEpic(world, state.elder ? 'elder_dragon' : 'dragon');
      if (!state.elder) {
        state.dragonCount++;
        const element = state.rift ?? DRAGON_ELEMENTS[Math.floor(Math.random() * DRAGON_ELEMENTS.length)]!;
        if (state.dragonCount === 3) state.rift = element;
        entity.data.dragonElement = element;
        // Le spawn élémentaire enrichit l'événement déjà émis sans doubler le son/l'annonce.
        const event = world.events.pending[world.events.pending.length - 1];
        if (event?.type === 'objectiveSpawn') event.element = element;
      }
      return [entity.id];
    });
    if (world.time >= scaled(TIMERS.heraldSpawn)) {
      for (const id of state.grubs.ids) world.remove(id);
      state.grubs.ids = []; state.grubs.done = true;
    }
    spawn('grubs', () => [spawnEpic(world, 'grub', -180, -100).id, spawnEpic(world, 'grub', 180, -100).id, spawnEpic(world, 'grub', 0, 200).id]);
    if (world.time >= scaled(TIMERS.heraldDespawn)) {
      for (const id of state.herald.ids) world.remove(id);
      state.herald.ids = []; state.herald.done = true;
    } else spawn('herald', () => [spawnEpic(world, 'herald').id]);
    spawn('leviathan', () => [spawnEpic(world, 'leviathan').id]);
    for (const entity of world.entities.values()) {
      const monster = entity.data.monster as MonsterState | undefined;
      if (monster?.epic) {
        if (entity.dead) { retire(world, entity); continue; }
        const target = world.get(entity.attackTarget);
        if (updateMonster(world, entity, dt) && target && ['dragon', 'elder_dragon', 'leviathan'].includes(entity.subtype ?? '')) {
          const fxKey = entity.subtype === 'dragon' ? `dragon_${String(entity.data.dragonElement)}` : String(entity.subtype);
          world.events.emit({ type: 'areaEffect', source: entity.id, pos: { ...target.pos }, radius: 300, duration: 0.5, fxKey });
          for (const victim of world.query('champion')) if (!victim.dead && victim.id !== target.id && dist2(victim.pos, target.pos) <= 300 ** 2) applyDamage(world, { source: entity, target: victim, amount: entity.stats.ad * 0.5, type: 'magic', ability: fxKey });
        }
      }
      const summon = entity.data.summonedHerald as { owner: number; expiresAt: number; nextCharge: number } | undefined;
      if (summon) {
        if (entity.dead || world.time >= summon.expiresAt) { world.remove(entity.id); continue; }
        const tower = nearest(world, entity, 30000, target => target.kind === 'tower' && target.team !== entity.team && isStructureAttackable(world, target));
        if (tower) {
          entity.moveTarget = tower.pos;
          if (dist2(entity.pos, tower.pos) <= 450 ** 2 && world.time >= summon.nextCharge) {
            summon.nextCharge = world.time + 10;
            world.events.emit({ type: 'dash', source: entity.id, from: { ...entity.pos }, to: { ...tower.pos }, duration: 0.4, fxKey: 'herald_charge' });
            applyDamage(world, { source: entity, target: tower, amount: 2000, type: 'true', ability: 'herald_charge' });
            entity.hp -= entity.stats.maxHp * 0.35;
            if (entity.hp <= 0) entity.dead = true;
          }
        }
      }
      const grubling = entity.data.grubling as { target: number; expiresAt: number } | undefined;
      if (grubling) {
        const target = world.get(grubling.target);
        if (!target || target.dead || entity.dead || world.time >= grubling.expiresAt) world.remove(entity.id);
        else {
          entity.moveTarget = target.pos;
          if (dist2(entity.pos, target.pos) <= 200 ** 2 && world.time >= Number(entity.data.pveNextAttack ?? 0)) {
            entity.data.pveNextAttack = world.time + 1;
            applyDamage(world, { source: entity, target, amount: entity.stats.ad, type: 'physical', ability: 'grubling' });
          }
        }
      }
      const burn = entity.data.elderBurn as { source: number; until: number; next: number } | undefined;
      if (burn && !entity.dead && world.time >= burn.next && world.time <= burn.until) {
        const source = world.get(burn.source); burn.next += 1;
        if (source && !source.dead && source.data.elderBuff && (source.data.elderBuff as { expiresAt: number }).expiresAt > world.time) {
          applyDamage(world, { source, target: entity, amount: 40, type: 'true', ability: 'elder_burn' });
          if (!entity.dead && entity.hp / entity.stats.maxHp < 0.2) applyDamage(world, { source, target: entity, amount: entity.hp + entity.shield + 1, type: 'true', ability: 'elder_execute' });
        }
      }
      for (const key of ['leviathanBuff', 'elderBuff'] as const) {
        const buff = entity.data[key] as { expiresAt: number } | undefined;
        if (buff && (entity.dead || world.time >= buff.expiresAt)) entity.data[key] = undefined;
      }
      if (entity.kind === 'minion' && entity.team !== 'neutral' && !entity.dead && !entity.data.grubling) {
        let empowered = false;
        for (const champ of world.query('champion')) if (champ.team === entity.team && !champ.dead && champ.data.leviathanBuff && dist2(champ.pos, entity.pos) <= 1400 ** 2) { empowered = true; break; }
        if (empowered && !entity.data.leviathanMinion) {
          entity.data.leviathanMinion = true; entity.stats.ad *= 1.5; entity.stats.armor += 50; entity.stats.mr += 50; entity.stats.attackRange += 100;
        } else if (!empowered && entity.data.leviathanMinion) {
          delete entity.data.leviathanMinion; entity.stats.ad /= 1.5; entity.stats.armor -= 50; entity.stats.mr -= 50; entity.stats.attackRange -= 100;
        }
      }
    }
    updatePveBuffs(world, dt);
  },
};

/** Effets du mock uniquement. Une API addBuff externe est responsable des statistiques et de la ténacité. */
function updatePveBuffs(world: World, dt: number): void {
  for (const entity of world.query('champion')) {
    const buffs = entity.data.pveBuffs as Record<string, { expiresAt: number; stacks: number; params?: Record<string, number> }> | undefined;
    if (!buffs) continue;
    const previous = entity.data.pveStatBonus as Record<string, number> | undefined;
    const keys = ['ad', 'ap', 'armor', 'mr', 'abilityHaste', 'attackSpeed', 'moveSpeed'] as const;
    for (const key of keys) entity.stats[key] -= previous?.[key] ?? 0;
    const bonus: Record<string, number> = {};
    for (const [id, buff] of Object.entries(buffs)) {
      if (buff.expiresAt <= world.time || (entity.dead && ['leviathan', 'elder', 'blue_buff', 'red_buff'].includes(id))) {
        delete buffs[id]; world.events.emit({ type: 'buffRemoved', target: entity.id, buffId: id }); continue;
      }
      if (entity.dead) continue;
      const params = buff.params ?? {};
      for (const key of keys) bonus[key] = (bonus[key] ?? 0) + ((params[key] ?? 0) + entity.stats[key] * (params[`${key}Percent`] ?? 0)) * buff.stacks;
      if (params.outOfCombatMoveSpeed && world.time - Number(entity.data.pveLastCombat ?? -Infinity) > 5) bonus.moveSpeed = (bonus.moveSpeed ?? 0) + entity.stats.moveSpeed * params.outOfCombatMoveSpeed * buff.stacks;
      if (params.moveSpeedPercent) bonus.moveSpeed = (bonus.moveSpeed ?? 0) + entity.stats.moveSpeed * params.moveSpeedPercent;
      if (params.missingHpRegen) entity.hp = Math.min(entity.stats.maxHp, entity.hp + (entity.stats.maxHp - entity.hp) * params.missingHpRegen * buff.stacks * dt / 5);
    }
    entity.data.pveStatBonus = bonus;
    for (const key of keys) entity.stats[key] += bonus[key] ?? 0;
  }
}
