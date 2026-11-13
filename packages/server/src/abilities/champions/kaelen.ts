import { addBuff } from '../../combat/buffs.ts';
import { applyShield } from '../effects.ts';
import { cc, hit, param, type AbilityHandler } from '../context.ts';
import { enemiesInRadius, startDash } from '../primitives.ts';
export const kaelen: Record<'Q' | 'W' | 'E' | 'R', AbilityHandler> = {
  Q(ctx) { startDash(ctx.world, ctx.source, ctx.point, { fxKey: ctx.castIndex === 2 ? 'kaelen_q_3' : ctx.def.fxKey, speed: ctx.def.speed!, onEnd() {
    for (const target of enemiesInRadius(ctx.world, ctx.source, ctx.source.pos, ctx.def.radius!)) { hit(ctx, target); if (ctx.castIndex === 2) cc(ctx, target, 'knockup', param(ctx, 'knockup')); }
  } }); },
  W(ctx) { addBuff(ctx.world, ctx.source, 'kaelen_w', { duration: param(ctx, 'window'), source: ctx.source, params: { stun: param(ctx, 'stun') }, definition: { fxKey: ctx.def.fxKey } }); },
  E(ctx) {
    const baseAd = ctx.source.stats.ad - (ctx.source.data.baseAd as number ?? ctx.source.stats.ad);
    applyShield(ctx.world, { source: ctx.source, target: ctx.source, amount: param(ctx, 'shield') + Math.max(0, baseAd) * param(ctx, 'shieldBonusAd'), duration: param(ctx, 'shieldDuration'), buffId: 'kaelen_e', fxKey: ctx.def.fxKey });
    startDash(ctx.world, ctx.source, ctx.point, { fxKey: ctx.def.fxKey, speed: ctx.def.speed! });
  },
  R(ctx) { const target = ctx.target!; hit(ctx, target, 1 + param(ctx, 'missingHpAmp') * (1 - target.hp / target.stats.maxHp)); },
};
