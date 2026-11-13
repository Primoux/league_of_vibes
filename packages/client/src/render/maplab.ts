/**
 * Démo `?maplab` : carte + moteur client sans serveur. Propriétaire : claude-client.
 *
 * Un mini-serveur local simule à 30 Hz un champion contrôlable, un champion ennemi, des sbires sur les
 * 3 voies et les monstres de jungle, et envoie des snapshots à 20 Hz avec latence artificielle
 * (`?lag=ms`, défaut 80 ; `?jitter=ms`, défaut 20). Permet de tester caméra, input, picking,
 * interpolation et prédiction.
 */
import {
  CAMPS,
  FOUNTAINS,
  INHIBITORS,
  NEXUS,
  SNAPSHOT_RATE,
  TICK_DT,
  TOWERS,
  lanePathFor,
  moveTowards,
  type ClientMsg,
  type EntitySnapshot,
  type Lane,
  type SelfState,
  type Snapshot,
  type Team,
  type Vec2,
} from '@lov/shared';
import { installControls } from '../input/controls.ts';
import { net } from '../net/client.ts';
import { store } from '../state.ts';
import { GameRenderer } from './GameRenderer.ts';

interface SimEntity extends EntitySnapshot {
  target?: Vec2;
  path?: Vec2[];
  speed: number;
}

export function startMapLab(canvas: HTMLCanvasElement): void {
  const params = new URLSearchParams(location.search);
  const lag = Number(params.get('lag') ?? 80);
  const jitter = Number(params.get('jitter') ?? 20);

  const q = params.get('quality');
  if (q === 'low' || q === 'high') store.set({ settings: { ...store.state.settings, quality: q } });
  const renderer = new GameRenderer(canvas);
  installControls(renderer);

  let nextId = 1;
  const ents = new Map<number, SimEntity>();
  const add = (e: Omit<SimEntity, 'id' | 'facing' | 'hp' | 'maxHp'> & Partial<SimEntity>): SimEntity => {
    const full: SimEntity = { id: nextId++, facing: 0, hp: 1000, maxHp: 1000, ...e };
    ents.set(full.id, full);
    return full;
  };

  const me = add({ kind: 'champion', team: 'blue', subtype: 'ilyra', x: FOUNTAINS.blue.x + 600, y: FOUNTAINS.blue.y + 600, speed: 335, level: 1, anim: 'idle', mana: 400, maxMana: 400 });
  add({ kind: 'champion', team: 'red', subtype: 'zephyr', x: 5600, y: 5600, speed: 335, level: 3, anim: 'idle' });
  for (const t of TOWERS) add({ kind: 'tower', team: t.team, subtype: t.tier, x: t.pos.x, y: t.pos.y, speed: 0, hp: 3500, maxHp: 3500 });
  for (const i of INHIBITORS) add({ kind: 'inhibitor', team: i.team, x: i.pos.x, y: i.pos.y, speed: 0 });
  for (const team of ['blue', 'red'] as const) add({ kind: 'nexus', team, x: NEXUS[team].x, y: NEXUS[team].y, speed: 0, hp: 5000, maxHp: 5000 });
  for (const c of CAMPS) {
    add({ kind: 'monster', team: 'neutral', subtype: c.monster, x: c.pos.x, y: c.pos.y, speed: 0 });
    for (const s of c.smalls ?? []) add({ kind: 'monster', team: 'neutral', subtype: s.monster, x: c.pos.x + s.offset.x, y: c.pos.y + s.offset.y, speed: 0 });
  }
  add({ kind: 'monster', team: 'neutral', subtype: 'dragon', x: 7900, y: 4100, speed: 0, hp: 4000, maxHp: 4000 });
  add({ kind: 'monster', team: 'neutral', subtype: 'leviathan', x: 4100, y: 7900, speed: 0, hp: 9000, maxHp: 9000 });

  if (params.has('zoo')) {
    // vitrine de tous les modèles, en ligne dans la base bleue
    const kinds: [EntitySnapshot['kind'], string, Team | 'neutral'][] = [
      ['minion', 'melee', 'blue'], ['minion', 'caster', 'blue'], ['minion', 'cannon', 'blue'], ['minion', 'super', 'blue'],
      ['minion', 'melee', 'red'], ['minion', 'caster', 'red'], ['minion', 'cannon', 'red'], ['minion', 'super', 'red'],
      ['monster', 'wolves', 'neutral'], ['monster', 'wolf_small', 'neutral'], ['monster', 'raptors', 'neutral'], ['monster', 'raptor_small', 'neutral'],
      ['monster', 'krugs', 'neutral'], ['monster', 'krug_small', 'neutral'], ['monster', 'gromp', 'neutral'], ['monster', 'scuttle', 'neutral'],
      ['monster', 'blue_buff', 'neutral'], ['monster', 'red_buff', 'neutral'], ['monster', 'grub', 'neutral'], ['ward', '', 'blue'],
      ['monster', 'dragon', 'neutral'], ['monster', 'elder_dragon', 'neutral'], ['monster', 'herald', 'neutral'], ['monster', 'leviathan', 'neutral'],
    ];
    kinds.forEach(([kind, subtype, team], i) => {
      const col = i % 4;
      const row = Math.floor(i / 4);
      const big = row >= 5;
      add({ kind, subtype, team, x: big ? 3700 + col * 650 : 1500 + col * 330, y: big ? 3700 + col * 650 : 3300 - row * 330, speed: 0, anim: i % 3 === 0 ? 'run' : 'idle', facing: -Math.PI / 2 + 0.6 });
    });
  }

  if (params.has('bench')) {
    // scénario PLAN §11 : 8 champions + 60 sbires dans le champ de la caméra (mid), en mouvement
    const ids = ['grondak', 'tharox', 'kaelen', 'vrenna', 'ilyra', 'zephyr', 'sable', 'corvin'];
    ids.forEach((subtype, i) => {
      const team: Team = i < 4 ? 'blue' : 'red';
      add({ kind: 'champion', team, subtype, x: 5300 + (i % 4) * 250, y: 5000 + (i < 4 ? 0 : 700) + (i % 4) * 120, speed: 335, level: 9, anim: 'run', mana: 300, maxMana: 500 });
    });
    for (let i = 0; i < 60; i++) {
      const team: Team = i % 2 ? 'blue' : 'red';
      const sub = (['melee', 'caster', 'cannon', 'super'] as const)[i % 7 === 0 ? 2 : i % 11 === 0 ? 3 : i % 3 === 0 ? 1 : 0];
      add({ kind: 'minion', team, subtype: sub, x: 4800 + (i % 10) * 140, y: 4800 + Math.floor(i / 10) * 160, speed: 300, anim: i % 2 ? 'run' : 'attack', hp: 300 + (i % 5) * 50, maxHp: 500 });
    }
    // les champions tournent en rond pour solliciter interpolation et animations
    setInterval(() => {
      for (const e of ents.values()) if (e.kind === 'champion' && e !== me && !e.target) e.target = { x: 5000 + Math.random() * 1500, y: 5000 + Math.random() * 1500 };
    }, 500);
  }

  const spawnWave = () => {
    for (const team of ['blue', 'red'] as Team[]) {
      for (const lane of ['top', 'mid', 'bot'] as Lane[]) {
        const path = lanePathFor(lane, team);
        for (let i = 0; i < 6; i++) {
          const start = path[0]!;
          add({ kind: 'minion', team, subtype: i < 3 ? 'melee' : 'caster', x: start.x - i * 60, y: start.y - i * 60, speed: 325, path: path.slice(1), anim: 'run', hp: 450, maxHp: 450 });
        }
      }
    }
  };
  spawnWave();
  setInterval(spawnWave, 30000);

  net.setLoopback((msg: ClientMsg) => {
    if (msg.t === 'move' || msg.t === 'attackMove') me.target = { ...msg.pos };
    else if (msg.t === 'stop') me.target = undefined;
    else if (msg.t === 'summoner' && msg.targetPos) {
      // « Flash » de test : téléportation 400 u vers le curseur
      const p = moveTowards(me, msg.targetPos, 400);
      me.x = p.x;
      me.y = p.y;
      me.target = undefined;
    }
  });

  store.set({ connected: true, playerId: 'lab' });
  net.inject({
    t: 'gameStart',
    start: { you: 'lab', team: 'blue', entityId: me.id, serverTime: Date.now(), players: [{ id: 'lab', name: 'Labo', team: 'blue', championId: 'ilyra', summoners: ['flash', 'ignite'] }] },
  });
  store.set({ cameraLocked: false });
  const aimSlot = params.get('aim');
  if (aimSlot === 'Q' || aimSlot === 'W' || aimSlot === 'E' || aimSlot === 'R') setTimeout(() => store.set({ targeting: { kind: 'ability', slot: aimSlot } }), 500);
  if (params.has('cx')) {
    const pos = { x: Number(params.get('cx')), y: Number(params.get('cy') ?? params.get('cx')) };
    store.set({ cameraFocus: { pos, seq: 1 } });
  }

  let tick = 0;
  const start = performance.now();
  const step = (e: SimEntity) => {
    if (e.path && e.path.length && !e.target) e.target = e.path.shift();
    if (!e.target) {
      if (e.anim === 'run') e.anim = 'idle';
      return;
    }
    const n = moveTowards(e, e.target, e.speed * TICK_DT);
    e.facing = Math.atan2(e.target.y - e.y, e.target.x - e.x);
    e.x = n.x;
    e.y = n.y;
    e.anim = 'run';
    if (n.x === e.target.x && n.y === e.target.y) e.target = undefined;
    if (e.kind === 'minion' && !e.target && (!e.path || !e.path.length)) ents.delete(e.id);
  };

  setInterval(() => {
    tick++;
    for (const e of ents.values()) step(e);
    if (tick % (30 / SNAPSHOT_RATE) < 1) send();
  }, TICK_DT * 1000);

  const self: SelfState = {
    entityId: me.id,
    gold: 500,
    xp: 0,
    xpToNext: 280,
    level: 1,
    skillPoints: 1,
    abilityLevels: { Q: 1, W: 1, E: 1, R: 0 },
    cooldowns: { Q: 0, W: 0, E: 0, R: 0, D: 0, F: 0 },
    summoners: { D: 'flash', F: 'ignite' },
    stats: { maxHp: 1000, hpRegen: 5, maxMana: 400, manaRegen: 7, ad: 50, ap: 0, armor: 20, mr: 30, attackSpeed: 0.65, attackRange: 550, moveSpeed: 335, critChance: 0, abilityHaste: 0, lifesteal: 0, armorPen: 0, magicPen: 0 },
    canShop: true,
  };

  function send(): void {
    const snap: Snapshot = {
      tick,
      time: (performance.now() - start) / 1000,
      serverTime: Date.now(),
      entities: [...ents.values()].map(({ target: _t, path: _p, speed: _s, ...rest }) => ({ ...rest, x: Math.round(rest.x), y: Math.round(rest.y) })),
      events: [],
      self,
      score: { blue: { kills: 0, towers: 0, dragons: [], grubs: 0 }, red: { kills: 0, towers: 0, dragons: [], grubs: 0 } },
    };
    setTimeout(() => net.inject({ t: 'snapshot', snap }), Math.max(0, lag / 2 + (Math.random() - 0.5) * jitter));
  }

  const help = document.createElement('div');
  help.style.cssText = 'position:fixed;left:12px;top:12px;padding:10px 14px;background:rgba(10,14,28,.8);color:#e8e2cf;font:13px system-ui;border:1px solid #6b5a2e;border-radius:6px;pointer-events:none;line-height:1.5';
  help.innerHTML = `<b>Maplab</b> — latence ${lag} ms ±${jitter / 2}<br>Clic droit : déplacer / attaquer · D : flash test · Y : verrouiller caméra<br>Espace : centrer · molette : zoom · bords / flèches : défiler`;
  document.body.appendChild(help);
}
