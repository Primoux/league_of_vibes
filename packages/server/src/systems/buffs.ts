import type { System } from '../world.ts';
import { initBuffs, updateBuffs } from '../combat/buffs.ts';
export const buffsSystem: System = { name: 'buffs', init: initBuffs, update: updateBuffs };
