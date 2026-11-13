/**
 * Point d'entrée serveur : HTTP (sert le build client en production) + WebSocket sur /ws.
 * Propriétaire : claude-server.
 */
import { randomUUID } from 'node:crypto';
import { createReadStream, existsSync, statSync } from 'node:fs';
import { createServer } from 'node:http';
import { extname, join, normalize } from 'node:path';
import { fileURLToPath } from 'node:url';
import { WebSocketServer, type WebSocket } from 'ws';
import { PROTOCOL_VERSION, WS_PATH, decode, encode, type ClientMsg, type ServerMsg } from '@lov/shared';
import { Room } from './room.ts';

const PORT = Number(process.env.PORT ?? 3000);
const DEV_LOBBY = process.env.DEV_LOBBY === '1';
const CLIENT_DIST = fileURLToPath(new URL('../../client/dist', import.meta.url));
const MIME: Record<string, string> = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.svg': 'image/svg+xml', '.png': 'image/png', '.glb': 'model/gltf-binary', '.ogg': 'audio/ogg', '.mp3': 'audio/mpeg', '.json': 'application/json' };

const rooms = new Map<string, Room>();

function newCode(): string {
  const letters = 'ABCDEFGHJKLMNPQRSTUVWXYZ';
  let code = '';
  do code = Array.from({ length: 5 }, () => letters[Math.floor(Math.random() * letters.length)]).join('');
  while (rooms.has(code));
  return code;
}

const http = createServer((req, res) => {
  const url = new URL(req.url ?? '/', 'http://x');
  if (url.pathname === '/health') return void res.end('ok');
  let file = normalize(join(CLIENT_DIST, url.pathname));
  if (!file.startsWith(CLIENT_DIST) || !existsSync(file) || statSync(file).isDirectory()) file = join(CLIENT_DIST, 'index.html');
  if (!existsSync(file)) {
    res.statusCode = 404;
    return void res.end('Client non construit : lancez `npm run build` ou utilisez `npm run dev` (Vite sur :5173).');
  }
  res.setHeader('Content-Type', MIME[extname(file)] ?? 'application/octet-stream');
  createReadStream(file).pipe(res);
});

const wss = new WebSocketServer({ server: http, path: WS_PATH });

wss.on('connection', (ws: WebSocket) => {
  let playerId: string = randomUUID();
  let name = 'Invocateur';
  let room: Room | null = null;
  const conn = { send: (raw: string) => ws.readyState === ws.OPEN && ws.send(raw) };
  const send = (m: ServerMsg) => conn.send(encode(m));

  ws.on('message', (raw) => {
    let msg: ClientMsg;
    try {
      msg = decode<ClientMsg>(raw.toString());
    } catch {
      return;
    }
    switch (msg.t) {
      case 'hello':
        name = msg.name.slice(0, 16) || name;
        if (msg.reconnectToken) playerId = msg.reconnectToken; // TODO(claude-server): vrai token signé
        send({ t: 'welcome', playerId, reconnectToken: playerId, version: PROTOCOL_VERSION });
        return;
      case 'createRoom': {
        room?.leave(playerId);
        room = new Room(newCode(), DEV_LOBBY || !!msg.devMode);
        rooms.set(room.code, room);
        room.join(playerId, name, conn);
        return;
      }
      case 'joinRoom': {
        const target = rooms.get(msg.code.toUpperCase());
        if (!target) return send({ t: 'error', message: 'Salle introuvable' });
        room?.leave(playerId);
        const err = target.join(playerId, name, conn);
        if (err) return send({ t: 'error', message: err });
        room = target;
        return;
      }
      case 'leaveRoom':
        room?.leave(playerId);
        room = null;
        return;
      default:
        room?.handle(playerId, msg);
    }
  });

  ws.on('close', () => {
    if (!room) return;
    room.leave(playerId);
    if (room.empty) {
      room.stop();
      rooms.delete(room.code);
    }
  });
});

http.listen(PORT, () => console.log(`[lov] serveur sur http://localhost:${PORT} (ws ${WS_PATH})${DEV_LOBBY ? ' [DEV_LOBBY]' : ''}`));
