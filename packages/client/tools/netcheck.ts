/**
 * Diagnostic réseau du client — UNIQUEMENT pour les tests. Propriétaire : claude-client.
 *
 * Un « client sonde » fait tourner le VRAI pipeline client (SnapshotBuffer, ServerClock, MovePredictor) à 60 Hz
 * contre le vrai serveur, avec latence/gigue simulées, pendant que d'autres clients jouent au hasard. Mesure ce
 * qu'un joueur ressentirait : marge du tampon d'interpolation, saccades (« pops »), gels, erreurs et recalages
 * de la prédiction, cadence et débit des snapshots.
 *
 * Usage : npx tsx packages/client/tools/netcheck.ts [--url ws://localhost:3000/ws] [--players 8] [--seconds 40]
 *         [--lag 60] [--jitter 30] [--loss 0]
 *   --lag    latence aller simulée (ms), appliquée dans les deux sens
 *   --jitter gigue uniforme ± jitter/2 (ms) sur chaque message reçu (les messages peuvent se doubler)
 * Serveur conseillé : SKIP_DRAFT=1 (sinon le draft dev est joué automatiquement, ~20 s).
 */
import { WebSocket } from 'ws';
import { CHAMPION_LIST, INTERP_DELAY_MS, PROTOCOL_VERSION, isInWall, type ClientMsg, type EntitySnapshot, type GameEvent, type ServerMsg, type Snapshot, type Vec2 } from '@lov/shared';
import { ServerClock } from '../src/net/clock.ts';
import { MovePredictor, SNAP_DIST, segmentBlocked } from '../src/net/prediction.ts';
import { SnapshotBuffer, TELEPORT_DIST, type Interpolated } from '../src/net/snapshots.ts';

const arg = (name: string, def: string): string => {
  const i = process.argv.indexOf(`--${name}`);
  return i >= 0 ? (process.argv[i + 1] ?? def) : def;
};
const URL_ = arg('url', 'ws://localhost:3000/ws');
const PLAYERS = Number(arg('players', '8'));
const SECONDS = Number(arg('seconds', '40'));
const LAG = Number(arg('lag', '60'));
const JITTER = Number(arg('jitter', '30'));

const rnd = (lo: number, hi: number) => lo + Math.random() * (hi - lo);
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const pct = (a: number[], p: number) => {
  if (!a.length) return NaN;
  const s = [...a].sort((x, y) => x - y);
  return s[Math.min(s.length - 1, Math.floor((p / 100) * s.length))]!;
};

// ------------------------------------------------------------------ clients

interface Client {
  i: number;
  ws: WebSocket;
  id?: string;
  entityId?: number;
  inGame: boolean;
  visible: number[];
  onMsg?: (m: ServerMsg) => void;
}

let roomCode: string | null = null;
const clients: Client[] = [];

function send(c: Client, m: ClientMsg, delay = 0): void {
  const go = () => c.ws.readyState === c.ws.OPEN && c.ws.send(JSON.stringify(m));
  if (delay > 0) setTimeout(go, delay);
  else go();
}

function connect(i: number, lagIn: () => number): Promise<Client> {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(URL_, { perMessageDeflate: true });
    const c: Client = { i, ws, inGame: false, visible: [] };
    ws.on('open', () => {
      send(c, { t: 'hello', name: i === 0 ? 'Sonde' : `Bot${i}`, version: PROTOCOL_VERSION });
      resolve(c);
    });
    ws.on('error', reject);
    ws.on('message', (data) => {
      const raw = data.toString();
      if (i === 0) bytes += raw.length;
      const m = JSON.parse(raw) as ServerMsg;
      const d = lagIn();
      if (d > 0) setTimeout(() => handle(c, m), d);
      else handle(c, m);
    });
  });
}

function handle(c: Client, m: ServerMsg): void {
  switch (m.t) {
    case 'welcome':
      c.id = m.playerId;
      break;
    case 'lobby':
      if (c.i === 0 && !roomCode) {
        roomCode = m.state.code;
        for (const o of clients) if (o.i !== 0) send(o, { t: 'joinRoom', code: roomCode });
      }
      break;
    case 'draft': {
      const d = m.state;
      const me = d.picks.find((p) => p.playerId === c.id);
      if (c.id && (d.turns[d.currentTurn] ?? []).includes(c.id) && me && !me.locked) {
        const taken = new Set(d.picks.map((p) => p.championId));
        const free = CHAMPION_LIST.filter((ch) => !taken.has(ch.id));
        const pick = free[Math.floor(Math.random() * free.length)];
        if (pick) send(c, { t: 'draftLock', championId: pick.id });
      }
      break;
    }
    case 'gameStart':
      c.inGame = true;
      c.entityId = m.start.entityId;
      break;
    case 'snapshot':
      if (m.snap.entities.length) c.visible = m.snap.entities.filter((e) => e.id !== c.entityId && e.team !== 'neutral').map((e) => e.id);
      break;
    default:
      break;
  }
  c.onMsg?.(m);
}

// ------------------------------------------------------------------ mesures de la sonde

let bytes = 0;
const snapIntervals: number[] = [];
let lastSnapRecv = 0;
let snaps = 0;
let staleDropped = 0;
const margins: number[] = []; // latestServerTime - renderTime (ms) à chaque frame
let starvedFrames = 0; // renderTime au-delà du dernier snapshot (extrapolation)
let frames = 0;
let pops = 0; // déplacements affichés anormaux (> 40 u en une frame, hors téléportation)
let popEntities = new Set<number>();
const popSamples: string[] = [];
let entityFrames = 0;
const predErrors: number[] = []; // |prédit - serveur| au même instant serveur (u)
let predSnaps = 0; // recalages de la prédiction (> SNAP_DIST)
const selfJumps: number[] = [];
let rtt = 0;
const events = new Map<string, number>();
let maxEntities = 0;
let churnAdd = 0;
let churnRemove = 0;
const errors: string[] = [];

async function main(): Promise<void> {
  const lagIn = () => Math.max(0, LAG + (Math.random() - 0.5) * JITTER);
  for (let i = 0; i < PLAYERS; i++) clients.push(await connect(i, i === 0 ? lagIn : () => 0));
  const probe = clients[0]!;
  await sleep(200);
  send(probe, { t: 'createRoom', devMode: true });
  for (let k = 0; k < 50 && !(roomCode && clients.every((c) => c.id)); k++) await sleep(100);
  await sleep(300);
  send(probe, { t: 'startGame' });
  console.log(`[netcheck] salle ${roomCode}, ${PLAYERS} joueurs, latence ${LAG} ms ± ${JITTER / 2} ms`);
  for (let k = 0; k < 600 && !clients.every((c) => c.inGame); k++) await sleep(100);
  if (!clients.every((c) => c.inGame)) throw new Error('la partie ne démarre pas');

  // ---------------------------------------------------------------- pipeline client réel
  const buffer = new SnapshotBuffer();
  const clock = new ServerClock();
  const predictor = new MovePredictor(segmentBlocked);
  const selfId = probe.entityId!;
  let speed = 335;
  let alive = true;
  /** historique des positions prédites, indexé par temps serveur estimé (pour comparer quand le snapshot arrive) */
  const predHist: { t: number; x: number; y: number }[] = [];

  probe.onMsg = (m) => {
    if (m.t === 'pong' && m.clientTime !== undefined) {
      const s = performance.now() - m.clientTime;
      rtt = rtt ? rtt * 0.7 + s * 0.3 : s;
      clock.setRtt(rtt);
      return;
    }
    if (m.t !== 'snapshot') return;
    const snap: Snapshot = m.snap;
    const now = Date.now();
    if (lastSnapRecv) snapIntervals.push(now - lastSnapRecv);
    lastSnapRecv = now;
    snaps++;
    clock.sample(snap.serverTime, now);
    const before = buffer.entities.size;
    let removed = 0;
    if (!buffer.push(snap, () => removed++)) {
      staleDropped++;
      return;
    }
    churnRemove += removed;
    churnAdd += Math.max(0, buffer.entities.size - before + removed);
    maxEntities = Math.max(maxEntities, buffer.entities.size);
    if (snap.self) speed = snap.self.stats.moveSpeed;
    const me = buffer.entities.get(selfId);
    alive = !!me && !me.dead;
    for (const ev of snap.events as GameEvent[]) {
      events.set(ev.type, (events.get(ev.type) ?? 0) + 1);
      if (ev.type === 'dash' && ev.source === selfId) predictor.suspend(ev.duration + 0.1);
    }
    // erreur de prédiction : position prédite au temps serveur de ce snapshot vs position serveur
    if (me && alive && predHist.length > 2) {
      const t = snap.serverTime;
      for (let i = predHist.length - 1; i > 0; i--) {
        const a = predHist[i - 1]!;
        const b = predHist[i]!;
        if (a.t <= t && b.t >= t) {
          const k = b.t === a.t ? 0 : (t - a.t) / (b.t - a.t);
          predErrors.push(Math.hypot(a.x + (b.x - a.x) * k - me.x, a.y + (b.y - a.y) * k - me.y));
          break;
        }
      }
    }
  };
  const pingTimer = setInterval(() => send(probe, { t: 'netPing', clientTime: performance.now() }, LAG), 1000);

  // ordres : la sonde se déplace comme un joueur (clic toutes les 0,8–2 s), les autres jouent au hasard
  const orderTimer = setInterval(() => {
    for (const c of clients) {
      if (c.i === 0) continue;
      const r = Math.random();
      if (r < 0.5) send(c, { t: 'move', pos: { x: rnd(500, 11500), y: rnd(500, 11500) } });
      else if (r < 0.7) send(c, { t: 'attackMove', pos: { x: rnd(4000, 8000), y: rnd(4000, 8000) } });
      else if (r < 0.85 && c.visible.length) send(c, { t: 'attack', targetId: c.visible[Math.floor(Math.random() * c.visible.length)]! });
      else send(c, { t: 'cast', slot: (['Q', 'W', 'E', 'R'] as const)[Math.floor(Math.random() * 4)]!, targetPos: { x: rnd(0, 12000), y: rnd(0, 12000) } });
    }
  }, 250);
  let nextProbeOrder = 0;

  const rendered = new Map<number, Interpolated>();
  const tmp: Interpolated = { x: 0, y: 0, facing: 0 };
  const vel = { x: 0, y: 0 };
  let lastFrame = performance.now();
  let lastSelf: Vec2 | null = null;
  const t0 = Date.now();

  const frameTimer = setInterval(() => {
    const nowP = performance.now();
    const dt = Math.min(0.1, (nowP - lastFrame) / 1000);
    lastFrame = nowP;
    const now = Date.now();
    if (!clock.ready) return;
    clock.update(dt * 1000);
    const rt = clock.serverNow(now) - INTERP_DELAY_MS;
    frames++;
    margins.push(buffer.latestServerTime - rt);
    if (rt > buffer.latestServerTime) starvedFrames++;

    // ordre de la sonde (comme un clic droit) + prédiction locale
    const me = buffer.entities.get(selfId);
    if (me && now > nextProbeOrder) {
      nextProbeOrder = now + rnd(800, 2000);
      let target: Vec2 | null = null;
      for (let k = 0; k < 10 && !target; k++) {
        const p = { x: me.x + rnd(-1500, 1500), y: me.y + rnd(-1500, 1500) };
        if (p.x > 300 && p.y > 300 && p.x < 11700 && p.y < 11700 && !isInWall(p)) target = p;
      }
      if (target) {
        send(probe, { t: 'move', pos: target }, LAG);
        predictor.command(target, rtt / 1000);
      }
    }
    const last = buffer.latestSample(selfId);
    if (me && last) {
      buffer.velocity(selfId, vel);
      const lead = (clock.serverNow(now) - last.t) / 1000 + rtt / 2000;
      const before = { x: predictor.pos.x, y: predictor.pos.y };
      const p = predictor.update(dt, { x: last.x, y: last.y, vx: vel.x, vy: vel.y, lead }, speed, alive);
      const jump = Math.hypot(p.x - before.x, p.y - before.y);
      if (lastSelf) selfJumps.push(jump);
      if (jump > SNAP_DIST * 0.9) predSnaps++;
      lastSelf = { x: p.x, y: p.y };
      predHist.push({ t: clock.serverNow(now), x: p.x, y: p.y });
      if (predHist.length > 120) predHist.shift();
    }

    // interpolation de toutes les autres entités : détection des saccades
    for (const [id, e] of buffer.entities as Map<number, EntitySnapshot>) {
      if (id === selfId || e.kind === 'tower' || e.kind === 'inhibitor' || e.kind === 'nexus') continue;
      if (!buffer.sample(id, rt, tmp)) continue;
      entityFrames++;
      const prev = rendered.get(id);
      if (prev) {
        const d = Math.hypot(tmp.x - prev.x, tmp.y - prev.y);
        if (d > 40 && d < TELEPORT_DIST) {
          pops++;
          popEntities.add(id);
          if (popSamples.length < 8) popSamples.push(`${e.kind}/${e.subtype ?? ''} ${d.toFixed(0)}u`);
        }
        prev.x = tmp.x;
        prev.y = tmp.y;
      } else rendered.set(id, { ...tmp });
    }
    for (const id of rendered.keys()) if (!buffer.entities.has(id)) rendered.delete(id);
  }, 1000 / 60);

  await sleep(SECONDS * 1000);
  clearInterval(frameTimer);
  clearInterval(orderTimer);
  clearInterval(pingTimer);
  const dur = (Date.now() - t0) / 1000;

  // ---------------------------------------------------------------- rapport
  const f = (n: number, d = 0) => (Number.isFinite(n) ? n.toFixed(d) : '—');
  console.log(`\n=== netcheck (${f(dur)} s, ${frames} frames, RTT mesuré ${f(rtt)} ms) ===`);
  console.log(`snapshots   : ${f(snaps / dur, 1)}/s, intervalle p50 ${f(pct(snapIntervals, 50))} ms, p95 ${f(pct(snapIntervals, 95))} ms, max ${f(Math.max(...snapIntervals))} ms, obsolètes ignorés ${staleDropped}`);
  console.log(`débit       : ${f(bytes / 1024 / dur, 1)} Ko/s (JSON décompressé)`);
  console.log(`tampon      : marge p5 ${f(pct(margins, 5))} ms, p50 ${f(pct(margins, 50))} ms ; frames en extrapolation ${f((100 * starvedFrames) / Math.max(1, frames), 1)} %`);
  console.log(`entités     : max ${maxEntities}, apparitions ${churnAdd}, disparitions ${churnRemove}`);
  console.log(`saccades    : ${pops} sur ${entityFrames} entité-frames (${f((1e4 * pops) / Math.max(1, entityFrames), 2)} ‱), ${popEntities.size} entités ${popSamples.length ? '— ex. ' + popSamples.join(', ') : ''}`);
  console.log(`prédiction  : erreur p50 ${f(pct(predErrors, 50))} u, p95 ${f(pct(predErrors, 95))} u, max ${f(Math.max(...predErrors))} u ; recalages ${predSnaps} ; saut/frame p99 ${f(pct(selfJumps, 99), 1)} u`);
  console.log(`événements  : ${[...events].sort((a, b) => b[1] - a[1]).map(([k, v]) => `${k} ${v}`).join(', ')}`);
  if (errors.length) console.log(`erreurs     : ${errors.join(' | ')}`);
  for (const c of clients) c.ws.close();
  process.exit(0);
}

main().catch((e) => {
  console.error('[netcheck] échec :', e);
  process.exit(1);
});
