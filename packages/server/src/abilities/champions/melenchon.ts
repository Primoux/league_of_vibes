import { getChampion } from '@lov/shared';
import type { World } from '../../world.ts';
import { addBuff, removeBuff } from '../../combat/buffs.ts';
import { applyShield } from '../effects.ts';
import { cc, hit, line, param, type AbilityHandler } from '../context.ts';
import { blink, delayedArea, enemiesInRadius } from '../primitives.ts';

export const melenchon: Record<'Q' | 'W' | 'E' | 'R', AbilityHandler> = {
  Q(ctx) { line(ctx, (target) => { hit(ctx, target); cc(ctx, target, 'slow', param(ctx, 'slowDuration'), param(ctx, 'slow')); }); },
  W(ctx) {
    delayedArea(ctx.world, ctx.source, ctx.point, ctx.def.radius!, param(ctx, 'delay'), ctx.def.fxKey, () => {
      for (const target of enemiesInRadius(ctx.world, ctx.source, ctx.point, ctx.def.radius!)) {
        hit(ctx, target);
        addBuff(ctx.world, target, 'silence', { source: ctx.source, duration: param(ctx, 'silence'), definition: { fxKey: ctx.def.fxKey } });
      }
    });
  },
  E(ctx) {
    const origin = { ...ctx.source.pos };
    blink(ctx.world, ctx.source, ctx.point, ctx.def.range, ctx.def.fxKey);
    applyShield(ctx.world, { source: ctx.source, target: ctx.source, amount: param(ctx, 'shield') + ctx.source.stats.ap * param(ctx, 'shieldAp'), duration: param(ctx, 'duration'), buffId: 'melenchon_e', fxKey: ctx.def.fxKey });
    delayedArea(ctx.world, ctx.source, origin, ctx.def.radius!, param(ctx, 'delay'), ctx.def.fxKey, () => {
      for (const target of enemiesInRadius(ctx.world, ctx.source, origin, ctx.def.radius!)) hit(ctx, target);
    });
  },
  R(ctx) {
    const source = ctx.source;
    applyShield(ctx.world, { source, target: source, amount: source.stats.maxHp * param(ctx, 'shieldMaxHp') + source.stats.ap * param(ctx, 'shieldAp'), duration: param(ctx, 'duration'), buffId: 'melenchon_r', fxKey: ctx.def.fxKey });
    ctx.world.events.emit({ type: 'areaEffect', source: source.id, fxKey: ctx.def.fxKey, pos: { ...source.pos }, radius: ctx.def.radius!, duration: param(ctx, 'duration') });
    for (const target of enemiesInRadius(ctx.world, source, source.pos, ctx.def.radius!)) {
      hit(ctx, target); cc(ctx, target, 'taunt', param(ctx, 'taunt'));
    }
  },
};
const initialized = new WeakSet<World>();
export function installMelenchonPassive(world: World): void {
  if (initialized.has(world)) return;
  initialized.add(world);
  const lastStackAt = new Map<number, number>();
  world.hooks.afterDamage.push((ctx, dealt) => {
    const source = ctx.credit ?? ctx.source;
    if (!source?.champ || source.dead || source.champ.championId !== 'melenchon' || !ctx.target.champ || source.team === ctx.target.team || !ctx.ability?.startsWith('melenchon_') || dealt <= 0) return;
    const p = getChampion('melenchon').passive.params!;
    if (world.time - (lastStackAt.get(source.id) ?? -Infinity) < Number(p.hitLockout)) return;
    lastStackAt.set(source.id, world.time);
    const buff = addBuff(world, source, 'melenchon_indignation', { source, duration: Number(p.stackDuration), definition: { maxStacks: Number(p.maxStacks), fxKey: 'melenchon_p' } });
    if (!buff || buff.stacks < Number(p.maxStacks)) return;
    removeBuff(world, source, buff.id, source.id);
    applyShield(world, { source, target: source, amount: Number(p.shield) + source.champ.level * Number(p.shieldPerLevel) + source.stats.ap * Number(p.shieldAp), duration: Number(p.duration), buffId: 'melenchon_p', fxKey: 'melenchon_p' });
    addBuff(world, source, 'melenchon_p_speed', { source, duration: Number(p.duration), params: { statPercentMods: { moveSpeed: Number(p.moveSpeed) } }, definition: { fxKey: 'melenchon_p' } });
  });
  world.events.on('death', ({ target }) => { lastStackAt.delete(target); });
  world.events.on('respawn', ({ target }) => { lastStackAt.delete(target); });
}
