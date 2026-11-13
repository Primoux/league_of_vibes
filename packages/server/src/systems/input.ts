import { MAP_SIZE, clamp, type Vec2 } from '@lov/shared';
import type { System } from '../world.ts';
import { cancelAttack, orderAttackMove, attackState } from './combat.ts';

/**
 * Propriétaire : claude-server. Convertit les commandes de déplacement/attaque en ordres sur l'entité.
 * Les commandes `cast`, `summoner`, `levelAbility`, `buy`, `sell`, `recall`, `placeWard` sont lues
 * directement dans world.commands par les systèmes concernés (abilities, summoners, leveling, economy).
 */
const clampPos = (p: Vec2): Vec2 => ({ x: clamp(p.x, 0, MAP_SIZE), y: clamp(p.y, 0, MAP_SIZE) });

export const inputSystem: System = {
  name: 'input',
  update(world) {
    for (const { playerId, msg } of world.commands) {
      const ent = world.championOf(playerId);
      if (!ent || ent.dead) continue;
      switch (msg.t) {
        case 'move':
          cancelAttack(ent);
          ent.moveTarget = clampPos(msg.pos);
          ent.path = undefined;
          ent.attackTarget = undefined;
          attackState(ent).attackMove = undefined;
          attackState(ent).chasing = false;
          break;
        case 'attackMove':
          cancelAttack(ent);
          orderAttackMove(ent, clampPos(msg.pos));
          break;
        case 'attack': {
          const t = world.get(msg.targetId);
          if (!t || t.team === ent.team || t.dead) break;
          if (ent.attackTarget !== t.id) cancelAttack(ent);
          ent.attackTarget = t.id;
          attackState(ent).attackMove = undefined;
          break;
        }
        case 'stop':
          cancelAttack(ent);
          ent.moveTarget = undefined;
          ent.path = undefined;
          ent.attackTarget = undefined;
          attackState(ent).attackMove = undefined;
          attackState(ent).chasing = false;
          break;
        default:
          break;
      }
    }
  },
};
