import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import { WebSocket } from 'ws';
import { PROTOCOL_VERSION, WS_PATH, type ClientMsg, type ServerMsg } from '@lov/shared';
import { createGameServer, type GameServer } from '../src/net/server.ts';
import { RateLimiter, parseClientMsg } from '../src/net/validate.ts';

describe('validation des messages', () => {
  it('accepte les messages valides et rejette les malformés', () => {
    expect(parseClientMsg(JSON.stringify({ t: 'move', pos: { x: 10, y: 20 } }))).toEqual({ t: 'move', pos: { x: 10, y: 20 } });
    expect(parseClientMsg('{"t":"move","pos":{"x":"a","y":1}}')).toBeNull();
    expect(parseClientMsg('{"t":"move","pos":{"x":1e309,"y":1}}')).toBeNull();
    expect(parseClientMsg('{"t":"attack","targetId":-1}')).toBeNull();
    expect(parseClientMsg('{"t":"inconnu"}')).toBeNull();
    expect(parseClientMsg('pas du json')).toBeNull();
    expect(parseClientMsg(JSON.stringify({ t: 'draftSummoners', summoners: ['flash', 'nuke'] }))).toBeNull();
    expect(parseClientMsg(JSON.stringify({ t: 'chat', text: 'x'.repeat(500), teamOnly: true }))).toBeNull();
    expect(parseClientMsg(JSON.stringify({ t: 'cast', slot: 'Q', targetPos: { x: 1, y: 2 } }))).not.toBeNull();
  });

  it('rate-limit à jetons', () => {
    let t = 0;
    const rl = new RateLimiter(10, 5, () => t);
    let ok = 0;
    for (let i = 0; i < 20; i++) if (rl.take()) ok++;
    expect(ok).toBe(5);
    t += 1000;
    for (let i = 0; i < 20; i++) if (rl.take()) ok++;
    expect(ok).toBe(10);
  });
});

class Client {
  ws: WebSocket;
  msgs: ServerMsg[] = [];
  private waiters: { pred: (m: ServerMsg) => boolean; res: (m: ServerMsg) => void }[] = [];
  constructor(url: string) {
    this.ws = new WebSocket(url);
    this.ws.on('message', (d) => {
      const m = JSON.parse(d.toString()) as ServerMsg;
      this.msgs.push(m);
      this.waiters = this.waiters.filter((w) => (w.pred(m) ? (w.res(m), false) : true));
    });
  }
  open() {
    return new Promise<void>((r) => this.ws.once('open', () => r()));
  }
  send(m: ClientMsg) {
    this.ws.send(JSON.stringify(m));
  }
  wait<T extends ServerMsg['t']>(t: T, pred: (m: Extract<ServerMsg, { t: T }>) => boolean = () => true, ms = 3000): Promise<Extract<ServerMsg, { t: T }>> {
    const found = this.msgs.find((m) => m.t === t && pred(m as Extract<ServerMsg, { t: T }>));
    if (found) return Promise.resolve(found as Extract<ServerMsg, { t: T }>);
    return new Promise((res, rej) => {
      const timer = setTimeout(() => rej(new Error(`timeout ${t}`)), ms);
      this.waiters.push({ pred: (m) => m.t === t && pred(m as Extract<ServerMsg, { t: T }>), res: (m) => (clearTimeout(timer), res(m as Extract<ServerMsg, { t: T }>)) });
    });
  }
}

describe('serveur WebSocket', () => {
  let server: GameServer;
  let url: string;
  beforeAll(async () => {
    server = await createGameServer({ port: 0, log: () => {} });
    url = `ws://127.0.0.1:${server.port()}${WS_PATH}`;
  });
  afterAll(() => server.close());

  it('hello → welcome avec token secret, création et jonction de salle, ping réseau', async () => {
    const a = new Client(url);
    await a.open();
    a.send({ t: 'hello', name: 'Alice', version: PROTOCOL_VERSION });
    const w = await a.wait('welcome');
    expect(w.reconnectToken).not.toBe(w.playerId);
    a.send({ t: 'createRoom', devMode: true });
    const lobby = await a.wait('lobby');
    const b = new Client(url);
    await b.open();
    b.send({ t: 'hello', name: 'Bob', version: PROTOCOL_VERSION });
    await b.wait('welcome');
    b.send({ t: 'joinRoom', code: lobby.state.code.toLowerCase() });
    const l2 = await b.wait('lobby', (m) => m.state.players.length === 2);
    expect(l2.state.players.map((p) => p.team).sort()).toEqual(['blue', 'red']);
    b.send({ t: 'netPing', clientTime: 42 });
    expect((await b.wait('pong')).clientTime).toBe(42);
    a.ws.close();
    b.ws.close();
  });

  it('reconnexion avec le token pendant le draft', async () => {
    const a = new Client(url);
    await a.open();
    a.send({ t: 'hello', name: 'Alice', version: PROTOCOL_VERSION });
    const w = await a.wait('welcome');
    a.send({ t: 'createRoom', devMode: true });
    await a.wait('lobby');
    a.send({ t: 'startGame' });
    await a.wait('draft');
    a.ws.close();
    await new Promise((r) => setTimeout(r, 50));
    const a2 = new Client(url);
    await a2.open();
    a2.send({ t: 'hello', name: 'Alice', version: PROTOCOL_VERSION, reconnectToken: w.reconnectToken });
    const w2 = await a2.wait('welcome');
    expect(w2.playerId).toBe(w.playerId);
    const d = await a2.wait('draft');
    expect(d.state.picks[0]!.playerId).toBe(w.playerId);
    // Usurper le playerId public ne donne pas la session.
    const c = new Client(url);
    await c.open();
    c.send({ t: 'hello', name: 'Eve', version: PROTOCOL_VERSION, reconnectToken: w.playerId });
    expect((await c.wait('welcome')).playerId).not.toBe(w.playerId);
    a2.ws.close();
    c.ws.close();
  });

  it('/health', async () => {
    const res = await fetch(`http://127.0.0.1:${server.port()}/health`);
    expect((await res.json()) as { ok: boolean }).toMatchObject({ ok: true });
  });
});
