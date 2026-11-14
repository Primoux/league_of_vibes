/**
 * Vue « monde » côté UI : cache d'entités (vue complète fournie par net/), kill feed, annonces, chat
 * et effets d'interface (« juice » : montée de niveau, or gagné, objet acheté). Propriétaire : mistral-ui.
 */
import { CHAMPIONS, type EntityId, type EntitySnapshot, type GameEvent, type Snapshot, type Team, type Vec2 } from '@lov/shared';
import { store, type ClientState } from '../../state.ts';
import { FeedTracker, trimFeed, type Announcement, type FeedEntry, type UnitRef } from '../logic/feed.ts';
import { MiniStore, useMini } from '../useStore.ts';

// ------------------------------------------------------------------ entités
export const entities = new Map<EntityId, EntitySnapshot>();

export interface WorldTick {
  /** Incrémenté à chaque snapshot appliqué (permet aux composants de se re-rendre). */
  version: number;
  time: number;
}
export const worldTick = new MiniStore<WorldTick>({ version: 0, time: 0 });

// ------------------------------------------------------------------ état propre à l'UI
// Les panneaux (shopOpen, scoreboardOpen, statsOpen, settingsOpen) et loadingEndsAt sont dans le store
// de claude-client (pilotés par l'input) ; ici seulement le kill feed et les annonces.
export interface UiState {
  feed: FeedEntry[];
  announce: Announcement | null;
}
export const ui = new MiniStore<UiState>({ feed: [], announce: null });

/** Messages de chat (événements `chat` du serveur, déjà filtrés par équipe côté serveur). */
export interface ChatLine {
  key: number;
  at: number; // Date.now() de réception
  playerId: string;
  text: string;
  teamOnly: boolean;
}
export const chat = new MiniStore<{ lines: ChatLine[]; open: boolean; all: boolean }>({ lines: [], open: false, all: false });

/** Effets d'interface déclenchés par les événements du joueur local. `seq` change = rejouer l'animation. */
export interface Juice {
  levelSeq: number;
  level: number;
  golds: { key: number; amount: number }[];
  boughtSeq: number;
  boughtItem: string | null;
}
export const juice = new MiniStore<Juice>({ levelSeq: 0, level: 1, golds: [], boughtSeq: 0, boughtItem: null });
let juiceKey = 1;
let pendingGold = 0;
let goldTimer: ReturnType<typeof setTimeout> | null = null;

function flushGold(): void {
  goldTimer = null;
  const amount = Math.round(pendingGold);
  pendingGold = 0;
  if (amount < 15) return; // or passif : pas d'effet
  const key = juiceKey++;
  juice.set({ golds: [...juice.state.golds, { key, amount }].slice(-4) });
  setTimeout(() => juice.set({ golds: juice.state.golds.filter((g) => g.key !== key) }), 1400);
}

function onSelfEvents(events: readonly GameEvent[]): void {
  const me = store.state.snapshot?.self?.entityId ?? store.state.game?.entityId;
  for (const e of events) {
    if (e.type === 'chat') {
      chat.set({ lines: [...chat.state.lines, { key: juiceKey++, at: Date.now(), playerId: e.playerId, text: e.text, teamOnly: e.teamOnly }].slice(-40) });
      continue;
    }
    if (me === undefined) continue;
    if (e.type === 'levelUp' && e.target === me) juice.set({ levelSeq: juice.state.levelSeq + 1, level: e.level });
    else if (e.type === 'itemBought' && e.target === me) juice.set({ boughtSeq: juice.state.boughtSeq + 1, boughtItem: e.itemId });
    else if (e.type === 'goldGain' && e.target === me) {
      pendingGold += e.amount;
      if (!goldTimer) goldTimer = setTimeout(flushGold, 250);
    }
  }
}

/** Minimap -> caméra (claude-client lit `cameraFocus` et déverrouille la caméra). */
export function focusCamera(pos: Vec2): void {
  const seq = (store.state.cameraFocus?.seq ?? 0) + 1;
  store.set({ cameraFocus: { pos, seq }, cameraLocked: false });
}

/** Dernier état connu de chaque champion par joueur (survit au brouillard de guerre, pour le tableau des scores). */
export interface ChampMemo {
  entityId: EntityId;
  championId: string;
  team: Team;
  level: number;
  dead: boolean;
  respawnIn?: number;
  kills: number;
  deaths: number;
  assists: number;
  cs: number;
  items: (string | null)[];
}
export const champMemo = new Map<string, ChampMemo>();

/** Re-rend le composant à chaque snapshot appliqué si la valeur sélectionnée (lue dans `entities`) change. */
export function useWorld<R>(select: () => R, eq?: (a: R, b: R) => boolean): R {
  return useMini(worldTick, select, eq);
}

// ------------------------------------------------------------------ lookups
export function unitRef(id: EntityId): UnitRef {
  const e = entities.get(id);
  if (!e) return { id, kind: 'unknown', team: 'neutral' };
  const name = e.kind === 'champion' && e.subtype ? (CHAMPIONS[e.subtype]?.name ?? e.subtype) : undefined;
  return { id, kind: e.kind, team: e.team, subtype: e.subtype, name };
}

export function selfEntity(s: ClientState = store.state): EntitySnapshot | undefined {
  const id = s.snapshot?.self?.entityId ?? s.game?.entityId;
  return id === undefined ? undefined : entities.get(id);
}

export function myTeam(s: ClientState = store.state): Team {
  return s.game?.team ?? 'blue';
}

export function champions(): EntitySnapshot[] {
  const out: EntitySnapshot[] = [];
  entities.forEach((e) => {
    if (e.kind === 'champion') out.push(e);
  });
  return out;
}

// ------------------------------------------------------------------ application des snapshots
let tracker: FeedTracker | null = null;
let lastSnap: Snapshot | null = null;
let lastGame: ClientState['game'] = null;
const announceQueue: Announcement[] = [];
let announceTimer: ReturnType<typeof setTimeout> | null = null;
const ANNOUNCE_MS = 3200;

function reset(): void {
  entities.clear();
  champMemo.clear();
  chat.set({ lines: [], open: false });
  tracker = null;
  announceQueue.length = 0;
  ui.set({ feed: [], announce: null });
}

/**
 * `full` : `snap.entities` est la vue complète (cas normal : net/ de claude-client fusionne les deltas).
 * Sinon (snapshot delta brut) : fusion + `removed`.
 */
export function applySnapshot(snap: Snapshot, team: Team, full = true): void {
  if (full) entities.clear();
  else if (snap.removed) for (const id of snap.removed) entities.delete(id);
  for (const e of snap.entities) {
    const prev = full ? undefined : entities.get(e.id);
    const merged = prev ? Object.assign(prev, e) : e;
    entities.set(e.id, merged);
    if (merged.kind === 'champion' && merged.champ && merged.team !== 'neutral') {
      const c = merged.champ;
      champMemo.set(c.playerId, {
        entityId: merged.id,
        championId: merged.subtype ?? '',
        team: merged.team,
        level: merged.level ?? 1,
        dead: !!merged.dead,
        respawnIn: c.respawnIn,
        kills: c.kills,
        deaths: c.deaths,
        assists: c.assists,
        cs: c.cs,
        items: c.items,
      });
    }
  }
  if (!tracker) tracker = new FeedTracker(team, unitRef);
  tracker.setTeam(team);
  entities.forEach((e) => {
    if (e.kind === 'champion' && !e.dead) tracker!.markAlive(e.id);
  });
  if (snap.events.length) pushEvents(snap.events, snap.time);
  else if (ui.state.feed.length) {
    const trimmed = trimFeed(ui.state.feed, snap.time);
    if (trimmed.length !== ui.state.feed.length) ui.set({ feed: trimmed });
  }
  worldTick.set({ version: worldTick.state.version + 1, time: snap.time });
}

export function pushEvents(events: readonly GameEvent[], time: number): void {
  onSelfEvents(events);
  if (!tracker) return;
  const { feed, announce } = tracker.process(events, time);
  if (feed.length) ui.set({ feed: trimFeed([...ui.state.feed, ...feed], time) });
  for (const a of announce) queueAnnouncement(a);
}

export function queueAnnouncement(a: Announcement): void {
  announceQueue.push(a);
  announceQueue.sort((x, y) => y.priority - x.priority);
  if (announceQueue.length > 4) announceQueue.length = 4;
  if (!ui.state.announce) nextAnnouncement();
}

function nextAnnouncement(): void {
  const next = announceQueue.shift() ?? null;
  ui.set({ announce: next });
  if (announceTimer) clearTimeout(announceTimer);
  announceTimer = next ? setTimeout(nextAnnouncement, ANNOUNCE_MS) : null;
}

let installed = false;
export function installUiWorld(): void {
  if (installed) return;
  installed = true;
  store.subscribe((s) => {
    if (s.game !== lastGame) {
      lastGame = s.game;
      reset();
    }
    if (s.snapshot && s.snapshot !== lastSnap) {
      lastSnap = s.snapshot;
      applySnapshot(s.snapshot, myTeam(s));
    }
  });
}
