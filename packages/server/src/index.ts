/**
 * Point d'entrée serveur : HTTP (sert le build client en production) + WebSocket sur /ws.
 * Propriétaire : claude-server. Logique dans net/server.ts.
 */
import { fileURLToPath } from 'node:url';
import { WS_PATH } from '@lov/shared';
import { createGameServer } from './net/server.ts';

const PORT = Number(process.env.PORT ?? 3000);
const DEV_LOBBY = process.env.DEV_LOBBY === '1';
const SKIP_DRAFT = process.env.SKIP_DRAFT === '1';
const CLIENT_DIST = fileURLToPath(new URL('../../client/dist', import.meta.url));

const server = await createGameServer({ port: PORT, devLobby: DEV_LOBBY, skipDraft: SKIP_DRAFT, clientDist: CLIENT_DIST });
console.log(`[lov] serveur sur http://localhost:${server.port()} (ws ${WS_PATH})${DEV_LOBBY ? ' [DEV_LOBBY]' : ''}${SKIP_DRAFT ? ' [SKIP_DRAFT]' : ''}`);

const shutdown = () => void server.close().then(() => process.exit(0));
process.on('SIGINT', shutdown);
process.on('SIGTERM', shutdown);
