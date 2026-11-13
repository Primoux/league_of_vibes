import { describe, expect, it } from 'vitest';
import { DRAFT_TURN_SECONDS, LOADING_SECONDS, type ServerMsg } from '@lov/shared';
import { FINALIZE_SECONDS, Room } from '../src/room.ts';
import { combatSystem } from '../src/systems/combat.ts';
import { inputSystem } from '../src/systems/input.ts';
import { movementSystem } from '../src/systems/movement.ts';
import { visionSystem } from '../src/systems/vision.ts';

class FakeConn {
  msgs: ServerMsg[] = [];
  send(raw: string) {
    this.msgs.push(JSON.parse(raw) as ServerMsg);
  }
  last<T extends ServerMsg['t']>(t: T): Extract<ServerMsg, { t: T }> | undefined {
    for (let i = this.msgs.length - 1; i >= 0; i--) if (this.msgs[i]!.t === t) return this.msgs[i] as Extract<ServerMsg, { t: T }>;
    return undefined;
  }
}

function setup(n = 8) {
  let now = 1_000_000;
  const clock = { now: () => now };
  const room = new Room('ABCDE', false, { clock, manual: true, random: () => 0, systems: () => [inputSystem, movementSystem, combatSystem, visionSystem] });
  const conns = new Map<string, FakeConn>();
  for (let i = 0; i < n; i++) {
    const c = new FakeConn();
    conns.set(`p${i}`, c);
    room.join(`p${i}`, `Joueur${i}`, c);
  }
  return { room, conns, advance: (s: number) => ((now += s * 1000), room.update()) };
}

const ready = (room: Room) => {
  for (const id of room.players.keys()) room.handle(id, { t: 'setReady', ready: true });
};

describe('Room — lobby et draft', () => {
  it('équilibre les équipes et exige 8 joueurs prêts', () => {
    const { room, conns } = setup();
    const teams = [...room.players.values()].map((p) => p.team);
    expect(teams.filter((t) => t === 'blue')).toHaveLength(4);
    room.handle('p0', { t: 'startGame' });
    expect(room.phase).toBe('lobby');
    expect(conns.get('p0')!.last('error')).toBeDefined();
    ready(room);
    room.handle('p1', { t: 'startGame' }); // pas hôte
    expect(room.phase).toBe('lobby');
    room.handle('p0', { t: 'startGame' });
    expect(room.phase).toBe('draft');
  });

  it('ordre 1-2-2-2-1 et picks uniques', () => {
    const { room } = setup();
    ready(room);
    room.handle('p0', { t: 'startGame' });
    const d = room.draft!;
    const team = (id: string) => room.players.get(id)!.team;
    expect(d.turns.map((t) => t.length)).toEqual([1, 2, 2, 2, 1]);
    expect(d.turns.map((t) => team(t[0]!))).toEqual(['blue', 'red', 'blue', 'red', 'blue']);
    const [b1] = d.turns[0]!;
    const [r1, r2] = d.turns[1]!;
    // Hors tour : refusé
    room.handle(r1!, { t: 'draftLock', championId: 'nyx' });
    expect(d.picks.get(r1!)!.locked).toBe(false);
    room.handle(b1!, { t: 'draftLock', championId: 'nyx' });
    expect(d.currentTurn).toBe(1);
    // Champion déjà pris : refusé
    room.handle(r1!, { t: 'draftLock', championId: 'nyx' });
    expect(d.picks.get(r1!)!.locked).toBe(false);
    room.handle(r1!, { t: 'draftLock', championId: 'sable' });
    expect(d.currentTurn).toBe(1);
    room.handle(r2!, { t: 'draftLock', championId: 'ilyra' });
    expect(d.currentTurn).toBe(2);
  });

  it('timeout : lock du survol ou pick aléatoire, puis finalisation, chargement et partie', () => {
    const { room, conns, advance } = setup();
    ready(room);
    room.handle('p0', { t: 'startGame' });
    const d = room.draft!;
    const [b1] = d.turns[0]!;
    room.handle(b1!, { t: 'draftHover', championId: 'zephyr' });
    advance(DRAFT_TURN_SECONDS + 0.1);
    expect(d.picks.get(b1!)!.championId).toBe('zephyr');
    for (let i = 0; i < 4; i++) advance(DRAFT_TURN_SECONDS + 0.1);
    expect(d.finalizeEndsAt).toBeDefined();
    const ids = [...d.picks.values()].map((p) => p.championId);
    expect(new Set(ids).size).toBe(8);
    room.handle(b1!, { t: 'draftSummoners', summoners: ['flash', 'smite'] });
    room.handle(b1!, { t: 'draftSummoners', summoners: ['flash', 'flash'] }); // invalide
    expect(d.picks.get(b1!)!.summoners).toEqual(['flash', 'smite']);
    advance(FINALIZE_SECONDS + 0.1);
    expect(room.phase).toBe('loading');
    expect(conns.get('p3')!.last('loading')).toBeDefined();
    advance(LOADING_SECONDS + 0.1);
    expect(room.phase).toBe('game');
    const start = conns.get(b1!)!.last('gameStart')!.start;
    expect(start.players).toHaveLength(8);
    expect(start.players.find((p) => p.id === b1)).toMatchObject({ championId: 'zephyr', summoners: ['flash', 'smite'] });
    expect(room.world!.championOf(b1!)!.champ!.summoners).toEqual({ D: 'flash', F: 'smite' });
  });

  it("les survols ne sont visibles que des alliés", () => {
    const { room, conns } = setup();
    ready(room);
    room.handle('p0', { t: 'startGame' });
    const [b1] = room.draft!.turns[0]!;
    room.handle(b1!, { t: 'draftHover', championId: 'kaelen' });
    const blueConn = [...room.players.values()].find((p) => p.team === 'blue' && p.id !== b1)!;
    const redConn = [...room.players.values()].find((p) => p.team === 'red')!;
    const hoverSeenBy = (id: string) => conns.get(id)!.last('draft')!.state.picks.find((p) => p.playerId === b1)!.hoverChampionId;
    expect(hoverSeenBy(blueConn.id)).toBe('kaelen');
    expect(hoverSeenBy(redConn.id)).toBeNull();
  });
});

describe('Room — partie', () => {
  function inGame() {
    const s = setup(2);
    const room = s.room;
    room.devMode = true;
    room.handle('p0', { t: 'startGame' });
    for (const turn of room.draft!.turns) for (const id of turn) room.handle(id, { t: 'draftLock', championId: id === 'p0' ? 'grondak' : 'sable' });
    s.advance(FINALIZE_SECONDS + 0.1);
    s.advance(LOADING_SECONDS + 0.1);
    expect(room.phase).toBe('game');
    return s;
  }

  it('envoie un snapshot complet puis des deltas', () => {
    const { room, conns } = inGame();
    for (let i = 0; i < 6; i++) room.tickGame();
    const snaps = conns.get('p0')!.msgs.filter((m) => m.t === 'snapshot').map((m) => (m as Extract<ServerMsg, { t: 'snapshot' }>).snap);
    expect(snaps.length).toBeGreaterThanOrEqual(3);
    expect(snaps[0]!.removed).toBeUndefined();
    expect(snaps[0]!.self?.entityId).toBe(room.world!.championOf('p0')!.id);
    expect(snaps[1]!.removed).toEqual([]);
    // Rien ne bouge : delta vide.
    expect(snaps[2]!.entities).toHaveLength(0);
  });

  it('reconnexion : gameStart renvoyé et snapshot complet', () => {
    const { room } = inGame();
    room.tickGame();
    room.leave('p1');
    expect(room.players.get('p1')!.connected).toBe(false);
    const c2 = new FakeConn();
    expect(room.join('p1', 'Joueur1', c2)).toBeNull();
    expect(c2.last('gameStart')!.start.you).toBe('p1');
    room.tickGame();
    room.tickGame();
    const snap = (c2.msgs.find((m) => m.t === 'snapshot') as Extract<ServerMsg, { t: 'snapshot' }>).snap;
    expect(snap.removed).toBeUndefined();
    expect(snap.entities.length).toBeGreaterThan(0);
  });

  it('fin de partie : gameOver avec stats puis retour au lobby', () => {
    const { room, conns } = inGame();
    room.world!.winner = 'red';
    room.tickGame();
    expect(room.phase).toBe('end');
    const over = conns.get('p0')!.last('gameOver')!.result;
    expect(over.winner).toBe('red');
    expect(over.stats).toHaveLength(2);
    expect(over.stats[0]).toMatchObject({ kills: 0, deaths: 0 });
    room.handle('p1', { t: 'returnToLobby' });
    expect(room.phase).toBe('lobby');
    expect(room.world).toBeNull();
  });
});
