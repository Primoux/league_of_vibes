import type { AbilitySlot } from '@lov/shared';
import type { AbilityHandler } from './context.ts';
import { grondak } from './champions/grondak.ts';
import { ilyra } from './champions/ilyra.ts';
export const CHAMPION_ABILITIES: Record<string, Record<AbilitySlot, AbilityHandler>> = { grondak, ilyra };
