import type { System } from '../world.ts';
import { updateAbilities } from '../abilities/engine.ts';
import { initPassives } from '../abilities/passives.ts';
export const abilitiesSystem: System = { name: 'abilities', init: initPassives, update: updateAbilities };
