import { getChampion } from '@lov/shared';
import { addBuff, removeBuff } from '../../combat/buffs.ts';
import type { World } from '../../world.ts';
import { cc, hit, line, param, type AbilityHandler } from '../context.ts';
import { applyDamage, applyShield } from '../effects.ts';
import { blink, delayedArea, enemiesInRadius, schedule } from '../primitives.ts';

export const master67: Record<'Q' | 'W' | 'E' | 'R', AbilityHandler> = {
  Q(ctx) { line(ctx, (target) => { hit(ctx, target); cc(ctx, target, 'slow', param(ctx, 'slowDuration'), param(ctx, 'slow')); }); },
  W(ctx) {
    const point = { ...ctx.point };
    delayedArea(ctx.world, ctx.source, point, ctx.def.radius!, param(ctx, 'delay'), ctx.def.fxKey, () => {
      for (const target of enemiesInRadius(ctx.world, ctx.source, point, ctx.def.radius!)) { hit(ctx, target); cc(ctx, target, 'root', param(ctx, 'root')); }
    });
  },
  E(ctx) {
    blink(ctx.world, ctx.source, ctx.point, ctx.def.range, ctx.def.fxKey);
    applyShield(ctx.world, { source: ctx.source, target: ctx.source, amount: param(ctx, 'shield') + ctx.source.stats.ap * param(ctx, 'shieldAp'), duration: param(ctx, 'duration'), buffId: ctx.def.id });
  },
  R(ctx) {
    const pulse = (seven: boolean): void => {
      if (ctx.source.dead) return;
      ctx.world.events.emit({ type: 'areaEffect', source: ctx.source.id, fxKey: `master67_r_${seven ? 'seven' : 'six'}`, pos: { ...ctx.source.pos }, radius: ctx.def.radius!, duration: .67 });
      for (const target of enemiesInRadius(ctx.world, ctx.source, ctx.source.pos, ctx.def.radius!)) {
        hit(ctx, target);
        if (seven) cc(ctx, target, 'knockup', param(ctx, 'knockup'));
        else cc(ctx, target, 'slow', param(ctx, 'slowDuration'), param(ctx, 'slow'));
      }
    };
    pulse(false);
    schedule(ctx.world, param(ctx, 'delay'), () => pulse(true));
  },
};

const initialized = new WeakSet<World>();
export function installMaster67Passive(world: World): void {
  if (initialized.has(world)) return;
  initialized.add(world);
  const lastHit = new Map<number, number>();
  world.hooks.afterDamage.push((ctx, dealt) => {
    const source = ctx.credit ?? ctx.source;
    if (!source?.champ || source.dead || source.champ.championId !== 'master67' || !ctx.target.champ || source.team === ctx.target.team || dealt <= 0 || !ctx.ability?.startsWith('master67_') || ctx.ability === 'master67_p') return;
    const p = getChampion('master67').passive.params!;
    if (world.time - (lastHit.get(source.id) ?? -Infinity) < Number(p.hitLockout) - 1e-8) return;
    lastHit.set(source.id, world.time);
    const stacks = addBuff(world, source, 'master67_count', { source, duration: Number(p.duration), definition: { maxStacks: Number(p.maxStacks), fxKey: 'master67_p' } });
    if (!stacks || stacks.stacks < Number(p.maxStacks)) return;
    removeBuff(world, source, stacks.id, source.id);
    // afterDamage s'exécute avant la résolution de la mort : ne pas refrapper des PV déjà à zéro.
    if (ctx.target.hp > 0 && !ctx.target.dead) applyDamage(world, { source, target: ctx.target, amount: Number(p.damage) + source.stats.ap * Number(p.damageAp), type: 'magic', ability: 'master67_p' });
  });
  world.events.on('death', ({ target }) => lastHit.delete(target));
  world.events.on('respawn', ({ target }) => lastHit.delete(target));
}
