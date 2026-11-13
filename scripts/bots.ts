/**
 * Bots de charge WebSocket — UNIQUEMENT pour les tests/CI, jamais en jeu.
 * Usage : npx tsx scripts/bots.ts [--url ws://localhost:3000/ws] [--bots 8] [--seconds 30]
 *
 * Le premier bot crée une salle (mode dev), les autres la rejoignent, l'hôte lance, chacun locke un champion
 * à son tour, puis tous envoient des ordres aléatoires (move / attackMove / attack / cast). Affiche à la fin le
 * débit reçu par client, la cadence des snapshots et les erreurs.
 */
import { WebSocket } from 'ws';
import { CHAMPION_LIST, PROTOCOL_VERSION, type ClientMsg, type ServerMsg } from '@lov/shared';

const arg = (name: string, def: string): string => {
  const i = process.argv.indexOf(`--${name}`);
  return i >= 0 ? (process.argv[i + 1] ?? def) : def;
};
const URL_ = arg('url', 'ws://localhost:3000/ws');
const N = Number(arg('bots', '8'));
const SECONDS = Number(arg('seconds', '30'));

interface Bot {
  i: number;
  ws: WebSocket;
  id?: string;
  bytes: number;
  snaps: number;
  errors: string[];
  inGame: boolean;
  entityId?: number;
  visible: number[];
  firstSnapAt?: number;
}

const send = (b: Bot, m: ClientMsg) => b.ws.readyState === b.ws.OPEN && b.ws.send(JSON.stringify(m));
let roomCode: string | null = null;
const bots: Bot[] = [];

function makeBot(i: number): Promise<Bot> {
  return new Promise((resolve) => {
    const ws = new WebSocket(URL_, { perMessageDeflate: true });
    const b: Bot = { i, ws, bytes: 0, snaps: 0, errors: [], inGame: false, visible: [] };
    ws.on('open', () => {
      send(b, { t: 'hello', name: `Bot${i}`, version: PROTOCOL_VERSION });
      resolve(b);
    });
    ws.on('message', (data) => {
      const raw = data.toString();
      b.bytes += raw.length;
      const m = JSON.parse(raw) as ServerMsg;
      onMsg(b, m);
    });
    ws.on('error', (e) => b.errors.push(String(e)));
  });
}

function onMsg(b: Bot, m: ServerMsg): void {
  switch (m.t) {
    case 'welcome':
      b.id = m.playerId;
      break;
    case 'error':
      b.errors.push(m.message);
      break;
    case 'lobby':
      if (b.i === 0 && !roomCode) {
        roomCode = m.state.code;
        for (const o of bots) if (o.i !== 0) send(o, { t: 'joinRoom', code: roomCode });
      }
      break;
    case 'draft': {
      const d = m.state;
      const turn = d.turns[d.currentTurn] ?? [];
      const me = d.picks.find((p) => p.playerId === b.id);
      if (b.id && turn.includes(b.id) && me && !me.locked) {
        const taken = new Set(d.picks.map((p) => p.championId));
        const free = CHAMPION_LIST.filter((c) => !taken.has(c.id));
        const pick = free[Math.floor(Math.random() * free.length)];
        if (pick) send(b, { t: 'draftLock', championId: pick.id });
      }
      break;
    }
    case 'gameStart':
      b.inGame = true;
      b.entityId = m.start.entityId;
      break;
    case 'snapshot':
      b.snaps++;
      b.firstSnapAt ??= Date.now();
      if (m.snap.entities.length) b.visible = m.snap.entities.filter((e) => e.id !== b.entityId).map((e) => e.id);
      break;
    default:
      break;
  }
}

const rnd = (lo: number, hi: number) => lo + Math.random() * (hi - lo);

async function main(): Promise<void> {
  for (let i = 0; i < N; i++) bots.push(await makeBot(i));
  await new Promise((r) => setTimeout(r, 200));
  send(bots[0]!, { t: 'createRoom', devMode: true });
  // Attendre que tout le monde soit dans la salle, puis lancer.
  for (let k = 0; k < 50 && !(roomCode && bots.every((b) => b.id)); k++) await new Promise((r) => setTimeout(r, 100));
  await new Promise((r) => setTimeout(r, 300));
  send(bots[0]!, { t: 'startGame' });
  console.log(`[bots] salle ${roomCode}, ${N} bots ; draft + finalisation + chargement (~20 s)...`);
  for (let k = 0; k < 600 && !bots.every((b) => b.inGame); k++) await new Promise((r) => setTimeout(r, 100));
  if (!bots.every((b) => b.inGame)) throw new Error('la partie ne démarre pas');
  for (const b of bots) b.bytes = 0;
  const t0 = Date.now();
  const timer = setInterval(() => {
    for (const b of bots) {
      const r = Math.random();
      if (r < 0.5) send(b, { t: 'move', pos: { x: rnd(500, 11500), y: rnd(500, 11500) } });
      else if (r < 0.7) send(b, { t: 'attackMove', pos: { x: rnd(4000, 8000), y: rnd(4000, 8000) } });
      else if (r < 0.85 && b.visible.length) send(b, { t: 'attack', targetId: b.visible[Math.floor(Math.random() * b.visible.length)]! });
      else send(b, { t: 'cast', slot: (['Q', 'W', 'E', 'R'] as const)[Math.floor(Math.random() * 4)]!, targetPos: { x: rnd(0, 12000), y: rnd(0, 12000) } });
    }
  }, 250);
  await new Promise((r) => setTimeout(r, SECONDS * 1000));
  clearInterval(timer);
  const dt = (Date.now() - t0) / 1000;
  let failed = false;
  for (const b of bots) {
    const kbs = b.bytes / 1024 / dt;
    console.log(`[bots] Bot${b.i}: ${(b.snaps / dt).toFixed(1)} snapshots/s, ${kbs.toFixed(1)} Ko/s (JSON décompressé), erreurs: ${b.errors.length ? b.errors.join(' | ') : 'aucune'}`);
    if (b.snaps / dt < 15) failed = true;
  }
  for (const b of bots) b.ws.close();
  if (failed) {
    console.error('[bots] ÉCHEC : cadence de snapshots trop basse');
    process.exit(1);
  }
  console.log('[bots] OK');
  process.exit(0);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
