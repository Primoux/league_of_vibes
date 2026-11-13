import { addBuff } from '../../combat/buffs.ts';
import { cc, hit, line, param, type AbilityHandler } from '../context.ts';
import { direction, enemiesInRadius, removeZone, spawnZone, startDash } from '../primitives.ts';
export const corvin: Record<'Q' | 'W' | 'E' | 'R', AbilityHandler> = {
  Q(ctx) { line(ctx, (target) => { hit(ctx, target); }, { pierce: true }); },
  W(ctx) {
    const traps = [...ctx.world.query('zone')].filter((e) => e.subtype === 'corvin_w' && e.data.ownerId === ctx.source.id);
    while (traps.length >= param(ctx, 'maxTraps')) removeZone(ctx.world, traps.shift()!.id);
    spawnZone(ctx.world, { source: ctx.source, pos: ctx.point, radius: ctx.def.radius!, duration: param(ctx, 'lifetime'), interval: 1 / 30, fxKey: ctx.def.fxKey, invisible: true,
      onTick(zone) {
        const enemies = enemiesInRadius(ctx.world, ctx.source, zone.pos, zone.radius);
        if (!enemies.length) return;
        enemies.sort((a, b) => Math.hypot(a.pos.x - zone.pos.x, a.pos.y - zone.pos.y) - Math.hypot(b.pos.x - zone.pos.x, b.pos.y - zone.pos.y));
        const target = enemies[0]!;
        hit(ctx, target); cc(ctx, target, 'root', param(ctx, 'root'));
        addBuff(ctx.world, target, 'revealed', { duration: 3, source: ctx.source, params: { revealed: true }, definition: { kind: 'debuff', fxKey: ctx.def.fxKey } });
        removeZone(ctx.world, zone.id);
      } });
  },
  E(ctx) {
    const dir = direction(ctx.source.pos, ctx.point, ctx.source.facing);
    const to = { x: ctx.source.pos.x - dir.x * ctx.def.range, y: ctx.source.pos.y - dir.y * ctx.def.range };
    startDash(ctx.world, ctx.source, to, { fxKey: ctx.def.fxKey, speed: ctx.def.speed!, crossWalls: true });
    addBuff(ctx.world, ctx.source, 'corvin_e', { duration: param(ctx, 'duration'), source: ctx.source, params: { statPercentMods: { moveSpeed: param(ctx, 'moveSpeed') } }, definition: { fxKey: ctx.def.fxKey } });
  },
  R(ctx) { spawnZone(ctx.world, { source: ctx.source, pos: ctx.point, radius: ctx.def.radius!, duration: param(ctx, 'duration'), interval: 1, fxKey: ctx.def.fxKey,
    onTick(zone) { for (const target of enemiesInRadius(ctx.world, ctx.source, zone.pos, zone.radius)) {
      hit(ctx, target);
      addBuff(ctx.world, target, 'grievous', { duration: 1.2, source: ctx.source, params: { grievous: param(ctx, 'grievous') } });
      addBuff(ctx.world, target, 'revealed', { duration: 1.2, source: ctx.source, params: { revealed: true }, definition: { kind: 'debuff', fxKey: ctx.def.fxKey } });
    } } }); },
};
