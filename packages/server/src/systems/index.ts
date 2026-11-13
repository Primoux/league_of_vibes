/**
 * Registre ordonné des systèmes de simulation. UNE ligne par système : c'est le seul point de contact
 * partagé entre agents côté serveur. Ne réordonner qu'en accord avec claude-server.
 */
import type { System } from '../world.ts';
import { abilitiesSystem } from './abilities.ts';
import { buffsSystem } from './buffs.ts';
import { combatSystem } from './combat.ts';
import { economySystem } from './economy.ts';
import { inputSystem } from './input.ts';
import { jungleSystem } from './jungle.ts';
import { levelingSystem } from './leveling.ts';
import { minionsSystem } from './minions.ts';
import { movementSystem } from './movement.ts';
import { objectivesSystem } from './objectives.ts';
import { structuresSystem } from './structures.ts';
import { summonersSystem } from './summoners.ts';
import { visionSystem } from './vision.ts';

export const createSystems = (): System[] => [
  inputSystem, // claude-server
  buffsSystem, // codex-champions (stats & CC avant que quiconque agisse)
  summonersSystem, // codex-champions
  abilitiesSystem, // codex-champions
  movementSystem, // claude-server
  combatSystem, // claude-server
  minionsSystem, // codex-objectives
  structuresSystem, // codex-objectives
  jungleSystem, // codex-objectives
  objectivesSystem, // codex-objectives
  levelingSystem, // codex-champions
  economySystem, // mistral-economy
  visionSystem, // claude-server
];
