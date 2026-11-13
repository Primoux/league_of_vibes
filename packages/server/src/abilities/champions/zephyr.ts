import { addBuff, removeBuff } from '../../combat/buffs.ts';
import { cc, hit, line, param, type AbilityHandler } from '../context.ts';
import { blink, delayedArea, enemiesInRadius, spawnProjectile, tether } from '../primitives.ts';
export const zephyr: Record<'Q' | 'W' | 'E' | 'R', AbilityHandler> = {
  Q(ctx) { line(ctx, (target) => { hit(ctx, target); }, { pierce: true, onEnd(pos) {
    if (!ctx.world.get(ctx.source.id)) return;
    spawnProjectile(ctx.world, { source: ctx.source, from: pos, to: ctx.source.pos, homingTo: ctx.source, speed: ctx.def.speed!, width: ctx.def.width!, fxKey: 'zephyr_q_return', pierce: true,
      onHit(target) { hit(ctx, target, 1, 'true'); } });
  } }); },
  W(ctx) {
    const origin = { ...ctx.source.pos };
    blink(ctx.world, ctx.source, ctx.point, ctx.def.range, ctx.def.fxKey);
    delayedArea(ctx.world, ctx.source, origin, ctx.def.radius!, param(ctx, 'delay'), `${ctx.def.fxKey}_explosion`, () => {
      for (const target of enemiesInRadius(ctx.world, ctx.source, origin, ctx.def.radius!)) hit(ctx, target);
    });
  },
  E(ctx) {
    addBuff(ctx.world, ctx.target!, 'zephyr_e', { source: ctx.source, duration: param(ctx, 'tetherTime'), definition: { kind: 'debuff', fxKey: ctx.def.fxKey } });
    tether(ctx.world, ctx.source, ctx.target!, param(ctx, 'tetherTime'), param(ctx, 'breakRange'), () => {
      removeBuff(ctx.world, ctx.target!, 'zephyr_e', ctx.source.id);
      hit(ctx, ctx.target!); cc(ctx, ctx.target!, 'root', param(ctx, 'root'));
    });
  },
  R(ctx) { line(ctx, (target) => { hit(ctx, target); }, { onlyChampions: true }); },
};
