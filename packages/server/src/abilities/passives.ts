import { getChampion } from '@lov/shared';
import { addBuff, getBuff, removeBuff } from '../combat/buffs.ts';
import { registerDamageModifier } from './combatHooks.ts';
import type { World } from '../world.ts';
import { updateGranite } from './champions/grondak.ts';
const initialized = new WeakSet<World>();
export function initPassives(world: World): void {
  if (initialized.has(world)) return;
  initialized.add(world);
  registerDamageModifier(world, ({ source, target }) => {
    const frostShield = getBuff(target, 'ilyra_e');
    if (frostShield && (frostShield.shieldRemaining ?? 0) > 0 && source.team !== target.team)
      addBuff(world, source, 'slow', { duration: 1, source: frostShield.source, params: { amount: .2 } });
  });
  world.events.on('damage', (event) => {
    const source = world.get(event.source), target = world.get(event.target);
    if (!source?.champ || !target || target.dead || event.amount <= 0 || source.team === target.team) return;
    if (source.champ.championId === 'ilyra' && event.ability?.startsWith('ilyra_') && !getBuff(target, 'ilyra_frost_immunity', source.id)) {
      const passive = getChampion('ilyra').passive.params!;
      const buff = addBuff(world, target, 'ilyra_frost', { duration: 4, source, definition: { kind: 'debuff', maxStacks: Number(passive.stacks), fxKey: 'ilyra_p' } });
      if (buff && buff.stacks >= Number(passive.stacks)) {
        removeBuff(world, target, buff.id, source.id);
        addBuff(world, target, 'stun', { duration: Number(passive.stun), source });
        addBuff(world, target, 'ilyra_frost_immunity', { duration: Number(passive.perTargetCooldown), source });
      }
    }
  });
}
export function updatePassives(world: World, _dt: number): void {
  for (const source of world.query('champion')) {
    if (!source.dead && source.champ?.championId === 'grondak') updateGranite({ world, source });
  }
}
