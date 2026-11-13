import { PITS, TEAMS, TIMERS, dist2, scaled, type DragonElement, type MonsterType, type Team } from '@lov/shared';
import { DRAGON_ELEMENTS } from '../../../shared/src/data/monsters.ts';
import type { Entity, System, World } from '../world.ts';
import { addBuff, applyDamage, retire, type BuffOptions } from '../world-ai/runtime.ts';
import { aggroMonster, spawnMonster, updateMonster, type MonsterState } from '../world-ai/monsters.ts';
import { installObjectiveEffects, updateObjectiveEffects } from '../world-ai/objective-effects.ts';
import { removeBuff, updateBuffs } from '../combat/buffs.ts';
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
const bearerLists = new WeakMap<World, Entity[]>();
const buffSystems = new WeakMap<World, boolean>();
const EXPIRING_BUFFS = ['leviathanBuff', 'elderBuff'] as const;
const BREATH_MONSTERS = new Set(['dragon', 'elder_dragon', 'leviathan']);
function ready(world: World, slot: Slot): boolean { return !slot.done && slot.ids.length === 0 && world.time >= slot.next; }
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
    for (const champ of world.query('champion')) if (champ.team === team) grant(world, champ, `soul_${progress.soul}`, { duration: 1e9, source: killer, params: progress.soul === 'cloud' ? { moveSpeedPercent: 0.15 } : { soul: 1 } });
    state.elder = true;
    world.events.emit({ type: 'announcement', key: 'dragon_soul', team, params: { element: progress.soul } });
  }
}
function syncPersistentObjectives(world: World, champion: Entity): void {
  if (champion.team === 'neutral') return;
  const progress = states.get(world)?.teams[champion.team]; if (!progress) return;
  for (const element of DRAGON_ELEMENTS) {
    const stacks = progress.dragons.filter(type => type === element).length;
    if (stacks) addBuff(world, champion, `dragon_${element}`, { duration: 1e9, stacks, params: dragonParams[element] });
  }
  if (progress.soul) addBuff(world, champion, `soul_${progress.soul}`, { duration: 1e9, params: progress.soul === 'cloud' ? { moveSpeedPercent: 0.15 } : { soul: 1 } });
  if (progress.grubs) addBuff(world, champion, 'grub_siege', { duration: 1e9, stacks: progress.grubs, params: { structureDamage: 8 } });
}
interface HeraldEye { expiresAt: number }
/** Point de raccordement de l'objet actif pour economy. Ne consomme l'œil qu'après validation. */
export function useHeraldEye(world: World, owner: Entity): boolean {
  const eye = owner.data.heraldEye as HeraldEye | undefined;
  if (!eye || owner.dead || owner.team === 'neutral' || world.time >= eye.expiresAt || world.winner) return false;
  summonHerald(world, owner, owner.pos);
  delete owner.data.heraldEye;
  return true;
}
function summonHerald(world: World, owner: Entity, pos: { x: number; y: number }): void {
  if (owner.team === 'neutral') return;
  const herald = spawnMonster(world, 'herald', pos, 30000);
  herald.team = owner.team; herald.subtype = 'summoned_herald'; herald.data.owner = owner.id; herald.data.summonedHerald = { owner: owner.id, expiresAt: world.time + 180, nextCharge: 0 };
  delete herald.data.monster; delete herald.data.rewards;
  world.events.emit({ type: 'announcement', key: 'herald_summoned', team: owner.team });
}
function spawnEpic(world: World, type: MonsterType, x = 0, y = 0, element?: DragonElement): Entity {
  const pit = type === 'dragon' || type === 'elder_dragon' ? PITS.dragon : PITS.leviathan;
  const entity = spawnMonster(world, type, { x: pit.pos.x + x, y: pit.pos.y + y }, pit.radius, { epic: true, home: { ...pit.pos } });
  if (element) entity.data.dragonElement = element;
  world.events.emit({ type: 'objectiveSpawn', monster: type, pos: { ...entity.pos }, element });
  return entity;
}
function heraldTower(world: World, herald: Entity): Entity | undefined {
  let best: Entity | undefined; let distance = 30000 ** 2;
  for (const target of world.query('tower')) {
    if (target.dead || target.team === herald.team) continue;
    const d = dist2(herald.pos, target.pos);
    if (d > distance || (d === distance && best && target.id > best.id) || !isStructureAttackable(world, target)) continue;
    best = target; distance = d;
  }
  return best;
}
function epicBreath(world: World, entity: Entity, target: Entity): void {
  if (!BREATH_MONSTERS.has(entity.subtype ?? '')) return;
  const fxKey = entity.subtype === 'dragon' ? `dragon_${String(entity.data.dragonElement)}` : String(entity.subtype);
  world.events.emit({ type: 'areaEffect', source: entity.id, pos: { ...target.pos }, radius: 300, duration: 0.5, fxKey });
  for (const victim of world.query('champion')) if (!victim.dead && victim.id !== target.id && dist2(victim.pos, target.pos) <= 300 ** 2) applyDamage(world, { source: entity, target: victim, amount: entity.stats.ad * 0.5, type: 'magic', ability: fxKey });
}
export const objectivesSystem: System = {
  name: 'objectives',
  init(world) {
    const empty = (): TeamObjectives => ({ dragons: [], grubs: 0, leviathanUntil: 0, elderUntil: 0 });
    const slot = (timer: number): Slot => ({ next: scaled(timer), ids: [], done: false });
    const state: ObjectiveState = { dragon: slot(TIMERS.dragonFirstSpawn), grubs: slot(TIMERS.grubsSpawn), herald: slot(TIMERS.heraldSpawn), leviathan: slot(TIMERS.leviathanSpawn), dragonCount: 0, elder: false, teams: { blue: empty(), red: empty() } };
    states.set(world, state);
    bearerLists.set(world, []);
    let hasBuffs = false; for (const system of world.systems) if (system.name === 'buffs') hasBuffs = true;
    buffSystems.set(world, hasBuffs);
    world.hooks.objectiveTimers.push(team => getObjectiveTimers(world, team));
    world.events.on('respawn', event => { const champion = world.get(event.target); if (champion) syncPersistentObjectives(world, champion); });
    installObjectiveEffects(world, state);
    world.hooks.afterAttackHit.push(ctx => {
      if ((ctx.attacker.data.monster as MonsterState | undefined)?.epic) epicBreath(world, ctx.attacker, ctx.target);
    });
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
            delete grub.data.monster; delete grub.data.rewards; grub.data.xpReward = 0;
            grub.hp = grub.stats.maxHp = 250; grub.stats.ad = 25; grub.stats.moveSpeed = 400;
            grub.data.owner = source.id; grub.data.grubling = { target: target.id, expiresAt: world.time + 15 };
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
    if (ready(world, state.dragon)) {
      let element: DragonElement | undefined;
      if (!state.elder) {
        state.dragonCount++;
        element = state.rift ?? DRAGON_ELEMENTS[Math.floor(Math.random() * DRAGON_ELEMENTS.length)]!;
        if (state.dragonCount === 3) state.rift = element;
      }
      const entity = spawnEpic(world, state.elder ? 'elder_dragon' : 'dragon', 0, 0, element);
      state.dragon.ids = [entity.id];
    }
    if (world.time >= scaled(TIMERS.heraldSpawn)) {
      for (const id of state.grubs.ids) world.remove(id);
      state.grubs.ids.length = 0; state.grubs.done = true;
    }
    if (ready(world, state.grubs)) state.grubs.ids = [spawnEpic(world, 'grub', -180, -100).id, spawnEpic(world, 'grub', 180, -100).id, spawnEpic(world, 'grub', 0, 200).id];
    if (world.time >= scaled(TIMERS.heraldDespawn)) {
      for (const id of state.herald.ids) world.remove(id);
      state.herald.ids.length = 0; state.herald.done = true;
    } else if (ready(world, state.herald)) state.herald.ids = [spawnEpic(world, 'herald').id];
    if (ready(world, state.leviathan)) state.leviathan.ids = [spawnEpic(world, 'leviathan').id];
    const bearers = bearerLists.get(world)!; bearers.length = 0;
    for (const champion of world.query('champion')) {
      const buff = champion.data.leviathanBuff as { expiresAt: number } | undefined;
      if (!champion.dead && buff && buff.expiresAt > world.time) bearers.push(champion);
    }
    for (const entity of world.entities.values()) {
      const eye = entity.data.heraldEye as HeraldEye | undefined;
      if (eye && world.time >= eye.expiresAt) delete entity.data.heraldEye;
      const economy = entity.data.economy as { colossusCall?: { team: Team; x: number; y: number } | null } | undefined;
      if (economy?.colossusCall) {
        const call = economy.colossusCall; economy.colossusCall = null;
        if (!entity.dead && entity.champ && call.team === entity.team && Number.isFinite(call.x) && Number.isFinite(call.y) && call.x >= 0 && call.x <= 12000 && call.y >= 0 && call.y <= 12000) summonHerald(world, entity, { x: call.x, y: call.y });
      }
      const monster = entity.data.monster as MonsterState | undefined;
      if (monster?.epic) {
        if (entity.dead) { retire(world, entity); continue; }
        const target = world.get(entity.attackTarget);
        if (updateMonster(world, entity, dt) && target && BREATH_MONSTERS.has(entity.subtype ?? '')) {
          epicBreath(world, entity, target);
        }
      }
      const summon = entity.data.summonedHerald as { owner: number; expiresAt: number; nextCharge: number } | undefined;
      if (summon) {
        if (entity.dead || world.time >= summon.expiresAt) { world.remove(entity.id); continue; }
        const tower = heraldTower(world, entity);
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
      for (const key of EXPIRING_BUFFS) {
        const buff = entity.data[key] as { expiresAt: number } | undefined;
        if (buff && (entity.dead || world.time >= buff.expiresAt)) entity.data[key] = undefined;
      }
      if (entity.kind === 'minion' && entity.team !== 'neutral' && !entity.dead && !entity.data.grubling) {
        let empowered = false;
        for (const champ of bearers) if (champ.team === entity.team && dist2(champ.pos, entity.pos) <= 1400 ** 2) { empowered = true; break; }
        if (empowered && world.time >= Number(entity.data.nextLeviathanRefresh ?? 0)) {
          entity.data.leviathanMinion = true; entity.data.nextLeviathanRefresh = world.time + 0.75;
          addBuff(world, entity, 'leviathan_minion', { duration: 1, params: { adPercent: 0.5, armor: 50, mr: 50, attackRange: 100 } });
        } else if (!empowered && entity.data.leviathanMinion) {
          delete entity.data.leviathanMinion; delete entity.data.nextLeviathanRefresh; removeBuff(world, entity, 'leviathan_minion');
        }
      }
    }
    if (!buffSystems.get(world)) updateBuffs(world, dt);
    updateObjectiveEffects(world, state, dt);
  },
};
