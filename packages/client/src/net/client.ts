/**
 * Connexion WebSocket au serveur. Propriétaire : claude-client.
 * TODO(claude-client) : reconnexion auto, ping/latence, buffer de snapshots pour l'interpolation.
 */
import { PROTOCOL_VERSION, WS_PATH, decode, encode, type ClientMsg, type ServerMsg } from '@lov/shared';
import { store } from '../state.ts';

type Handler = (msg: ServerMsg) => void;

export class NetClient {
  private ws: WebSocket | null = null;
  private handlers = new Set<Handler>();
  private queue: string[] = [];

  connect(name: string): void {
    const proto = location.protocol === 'https:' ? 'wss' : 'ws';
    const ws = new WebSocket(`${proto}://${location.host}${WS_PATH}`);
    this.ws = ws;
    ws.onopen = () => {
      store.set({ connected: true });
      ws.send(encode({ t: 'hello', name, version: PROTOCOL_VERSION, reconnectToken: sessionGet('lov.token') ?? undefined }));
      this.queue.forEach((m) => ws.send(m));
      this.queue = [];
    };
    ws.onclose = () => store.set({ connected: false });
    ws.onmessage = (ev) => {
      const msg = decode<ServerMsg>(ev.data as string);
      this.route(msg);
      this.handlers.forEach((h) => h(msg));
    };
  }

  send(msg: ClientMsg): void {
    const raw = encode(msg);
    if (this.ws?.readyState === WebSocket.OPEN) this.ws.send(raw);
    else this.queue.push(raw);
  }

  on(h: Handler): () => void {
    this.handlers.add(h);
    return () => this.handlers.delete(h);
  }

  private route(msg: ServerMsg): void {
    switch (msg.t) {
      case 'welcome':
        sessionSet('lov.token', msg.reconnectToken);
        store.set({ playerId: msg.playerId });
        break;
      case 'error':
        store.set({ error: msg.message });
        break;
      case 'lobby':
        store.set({ lobby: msg.state, error: null });
        break;
      case 'draft':
        store.set({ draft: msg.state });
        break;
      case 'gameStart':
        store.set({ game: msg.start });
        break;
      case 'snapshot':
        store.set({ snapshot: msg.snap });
        break;
      case 'gameOver':
        store.set({ result: msg.result });
        break;
      default:
        break;
    }
  }
}

function sessionGet(k: string): string | null {
  try {
    return sessionStorage.getItem(k);
  } catch {
    return null;
  }
}
function sessionSet(k: string, v: string): void {
  try {
    sessionStorage.setItem(k, v);
  } catch {
    /* stockage indisponible */
  }
}

export const net = new NetClient();
