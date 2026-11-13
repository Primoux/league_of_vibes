import { applyShield } from '../../combat/damage.ts';
import { cc, hit, line, param, type AbilityHandler } from '../context.ts';
import { delayedArea, enemiesInRadius, spawnZone } from '../primitives.ts';
export const ilyra: Record<'Q' | 'W' | 'E' | 'R', AbilityHandler> = {
  Q(ctx) { line(ctx, (target) => { hit(ctx, target); cc(ctx, target, 'slow', param(ctx, 'slowDuration'), param(ctx, 'slow')); }); },
  W(ctx) { delayedArea(ctx.world, ctx.source, ctx.point, ctx.def.radius!, param(ctx, 'delay'), ctx.def.fxKey, () => {
    for (const target of enemiesInRadius(ctx.world, ctx.source, ctx.point, ctx.def.radius!)) { hit(ctx, target); cc(ctx, target, 'slow', param(ctx, 'slowDuration'), param(ctx, 'slow')); }
  }); },
  E(ctx) {
    applyShield(ctx.world, { source: ctx.source, target: ctx.target!, amount: param(ctx, 'shield') + ctx.source.stats.ap * param(ctx, 'shieldAp'), duration: param(ctx, 'duration'), buffId: 'ilyra_e', fxKey: ctx.def.fxKey });
  },
  R(ctx) {
    spawnZone(ctx.world, { source: ctx.source, pos: ctx.point, radius: ctx.def.radius!, duration: param(ctx, 'duration'), interval: param(ctx, 'tickRate', 1), fxKey: ctx.def.fxKey,
      onTick(zone, elapsed) { for (const target of enemiesInRadius(ctx.world, ctx.source, zone.pos, zone.radius)) {
        hit(ctx, target); cc(ctx, target, 'slow', 1.1, Math.min(param(ctx, 'maxSlow'), .15 * elapsed));
      } } });
  },
};
