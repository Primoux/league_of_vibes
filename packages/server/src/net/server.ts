/**
 * Serveur HTTP (build client + /health) et WebSocket (/ws). Propriétaire : claude-server.
 * Sessions : chaque connexion reçoit un `reconnectToken` SECRET (différent du playerId public) ; un `hello`
 * avec ce token reprend le même joueur et sa salle (reconnexion en cours de partie).
 */
import { randomUUID } from 'node:crypto';
import { createReadStream, existsSync, statSync } from 'node:fs';
import { createServer, type Server } from 'node:http';
import { extname, join, normalize } from 'node:path';
import { WebSocketServer, type WebSocket } from 'ws';
import { PROTOCOL_VERSION, WS_PATH, encode, type PlayerId, type ServerMsg } from '@lov/shared';
import { EMPTY_ROOM_GRACE_MS, Room } from '../room.ts';
import { RateLimiter, parseClientMsg } from './validate.ts';

const MIME: Record<string, string> = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript',
  '.css': 'text/css',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.glb': 'model/gltf-binary',
  '.ogg': 'audio/ogg',
  '.mp3': 'audio/mpeg',
  '.wav': 'audio/wav',
  '.json': 'application/json',
  '.woff2': 'font/woff2',
};

/** Messages / s autorisés par connexion (moyenne) et rafale. Au-delà de MAX_STRIKES dépassements : déconnexion. */
export const MSG_RATE = 40;
export const MSG_BURST = 80;
export const MAX_STRIKES = 400;

interface Session {
  playerId: PlayerId;
  name: string;
  room: Room | null;
}

export interface GameServerOptions {
  port: number;
  devLobby?: boolean;
  clientDist?: string;
  log?: (msg: string) => void;
}

export interface GameServer {
  http: Server;
  rooms: Map<string, Room>;
  close(): Promise<void>;
  port(): number;
}

export function createGameServer(opts: GameServerOptions): Promise<GameServer> {
  const log = opts.log ?? ((m: string) => console.log(m));
  const rooms = new Map<string, Room>();
  const sessions = new Map<string, Session>(); // token secret -> session
  const started = Date.now();

  const newCode = (): string => {
    const letters = 'ABCDEFGHJKLMNPQRSTUVWXYZ';
    let code = '';
    do code = Array.from({ length: 5 }, () => letters[Math.floor(Math.random() * letters.length)]).join('');
    while (rooms.has(code));
    return code;
  };

  const http = createServer((req, res) => {
    const url = new URL(req.url ?? '/', 'http://x');
    if (url.pathname === '/health') {
      res.setHeader('Content-Type', 'application/json');
      let players = 0;
      for (const r of rooms.values()) for (const p of r.players.values()) if (p.connected) players++;
      return void res.end(JSON.stringify({ ok: true, uptime: Math.round((Date.now() - started) / 1000), rooms: rooms.size, players }));
    }
    const dist = opts.clientDist;
    if (!dist) {
      res.statusCode = 404;
      return void res.end('Pas de client');
    }
    let file = normalize(join(dist, decodeURIComponent(url.pathname)));
    if (!file.startsWith(dist) || !existsSync(file) || statSync(file).isDirectory()) file = join(dist, 'index.html');
    if (!existsSync(file)) {
      res.statusCode = 404;
      return void res.end('Client non construit : lancez `npm run build` ou utilisez `npm run dev` (Vite sur :5173).');
    }
    res.setHeader('Content-Type', MIME[extname(file)] ?? 'application/octet-stream');
    if (file.includes(`${join(dist, 'assets')}`)) res.setHeader('Cache-Control', 'public, max-age=31536000, immutable');
    createReadStream(file).pipe(res);
  });

  // permessage-deflate : les snapshots JSON se compressent ~10x (≈ 12 Ko/s par client dans le pire cas mesuré).
  const wss = new WebSocketServer({
    server: http,
    path: WS_PATH,
    maxPayload: 8 * 1024,
    perMessageDeflate: { threshold: 512, zlibDeflateOptions: { level: 3 }, serverNoContextTakeover: false, clientNoContextTakeover: true },
  });

  const janitor = setInterval(() => {
    const now = Date.now();
    for (const [code, room] of rooms) {
      if (!room.empty) continue;
      if (room.phase === 'lobby' || (room.emptySince !== null && now - room.emptySince > EMPTY_ROOM_GRACE_MS)) {
        room.stop();
        rooms.delete(code);
        for (const [tok, s] of sessions) if (s.room === room) sessions.delete(tok);
      }
    }
  }, 5000);

  wss.on('connection', (ws: WebSocket) => {
    let token: string | null = null;
    let session: Session | null = null;
    const limiter = new RateLimiter(MSG_RATE, MSG_BURST);
    const conn = { send: (raw: string) => void (ws.readyState === ws.OPEN && ws.send(raw)) };
    const send = (m: ServerMsg) => conn.send(encode(m));

    ws.on('message', (data) => {
      if (!limiter.take()) {
        if (limiter.strikes > MAX_STRIKES) ws.close(1008, 'spam');
        return;
      }
      const msg = parseClientMsg(data.toString());
      if (!msg) return;
      if (msg.t === 'netPing') return send({ t: 'pong', serverTime: Date.now(), clientTime: msg.clientTime });

      if (msg.t === 'hello') {
        const name = msg.name.trim().slice(0, 16) || 'Invocateur';
        const prev = msg.reconnectToken ? sessions.get(msg.reconnectToken) : undefined;
        if (prev && msg.reconnectToken) {
          token = msg.reconnectToken;
          session = prev;
        } else {
          token = randomUUID();
          session = { playerId: randomUUID().slice(0, 8), name, room: null };
          sessions.set(token, session);
        }
        send({ t: 'welcome', playerId: session.playerId, reconnectToken: token, version: PROTOCOL_VERSION });
        if (msg.version !== PROTOCOL_VERSION) send({ t: 'error', message: 'Version du client différente du serveur : recharge la page.' });
        // Reprise de la salle (reconnexion).
        if (prev?.room && rooms.get(prev.room.code) === prev.room) prev.room.join(prev.playerId, prev.name, conn);
        else if (prev) prev.room = null;
        return;
      }
      if (!session) return send({ t: 'error', message: 'hello attendu' });
      const s = session;

      switch (msg.t) {
        case 'createRoom': {
          s.room?.leave(s.playerId);
          const room = new Room(newCode(), !!opts.devLobby || !!msg.devMode);
          rooms.set(room.code, room);
          room.join(s.playerId, s.name, conn);
          s.room = room;
          log(`[lov] salle ${room.code} créée${room.devMode ? ' (dev)' : ''}`);
          return;
        }
        case 'joinRoom': {
          const target = rooms.get(msg.code.trim().toUpperCase());
          if (!target) return send({ t: 'error', message: 'Salle introuvable' });
          if (s.room !== target) s.room?.leave(s.playerId);
          const err = target.join(s.playerId, s.name, conn);
          if (err) return send({ t: 'error', message: err });
          s.room = target;
          return;
        }
        case 'leaveRoom':
          s.room?.leave(s.playerId);
          s.room = null;
          return;
        default:
          s.room?.handle(s.playerId, msg);
      }
    });

    ws.on('close', () => {
      if (!session?.room) return;
      // On ne quitte la salle que si cette connexion est toujours celle du joueur (pas d'onglet plus récent).
      const p = session.room.players.get(session.playerId);
      if (p && p.conn === conn) session.room.leave(session.playerId);
    });
  });

  return new Promise((resolve) => {
    http.listen(opts.port, () => {
      resolve({
        http,
        rooms,
        port: () => {
          const a = http.address();
          return typeof a === 'object' && a ? a.port : opts.port;
        },
        close: () =>
          new Promise<void>((done) => {
            clearInterval(janitor);
            for (const r of rooms.values()) r.stop();
            for (const c of wss.clients) c.terminate();
            wss.close();
            http.close(() => done());
          }),
      });
    });
  });
}
