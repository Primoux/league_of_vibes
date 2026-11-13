import type { EntitySnapshot, PlayerId, SelfState, Snapshot, Team } from '@lov/shared';
import type { Entity, World } from '../world.ts';

/**
 * Propriétaire : claude-server. BOOTSTRAP : snapshot complet sans filtrage de vision.
 * TODO(claude-server) : delta (removed[]), filtrage par vision d'équipe, quantification.
 */
export function toEntitySnapshot(e: Entity): EntitySnapshot {
  const s: EntitySnapshot = {
    id: e.id,
    kind: e.kind,
    team: e.team,
    x: Math.round(e.pos.x),
    y: Math.round(e.pos.y),
    facing: Math.round(e.facing * 100) / 100,
    hp: Math.round(e.hp),
    maxHp: Math.round(e.stats.maxHp),
    subtype: e.subtype,
    anim: e.anim,
  };
  if (e.shield > 0) s.shield = Math.round(e.shield);
  if (e.dead) s.dead = true;
  if (e.champ) {
    s.mana = Math.round(e.mana);
    s.maxMana = Math.round(e.stats.maxMana);
    s.level = e.champ.level;
    s.ownerId = e.champ.playerId;
    s.champ = {
      playerId: e.champ.playerId,
      name: e.champ.name,
      kills: e.champ.kills,
      deaths: e.champ.deaths,
      assists: e.champ.assists,
      cs: e.champ.cs,
      items: e.champ.items,
      respawnIn: e.dead ? e.champ.respawnIn : undefined,
    };
  }
  return s;
}

export function selfState(e: Entity): SelfState | undefined {
  const c = e.champ;
  if (!c) return undefined;
  return {
    entityId: e.id,
    gold: Math.floor(c.gold),
    xp: c.xp,
    xpToNext: 0,
    level: c.level,
    skillPoints: c.skillPoints,
    abilityLevels: c.abilityLevels,
    cooldowns: c.cooldowns,
    summoners: c.summoners,
    stats: e.stats,
    canShop: true,
  };
}

export function buildSnapshot(world: World, playerId: PlayerId, _team: Team, events: Snapshot['events']): Snapshot {
  const self = world.championOf(playerId);
  return {
    tick: world.tick,
    time: world.time,
    serverTime: Date.now(),
    entities: [...world.entities.values()].map(toEntitySnapshot),
    events,
    self: self ? selfState(self) : undefined,
    score: {
      blue: { kills: 0, towers: 0, dragons: [], grubs: 0 },
      red: { kills: 0, towers: 0, dragons: [], grubs: 0 },
    },
  };
}
