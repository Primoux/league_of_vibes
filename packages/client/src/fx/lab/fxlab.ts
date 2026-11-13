/**
 * ?fxlab — banc d'essai des FX sans serveur réseau. Propriétaire : claude-fx.
 *
 * La VRAIE simulation serveur tourne dans la page (`labSim.ts`) : chaque bouton envoie une commande
 * (cast, attaque, sort d'invocateur) et les FX reçoivent exactement les événements du jeu. Les modèles de
 * champions sont pilotés comme par le renderer (position, orientation, anim, buffs des snapshots).
 *
 * Paramètres d'URL (captures reproductibles) :
 *   &champ=ilyra              champion du lanceur
 *   &cast=ilyra:W,grondak:R    actions jouées en boucle (slots P Q W E R, AA, DIE, RECALL, SUM_flash…)
 *   &every=2                  période de la boucle
 *   &at=0.6                   pas fixe de 1/60 s jusqu'à t puis image figée
 *   &aim=sable:W              indicateur de visée permanent
 *   &close=1|2                caméra rapprochée (lanceur | cibles)
 *   &showcase=1               vitrine des matériaux
 *   &spam=1                   charge aléatoire
 */
import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import {
  CHAMPION_LIST,
  CHAMPIONS,
  RENDER_SCALE,
  SUMMONERS,
  SUMMONER_IDS,
  type AbilitySlot,
  type ChampionDef,
  type EntitySnapshot,
  type GameEvent,
  type SummonerId,
  type Team,
  type Vec2,
} from '@lov/shared';
import { createChampionView, type AnimatedChampionView } from '../../render/champions/index.ts';
import { store } from '../../state.ts';
import { fxBus } from '../FxBus.ts';
import type { FxHost } from '../core/host.ts';
import { fxIndicators, installFx, setFxQuality } from '../index.ts';
import { LabSim, type SimActor } from './labSim.ts';
import { createShowcase } from './showcase.ts';

const SLOTS = ['P', 'Q', 'W', 'E', 'R'] as const;
/** Sorts ciblés qui visent un allié. */
const ALLY_TARGETED = new Set(['ilyra_e', 'solenne_w', 'solenne_e']);

interface Dummy {
  actor: SimActor;
  view: AnimatedChampionView;
  championId: string;
  anim: string;
}

const P = {
  caster: { x: 4300, y: 4300 },
  ally: { x: 4050, y: 4650 },
  main: { x: 4950, y: 4480 },
  t2: { x: 4780, y: 4900 },
  t3: { x: 5080, y: 4120 },
};
const toW = (p: Vec2, h = 0): THREE.Vector3 => new THREE.Vector3(p.x * RENDER_SCALE, h, -p.y * RENDER_SCALE);
const EMPTY_SCORE = { blue: { kills: 0, towers: 0, dragons: [], grubs: 0 }, red: { kills: 0, towers: 0, dragons: [], grubs: 0 } };

export function startFxLab(): void {
  document.title = 'League of Vibes — FX Lab';
  document.getElementById('game')?.remove();
  document.getElementById('ui')?.remove();
  const canvas = document.createElement('canvas');
  canvas.style.cssText = 'position:fixed;inset:0;width:100%;height:100%;display:block';
  document.body.style.margin = '0';
  document.body.appendChild(canvas);
  const params = new URLSearchParams(location.search);

  // ------------------------------------------------------------ scène
  const renderer = new THREE.WebGLRenderer({ canvas, antialias: false, powerPreference: 'high-performance' });
  renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
  renderer.setSize(innerWidth, innerHeight, false);
  renderer.toneMapping = THREE.ACESFilmicToneMapping;
  renderer.toneMappingExposure = 1.05;
  renderer.shadowMap.enabled = true;
  renderer.info.autoReset = false; // le composer fait plusieurs rendus par frame : on compte la frame entière
  const scene = new THREE.Scene();
  scene.background = new THREE.Color(0x0e1424);
  scene.fog = new THREE.Fog(0x0e1424, 30, 70);
  const camera = new THREE.PerspectiveCamera(40, innerWidth / innerHeight, 0.1, 200);
  const center = toW({ x: 4600, y: 4480 });
  camera.position.set(center.x, 22, center.z + 14);
  const controls = new OrbitControls(camera, canvas);
  controls.target.copy(center);
  controls.enableDamping = true;
  const close = params.get('close');
  if (close) {
    const focus = close === '2' ? toW(P.main, 1) : toW(P.caster, 1);
    controls.target.copy(focus);
    camera.position.set(focus.x + 1.6, focus.y + 2.2, focus.z + (close === '2' ? 7 : 4.2));
  }

  scene.add(new THREE.HemisphereLight(0xcfe0ff, 0x30402a, 1.1));
  const sun = new THREE.DirectionalLight(0xfff0d8, 2.2);
  sun.position.set(center.x + 8, 20, center.z + 6);
  sun.target.position.copy(center);
  sun.castShadow = true;
  sun.shadow.mapSize.set(1024, 1024);
  Object.assign(sun.shadow.camera, { left: -15, right: 15, top: 15, bottom: -15 });
  scene.add(sun, sun.target);
  const ground = new THREE.Mesh(new THREE.CircleGeometry(30, 64), new THREE.MeshStandardMaterial({ color: 0x35512d, roughness: 0.95 }));
  ground.rotation.x = -Math.PI / 2;
  ground.position.copy(center);
  ground.receiveShadow = true;
  scene.add(ground);
  const grid = new THREE.GridHelper(40, 40, 0x4d6b44, 0x40593a);
  grid.position.set(center.x, 0.005, center.z);
  scene.add(grid);

  // ------------------------------------------------------------ simulation + modèles
  const dummies = new Map<number, Dummy>();
  const host: FxHost = { scene, camera, renderer, getEntityObject: (id) => dummies.get(id)?.view.object3d };
  const fx = installFx(host, { manualUpdate: true, ownPost: true });
  const sim = new LabSim((e) => fxBus.dispatch(e));
  // debug : inspection du runtime depuis la console (lab uniquement)
  (globalThis as { __fxlab?: unknown }).__fxlab = { fx, sim, scene };
  let eventLog: GameEvent[] = [];
  fxBus.subscribe((e) => {
    if (e.type !== 'damage' && e.type !== 'autoAttack') eventLog.push(e);
  });

  const mount = (championId: string, team: Team, pos: Vec2): Dummy => {
    const actor = sim.spawn(championId, team, pos);
    const view = createChampionView({ id: actor.id, subtype: championId, team })!;
    scene.add(view.object3d);
    const d: Dummy = { actor, view, championId, anim: '' };
    dummies.set(actor.id, d);
    return d;
  };
  const unmount = (d: Dummy): void => {
    d.view.dispose();
    sim.remove(d.actor.id);
    dummies.delete(d.actor.id);
  };

  let caster = mount(params.get('champ') ?? 'grondak', 'blue', P.caster);
  const ally = mount('solenne', 'blue', P.ally);
  const targets = [mount('kaelen', 'red', P.main), mount('sable', 'red', P.t2), mount('nyx', 'red', P.t3)];
  const main = targets[0]!;
  // &stress=1 : 4v4 complet (8 champions différents) pour mesurer un teamfight
  if (params.get('stress') === '1') {
    mount('vrenna', 'blue', { x: 4450, y: 4050 });
    mount('zephyr', 'blue', { x: 4150, y: 4400 });
    targets.push(mount('corvin', 'red', { x: 5200, y: 4700 }));
  }

  const useChampion = (champ: ChampionDef): void => {
    if (caster.championId === champ.id) return;
    unmount(caster);
    caster = mount(champ.id, 'blue', P.caster);
  };

  // ------------------------------------------------------------ actions
  let attackUntil = -1;
  let clock = 0;
  const mainPos = (): Vec2 => {
    const t = sim.entity(main.actor.id);
    return t ? { ...t.pos } : { ...P.main };
  };
  const castAbility = (champ: ChampionDef, slot: AbilitySlot | 'P'): void => {
    useChampion(champ);
    if (slot === 'P') {
      // les passifs se déclenchent en combat : quelques secondes d'auto-attaques
      sim.attack(caster.actor.id, main.actor.id);
      attackUntil = clock + 3;
      return;
    }
    const def = champ.abilities[slot];
    const target = ALLY_TARGETED.has(def.id) ? ally : main;
    const t = sim.entity(target.actor.id);
    sim.cast(caster.actor.id, slot, target.actor, t ? { ...t.pos } : mainPos());
  };
  const autoAttack = (champ: ChampionDef): void => {
    useChampion(champ);
    sim.attack(caster.actor.id, main.actor.id);
    attackUntil = clock + 1.6;
  };
  const castSummoner = (spell: SummonerId): void => {
    sim.summoner(caster.actor.id, spell, spell === 'teleport' ? ally.actor : main.actor, mainPos());
  };
  const killTarget = (): void => sim.kill(main.actor.id, caster.actor.id);
  let recalling = false;
  const toggleRecall = (): void => {
    recalling = !recalling;
    fxBus.dispatch(recalling ? { type: 'recallStart', target: caster.actor.id } : { type: 'recallEnd', target: caster.actor.id, completed: false });
  };
  /** Événements du monde que la simulation du lab n'inclut pas (économie, objectifs, structures). */
  const WORLD: Record<string, [string, () => void]> = {
    LEVEL: ['Niveau', () => fxBus.dispatch({ type: 'levelUp', target: caster.actor.id, level: 6 })],
    GOLD: ['Or', () => fxBus.dispatch({ type: 'goldGain', target: caster.actor.id, amount: 300 })],
    TOWER: ['Tour', () => fxBus.dispatch({ type: 'structureDestroyed', target: targets[2]!.actor.id, kind: 'tower', team: 'red' })],
    PLATE: ['Plaque', () => fxBus.dispatch({ type: 'structureDestroyed', target: targets[2]!.actor.id, kind: 'tower', team: 'red', plate: true })],
    NEXUS: ['Nexus', () => fxBus.dispatch({ type: 'structureDestroyed', target: targets[1]!.actor.id, kind: 'nexus', team: 'red' })],
    DRAKE: ['Drake', () => fxBus.dispatch({ type: 'objectiveTaken', monster: 'dragon', team: 'blue', killer: caster.actor.id, element: 'infernal' })],
    BLUE: ['Buff bleu', () => fxBus.dispatch({ type: 'buffApplied', target: caster.actor.id, buffId: 'blue_buff', duration: 8 })],
    RED: ['Buff rouge', () => fxBus.dispatch({ type: 'buffApplied', target: caster.actor.id, buffId: 'red_buff', duration: 8 })],
    LEVI: ['Léviathan', () => fxBus.dispatch({ type: 'buffApplied', target: caster.actor.id, buffId: 'leviathan', duration: 8 })],
  };
  const resetAll = (): void => {
    sim.reset();
    attackUntil = -1;
    sim.stop(caster.actor.id);
  };

  // ------------------------------------------------------------ interface
  const style = document.createElement('style');
  style.textContent = `
    .fxlab{position:fixed;top:0;left:0;bottom:0;width:340px;overflow:auto;background:rgba(10,14,26,.88);color:#dfe8ff;
      font:12px/1.4 system-ui,sans-serif;padding:10px;box-sizing:border-box;border-right:1px solid #2a3557}
    .fxlab h1{font-size:15px;margin:0 0 8px} .fxlab h2{font-size:12px;margin:12px 0 4px;color:#9fb3e6;text-transform:uppercase;letter-spacing:.05em}
    .fxlab .row{display:flex;align-items:center;gap:4px;margin:3px 0;flex-wrap:wrap} .fxlab .name{flex:1;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
    .fxlab button{background:#1d2742;color:#dfe8ff;border:1px solid #34446f;border-radius:4px;padding:3px 7px;cursor:pointer;font:inherit}
    .fxlab button:hover{background:#2b3a63}
    .fxlab .sw{width:10px;height:10px;border-radius:2px;display:inline-block}
    .fxlab .stats{font:11px/1.5 ui-monospace,monospace;color:#b8c6ea;white-space:pre}
    .fxlab .log{font:10px/1.4 ui-monospace,monospace;color:#8fa2d6;white-space:pre;max-height:160px;overflow:auto}
    .fxlab label{display:flex;gap:6px;align-items:center;margin:3px 0}
  `;
  document.head.appendChild(style);
  const panel = document.createElement('div');
  panel.className = 'fxlab';
  document.body.appendChild(panel);
  const el = <K extends keyof HTMLElementTagNameMap>(tag: K, props: Partial<HTMLElementTagNameMap[K]> = {}, ...kids: (Node | string)[]): HTMLElementTagNameMap[K] => {
    const n = Object.assign(document.createElement(tag), props);
    n.append(...kids);
    return n;
  };
  panel.append(el('h1', {}, 'FX Lab — League of Vibes'));
  const stats = el('div', { className: 'stats' });
  panel.append(stats);

  panel.append(el('h2', {}, 'Options'));
  let spam = params.get('spam') === '1';
  let showAim = true;
  let moving = false;
  const toggle = (label: string, init: boolean, fn: (v: boolean) => void): void => {
    const input = el('input', { type: 'checkbox', checked: init });
    input.addEventListener('change', () => fn(input.checked));
    panel.append(el('label', {}, input, label));
  };
  toggle('Qualité haute (post-process)', true, (v) => setFxQuality(v ? 'high' : 'low'));
  toggle('Indicateur de visée au survol', true, (v) => {
    showAim = v;
    if (!v) fxIndicators.hide();
  });
  toggle('Cibles mobiles', false, (v) => (moving = v));
  toggle('Spam aléatoire (charge)', spam, (v) => (spam = v));
  const bloom = el('input', { type: 'range', min: '0', max: '2', step: '0.05', value: '0.55' });
  bloom.addEventListener('input', () => fx.post?.setBloom(Number(bloom.value)));
  panel.append(el('label', {}, 'Bloom', bloom));
  panel.append(el('div', { className: 'row' }, el('button', { onclick: resetAll }, 'Replacer'), el('button', { onclick: killTarget }, 'Tuer la cible'), el('button', { onclick: toggleRecall }, 'Rappel')));

  panel.append(el('h2', {}, 'Champions (niveau 18)'));
  for (const champ of CHAMPION_LIST) {
    const row = el('div', { className: 'row' });
    const sw = el('span', { className: 'sw' });
    sw.style.background = champ.palette.primary;
    row.append(sw, el('span', { className: 'name', title: champ.title }, champ.name));
    const aa = el('button', { title: 'Auto-attaque' }, 'AA');
    aa.addEventListener('click', () => autoAttack(champ));
    row.append(aa);
    for (const slot of SLOTS) {
      const def = slot === 'P' ? champ.passive : champ.abilities[slot];
      const b = el('button', { title: `${def.name} — ${def.description}` }, slot);
      b.addEventListener('click', () => castAbility(champ, slot));
      if (slot !== 'P') {
        b.addEventListener('mouseenter', () => showAim && fxIndicators.showAbility(def, P.caster, P.main));
        b.addEventListener('mouseleave', () => fxIndicators.hide());
      }
      row.append(b);
    }
    panel.append(row);
  }

  panel.append(el('h2', {}, "Sorts d'invocateur"));
  const sumRow = el('div', { className: 'row' });
  for (const id of SUMMONER_IDS) {
    const def = SUMMONERS[id];
    const b = el('button', { title: def.description }, def.name);
    b.addEventListener('click', () => castSummoner(id));
    b.addEventListener('mouseenter', () => showAim && fxIndicators.showSummoner(def, P.caster, P.main));
    b.addEventListener('mouseleave', () => fxIndicators.hide());
    sumRow.append(b);
  }
  panel.append(sumRow);
  panel.append(el('h2', {}, 'Monde'));
  const worldRow = el('div', { className: 'row' });
  for (const [label, fn] of Object.values(WORLD)) worldRow.append(el('button', { onclick: fn }, label));
  panel.append(worldRow);
  panel.append(el('h2', {}, 'Événements reçus'));
  const log = el('div', { className: 'log' });
  panel.append(log);

  // ------------------------------------------------------------ paramètres d'URL
  const autoCasts = (params.get('cast') ?? '')
    .split(',')
    .map((x) => x.split(':'))
    .filter((x): x is [string, string] => x.length === 2 && !!CHAMPIONS[x[0]!]);
  const every = Number(params.get('every') ?? 2);
  const freezeAt = params.has('at') ? Number(params.get('at')) : null;
  const runOne = (champId: string, slot: string): void => {
    const champ = CHAMPIONS[champId]!;
    if (slot === 'AA') autoAttack(champ);
    else if (slot === 'DIE') killTarget();
    else if (slot === 'RECALL') toggleRecall();
    else if (WORLD[slot]) WORLD[slot][1]();
    else if (slot.startsWith('SUM_')) {
      useChampion(champ);
      castSummoner(slot.slice(4) as SummonerId);
    } else if ((SLOTS as readonly string[]).includes(slot)) castAbility(champ, slot as AbilitySlot | 'P');
  };
  const runAuto = (): void => {
    autoCasts.forEach(([c, slot], i) => fx.runtime.after(i * 0.15, () => runOne(c, slot)));
    if (autoCasts.length) fx.runtime.after(every, runAuto);
  };
  runAuto();
  const [aimChamp, aimSlot] = (params.get('aim') ?? '').split(':');
  const aimDef = aimChamp ? CHAMPIONS[aimChamp]?.abilities[aimSlot as AbilitySlot] : undefined;
  if (aimDef) fxIndicators.showAbility(aimDef, P.caster, P.main);
  if (params.get('quality') === 'low') setFxQuality('low');
  // &as=red : on regarde la scène comme un joueur rouge → le lanceur bleu est un ennemi (teinte des zones)
  if (params.get('as') === 'red') store.set({ game: { you: 'lab', team: 'red', entityId: -1, players: [], serverTime: 0 } });
  const showcase = params.get('showcase') === '1' ? createShowcase(fx.runtime, toW({ x: 4400, y: 3950 })) : null;

  // ------------------------------------------------------------ boucle
  addEventListener('resize', () => {
    renderer.setSize(innerWidth, innerHeight, false);
    camera.aspect = innerWidth / innerHeight;
    camera.updateProjectionMatrix();
    fx.post?.setSize(innerWidth, innerHeight);
  });

  let spamAcc = 0;
  let stressAcc = 0;
  const stress = params.get('stress') === '1';
  let moveFlip = 0;
  const simulate = (dt: number): void => {
    clock += dt;
    if (attackUntil > 0 && clock > attackUntil) {
      attackUntil = -1;
      sim.stop(caster.actor.id);
    }
    if (moving && Math.floor(clock / 1.5) !== moveFlip) {
      moveFlip = Math.floor(clock / 1.5);
      for (const t of targets) sim.moveTo(t.actor.id, { x: t.actor.home.x + (moveFlip % 2 ? 250 : -250), y: t.actor.home.y + (moveFlip % 2 ? 150 : -150) });
    }
    if (stress) {
      // tous les champions lancent des sorts en continu (charge de teamfight)
      stressAcc += dt;
      while (stressAcc > 0.25) {
        stressAcc -= 0.25;
        const all = [...dummies.values()];
        const d = all[Math.floor(Math.random() * all.length)]!;
        const slot = (['Q', 'W', 'E', 'R'] as const)[Math.floor(Math.random() * 4)]!;
        const foes = all.filter((o) => o.actor.team !== d.actor.team);
        const target = foes[Math.floor(Math.random() * foes.length)] ?? main;
        const t = sim.entity(target.actor.id);
        sim.cast(d.actor.id, slot, target.actor, t ? { ...t.pos } : P.main);
        if (Math.random() < 0.3) sim.attack(d.actor.id, target.actor.id);
      }
    }
    if (spam) {
      spamAcc += dt;
      while (spamAcc > 0.5) {
        spamAcc -= 0.5;
        castAbility(CHAMPIONS[caster.championId]!, SLOTS[1 + Math.floor(Math.random() * 4)]!);
      }
    }
    sim.update(dt);
    // les modèles suivent la simulation comme le renderer suit les snapshots
    const snaps: EntitySnapshot[] = [];
    for (const d of dummies.values()) {
      const snap = sim.snapshot(d.actor.id);
      if (!snap) continue;
      snaps.push(snap);
      const o = d.view.object3d;
      o.position.set(d.actor.x * RENDER_SCALE, 0, -d.actor.y * RENDER_SCALE);
      o.rotation.y = d.actor.facing;
      const anim = snap.dead ? 'death' : (snap.anim ?? 'idle');
      if (anim !== d.anim) {
        d.anim = anim;
        d.view.playAnim(anim);
      }
      d.view.update(snap, dt);
    }
    store.set({ snapshot: { tick: 0, time: clock, serverTime: 0, events: [], entities: snaps, score: EMPTY_SCORE } });
    showcase?.update(dt);
    fx.update(dt);
  };

  let last = performance.now();
  let fpsAcc = 0;
  let fpsFrames = 0;
  const loop = (now: number): void => {
    const dt = Math.min(0.1, (now - last) / 1000);
    last = now;
    if (freezeAt === null) simulate(dt);
    else for (let i = 0; i < 240 && clock < freezeAt - 1e-6; i++) simulate(Math.min(1 / 60, freezeAt - clock));
    controls.update();
    renderer.info.reset();
    fx.post?.render();

    fpsAcc += dt;
    fpsFrames++;
    if (fpsAcc >= 0.5) {
      const info = renderer.info;
      stats.textContent =
        `fps            ${(fpsFrames / fpsAcc).toFixed(0)}\n` +
        `FX cpu/frame   ${fx.stats.updateMs.toFixed(2)} ms\n` +
        `effets actifs  ${fx.runtime.activeEffects}\n` +
        `projectiles    ${fx.dispatcher.inFlight}\n` +
        `particules ém. ${fx.runtime.ps.emitted}\n` +
        `draw calls     ${info.render.calls}\n` +
        `événements     ${fx.dispatcher.stats.events} (génériques ${fx.dispatcher.stats.defaults}, dédiés ${fx.dispatcher.stats.custom})`;
      fpsAcc = 0;
      fpsFrames = 0;
    }
    if (eventLog.length) {
      log.textContent = [...eventLog.map(describe).reverse(), log.textContent ?? ''].join('\n').split('\n').slice(0, 60).join('\n');
      eventLog = [];
    }
    requestAnimationFrame(loop);
  };
  requestAnimationFrame(loop);
}

function describe(e: GameEvent): string {
  const key = 'fxKey' in e && e.fxKey ? ` [${e.fxKey}]` : '';
  switch (e.type) {
    case 'buffApplied':
      return `buff+ ${e.buffId}${key} ${e.duration.toFixed(1)}s${e.stacks ? ` ×${e.stacks}` : ''}`;
    case 'buffRemoved':
      return `buff- ${e.buffId}${key}`;
    case 'areaEffect':
      return `area${key} r=${e.radius} ${e.duration.toFixed(2)}s`;
    case 'dash':
      return `dash${key} ${e.duration.toFixed(2)}s`;
    case 'abilityCast':
      return `cast ${e.championId} ${e.slot}`;
    default:
      return `${e.type}${key}`;
  }
}
