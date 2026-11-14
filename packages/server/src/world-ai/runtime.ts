/** Adaptation des paramètres PvE aux API centrales de dégâts et de buffs. */
import { dist2, type DamageType, type Stats } from '@lov/shared';
import type { Entity, World } from '../world.ts';
import { applyDamage as coreDamage } from '../combat/damage.ts';
import { addBuff as coreBuff, type BuffParams } from '../combat/buffs.ts';
export const stats = (hp: number, ad: number, armor = 0, range = 125, speed = 0, attackSpeed = 1): Stats => ({
  maxHp: hp, hpRegen: 0, maxMana: 0, manaRegen: 0, ad, ap: 0, armor, mr: armor,
  attackSpeed, attackRange: range, moveSpeed: speed, critChance: 0, abilityHaste: 0, lifesteal: 0, armorPen: 0, magicPen: 0,
});
export interface DamageRequest { source: Entity; target: Entity; amount: number; type: DamageType; ability?: string }
export interface BuffOptions { duration: number; source?: Entity; stacks?: number; params?: Record<string, number> }
interface Apis { applyDamage: (world: World, request: DamageRequest) => void; addBuff: (world: World, target: Entity, id: string, options: BuffOptions) => void }
export const isStructure = (e: Entity): boolean => e.kind === 'tower' || e.kind === 'inhibitor' || e.kind === 'nexus';
const defaults: Apis = {
  applyDamage(world, request) { coreDamage(world, request); },
  addBuff(world, target, id, options) {
    if (target.dead) return;
    const raw = options.params ?? {};
    const flat: Partial<Stats> = {}; const percent: Partial<Stats> = {};
    const keys = ['ad', 'ap', 'armor', 'mr', 'abilityHaste', 'moveSpeed', 'manaRegen', 'attackRange'] as const;
    for (const key of keys) { if (raw[key]) flat[key] = raw[key]; if (raw[`${key}Percent`]) percent[key] = raw[`${key}Percent`]; }
    if (raw.attackSpeed) percent.attackSpeed = raw.attackSpeed;
    const permanent = id.startsWith('dragon_') || (id.startsWith('soul_') && !id.endsWith('_speed') && !id.endsWith('_slow')) || id === 'grub_siege';
    const params: BuffParams = { ...raw, statMods: flat, statPercentMods: percent, persistOnDeath: permanent, tenacity: (raw.tenacity ?? 0) * (options.stacks ?? 1) };
    const source = permanent ? undefined : options.source;
    // maxStacks borne l'ajout de l'API au total autoritaire fourni par le domaine.
    coreBuff(world, target, id, { ...options, source, params, definition: {
      name: id.startsWith('dragon_') ? 'Faveur élémentaire' : id.startsWith('soul_') ? 'Âme élémentaire' : id === 'leviathan' ? 'Puissance du Léviathan' : id === 'elder' ? 'Fureur ancestrale' : id === 'blue_buff' ? 'Éveil azur' : id === 'red_buff' ? 'Braise écarlate' : 'Faveur de la Faille',
      maxStacks: Math.max(1, options.stacks ?? 1), ...(raw.slow ? { kind: 'cc', cc: 'slow' } : {}),
    } });
  },
};
const apis = new WeakMap<World, Apis>();
export function configurePveApis(world: World, overrides: Partial<Apis>): void { apis.set(world, { ...defaults, ...overrides }); }
export function applyDamage(world: World, request: DamageRequest): void { (apis.get(world) ?? defaults).applyDamage(world, request); }
export function addBuff(world: World, target: Entity, id: string, options: BuffOptions): void { (apis.get(world) ?? defaults).addBuff(world, target, id, options); }
export function inRange(a: Entity, b: Entity, range: number): boolean { return dist2(a.pos, b.pos) <= (range + b.radius) ** 2; }
export function nearest(world: World, source: Entity, range: number, predicate: (e: Entity) => boolean): Entity | undefined {
  let best: Entity | undefined; let distance = range * range;
  for (const e of world.entities.values()) {
    if (e.dead || e.id === source.id) continue;
    const d = dist2(source.pos, e.pos);
    if (d > distance || !predicate(e)) continue;
    if (d < distance || (d === distance && (!best || e.id < best.id))) { best = e; distance = d; }
  }
  return best;
}
/** En partie complète, le combat central gère windup, projectiles et hooks. Fallback isolé pour tests de domaine. */
const coreCombat = new WeakMap<World, boolean>();
export function usesCoreCombat(world: World): boolean {
  let enabled = coreCombat.get(world);
  if (enabled === undefined) {
    enabled = false;
    for (const system of world.systems) if (system.name === 'combat') { enabled = true; break; }
    coreCombat.set(world, enabled);
  }
  return enabled;
}
export function attack(world: World, source: Entity, target: Entity, amount = source.stats.ad): boolean {
  source.attackTarget = target.id;
  if (usesCoreCombat(world)) return false;
  if (!world.allow(world.hooks.canAttack, source)) return false;
  for (const hook of world.hooks.isTargetable) if (!hook(target, source, world)) return false;
  if (!inRange(source, target, source.stats.attackRange)) { source.moveTarget = target.pos; return false; }
  source.moveTarget = undefined;
  if (world.time < Number(source.data.pveNextAttack ?? 0)) return false;
  source.data.pveNextAttack = world.time + 1 / Math.max(0.1, source.stats.attackSpeed);
  source.anim = 'attack';
  world.events.emit({ type: 'autoAttack', source: source.id, target: target.id, ranged: source.stats.attackRange > 250 });
  applyDamage(world, { source, target, amount, type: 'physical' });
  return true;
}
export function retire(world: World, entity: Entity, delay = 2): void {
  if (!entity.dead) return;
  entity.data.pveRemoveAt ??= world.time + delay;
  if (world.time >= Number(entity.data.pveRemoveAt)) world.remove(entity.id);
}
