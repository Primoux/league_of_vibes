import { addBuff, getBuff, removeBuff } from '../../combat/buffs.ts';
import { applyDamage } from '../../combat/damage.ts';
import { cc, hit, param, scaling, type AbilityHandler } from '../context.ts';
import { blink, schedule } from '../primitives.ts';
export const nyx: Record<'Q' | 'W' | 'E' | 'R', AbilityHandler> = {
  Q(ctx) {
    // Mark before damage so a lethal Q also resets its cooldown.
    addBuff(ctx.world, ctx.target!, 'nyx_q_reset', { source: ctx.source, duration: param(ctx, 'resetWindow'), definition: { kind: 'debuff', fxKey: ctx.def.fxKey } });
    hit(ctx, ctx.target!);
  },
  W(ctx) { addBuff(ctx.world, ctx.source, 'invisible', { source: ctx.source, duration: param(ctx, 'duration'), params: { statPercentMods: { moveSpeed: param(ctx, 'moveSpeed') } }, definition: { fxKey: ctx.def.fxKey } }); },
  E(ctx) {
    const target = ctx.target!, offset = target.radius + ctx.source.radius + 20;
    const point = { x: target.pos.x + Math.cos(target.facing) * -offset, y: target.pos.y + Math.sin(target.facing) * -offset };
    blink(ctx.world, ctx.source, point, ctx.def.range + offset, ctx.def.fxKey);
    if (ctx.source.team !== target.team) { hit(ctx, target); cc(ctx, target, 'slow', param(ctx, 'slowDuration'), param(ctx, 'slow')); }
  },
  R(ctx) {
    const target = ctx.target!;
    const mark = addBuff(ctx.world, target, 'nyx_r', { source: ctx.source, duration: param(ctx, 'duration'), params: { stored: 0 }, definition: { kind: 'debuff', fxKey: ctx.def.fxKey } });
    if (!mark) return;
    schedule(ctx.world, param(ctx, 'duration'), () => {
      if (target.dead || !ctx.world.get(target.id)) return;
      // The reference remains valid on natural expiration, even if buffs ran first this tick.
      const current = getBuff(target, 'nyx_r', ctx.source.id);
      if (current && current !== mark) return;
      const amount = scaling(ctx.source, ctx.def.damage!, ctx.rank, target) + Number(mark.params.stored ?? 0) * param(ctx, 'storedPercent');
      removeBuff(ctx.world, target, 'nyx_r', ctx.source.id);
      applyDamage(ctx.world, { source: ctx.source, target, amount: amount * ctx.amplification, type: ctx.def.damage!.type, ability: ctx.def.id });
    });
  },
};
