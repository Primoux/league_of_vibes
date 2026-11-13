/**
 * Salle de jeu : lobby -> draft -> loading -> game -> end (-> lobby). Propriétaire : claude-server.
 *
 * Draft : ordre B1 – R1 R2 – B2 B3 – R3 R4 – B4 (1-2-2-2-1), DRAFT_TURN_SECONDS par tour, picks simultanés dans un
 * tour, survol visible des alliés uniquement, champion unique, auto-pick à l'expiration (survol si disponible,
 * sinon aléatoire). Puis FINALIZE_SECONDS pour les sorts d'invocateur, puis LOADING_SECONDS de chargement.
 *
 * L'horloge est injectable (`RoomOptions.clock`) et les boucles désactivables (`manual`) pour les tests :
 * appeler alors `room.update()` (timers de draft/chargement) et `room.tickGame()` (un tick de simulation).
 */
import {
  CHAMPION_LIST,
  CHAMPIONS,
  DRAFT_TURN_SECONDS,
  FOUNTAINS,
  LOADING_SECONDS,
  SNAPSHOT_RATE,
  SUMMONERS,
  TEAM_SIZE,
  TICK_DT,
  TICK_RATE,
  encode,
  type ClientMsg,
  type DraftPick,
  type DraftState,
  type GameOver,
  type GameStart,
  type LobbyPlayer,
  type LobbyState,
  type PlayerId,
  type RoomPhase,
  type ServerMsg,
  type SummonerId,
  type Team,
} from '@lov/shared';
import { ClientView, ScoreTracker, SnapshotFrame } from './net/snapshot.ts';
import { createSystems } from './systems/index.ts';
import { World, createChampion, type System } from './world.ts';

export const FINALIZE_SECONDS = 15;
/** Délai avant suppression d'une salle dont tous les joueurs sont déconnectés en cours de partie. */
export const EMPTY_ROOM_GRACE_MS = 3 * 60 * 1000;

export interface Conn {
  send(raw: string): void;
}

export interface Clock {
  now(): number;
}

export interface RoomOptions {
  clock?: Clock;
  /** Pas de setInterval : l'appelant pilote `update()` / `tickGame()` (tests). */
  manual?: boolean;
  /** Systèmes de simulation (défaut : createSystems()). */
  systems?: () => System[];
  /** Générateur aléatoire (auto-pick). */
  random?: () => number;
  /** Durées (s) du draft / de la finalisation / du chargement. Défaut : valeurs officielles, raccourcies en mode dev. */
  timings?: Partial<DraftTimings>;
  /** Lancer directement la partie (picks automatiques) sans draft — tests manuels (env SKIP_DRAFT=1). */
  skipDraft?: boolean;
}

export interface DraftTimings {
  turn: number;
  finalize: number;
  loading: number;
}

export const DEV_TIMINGS: DraftTimings = { turn: 10, finalize: 3, loading: 2 };

export interface RoomPlayer extends LobbyPlayer {
  conn: Conn | null;
  view: ClientView;
}

interface Draft {
  turns: PlayerId[][];
  currentTurn: number;
  turnEndsAt: number;
  finalizeEndsAt?: number;
  picks: Map<PlayerId, DraftPick>;
}

export class Room {
  phase: RoomPhase = 'lobby';
  players = new Map<PlayerId, RoomPlayer>();
  world: World | null = null;
  draft: Draft | null = null;
  loadingEndsAt = 0;
  result: GameOver | null = null;
  /** Instant (ms) où la salle est devenue vide, null si quelqu'un est connecté. */
  emptySince: number | null = null;
  private score: ScoreTracker | null = null;
  private gameStart: Omit<GameStart, 'you' | 'team' | 'entityId'> | null = null;
  private timer: ReturnType<typeof setInterval> | null = null;
  private loop: ReturnType<typeof setInterval> | null = null;
  private clock: Clock;
  private random: () => number;
  private snapAcc = 0;
  private timings: DraftTimings;

  constructor(
    public code: string,
    public devMode: boolean,
    private opts: RoomOptions = {},
  ) {
    this.clock = opts.clock ?? Date;
    this.random = opts.random ?? Math.random;
    const base = devMode ? DEV_TIMINGS : { turn: DRAFT_TURN_SECONDS, finalize: FINALIZE_SECONDS, loading: LOADING_SECONDS };
    this.timings = { ...base, ...opts.timings };
  }

  get empty(): boolean {
    return [...this.players.values()].every((p) => !p.connected);
  }

  private now(): number {
    return this.clock.now();
  }

  // ------------------------------------------------------------------ connexion

  /** Rejoint (ou reprend sa place après une déconnexion). Retourne un message d'erreur ou null. */
  join(id: PlayerId, name: string, conn: Conn): string | null {
    const existing = this.players.get(id);
    if (existing) {
      existing.conn = conn;
      existing.connected = true;
      this.emptySince = null;
      this.resync(existing);
      return null;
    }
    if (this.phase !== 'lobby') return 'La partie a déjà commencé';
    if (this.players.size >= TEAM_SIZE * 2) return 'Salle pleine';
    const blue = [...this.players.values()].filter((p) => p.team === 'blue').length;
    const team: Team = blue < TEAM_SIZE && blue <= this.players.size - blue ? 'blue' : 'red';
    this.players.set(id, {
      id,
      name,
      team,
      ready: false,
      isHost: this.players.size === 0,
      connected: true,
      conn,
      view: new ClientView(),
    });
    this.emptySince = null;
    this.broadcastLobby();
    return null;
  }

  /** Renvoie l'état courant à un joueur qui (re)vient. */
  private resync(p: RoomPlayer): void {
    this.broadcastLobby();
    if (this.phase === 'draft') this.send(p.conn, { t: 'draft', state: this.draftStateFor(p.team) });
    if (this.phase === 'loading') {
      this.send(p.conn, { t: 'draft', state: this.draftStateFor(p.team) });
      this.send(p.conn, { t: 'loading', endsAt: this.loadingEndsAt });
    }
    if (this.phase === 'game' && this.world) {
      p.view.reset(); // prochain snapshot complet
      this.sendGameStart(p);
    }
    if (this.phase === 'end' && this.result) this.send(p.conn, { t: 'gameOver', result: this.result });
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
    if (this.empty && this.emptySince === null) this.emptySince = this.now();
    this.broadcastLobby();
  }

  // ------------------------------------------------------------------ messages

  handle(id: PlayerId, msg: ClientMsg): void {
    const p = this.players.get(id);
    if (!p) return;
    switch (msg.t) {
      case 'setTeam': {
        if (this.phase !== 'lobby') return;
        const count = [...this.players.values()].filter((o) => o.team === msg.team).length;
        if (p.team !== msg.team && count < TEAM_SIZE) {
          p.team = msg.team;
          p.ready = false;
        }
        this.broadcastLobby();
        return;
      }
      case 'setRole':
        if (this.phase !== 'lobby') return;
        p.role = msg.role;
        this.broadcastLobby();
        return;
      case 'setReady':
        if (this.phase !== 'lobby') return;
        p.ready = msg.ready;
        this.broadcastLobby();
        return;
      case 'startGame':
        if (this.phase !== 'lobby') return;
        if (p.isHost && this.canStart()) {
          if (this.opts.skipDraft) this.startGame();
          else this.startDraft();
        }
        else this.send(p.conn, { t: 'error', message: 'Impossible de lancer : 8 joueurs prêts requis (ou mode dev).' });
        return;
      case 'draftHover':
        this.draftHover(p, msg.championId);
        return;
      case 'draftLock':
        this.draftLock(p, msg.championId);
        return;
      case 'draftSummoners':
        this.draftSummoners(p, msg.summoners);
        return;
      case 'returnToLobby':
        if (this.phase === 'end') this.backToLobby();
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

  // ------------------------------------------------------------------ draft

  startDraft(): void {
    const blue = [...this.players.values()].filter((p) => p.team === 'blue').map((p) => p.id);
    const red = [...this.players.values()].filter((p) => p.team === 'red').map((p) => p.id);
    const order: PlayerId[][] = [
      [blue[0]],
      [red[0], red[1]],
      [blue[1], blue[2]],
      [red[2], red[3]],
      [blue[3]],
    ].map((t) => t.filter((x): x is PlayerId => x !== undefined));
    const turns = order.filter((t) => t.length > 0);
    const picks = new Map<PlayerId, DraftPick>();
    for (const p of this.players.values()) {
      picks.set(p.id, {
        playerId: p.id,
        team: p.team,
        championId: null,
        hoverChampionId: null,
        locked: false,
        summoners: p.role === 'jungle' ? ['flash', 'smite'] : ['flash', 'ignite'],
      });
    }
    this.draft = { turns, currentTurn: 0, turnEndsAt: this.now() + this.timings.turn * 1000, picks };
    this.phase = 'draft';
    this.broadcastLobby();
    this.broadcastDraft();
    this.startTimer();
  }

  private isTaken(championId: string, except?: PlayerId): boolean {
    for (const pick of this.draft!.picks.values()) if (pick.playerId !== except && pick.locked && pick.championId === championId) return true;
    return false;
  }

  private draftHover(p: RoomPlayer, championId: string): void {
    const pick = this.draft?.picks.get(p.id);
    if (this.phase !== 'draft' || !pick || pick.locked || !CHAMPIONS[championId]) return;
    pick.hoverChampionId = championId;
    this.broadcastDraft();
  }

  private draftLock(p: RoomPlayer, championId: string): void {
    const d = this.draft;
    const pick = d?.picks.get(p.id);
    if (this.phase !== 'draft' || !d || !pick || pick.locked || d.finalizeEndsAt !== undefined) return;
    if (!d.turns[d.currentTurn]?.includes(p.id)) return this.send(p.conn, { t: 'error', message: "Ce n'est pas ton tour." });
    if (!CHAMPIONS[championId]) return;
    if (this.isTaken(championId)) return this.send(p.conn, { t: 'error', message: 'Champion déjà choisi.' });
    pick.championId = championId;
    pick.hoverChampionId = championId;
    pick.locked = true;
    this.advanceIfTurnDone();
    this.broadcastDraft();
  }

  private draftSummoners(p: RoomPlayer, summoners: [SummonerId, SummonerId]): void {
    const pick = this.draft?.picks.get(p.id);
    if (this.phase !== 'draft' || !pick) return;
    if (!Array.isArray(summoners) || summoners.length !== 2) return;
    const [a, b] = summoners;
    if (!SUMMONERS[a] || !SUMMONERS[b] || a === b) return;
    pick.summoners = [a, b];
    this.broadcastDraft();
  }

  private advanceIfTurnDone(): void {
    const d = this.draft!;
    const turn = d.turns[d.currentTurn] ?? [];
    if (!turn.every((id) => d.picks.get(id)!.locked)) return;
    d.currentTurn++;
    if (d.currentTurn >= d.turns.length) {
      d.finalizeEndsAt = this.now() + this.timings.finalize * 1000;
      d.turnEndsAt = d.finalizeEndsAt;
    } else {
      d.turnEndsAt = this.now() + this.timings.turn * 1000;
    }
  }

  /** Expiration du tour : lock du survol s'il est libre, sinon champion aléatoire disponible. */
  private autoPick(): void {
    const d = this.draft!;
    for (const id of d.turns[d.currentTurn] ?? []) {
      const pick = d.picks.get(id)!;
      if (pick.locked) continue;
      let champ = pick.hoverChampionId && !this.isTaken(pick.hoverChampionId) ? pick.hoverChampionId : null;
      if (!champ) {
        const free = CHAMPION_LIST.filter((c) => !this.isTaken(c.id)).map((c) => c.id);
        champ = free[Math.floor(this.random() * free.length)] ?? CHAMPION_LIST[0]!.id;
      }
      pick.championId = champ;
      pick.hoverChampionId = champ;
      pick.locked = true;
    }
    this.advanceIfTurnDone();
  }

  /** Vue du draft pour une équipe : survols et sorts d'invocateur des ennemis masqués. */
  draftStateFor(team: Team): DraftState {
    const d = this.draft!;
    return {
      turns: d.turns,
      currentTurn: d.currentTurn,
      turnEndsAt: d.turnEndsAt,
      ...(d.finalizeEndsAt !== undefined ? { finalizeEndsAt: d.finalizeEndsAt } : {}),
      picks: [...d.picks.values()].map((p) =>
        p.team === team || this.phase !== 'draft'
          ? { ...p, summoners: [...p.summoners] as [SummonerId, SummonerId] }
          : { ...p, hoverChampionId: null, summoners: ['flash', 'ignite'] as [SummonerId, SummonerId] },
      ),
    };
  }

  private broadcastDraft(): void {
    if (!this.draft) return;
    const views: Record<Team, string> = {
      blue: encode({ t: 'draft', state: this.draftStateFor('blue') }),
      red: encode({ t: 'draft', state: this.draftStateFor('red') }),
    };
    for (const p of this.players.values()) p.conn?.send(views[p.team]);
  }

  // ------------------------------------------------------------------ timers (draft, chargement, salle vide)

  /** Avance les timers hors simulation. Appelé toutes les 250 ms (ou manuellement en test). */
  update(): void {
    const now = this.now();
    if (this.phase === 'draft' && this.draft) {
      const d = this.draft;
      if (d.finalizeEndsAt !== undefined) {
        if (now >= d.finalizeEndsAt) this.startLoading();
      } else if (now >= d.turnEndsAt) {
        this.autoPick();
        this.broadcastDraft();
      }
    } else if (this.phase === 'loading' && now >= this.loadingEndsAt) {
      this.startGame();
    }
  }

  private startTimer(): void {
    if (this.opts.manual || this.timer) return;
    this.timer = setInterval(() => this.update(), 250);
  }

  private startLoading(): void {
    this.phase = 'loading';
    this.loadingEndsAt = this.now() + this.timings.loading * 1000;
    this.broadcastLobby();
    this.broadcastDraft();
    this.broadcast({ t: 'loading', endsAt: this.loadingEndsAt });
  }

  // ------------------------------------------------------------------ partie

  /** Crée le monde et démarre la simulation à partir des picks du draft (ou d'attributions par défaut). */
  startGame(): void {
    if (this.timer) clearInterval(this.timer);
    this.timer = null;
    const world = new World(this.opts.systems ? this.opts.systems() : createSystems());
    const used = new Set<string>();
    const slot: Record<Team, number> = { blue: 0, red: 0 };
    for (const p of this.players.values()) {
      const pick = this.draft?.picks.get(p.id);
      let championId = pick?.championId ?? null;
      if (!championId || used.has(championId)) championId = CHAMPION_LIST.find((c) => !used.has(c.id))!.id;
      used.add(championId);
      const i = slot[p.team]++;
      const f = FOUNTAINS[p.team];
      const sign = p.team === 'blue' ? 1 : -1;
      createChampion(world, {
        playerId: p.id,
        name: p.name,
        team: p.team,
        championId,
        summoners: pick?.summoners ?? ['flash', 'ignite'],
        pos: { x: f.x + sign * (150 + (i % 2) * 150), y: f.y + sign * (150 + Math.floor(i / 2) * 150) },
      });
    }
    this.score = new ScoreTracker(world);
    world.init();
    this.world = world;
    this.phase = 'game';
    this.gameStart = {
      serverTime: this.now(),
      players: [...this.players.values()].map((o) => {
        const c = world.championOf(o.id)!.champ!;
        return { id: o.id, name: o.name, team: o.team, championId: c.championId, summoners: [c.summoners.D, c.summoners.F] };
      }),
    };
    this.broadcastLobby();
    for (const p of this.players.values()) {
      p.view.reset();
      this.sendGameStart(p);
    }
    this.snapAcc = 0;
    if (!this.opts.manual) this.startLoop();
  }

  private sendGameStart(p: RoomPlayer): void {
    const ent = this.world?.championOf(p.id);
    if (!ent || !this.gameStart) return;
    this.send(p.conn, { t: 'gameStart', start: { ...this.gameStart, you: p.id, team: p.team, entityId: ent.id, serverTime: this.now() } });
  }

  private startLoop(): void {
    let last = this.now();
    let acc = 0;
    const stepMs = 1000 / TICK_RATE;
    this.loop = setInterval(() => {
      const now = this.now();
      acc += now - last;
      last = now;
      // Rattrape le retard sans spirale (max 4 ticks par réveil).
      let n = 0;
      while (acc >= stepMs && n < 4 && this.phase === 'game') {
        acc -= stepMs;
        n++;
        this.tickGame();
      }
      if (acc > stepMs * 4) acc = 0;
    }, stepMs / 2);
  }

  /** Un tick de simulation + envoi des snapshots au rythme SNAPSHOT_RATE. */
  tickGame(): void {
    const world = this.world;
    if (!world || this.phase !== 'game') return;
    world.step(TICK_DT);
    this.snapAcc += TICK_DT;
    if (this.snapAcc >= 1 / SNAPSHOT_RATE - 1e-6 || world.winner) {
      this.snapAcc -= 1 / SNAPSHOT_RATE;
      if (this.snapAcc > 1 / SNAPSHOT_RATE) this.snapAcc = 0;
      this.sendSnapshots();
    }
    if (world.winner) this.endGame(world.winner);
  }

  sendSnapshots(): void {
    const world = this.world!;
    const frame = new SnapshotFrame(world, world.events.drain(), this.score!.score());
    for (const p of this.players.values()) {
      if (!p.conn) continue;
      p.conn.send(encode({ t: 'snapshot', snap: p.view.build(frame, p.id, p.team) }));
    }
  }

  endGame(winner: Team): void {
    const world = this.world!;
    this.result = {
      winner,
      duration: Math.round(world.time),
      stats: [...this.players.values()].map((p) => {
        const e = world.championOf(p.id);
        const c = e?.champ;
        return {
          playerId: p.id,
          championId: c?.championId ?? '',
          team: p.team,
          kills: c?.kills ?? 0,
          deaths: c?.deaths ?? 0,
          assists: c?.assists ?? 0,
          cs: c?.cs ?? 0,
          gold: Math.round((e?.data.goldEarned as number | undefined) ?? c?.gold ?? 0),
          damageToChampions: Math.round(c?.damageToChampions ?? 0),
          items: c?.items ?? [],
        };
      }),
    };
    this.stop();
    this.phase = 'end';
    this.broadcastLobby();
    this.broadcast({ t: 'gameOver', result: this.result });
  }

  private backToLobby(): void {
    this.stop();
    this.phase = 'lobby';
    this.world = null;
    this.draft = null;
    this.result = null;
    this.gameStart = null;
    for (const [id, p] of this.players) {
      if (!p.connected) this.players.delete(id);
      else p.ready = false;
    }
    if (![...this.players.values()].some((p) => p.isHost)) {
      const first = this.players.values().next().value;
      if (first) first.isHost = true;
    }
    this.broadcastLobby();
  }

  stop(): void {
    if (this.loop) clearInterval(this.loop);
    if (this.timer) clearInterval(this.timer);
    this.loop = null;
    this.timer = null;
    if (this.phase === 'game') this.phase = 'end';
  }

  // ------------------------------------------------------------------ envoi

  lobbyState(): LobbyState {
    return {
      code: this.code,
      phase: this.phase,
      devMode: this.devMode,
      players: [...this.players.values()].map(({ conn: _c, view: _v, ...p }) => p),
    };
  }

  broadcastLobby(): void {
    this.broadcast({ t: 'lobby', state: this.lobbyState() });
  }

  broadcast(msg: ServerMsg): void {
    const raw = encode(msg);
    for (const p of this.players.values()) p.conn?.send(raw);
  }

  send(conn: Conn | null, msg: ServerMsg): void {
    conn?.send(encode(msg));
  }
}
