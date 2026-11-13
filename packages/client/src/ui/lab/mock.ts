/**
 * Faux serveur pour `?uilab` : génère lobby, draft, snapshots animés et résultats, et répond aux ClientMsg
 * (draftHover/Lock, buy/sell/undo, levelAbility, cast...). Aucun code de jeu réel ici.
 */
import {
  CAMPS,
  CHAMPIONS,
  DEFAULT_SUMMONERS,
  ITEMS,
  lanePathFor,
  TOWERS,
  INHIBITORS,
  NEXUS,
  type AbilitySlot,
  type ClientMsg,
  type DraftState,
  type EntitySnapshot,
  type GameEvent,
  type GameOver,
  type GameStart,
  type Lane,
  type LobbyState,
  type SelfState,
  type Snapshot,
  type Team,
  type Vec2,
} from '@lov/shared';
import { store } from '../../state.ts';
import { quoteItem } from '../logic/shop.ts';
import { ui } from '../game/world.ts';

const ME = 'p-me';
const NAMES = ['Toi', 'Mistral', 'Codex', 'Claudette', 'Vibeur', 'Kéké', 'Zozo', 'Lulu'];
const PLAYERS: { id: string; name: string; team: Team; role: 'top' | 'jungle' | 'mid' | 'bot'; champ: string }[] = [
  { id: ME, name: NAMES[0]!, team: 'blue', role: 'mid', champ: 'ilyra' },
  { id: 'p2', name: NAMES[1]!, team: 'blue', role: 'top', champ: 'kaelen' },
  { id: 'p3', name: NAMES[2]!, team: 'blue', role: 'jungle', champ: 'tharox' },
  { id: 'p4', name: NAMES[3]!, team: 'blue', role: 'bot', champ: 'sable' },
  { id: 'p5', name: NAMES[4]!, team: 'red', role: 'top', champ: 'grondak' },
  { id: 'p6', name: NAMES[5]!, team: 'red', role: 'jungle', champ: 'nyx' },
  { id: 'p7', name: NAMES[6]!, team: 'red', role: 'mid', champ: 'zephyr' },
  { id: 'p8', name: NAMES[7]!, team: 'red', role: 'bot', champ: 'corvin' },
];

export function mockLobby(count = 6): LobbyState {
  return {
    code: 'VIBES',
    phase: 'lobby',
    devMode: true,
    players: PLAYERS.slice(0, count).map((p, i) => ({ id: p.id, name: p.name, team: p.team, ready: i % 3 !== 1, isHost: i === 0, connected: i !== 5, role: i === 2 ? undefined : p.role })),
  };
}

const TURNS = [['p-me'], ['p5', 'p6'], ['p2', 'p3'], ['p7', 'p8'], ['p4']];

export function mockDraft(turn = 0): DraftState {
  const done = new Set(TURNS.slice(0, turn).flat());
  return {
    turns: TURNS,
    currentTurn: turn,
    turnEndsAt: Date.now() + 30_000,
    finalizeEndsAt: turn >= TURNS.length ? Date.now() + 15_000 : undefined,
    picks: PLAYERS.map((p) => ({
      playerId: p.id,
      team: p.team,
      championId: done.has(p.id) ? p.champ : null,
      hoverChampionId: !done.has(p.id) && p.id === 'p2' ? 'vrenna' : null,
      locked: done.has(p.id),
      summoners: p.role === 'jungle' ? ['flash', 'smite'] : [...DEFAULT_SUMMONERS],
    })),
  };
}

// ------------------------------------------------------------------ partie simulée
interface Sim {
  t: number;
  tick: number;
  ents: Map<number, EntitySnapshot>;
  self: SelfState;
  events: GameEvent[];
  score: Snapshot['score'];
  shopStack: { kind: 'buy' | 'sell'; itemId: string; slot: number; gold: number; prev: (string | null)[] }[];
  timer: ReturnType<typeof setInterval> | null;
  minionSeq: number;
}

let sim: Sim | null = null;

function champEnt(id: number, p: (typeof PLAYERS)[number], pos: Vec2): EntitySnapshot {
  const c = CHAMPIONS[p.champ]!;
  const lvl = 6 + (id % 5);
  const maxHp = Math.round(c.baseStats.maxHp + (c.perLevel.maxHp ?? 80) * (lvl - 1));
  const maxMana = c.resource === 'none' ? 0 : c.resource === 'energy' ? 200 : Math.round(c.baseStats.maxMana + (c.perLevel.maxMana ?? 25) * (lvl - 1));
  return {
    id,
    kind: 'champion',
    team: p.team,
    x: pos.x,
    y: pos.y,
    facing: 0,
    hp: maxHp * (0.4 + ((id * 37) % 60) / 100),
    maxHp,
    mana: maxMana * 0.7,
    maxMana,
    subtype: p.champ,
    ownerId: p.id,
    level: lvl,
    champ: {
      playerId: p.id,
      name: p.name,
      kills: (id * 3) % 5,
      deaths: (id * 7) % 4,
      assists: (id * 5) % 7,
      cs: 40 + ((id * 13) % 60),
      items: id === 1 ? ['doran_ring', 'boots', null, null, null, null, 'stealth_ward'] : [['long_sword', 'doran_blade', 'ruby_crystal'][id % 3]!, 'boots', id % 2 ? 'amplifying_tome' : null, null, null, null, 'stealth_ward'],
    },
  };
}

const LANES: Lane[] = ['top', 'mid', 'bot'];
const lanePos = (lane: Lane, team: Team, f: number): Vec2 => {
  const path = lanePathFor(lane, team);
  const segs = path.length - 1;
  const x = Math.max(0, Math.min(0.9999, f)) * segs;
  const i = Math.floor(x);
  const a = path[i]!;
  const b = path[i + 1]!;
  const t = x - i;
  return { x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t };
};

export function startGame(): void {
  stopGame();
  const ents = new Map<number, EntitySnapshot>();
  const spots: Vec2[] = [lanePos('mid', 'blue', 0.42), lanePos('top', 'blue', 0.45), { x: 3800, y: 5600 }, lanePos('bot', 'blue', 0.44), lanePos('top', 'red', 0.55), { x: 8300, y: 6300 }, lanePos('mid', 'red', 0.55), lanePos('bot', 'red', 0.57)];
  PLAYERS.forEach((p, i) => ents.set(i < 4 ? i + 1 : i + 6, champEnt(i < 4 ? i + 1 : i + 6, p, spots[i]!)));
  let id = 100;
  for (const t of TOWERS) ents.set(id, { id: id++, kind: 'tower', team: t.team, x: t.pos.x, y: t.pos.y, facing: 0, hp: 3000, maxHp: 3000, subtype: t.tier });
  for (const inh of INHIBITORS) ents.set(id, { id: id++, kind: 'inhibitor', team: inh.team, x: inh.pos.x, y: inh.pos.y, facing: 0, hp: 2500, maxHp: 2500 });
  for (const t of ['blue', 'red'] as Team[]) ents.set(id, { id: id++, kind: 'nexus', team: t, x: NEXUS[t].x, y: NEXUS[t].y, facing: 0, hp: 5500, maxHp: 5500 });
  for (const c of CAMPS.slice(0, 9)) ents.set(id, { id: id++, kind: 'monster', team: 'neutral', x: c.pos.x, y: c.pos.y, facing: 0, hp: 1500, maxHp: 1500, subtype: c.monster });
  ents.set(id, { id: id++, kind: 'monster', team: 'neutral', x: 7900, y: 4100, facing: 0, hp: 4000, maxHp: 4000, subtype: 'dragon' });

  const me = ents.get(1)!;
  sim = {
    t: 7 * 60 + 12,
    tick: 0,
    ents,
    events: [],
    score: { blue: { kills: 7, towers: 1, dragons: ['infernal'], grubs: 2 }, red: { kills: 5, towers: 0, dragons: ['ocean'], grubs: 1 } },
    shopStack: [],
    timer: null,
    minionSeq: 1000,
    self: {
      entityId: 1,
      gold: 1375,
      xp: 180,
      xpToNext: 480,
      level: me.level ?? 7,
      skillPoints: 1,
      abilityLevels: { Q: 3, W: 1, E: 1, R: 1 },
      cooldowns: { Q: 0, W: 4.2, E: 0, R: 38, D: 0, F: 92 },
      summoners: { D: 'flash', F: 'ignite' },
      stats: { ...CHAMPIONS.ilyra!.baseStats, ap: 58, abilityHaste: 10, maxHp: me.maxHp, maxMana: me.maxMana ?? 0 },
      canShop: true,
    },
  };
  me.buffs = [{ id: 'blue_buff', remaining: 74 }, { id: 'ilyra_frost', remaining: 3.2, stacks: 2 }];
  me.shield = 120;

  const players: GameStart['players'] = PLAYERS.map((p) => ({ id: p.id, name: p.name, team: p.team, championId: p.champ, summoners: p.role === 'jungle' ? ['flash', 'smite'] : [...DEFAULT_SUMMONERS] }));
  store.set({ playerId: ME, lobby: { ...mockLobby(8), phase: 'game' }, game: { you: ME, team: 'blue', entityId: 1, players, serverTime: Date.now() }, result: null });
  ui.set({ loadingEndsAt: null });
  sim.timer = setInterval(step, 50);
  step();
}

export function stopGame(): void {
  if (sim?.timer) clearInterval(sim.timer);
  sim = null;
}

function step(): void {
  if (!sim) return;
  const dt = 0.05;
  sim.t += dt;
  sim.tick++;
  const changed: EntitySnapshot[] = [];
  const removed: number[] = [];
  // champions : petites oscillations, régénération
  sim.ents.forEach((e) => {
    if (e.kind === 'champion') {
      if (!e.dead) {
        e.x += Math.cos(sim!.t * 0.7 + e.id) * 6;
        e.y += Math.sin(sim!.t * 0.6 + e.id * 2) * 6;
        e.hp = Math.min(e.maxHp, e.hp + e.maxHp * 0.002);
        if (e.maxMana) e.mana = Math.min(e.maxMana, (e.mana ?? 0) + 0.6);
        if (e.buffs) e.buffs = e.buffs.map((b) => ({ ...b, remaining: Math.max(0, b.remaining - dt) })).filter((b) => b.remaining > 0);
        if (e.shield) e.shield = Math.max(0, e.shield - 0.6);
      } else if (e.champ) {
        const r = Math.max(0, (e.champ.respawnIn ?? 0) - dt);
        e.champ = { ...e.champ, respawnIn: r };
        if (r <= 0) {
          e.dead = false;
          e.hp = e.maxHp;
        }
      }
      changed.push({ ...e, champ: e.champ ? { ...e.champ } : undefined });
    } else if (e.kind === 'minion') {
      const d = e as EntitySnapshot & { _f?: number; _lane?: Lane };
      d._f = (d._f ?? 0) + 0.0016;
      if (d._f >= 0.5) {
        sim!.ents.delete(e.id);
        removed.push(e.id);
        return;
      }
      const p = lanePos(d._lane ?? 'mid', e.team as Team, d._f);
      e.x = p.x;
      e.y = p.y;
      changed.push({ ...e });
    }
  });
  // vagues de sbires toutes les 6 s
  if (sim.tick % 120 === 1) {
    for (const team of ['blue', 'red'] as Team[])
      for (const lane of LANES)
        for (let k = 0; k < 4; k++) {
          const e = { id: sim.minionSeq++, kind: 'minion', team, x: 0, y: 0, facing: 0, hp: 450, maxHp: 450, subtype: k < 2 ? 'melee' : 'caster', _f: -k * 0.012, _lane: lane } as EntitySnapshot;
          sim.ents.set(e.id, e);
        }
  }
  // cooldowns
  const cd = { ...sim.self.cooldowns };
  (Object.keys(cd) as (keyof typeof cd)[]).forEach((k) => (cd[k] = Math.max(0, cd[k] - dt)));
  const self: SelfState = { ...sim.self, cooldowns: cd, gold: sim.self.gold + (sim.tick % 10 === 0 ? 2 : 0), xp: Math.min(sim.self.xpToNext, sim.self.xp + 0.4) };
  if (self.recalling !== undefined) {
    self.recalling = Math.max(0, self.recalling - dt);
    if (self.recalling <= 0) self.recalling = undefined;
  }
  sim.self = self;

  const snap: Snapshot = {
    tick: sim.tick,
    time: sim.t,
    serverTime: Date.now(),
    entities: sim.tick === 1 ? [...sim.ents.values()].map((e) => ({ ...e })) : changed,
    removed,
    events: sim.events.splice(0),
    self,
    objectiveTimers: { dragon: 10 * 60, leviathan: 15 * 60, herald: 11 * 60 + 15 },
    score: sim.score,
  };
  store.set({ snapshot: snap });
}

// ------------------------------------------------------------------ scénarios d'événements
export function labEvent(kind: string): void {
  if (!sim) return;
  const champ = (id: number) => sim!.ents.get(id)!;
  const kill = (victim: number, killer: number | undefined, assists: number[] = []) => {
    const v = champ(victim);
    v.dead = true;
    v.hp = 0;
    v.champ = { ...v.champ!, deaths: v.champ!.deaths + 1, respawnIn: 12 };
    if (killer !== undefined && sim!.ents.get(killer)?.kind === 'champion') {
      const k = champ(killer);
      k.champ = { ...k.champ!, kills: k.champ!.kills + 1 };
      sim!.score[k.team as Team].kills++;
    }
    sim!.events.push({ type: 'death', target: victim, killer, assists, bounty: 300 });
  };
  switch (kind) {
    case 'killEnemy':
      kill(10 + Math.floor(Math.random() * 4), 1, [2]);
      break;
    case 'killAlly':
      kill(2 + Math.floor(Math.random() * 3), 12, [13]);
      break;
    case 'multi':
      kill(10, 1);
      kill(11, 1);
      kill(12, 1);
      break;
    case 'ace':
      [10, 11, 12, 13].forEach((v, i) => kill(v, [1, 2, 3, 4][i]));
      break;
    case 'die':
      kill(1, 11, [12]);
      break;
    case 'execute':
      kill(13, 105);
      break;
    case 'tower':
      sim.events.push({ type: 'structureDestroyed', target: 116, kind: 'tower', team: 'red' });
      sim.score.blue.towers++;
      break;
    case 'towerLost':
      sim.events.push({ type: 'structureDestroyed', target: 101, kind: 'tower', team: 'blue' });
      sim.score.red.towers++;
      break;
    case 'inhib':
      sim.events.push({ type: 'structureDestroyed', target: 125, kind: 'inhibitor', team: 'red' });
      break;
    case 'drake': {
      const els = ['infernal', 'ocean', 'mountain', 'cloud', 'hextech', 'chemtech'] as const;
      const el = els[Math.floor(Math.random() * els.length)]!;
      sim.events.push({ type: 'objectiveTaken', monster: 'dragon', team: 'blue', killer: 3, element: el });
      sim.score.blue.dragons = [...sim.score.blue.dragons, el];
      break;
    }
    case 'leviathan':
      sim.events.push({ type: 'objectiveTaken', monster: 'leviathan', team: 'red', killer: 11 });
      break;
    case 'announce':
      sim.events.push({ type: 'announcement', key: 'minionsSpawn' });
      break;
    case 'levelUp':
      sim.self = { ...sim.self, level: sim.self.level + 1, skillPoints: sim.self.skillPoints + 1, xp: 0 };
      sim.events.push({ type: 'levelUp', target: 1, level: sim.self.level });
      break;
    case 'shopToggle':
      sim.self = { ...sim.self, canShop: !sim.self.canShop };
      break;
    case 'gold':
      sim.self = { ...sim.self, gold: sim.self.gold + 1500 };
      break;
    case 'end':
      endGame(true);
      break;
    case 'endLose':
      endGame(false);
      break;
    default:
      break;
  }
}

function endGame(win: boolean): void {
  if (!sim) startGame();
  const result: GameOver = {
    winner: win ? 'blue' : 'red',
    duration: 27 * 60 + 41,
    stats: PLAYERS.map((p, i) => {
      const e = sim!.ents.get(i < 4 ? i + 1 : i + 6)!;
      return {
        playerId: p.id,
        championId: p.champ,
        team: p.team,
        kills: e.champ!.kills + 3,
        deaths: e.champ!.deaths,
        assists: e.champ!.assists + 4,
        cs: e.champ!.cs * 2,
        gold: 9000 + i * 731,
        damageToChampions: 8000 + ((i * 4973) % 14000),
        items: ['doran_blade', 'boots', 'long_sword', 'ruby_crystal', i % 2 ? 'amplifying_tome' : null, null, 'stealth_ward'],
      };
    }),
  };
  stopGame();
  store.set({ result });
}

// ------------------------------------------------------------------ réponses aux messages client
export function handleMsg(m: ClientMsg): void {
  const s = store.state;
  switch (m.t) {
    case 'createRoom':
    case 'joinRoom':
      store.set({ playerId: ME, lobby: mockLobby(), connected: true });
      break;
    case 'setTeam':
    case 'setRole':
    case 'setReady': {
      if (!s.lobby) break;
      const players = s.lobby.players.map((p) => (p.id === ME ? { ...p, ...(m.t === 'setTeam' ? { team: m.team } : m.t === 'setRole' ? { role: m.role } : { ready: m.ready }) } : p));
      store.set({ lobby: { ...s.lobby, players } });
      break;
    }
    case 'startGame':
      store.set({ draft: mockDraft(0), lobby: s.lobby ? { ...s.lobby, phase: 'draft' } : null });
      break;
    case 'leaveRoom':
      stopGame();
      break;
    case 'draftHover':
    case 'draftLock': {
      if (!s.draft) break;
      const lock = m.t === 'draftLock';
      const picks = s.draft.picks.map((p) => (p.playerId === ME ? { ...p, hoverChampionId: m.championId, championId: lock ? m.championId : p.championId, locked: lock || p.locked } : p));
      let draft: DraftState = { ...s.draft, picks };
      if (lock) {
        // fait avancer la draft automatiquement
        draft = { ...mockDraft(1), picks: mockDraft(1).picks.map((p) => (p.playerId === ME ? picks.find((x) => x.playerId === ME)! : p)) };
      }
      store.set({ draft });
      break;
    }
    case 'draftSummoners':
      if (!s.draft) break;
      store.set({ draft: { ...s.draft, picks: s.draft.picks.map((p) => (p.playerId === ME ? { ...p, summoners: m.summoners } : p)) } });
      break;
    case 'levelAbility': {
      if (!sim || sim.self.skillPoints <= 0) break;
      sim.self = { ...sim.self, skillPoints: sim.self.skillPoints - 1, abilityLevels: { ...sim.self.abilityLevels, [m.slot]: sim.self.abilityLevels[m.slot] + 1 } };
      break;
    }
    case 'cast': {
      if (!sim) break;
      const ab = CHAMPIONS.ilyra!.abilities[m.slot as AbilitySlot];
      const rank = sim.self.abilityLevels[m.slot];
      if (rank <= 0 || sim.self.cooldowns[m.slot] > 0) break;
      const me = sim.ents.get(1)!;
      const cost = ab.cost[Math.min(ab.cost.length - 1, rank - 1)] ?? 0;
      if ((me.mana ?? 0) < cost) break;
      me.mana = (me.mana ?? 0) - cost;
      sim.self = { ...sim.self, cooldowns: { ...sim.self.cooldowns, [m.slot]: ab.cooldown[Math.min(ab.cooldown.length - 1, rank - 1)] ?? 5 } };
      break;
    }
    case 'summoner':
      if (sim && sim.self.cooldowns[m.slot] <= 0) sim.self = { ...sim.self, cooldowns: { ...sim.self.cooldowns, [m.slot]: 180 } };
      break;
    case 'recall':
      if (sim) sim.self = { ...sim.self, recalling: 8 };
      break;
    case 'buy': {
      if (!sim) break;
      const me = sim.ents.get(1)!;
      const inv = [...(me.champ!.items ?? [])];
      const q = quoteItem(m.itemId, inv, ITEMS);
      if (q.price > sim.self.gold) break;
      const prev = [...inv];
      for (const sl of q.consumedSlots) inv[sl] = null;
      const free = inv.findIndex((x, i) => i < 6 && x === null);
      if (free < 0) break;
      inv[free] = m.itemId;
      me.champ = { ...me.champ!, items: inv };
      sim.shopStack.push({ kind: 'buy', itemId: m.itemId, slot: free, gold: q.price, prev });
      sim.self = { ...sim.self, gold: sim.self.gold - q.price };
      sim.events.push({ type: 'itemBought', target: 1, itemId: m.itemId });
      break;
    }
    case 'sell': {
      if (!sim) break;
      const me = sim.ents.get(1)!;
      const inv = [...(me.champ!.items ?? [])];
      const id = inv[m.slot];
      if (!id) break;
      const prev = [...inv];
      inv[m.slot] = null;
      const refund = Math.floor((ITEMS[id]?.cost ?? 0) * 0.7);
      me.champ = { ...me.champ!, items: inv };
      sim.shopStack.push({ kind: 'sell', itemId: id, slot: m.slot, gold: -refund, prev });
      sim.self = { ...sim.self, gold: sim.self.gold + refund };
      break;
    }
    case 'undoShop': {
      const last = sim?.shopStack.pop();
      if (!sim || !last) break;
      const me = sim.ents.get(1)!;
      me.champ = { ...me.champ!, items: last.prev };
      sim.self = { ...sim.self, gold: sim.self.gold + last.gold };
      break;
    }
    case 'move':
      if (sim) {
        const me = sim.ents.get(1)!;
        me.x = m.pos.x;
        me.y = m.pos.y;
      }
      break;
    case 'ping':
      sim?.events.push({ type: 'ping', playerId: ME, pos: m.pos, kind: m.kind });
      break;
    default:
      break;
  }
}
