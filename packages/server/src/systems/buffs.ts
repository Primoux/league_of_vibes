import type { System } from '../world.ts';
import { updateBuffs } from '../combat/buffs.ts';
export const buffsSystem: System = { name: 'buffs', update: updateBuffs };
