import { addBuff } from '../../combat/buffs.ts';
import { applyHeal } from '../effects.ts';
import { cc, hit, line, param, type AbilityHandler } from '../context.ts';
import { distWithin, direction, enemiesInRadius, startDash } from '../kitUtils.ts';
export const vrenna: Record<'Q' | 'W' | 'E' | 'R', AbilityHandler> = {
  Q(ctx) {
    const origin = { ...ctx.source.pos };
    line(ctx, (target) => {
      hit(ctx, target);
      if (Math.hypot(target.pos.x - origin.x, target.pos.y - origin.y) >= ctx.def.range - param(ctx, 'tipLength')) {
        const dir = direction(target.pos, ctx.source.pos);
        cc(ctx, target, 'knockup', .25);
        startDash(ctx.world, target, { x: target.pos.x + dir.x * param(ctx, 'pull'), y: target.pos.y + dir.y * param(ctx, 'pull') }, { fxKey: `${ctx.def.fxKey}_pull`, speed: 1500, unstoppable: true });
      }
    }, { pierce: true });
  },
  W(ctx) {
    addBuff(ctx.world, ctx.source, 'vrenna_w', { source: ctx.source, duration: param(ctx, 'duration'), params: { statPercentMods: { attackSpeed: param(ctx, 'attackSpeed') } }, definition: { fxKey: ctx.def.fxKey } });
    applyHeal(ctx.world, { source: ctx.source, target: ctx.source, amount: (ctx.source.stats.maxHp - ctx.source.hp) * param(ctx, 'healMissing') });
  },
  E(ctx) { startDash(ctx.world, ctx.source, ctx.point, { fxKey: ctx.def.fxKey, speed: ctx.def.speed!, crossWalls: true, onEnd() {
    for (const target of enemiesInRadius(ctx.world, ctx.source, ctx.source.pos, ctx.def.radius!)) { hit(ctx, target); cc(ctx, target, 'slow', param(ctx, 'slowDuration'), param(ctx, 'slow')); }
  } }); },
  R(ctx) { startDash(ctx.world, ctx.source, ctx.target!.pos, { fxKey: ctx.def.fxKey, speed: ctx.def.speed!, crossWalls: true, target: ctx.target, onEnd() {
    if (distWithin(ctx.source, ctx.target!, 200)) { hit(ctx, ctx.target!); cc(ctx, ctx.target!, 'knockup', param(ctx, 'knockup')); }
  } }); },
};
