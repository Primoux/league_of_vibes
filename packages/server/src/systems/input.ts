import type { System } from '../world.ts';

/**
 * Propriétaire : claude-server. Convertit les commandes de déplacement/attaque en ordres sur l'entité.
 * Les commandes `cast`, `summoner`, `levelAbility`, `buy`, `sell`, `recall`, `placeWard` sont lues
 * directement dans world.commands par les systèmes concernés (abilities, summoners, leveling, economy).
 */
export const inputSystem: System = {
  name: 'input',
  update(world) {
    for (const { playerId, msg } of world.commands) {
      const ent = world.championOf(playerId);
      if (!ent || ent.dead) continue;
      switch (msg.t) {
        case 'move':
          ent.moveTarget = { ...msg.pos };
          ent.path = undefined;
          ent.attackTarget = undefined;
          break;
        case 'attack':
          ent.attackTarget = msg.targetId;
          break;
        case 'stop':
          ent.moveTarget = undefined;
          ent.path = undefined;
          ent.attackTarget = undefined;
          break;
        default:
          break;
      }
    }
  },
};
