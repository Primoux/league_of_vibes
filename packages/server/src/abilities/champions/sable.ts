import { addBuff } from '../../combat/buffs.ts';
import { cc, hit, line, param, type AbilityHandler } from '../context.ts';
import { direction, spawnProjectile, startDash } from '../primitives.ts';
export const sable: Record<'Q' | 'W' | 'E' | 'R', AbilityHandler> = {
  Q(ctx) { addBuff(ctx.world, ctx.source, 'sable_q', { source: ctx.source, duration: param(ctx, 'empowerWindow', 6), params: { rank: ctx.rank }, definition: { fxKey: ctx.def.fxKey } }); },
  W(ctx) {
    const dir = direction(ctx.source.pos, ctx.point, ctx.source.facing), angle = Math.atan2(dir.y, dir.x);
    const count = param(ctx, 'bullets'), spread = param(ctx, 'coneAngle') * Math.PI / 180, hitTargets = new Set<number>();
    for (let i = 0; i < count; i++) {
      const a = angle - spread / 2 + i * spread / Math.max(1, count - 1);
      spawnProjectile(ctx.world, { source: ctx.source, to: { x: ctx.source.pos.x + Math.cos(a) * ctx.def.range, y: ctx.source.pos.y + Math.sin(a) * ctx.def.range }, speed: ctx.def.speed ?? 2000, width: 30, fxKey: ctx.def.fxKey,
        onHit(target) { if (!hitTargets.has(target.id)) { hitTargets.add(target.id); hit(ctx, target); cc(ctx, target, 'slow', param(ctx, 'slowDuration'), param(ctx, 'slow')); } } });
    }
  },
  E(ctx) {
    addBuff(ctx.world, ctx.source, 'sable_e', { source: ctx.source, duration: param(ctx, 'empowerWindow'), definition: { fxKey: ctx.def.fxKey } });
    startDash(ctx.world, ctx.source, ctx.point, { fxKey: ctx.def.fxKey, speed: ctx.def.speed! });
  },
  R(ctx) { line(ctx, (target) => { hit(ctx, target); }, { pierce: true, stopOnChampion: true }); },
};
