/**
 * Salle de jeu : lobby -> draft -> loading -> game -> end. Propriétaire : claude-server.
 * BOOTSTRAP : pas de draft (chaque joueur reçoit un champion différent), pas d'écran de fin.
 * TODO(claude-server) : draft 1-2-2-2-1 avec timers, loading, fin de partie, reconnexion.
 */
import {
  CHAMPION_LIST,
  DEFAULT_SUMMONERS,
  FOUNTAINS,
  SNAPSHOT_RATE,
  TEAM_SIZE,
  TICK_RATE,
  encode,
  type ClientMsg,
  type LobbyPlayer,
  type LobbyState,
  type PlayerId,
  type RoomPhase,
  type ServerMsg,
  type Team,
} from '@lov/shared';
import { buildSnapshot } from './net/snapshot.ts';
import { createSystems } from './systems/index.ts';
import { World, createChampion } from './world.ts';

export interface Conn {
  send(raw: string): void;
}

export class Room {
  phase: RoomPhase = 'lobby';
  players = new Map<PlayerId, LobbyPlayer & { conn: Conn | null }>();
  world: World | null = null;
  private loop: ReturnType<typeof setInterval> | null = null;

  constructor(
    public code: string,
    public devMode: boolean,
  ) {}

  get empty(): boolean {
    return [...this.players.values()].every((p) => !p.connected);
  }

  join(id: PlayerId, name: string, conn: Conn): string | null {
    const existing = this.players.get(id);
    if (existing) {
      existing.conn = conn;
      existing.connected = true;
      this.broadcastLobby();
      return null;
    }
    if (this.phase !== 'lobby') return 'La partie a déjà commencé';
    if (this.players.size >= TEAM_SIZE * 2) return 'Salle pleine';
    const blue = [...this.players.values()].filter((p) => p.team === 'blue').length;
    const team: Team = blue < TEAM_SIZE && blue <= this.players.size - blue ? 'blue' : 'red';
    this.players.set(id, { id, name, team, ready: false, isHost: this.players.size === 0, connected: true, conn });
    this.broadcastLobby();
    return null;
  }

  leave(id: PlayerId): void {
    const p = this.players.get(id);
    if (!p) return;
    if (this.phase === 'lobby') {
      this.players.delete(id);
      if (p.isHost) {
        const next = this.players.values().next().value;
        if (next) next.isHost = true;
      }
    } else {
      p.connected = false;
      p.conn = null;
    }
    this.broadcastLobby();
  }

  handle(id: PlayerId, msg: ClientMsg): void {
    const p = this.players.get(id);
    if (!p) return;
    switch (msg.t) {
      case 'setTeam': {
        const count = [...this.players.values()].filter((o) => o.team === msg.team).length;
        if (this.phase === 'lobby' && count < TEAM_SIZE) p.team = msg.team;
        this.broadcastLobby();
        return;
      }
      case 'setRole':
        p.role = msg.role;
        this.broadcastLobby();
        return;
      case 'setReady':
        p.ready = msg.ready;
        this.broadcastLobby();
        return;
      case 'startGame':
        if (p.isHost && this.canStart()) this.startGame();
        else this.send(p.conn, { t: 'error', message: 'Impossible de lancer : 8 joueurs prêts requis (ou mode dev).' });
        return;
      default:
        if (this.phase === 'game' && this.world) this.world.commands.push({ playerId: id, msg });
    }
  }

  canStart(): boolean {
    const ps = [...this.players.values()];
    if (this.devMode) return ps.length >= 1;
    return ps.length === TEAM_SIZE * 2 && ps.every((p) => p.ready);
  }

  startGame(): void {
    this.phase = 'game';
    const world = new World(createSystems());
    let i = 0;
    for (const p of this.players.values()) {
      const champ = CHAMPION_LIST[i++ % CHAMPION_LIST.length]!;
      const f = FOUNTAINS[p.team];
      createChampion(world, {
        playerId: p.id,
        name: p.name,
        team: p.team,
        championId: champ.id,
        summoners: DEFAULT_SUMMONERS,
        pos: { x: f.x + (i % 3) * 120, y: f.y + Math.floor(i / 3) * 120 },
      });
    }
    world.init();
    this.world = world;
    this.broadcastLobby();
    for (const p of this.players.values()) {
      const ent = world.championOf(p.id)!;
      this.send(p.conn, {
        t: 'gameStart',
        start: {
          you: p.id,
          team: p.team,
          entityId: ent.id,
          serverTime: Date.now(),
          players: [...this.players.values()].map((o) => ({
            id: o.id,
            name: o.name,
            team: o.team,
            championId: world.championOf(o.id)!.champ!.championId,
            summoners: DEFAULT_SUMMONERS,
          })),
        },
      });
    }
    let acc = 0;
    const snapEvery = 1000 / SNAPSHOT_RATE;
    let last = Date.now();
    this.loop = setInterval(() => {
      world.step();
      const now = Date.now();
      acc += now - last;
      last = now;
      if (acc >= snapEvery) {
        acc = 0;
        const events = world.events.drain();
        for (const pl of this.players.values()) {
          if (pl.conn) this.send(pl.conn, { t: 'snapshot', snap: buildSnapshot(world, pl.id, pl.team, events) });
        }
      }
      if (world.winner) this.stop();
    }, 1000 / TICK_RATE);
  }

  stop(): void {
    if (this.loop) clearInterval(this.loop);
    this.loop = null;
    this.phase = 'end';
  }

  lobbyState(): LobbyState {
    return {
      code: this.code,
      phase: this.phase,
      devMode: this.devMode,
      players: [...this.players.values()].map(({ conn: _c, ...p }) => p),
    };
  }

  broadcastLobby(): void {
    const msg: ServerMsg = { t: 'lobby', state: this.lobbyState() };
    for (const p of this.players.values()) this.send(p.conn, msg);
  }

  send(conn: Conn | null, msg: ServerMsg): void {
    conn?.send(encode(msg));
  }
}
