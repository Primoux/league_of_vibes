import { addBuff } from '../../combat/buffs.ts';
import { applyHeal, applyShield } from '../effects.ts';
import { cc, hit, line, param, type AbilityHandler } from '../context.ts';
import { alliesInRadius, delayedArea } from '../primitives.ts';
import type { Entity } from '../../world.ts';
import { getChampion } from '@lov/shared';
function grace(ctx: import('../context.ts').CastContext, target: Entity): number {
  return 1 + Number(getChampion('solenne').passive.params!.ampPerMissing) * (1 - target.hp / target.stats.maxHp);
}
export const solenne: Record<'Q' | 'W' | 'E' | 'R', AbilityHandler> = {
  Q(ctx) { line(ctx, (target) => { hit(ctx, target); cc(ctx, target, 'root', param(ctx, 'root')); }); },
  W(ctx) {
    const target = ctx.target!, amount = param(ctx, 'heal') + ctx.source.stats.ap * param(ctx, 'healAp');
    applyHeal(ctx.world, { source: ctx.source, target, amount: amount * grace(ctx, target) });
    // Self targeting receives the main heal only, never two heals on one unit.
    if (target.id !== ctx.source.id) applyHeal(ctx.world, { source: ctx.source, target: ctx.source, amount: amount * param(ctx, 'selfPercent') * grace(ctx, ctx.source) });
  },
  E(ctx) {
    const target = ctx.target!, duration = param(ctx, 'duration');
    applyShield(ctx.world, { source: ctx.source, target, amount: (param(ctx, 'shield') + ctx.source.stats.ap * param(ctx, 'shieldAp')) * grace(ctx, target), duration, buffId: 'solenne_e', fxKey: ctx.def.fxKey });
    addBuff(ctx.world, target, 'solenne_e_speed', { source: ctx.source, duration, params: { statPercentMods: { attackSpeed: param(ctx, 'attackSpeed') } }, definition: { fxKey: ctx.def.fxKey } });
  },
  R(ctx) { delayedArea(ctx.world, ctx.source, ctx.point, ctx.def.radius!, param(ctx, 'delay'), ctx.def.fxKey, () => {
    for (const target of alliesInRadius(ctx.world, ctx.source, ctx.point, ctx.def.radius!)) {
      const amplification = grace(ctx, target);
      applyHeal(ctx.world, { source: ctx.source, target, amount: (param(ctx, 'heal') + ctx.source.stats.ap * param(ctx, 'healAp')) * amplification });
      applyShield(ctx.world, { source: ctx.source, target, amount: param(ctx, 'shield') * amplification, duration: param(ctx, 'duration'), buffId: 'solenne_r', fxKey: ctx.def.fxKey });
    }
  }); },
};
