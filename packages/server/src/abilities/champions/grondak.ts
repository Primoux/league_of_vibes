import { getChampion } from '@lov/shared';
import { applyShield } from '../../combat/damage.ts';
import { addBuff, getBuff, removeBuff } from '../../combat/buffs.ts';
import { cc, hit, line, param, type AbilityHandler } from '../context.ts';
import { delayedArea, distWithin, enemiesInRadius, startDash } from '../kitUtils.ts';

export const grondak: Record<'Q' | 'W' | 'E' | 'R', AbilityHandler> = {
  Q(ctx) { line(ctx, (target) => { hit(ctx, target); cc(ctx, target, 'slow', param(ctx, 'slowDuration'), param(ctx, 'slow')); }, { pierce: true }); },
  W(ctx) {
    addBuff(ctx.world, ctx.source, 'grondak_w', { duration: param(ctx, 'duration'), source: ctx.source, params: { damageReduction: param(ctx, 'damageReduction') }, definition: { fxKey: ctx.def.fxKey } });
    for (const enemy of enemiesInRadius(ctx.world, ctx.source, ctx.source.pos, ctx.def.radius!)) cc(ctx, enemy, 'taunt', param(ctx, 'taunt'));
  },
  E(ctx) {
    startDash(ctx.world, ctx.source, ctx.target!.pos, { fxKey: ctx.def.fxKey, speed: ctx.def.speed!, onEnd: () => {
      if (distWithin(ctx.source, ctx.target!, 200)) { hit(ctx, ctx.target!); cc(ctx, ctx.target!, 'stun', param(ctx, 'stun')); }
    } });
  },
  R(ctx) {
    delayedArea(ctx.world, ctx.source, ctx.point, ctx.def.radius!, param(ctx, 'delay'), ctx.def.fxKey, () => {
      for (const enemy of enemiesInRadius(ctx.world, ctx.source, ctx.point, ctx.def.radius!)) { hit(ctx, enemy); cc(ctx, enemy, 'knockup', param(ctx, 'knockup')); }
    });
  },
};
export function updateGranite(ctx: { world: import('../../world.ts').World; source: import('../../world.ts').Entity }): void {
  const { world, source } = ctx;
  const params = getChampion('grondak').passive.params!;
  const wait = Number(params.outOfCombat);
  const state = (source.data.grondak ??= { nextAt: world.time + wait, active: false }) as { nextAt: number; active: boolean };
  const shield = getBuff(source, 'grondak_p', source.id);
  if (shield && (shield.shieldRemaining ?? 0) > 0) { state.active = true; return; }
  if (state.active) { state.active = false; state.nextAt = world.time + wait; removeBuff(world, source, 'grondak_p', source.id); }
  state.nextAt = Math.max(state.nextAt, Number(source.data.lastChampionDamageAt ?? -wait) + wait);
  if (world.time >= state.nextAt) { applyShield(world, { source, target: source, amount: source.stats.maxHp * Number(params.shieldMaxHp), duration: 86400, buffId: 'grondak_p', fxKey: 'grondak_p' }); state.active = true; }
}
