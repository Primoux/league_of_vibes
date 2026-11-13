import { addBuff } from '../../combat/buffs.ts';
import { cc, hit, param, type AbilityHandler } from '../context.ts';
import { direction, enemiesInRadius, spawnZone, startDash } from '../primitives.ts';
export const tharox: Record<'Q' | 'W' | 'E' | 'R', AbilityHandler> = {
  Q(ctx) {
    const dir = direction(ctx.source.pos, ctx.point, ctx.source.facing);
    startDash(ctx.world, ctx.source, ctx.point, { fxKey: ctx.def.fxKey, speed: ctx.def.speed!, width: ctx.def.width, stopOnFirst: true, onHit(target) {
      hit(ctx, target);
      cc(ctx, target, 'knockup', .25);
      startDash(ctx.world, target, { x: target.pos.x + dir.x * param(ctx, 'knockback'), y: target.pos.y + dir.y * param(ctx, 'knockback') }, { fxKey: `${ctx.def.fxKey}_knockback`, speed: 1200, unstoppable: true });
    } });
  },
  W(ctx) {
    const duration = param(ctx, 'duration'), resists = param(ctx, 'resists');
    addBuff(ctx.world, ctx.source, 'tharox_w', { source: ctx.source, duration, params: { statMods: { armor: resists, mr: resists } }, definition: { fxKey: ctx.def.fxKey } });
    spawnZone(ctx.world, { source: ctx.source, pos: ctx.source.pos, follow: ctx.source, radius: ctx.def.radius!, duration, interval: 1, fxKey: ctx.def.fxKey,
      onTick(zone) { for (const target of enemiesInRadius(ctx.world, ctx.source, zone.pos, zone.radius)) hit(ctx, target); } });
  },
  E(ctx) { for (const target of enemiesInRadius(ctx.world, ctx.source, ctx.source.pos, ctx.def.radius!)) { hit(ctx, target); cc(ctx, target, 'slow', param(ctx, 'slowDuration'), param(ctx, 'slow')); } },
  R(ctx) { startDash(ctx.world, ctx.source, ctx.point, { fxKey: ctx.def.fxKey, speed: ctx.def.speed!, width: ctx.def.width,
    onHit(target) { hit(ctx, target); if (target.champ) cc(ctx, target, 'stun', param(ctx, 'stun')); } }); },
};
