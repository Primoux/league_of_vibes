import { getChampion, statAtLevel, type AbilityDef, type Scaling, type Vec2 } from '@lov/shared';
import { addBuff } from '../combat/buffs.ts';
import { applyDamage } from '../combat/damage.ts';
import type { Entity, World } from '../world.ts';
import { direction, spawnProjectile } from './primitives.ts';
export interface CastContext {
  world: World; source: Entity; def: AbilityDef; rank: number; point: Vec2; target?: Entity;
  castIndex: number; amplification: number;
}
export type AbilityHandler = (ctx: CastContext) => void;
export const atRank = (value: number | number[] | undefined, rank: number, fallback = 0): number => Array.isArray(value) ? value[Math.min(value.length - 1, Math.max(0, rank - 1))] ?? fallback : value ?? fallback;
export const param = (ctx: CastContext, key: string, fallback = 0): number => atRank(ctx.def.params?.[key], ctx.rank, fallback);
export function scaling(source: Entity, value: Scaling, rank: number, target?: Entity): number {
  const def = source.champ ? getChampion(source.champ.championId) : undefined;
  const baseAd = def ? statAtLevel(def.baseStats.ad, def.perLevel.ad ?? 0, source.champ!.level) : source.stats.ad;
  const baseHp = def ? statAtLevel(def.baseStats.maxHp, def.perLevel.maxHp ?? 0, source.champ!.level) : source.stats.maxHp;
  return atRank(value.base, rank) + (value.ad ?? 0) * source.stats.ad + (value.bonusAd ?? 0) * Math.max(0, source.stats.ad - baseAd)
    + (value.ap ?? 0) * source.stats.ap + (value.maxHp ?? 0) * source.stats.maxHp + (value.bonusHp ?? 0) * Math.max(0, source.stats.maxHp - baseHp)
    + (value.armor ?? 0) * source.stats.armor + (value.targetMaxHp ?? 0) * (target?.stats.maxHp ?? 0)
    + (value.targetMissingHp ?? 0) * Math.max(0, (target?.stats.maxHp ?? 0) - (target?.hp ?? 0));
}
export function hit(ctx: CastContext, target: Entity, multiplier = 1, type = ctx.def.damage?.type): number {
  if (!ctx.def.damage || !type) return 0;
  return applyDamage(ctx.world, { source: ctx.source, target, amount: scaling(ctx.source, ctx.def.damage, ctx.rank, target) * ctx.amplification * multiplier, type, ability: ctx.def.id });
}
export function cc(ctx: CastContext, target: Entity, type: 'stun' | 'root' | 'knockup' | 'taunt' | 'slow', duration: number, amount?: number): void {
  addBuff(ctx.world, target, type, { source: ctx.source, duration, params: { amount }, definition: { fxKey: ctx.def.fxKey } });
}
export function line(ctx: CastContext, onHit: (target: Entity) => void, options: { pierce?: boolean; stopOnChampion?: boolean; onlyChampions?: boolean; from?: Vec2; to?: Vec2; type?: string; onEnd?: (pos: Vec2) => void } = {}): Entity {
  const from = options.from ?? ctx.source.pos;
  const dir = direction(from, ctx.point, ctx.source.facing);
  const to = options.to ?? { x: from.x + dir.x * ctx.def.range, y: from.y + dir.y * ctx.def.range };
  return spawnProjectile(ctx.world, { source: ctx.source, from, to, speed: ctx.def.speed ?? 1800, width: ctx.def.width ?? 100,
    fxKey: options.type ?? ctx.def.fxKey, pierce: options.pierce, stopOnChampion: options.stopOnChampion, onlyChampions: options.onlyChampions,
    onHit, onEnd: options.onEnd });
}
