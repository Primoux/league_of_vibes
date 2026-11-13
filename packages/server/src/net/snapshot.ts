/**
 * Construction des snapshots par joueur. Propriétaire : claude-server.
 * - Filtrage par vision d'équipe (anti-triche) : structures toujours envoyées, ennemis invisibles jamais.
 * - Delta : après le premier snapshot complet, seules les entités modifiées sont envoyées + `removed`.
 * - Événements filtrés : pas d'événement concernant une unité ennemie invisible ; or/achats/rappel privés à l'équipe.
 */
import { FOUNTAINS, FOUNTAIN_RADIUS, dist, type EntityId, type EntitySnapshot, type GameEvent, type PlayerId, type SelfState, type Snapshot, type Team } from '@lov/shared';
import { isVisibleTo } from '../systems/vision.ts';
import type { Entity, World } from '../world.ts';

export function toEntitySnapshot(e: Entity, world?: World): EntitySnapshot {
  const s: EntitySnapshot = {
    id: e.id,
    kind: e.kind,
    team: e.team,
    x: Math.round(e.pos.x),
    y: Math.round(e.pos.y),
    facing: Math.round(e.facing * 100) / 100,
    hp: Math.round(e.hp),
    maxHp: Math.round(e.stats.maxHp),
  };
  if (e.subtype !== undefined) s.subtype = e.subtype;
  if (e.anim !== undefined) s.anim = e.anim;
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
    };
    if (e.dead) s.champ.respawnIn = Math.ceil(e.champ.respawnIn);
  }
  if (world) for (const fn of world.hooks.entitySnapshot) fn(e, s, world);
  return s;
}

export function selfState(e: Entity, world?: World): SelfState | undefined {
  const c = e.champ;
  if (!c) return undefined;
  const s: SelfState = {
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
    canShop: e.dead || (e.team !== 'neutral' && dist(e.pos, FOUNTAINS[e.team]) <= FOUNTAIN_RADIUS),
  };
  if (world) for (const fn of world.hooks.selfState) fn(e, s, world);
  return s;
}

export type Score = Snapshot['score'];

/** Score d'équipe tenu à jour à partir des événements (tours, drakes, larves) et des K des champions. */
export class ScoreTracker {
  readonly towers: Record<Team, number> = { blue: 0, red: 0 };
  readonly dragons: Record<Team, string[]> = { blue: [], red: [] };
  readonly grubs: Record<Team, number> = { blue: 0, red: 0 };

  constructor(private world: World) {
    world.events.on('structureDestroyed', (ev) => {
      if (ev.kind === 'tower' && !ev.plate) this.towers[ev.team === 'blue' ? 'red' : 'blue']++;
    });
    world.events.on('objectiveTaken', (ev) => {
      if (ev.monster === 'dragon' || ev.monster === 'elder_dragon') this.dragons[ev.team].push(ev.monster === 'elder_dragon' ? 'elder' : (ev.element ?? 'dragon'));
      else if (ev.monster === 'grub') this.grubs[ev.team]++;
    });
  }

  score(): Score {
    const kills = { blue: 0, red: 0 };
    for (const e of this.world.entities.values()) if (e.champ && e.team !== 'neutral') kills[e.team] += e.champ.kills;
    return {
      blue: { kills: kills.blue, towers: this.towers.blue, dragons: [...this.dragons.blue], grubs: this.grubs.blue },
      red: { kills: kills.red, towers: this.towers.red, dragons: [...this.dragons.red], grubs: this.grubs.red },
    };
  }
}

/** Cache partagé par cycle de snapshot : chaque entité n'est sérialisée qu'une fois pour tous les joueurs. */
export class SnapshotFrame {
  readonly snaps = new Map<EntityId, EntitySnapshot>();
  readonly json = new Map<EntityId, string>();
  readonly score: Score;
  readonly serverTime = Date.now();

  constructor(
    readonly world: World,
    readonly events: GameEvent[],
    score: Score,
  ) {
    this.score = score;
    for (const e of world.entities.values()) {
      if (e.kind === 'projectile' && e.data.hidden) continue;
      const s = toEntitySnapshot(e, world);
      this.snaps.set(e.id, s);
      this.json.set(e.id, JSON.stringify(s));
    }
  }
}

/** État de delta d'un client (dernière version envoyée de chaque entité). Remettre à zéro → prochain snapshot complet. */
export class ClientView {
  private sent = new Map<EntityId, string>();
  private full = true;

  reset(): void {
    this.sent.clear();
    this.full = true;
  }

  build(frame: SnapshotFrame, playerId: PlayerId, team: Team): Snapshot {
    const world = frame.world;
    const self = world.championOf(playerId);
    const entities: EntitySnapshot[] = [];
    const removed: EntityId[] = [];
    const seen = new Set<EntityId>();
    for (const e of world.entities.values()) {
      if (e !== self && !isVisibleTo(e, team, false)) continue;
      const json = frame.json.get(e.id);
      if (json === undefined) continue;
      seen.add(e.id);
      if (!this.full && this.sent.get(e.id) === json) continue;
      this.sent.set(e.id, json);
      entities.push(frame.snaps.get(e.id)!);
    }
    for (const id of this.sent.keys()) {
      if (!seen.has(id)) {
        removed.push(id);
        this.sent.delete(id);
      }
    }
    const snap: Snapshot = {
      tick: world.tick,
      time: Math.round(world.time * 1000) / 1000,
      serverTime: frame.serverTime,
      entities,
      events: filterEvents(world, frame.events, team),
      score: frame.score,
    };
    if (!this.full) snap.removed = removed;
    this.full = false;
    if (self) snap.self = selfState(self, world);
    let timers: Record<string, number> | undefined;
    for (const fn of world.hooks.objectiveTimers) {
      const t = fn(team, world);
      if (t) timers = { ...timers, ...t };
    }
    if (timers) snap.objectiveTimers = timers;
    return snap;
  }
}

/** Snapshot complet et autonome (tests, outils). */
export function buildSnapshot(world: World, playerId: PlayerId, team: Team, events: GameEvent[], score?: Score): Snapshot {
  const frame = new SnapshotFrame(world, events, score ?? new ScoreTracker(world).score());
  return new ClientView().build(frame, playerId, team);
}

function visibleId(world: World, id: EntityId | undefined, team: Team): boolean {
  if (id === undefined || id === 0) return false;
  const e = world.get(id);
  return !e || isVisibleTo(e, team, false);
}

function sameTeam(world: World, id: EntityId, team: Team): boolean {
  return world.get(id)?.team === team;
}

/** Filtre les événements du cycle pour une équipe. */
export function filterEvents(world: World, events: GameEvent[], team: Team): GameEvent[] {
  const out: GameEvent[] = [];
  for (const ev of events) {
    let keep: boolean;
    switch (ev.type) {
      case 'damage':
      case 'heal':
      case 'shield':
      case 'autoAttack':
        keep = visibleId(world, ev.source, team) || visibleId(world, ev.target, team);
        break;
      case 'death':
        keep = visibleId(world, ev.target, team) || (ev.killer !== undefined && sameTeam(world, ev.killer, team));
        break;
      case 'abilityCast':
      case 'summonerCast':
      case 'dash':
        keep = visibleId(world, ev.source, team);
        break;
      case 'projectileSpawn':
        keep = visibleId(world, ev.source, team) || visibleId(world, ev.id, team);
        break;
      case 'projectileHit':
        keep = visibleId(world, ev.id, team) || visibleId(world, ev.target, team);
        break;
      case 'areaEffect':
        keep = ev.source === 0 || visibleId(world, ev.source, team);
        break;
      case 'buffApplied':
      case 'buffRemoved':
      case 'levelUp':
        keep = visibleId(world, ev.target, team);
        break;
      case 'goldGain':
      case 'itemBought':
      case 'itemSold':
      case 'recallStart':
      case 'recallEnd':
      case 'respawn':
        keep = sameTeam(world, ev.target, team) || (ev.type !== 'goldGain' && visibleId(world, ev.target, team));
        break;
      case 'ping':
        keep = playerTeam(world, ev.playerId) === team;
        break;
      case 'chat':
        keep = !ev.teamOnly || playerTeam(world, ev.playerId) === team;
        break;
      default:
        keep = true; // structureDestroyed, objectiveSpawn/Taken, announcement : publics
    }
    if (keep) out.push(ev);
  }
  return out;
}

function playerTeam(world: World, playerId: PlayerId): Team | 'neutral' | undefined {
  return world.championOf(playerId)?.team;
}
