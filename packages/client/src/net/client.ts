/**
 * Connexion WebSocket au serveur. Propriétaire : claude-client.
 *
 * - Route les messages serveur vers le `store`.
 * - Snapshots : fusionnés dans `snapshots` (SnapshotBuffer, deltas appliqués), horloge `clock` synchronisée.
 *   `store.snapshot.entities` contient toujours la vue complète.
 * - Événements (`GameEvent`) : publiés sur `fxBus` au moment où le rendu interpolé atteint le snapshot
 *   (≈ INTERP_DELAY_MS après réception) pour que les FX coïncident avec les positions affichées.
 *   L'UI peut lire `store.snapshot.events` immédiatement.
 * - Reconnexion automatique (backoff) en relançant `hello` avec le token de session.
 */
import { INTERP_DELAY_MS, PROTOCOL_VERSION, WS_PATH, decode, encode, type ClientMsg, type GameEvent, type ServerMsg } from '@lov/shared';
import { fxBus } from '../fx/FxBus.ts';
import { store } from '../state.ts';
import { ServerClock } from './clock.ts';
import { SnapshotBuffer } from './snapshots.ts';

type Handler = (msg: ServerMsg) => void;

export const snapshots = new SnapshotBuffer();
export const clock = new ServerClock();

/** Temps serveur (ms) auquel le rendu interpolé affiche les entités. */
export function renderTime(localNow = Date.now()): number {
  return clock.serverNow(localNow) - INTERP_DELAY_MS;
}

interface PendingEvents {
  at: number; // temps serveur du snapshot
  events: GameEvent[];
}
const pending: PendingEvents[] = [];

/** Publie sur le fxBus les événements dont le snapshot est atteint par le rendu. Appelé à chaque frame. */
export function flushEvents(now: number): void {
  while (pending.length && pending[0]!.at <= now) {
    const p = pending.shift()!;
    for (const ev of p.events) fxBus.dispatch(ev);
  }
  // sécurité : onglet en arrière-plan, horloge incohérente… ne jamais accumuler
  while (pending.length > 40) for (const ev of pending.shift()!.events) fxBus.dispatch(ev);
}

const RECONNECT_DELAYS = [500, 1000, 2000, 4000, 8000];

export class NetClient {
  private ws: WebSocket | null = null;
  private handlers = new Set<Handler>();
  private queue: string[] = [];
  private name = 'Invocateur';
  private wantConnected = false;
  private attempts = 0;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private loopback: ((msg: ClientMsg) => void) | null = null;
  private pingTimer: ReturnType<typeof setInterval> | null = null;

  /** Démos hors ligne (?maplab) : les messages client sont passés à `fn` au lieu du WebSocket. */
  setLoopback(fn: ((msg: ClientMsg) => void) | null): void {
    this.loopback = fn;
  }

  /** Injecte un message comme s'il venait du serveur (démos, tests). */
  inject(msg: ServerMsg): void {
    this.route(msg);
    this.handlers.forEach((h) => h(msg));
  }

  connect(name: string): void {
    this.name = name;
    this.wantConnected = true;
    this.open();
  }

  disconnect(): void {
    this.wantConnected = false;
    if (this.reconnectTimer) clearTimeout(this.reconnectTimer);
    this.ws?.close();
  }

  private open(): void {
    if (this.ws && this.ws.readyState <= WebSocket.OPEN) return;
    const proto = location.protocol === 'https:' ? 'wss' : 'ws';
    const ws = new WebSocket(`${proto}://${location.host}${WS_PATH}`);
    this.ws = ws;
    ws.onopen = () => {
      this.attempts = 0;
      store.set({ connected: true, error: null });
      ws.send(encode({ t: 'hello', name: this.name, version: PROTOCOL_VERSION, reconnectToken: sessionGet('lov.token') ?? undefined }));
      this.queue.forEach((m) => ws.send(m));
      this.queue = [];
      if (this.pingTimer) clearInterval(this.pingTimer);
      const ping = () => ws.readyState === WebSocket.OPEN && ws.send(encode({ t: 'netPing', clientTime: performance.now() }));
      ping();
      this.pingTimer = setInterval(ping, 2000);
    };
    ws.onclose = () => {
      if (this.ws !== ws) return;
      store.set({ connected: false });
      this.ws = null;
      if (this.pingTimer) clearInterval(this.pingTimer);
      this.pingTimer = null;
      if (this.wantConnected) this.scheduleReconnect();
    };
    ws.onmessage = (ev) => {
      this.inject(decode<ServerMsg>(ev.data as string));
    };
  }

  private scheduleReconnect(): void {
    // on ne se reconnecte que si on a une session à reprendre
    if (!sessionGet('lov.token')) return;
    const delay = RECONNECT_DELAYS[Math.min(this.attempts, RECONNECT_DELAYS.length - 1)]!;
    this.attempts++;
    this.reconnectTimer = setTimeout(() => this.open(), delay);
  }

  send(msg: ClientMsg): void {
    if (this.loopback) {
      this.loopback(msg);
      return;
    }
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
      case 'loading':
        store.set({ loadingEndsAt: msg.endsAt });
        break;
      case 'gameStart':
        snapshots.clear();
        clock.reset();
        pending.length = 0;
        store.set({ game: msg.start, result: null, snapshot: null, selfEntity: null, cameraLocked: true });
        break;
      case 'snapshot':
        this.onSnapshot(msg.snap);
        break;
      case 'gameOver':
        store.set({ result: msg.result });
        break;
      case 'pong':
        if (msg.clientTime !== undefined) {
          const sample = performance.now() - msg.clientTime;
          const prev = store.state.rtt;
          const rtt = Math.round(prev === null ? sample : prev * 0.7 + sample * 0.3);
          clock.setRtt(rtt);
          store.set({ rtt });
        }
        break;
      default:
        break;
    }
  }

  private onSnapshot(snap: Extract<ServerMsg, { t: 'snapshot' }>['snap']): void {
    clock.sample(snap.serverTime, Date.now());
    if (!snapshots.push(snap)) return;
    const full = { ...snap, entities: snapshots.list() };
    const selfId = store.state.game?.entityId ?? snap.self?.entityId;
    const selfEntity = selfId !== undefined ? (snapshots.entities.get(selfId) ?? null) : null;
    store.set({ snapshot: full, selfEntity });
    if (snap.events.length) pending.push({ at: snap.serverTime, events: snap.events });
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
