/**
 * ?fxlab — banc d'essai des FX sans serveur. Propriétaire : claude-fx.
 * Mannequins, un bouton par sort de chaque champion et par sort d'invocateur, aperçu des indicateurs de visée,
 * mode « spam » pour mesurer la tenue en charge, réglages de qualité et de bloom.
 */
import * as THREE from 'three';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import { CHAMPION_LIST, RENDER_SCALE, SUMMONERS, SUMMONER_IDS, type AbilitySlot, type ChampionDef, type EntitySnapshot, type GameEvent, type Team, type Vec2 } from '@lov/shared';
import { createChampionView, type AnimatedChampionView } from '../../render/champions/index.ts';
import { store } from '../../state.ts';
import { fxBus } from '../FxBus.ts';
import type { FxHost } from '../core/host.ts';
import { fxIndicators, installFx, setFxQuality } from '../index.ts';
import { createShowcase } from './showcase.ts';
import { synthAbility, synthSummoner, type LabActor, type TimedEvent } from './synth.ts';

const SLOTS = ['P', 'Q', 'W', 'E', 'R'] as const;
const CASTER_ID = 1;
const TARGET_IDS = [2, 3, 4];

interface Dummy extends LabActor {
  view: AnimatedChampionView;
  mesh: THREE.Object3D;
  snap: EntitySnapshot;
  anim: string;
  home: Vec2;
  /** Point regardé quand il ne bouge pas. */
  lookAt: Vec2;
  dash: { from: Vec2; to: Vec2; t: number; duration: number } | null;
}

export function startFxLab(): void {
  document.title = 'League of Vibes — FX Lab';
  document.getElementById('game')?.remove();
  document.getElementById('ui')?.remove();
  const canvas = document.createElement('canvas');
  canvas.style.cssText = 'position:fixed;inset:0;width:100%;height:100%;display:block';
  document.body.style.margin = '0';
  document.body.appendChild(canvas);

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
  const center = new THREE.Vector3(13, 0, -10.5);
  camera.position.set(center.x, 22, center.z + 14);
  const controls = new OrbitControls(camera, canvas);
  controls.target.copy(center);
  controls.enableDamping = true;

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

  // ------------------------------------------------------------ mannequins (vrais modèles de champions)
  const dummies = new Map<number, Dummy>();
  const snapFor = (id: number, champ: string, team: Team): EntitySnapshot => ({
    id, kind: 'champion', team, subtype: champ, x: 0, y: 0, facing: 0, hp: 1000, maxHp: 1000, anim: 'idle',
  });
  /** Les FX lisent l'équipe et le champion des entités dans le store (comme en jeu) : on y publie les mannequins. */
  const publishSnapshot = (): void => {
    store.set({
      snapshot: {
        tick: 0, time: 0, serverTime: 0, events: [],
        entities: [...dummies.values()].map((d) => d.snap),
        score: { blue: { kills: 0, towers: 0, dragons: [], grubs: 0 }, red: { kills: 0, towers: 0, dragons: [], grubs: 0 } },
      },
    });
  };
  const mountView = (d: Dummy, champ: string): void => {
    d.view?.dispose();
    d.snap = { ...d.snap, subtype: champ };
    d.view = createChampionView(d.snap)!;
    d.mesh = d.view.object3d;
    d.anim = '';
    scene.add(d.mesh);
    if (dummies.has(d.id)) publishSnapshot();
  };
  const makeDummy = (id: number, pos: Vec2, champ: string, team: Team, lookAt: Vec2): Dummy => {
    const d = { id, pos: { ...pos }, home: { ...pos }, lookAt, dash: null, snap: snapFor(id, champ, team), anim: '' } as unknown as Dummy;
    mountView(d, champ);
    dummies.set(id, d);
    publishSnapshot();
    return d;
  };
  const params0 = new URLSearchParams(location.search);
  const casterPos = { x: 1000, y: 1050 };
  const caster = makeDummy(CASTER_ID, casterPos, params0.get('champ') ?? 'grondak', 'blue', { x: 1650, y: 1050 });
  const targets = [
    makeDummy(TARGET_IDS[0]!, { x: 1650, y: 1050 }, 'kaelen', 'red', casterPos),
    makeDummy(TARGET_IDS[1]!, { x: 1500, y: 1300 }, 'sable', 'red', casterPos),
    makeDummy(TARGET_IDS[2]!, { x: 1550, y: 820 }, 'nyx', 'red', casterPos),
  ];
  const mainTarget = targets[0]!;
  // &close=1 : caméra rapprochée sur le lanceur (inspection des modèles) ; &close=2 : sur les cibles
  const close = params0.get('close');
  if (close) {
    const focus = close === '2' ? new THREE.Vector3(15.6, 1, -10.5) : new THREE.Vector3(casterPos.x * RENDER_SCALE, 1, -casterPos.y * RENDER_SCALE);
    controls.target.copy(focus);
    camera.position.set(focus.x + 1.6, focus.y + 2.2, focus.z + (close === '2' ? 7 : 4.2));
  }

  const host: FxHost = { scene, camera, renderer, getEntityObject: (id) => dummies.get(id)?.mesh };
  const fx = installFx(host, { manualUpdate: true, ownPost: true });

  // Les dashes/blinks déplacent le mannequin concerné.
  fxBus.subscribe((e) => {
    if (e.type !== 'dash') return;
    const d = dummies.get(e.source);
    if (d) d.dash = { from: { ...e.from }, to: { ...e.to }, t: 0, duration: Math.max(0.01, e.duration) };
  });

  // ------------------------------------------------------------ lecture des séquences
  let projId = 1000;
  const nextId = (): number => projId++;
  const play = (seq: TimedEvent[]): void => {
    for (const { t, ev } of seq) fx.runtime.after(t, () => fxBus.dispatch(ev as GameEvent));
  };
  const resetPositions = (): void => {
    for (const d of dummies.values()) {
      d.pos = { ...d.home };
      d.dash = null;
      d.mesh.position.set(d.pos.x * RENDER_SCALE, 0, -d.pos.y * RENDER_SCALE);
    }
  };
  const useChampion = (champ: ChampionDef): void => {
    if (caster.snap.subtype !== champ.id) mountView(caster, champ.id);
  };
  const castAbility = (champ: ChampionDef, slot: AbilitySlot | 'P'): void => {
    useChampion(champ);
    if (Math.hypot(caster.pos.x - caster.home.x, caster.pos.y - caster.home.y) > 50) resetPositions();
    play(synthAbility(champ, slot, caster, mainTarget, nextId));
  };
  const autoAttack = (champ: ChampionDef): void => {
    useChampion(champ);
    caster.snap.anim = 'attack';
    const windup = champ.attackWindup / champ.baseStats.attackSpeed;
    fx.runtime.after(windup, () => {
      fxBus.dispatch({ type: 'autoAttack', source: caster.id, target: mainTarget.id, ranged: champ.ranged });
      fx.runtime.after(0.5, () => (caster.snap.anim = 'idle'));
    });
  };

  // ------------------------------------------------------------ interface
  const style = document.createElement('style');
  style.textContent = `
    .fxlab{position:fixed;top:0;left:0;bottom:0;width:330px;overflow:auto;background:rgba(10,14,26,.88);color:#dfe8ff;
      font:12px/1.4 system-ui,sans-serif;padding:10px;box-sizing:border-box;border-right:1px solid #2a3557}
    .fxlab h1{font-size:15px;margin:0 0 8px} .fxlab h2{font-size:12px;margin:12px 0 4px;color:#9fb3e6;text-transform:uppercase;letter-spacing:.05em}
    .fxlab .row{display:flex;align-items:center;gap:4px;margin:3px 0} .fxlab .name{flex:1;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
    .fxlab button{background:#1d2742;color:#dfe8ff;border:1px solid #34446f;border-radius:4px;padding:3px 7px;cursor:pointer;font:inherit}
    .fxlab button:hover{background:#2b3a63} .fxlab button.on{background:#3e5fb0}
    .fxlab .sw{width:10px;height:10px;border-radius:2px;display:inline-block}
    .fxlab .stats{font:11px/1.5 ui-monospace,monospace;color:#b8c6ea;white-space:pre}
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

  // options
  panel.append(el('h2', {}, 'Options'));
  let spam = false;
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
  toggle('Cible mobile', false, (v) => (moving = v));
  toggle('Spam aléatoire (charge)', false, (v) => (spam = v));
  const bloom = el('input', { type: 'range', min: '0', max: '2', step: '0.05', value: '0.55' });
  bloom.addEventListener('input', () => fx.post?.setBloom(Number(bloom.value)));
  panel.append(el('label', {}, 'Bloom', bloom));
  const toggleDeath = (): void => {
    mainTarget.snap.dead = !mainTarget.snap.dead;
    if (mainTarget.snap.dead) fxBus.dispatch({ type: 'death', target: mainTarget.id, killer: caster.id });
  };
  let recalling = false;
  const toggleRecall = (): void => {
    recalling = !recalling;
    fxBus.dispatch(recalling ? { type: 'recallStart', target: caster.id } : { type: 'recallEnd', target: caster.id, completed: false });
  };
  panel.append(
    el('div', { className: 'row' },
      el('button', { onclick: resetPositions }, 'Replacer'),
      el('button', { onclick: toggleDeath }, 'Mort / retour cible'),
      el('button', { onclick: toggleRecall }, 'Rappel'),
    ),
  );

  // champions
  panel.append(el('h2', {}, 'Champions'));
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
        b.addEventListener('mouseenter', () => showAim && fxIndicators.showAbility(def, caster.pos, mainTarget.pos));
        b.addEventListener('mouseleave', () => fxIndicators.hide());
      }
      row.append(b);
    }
    panel.append(row);
  }

  // sorts d'invocateur
  panel.append(el('h2', {}, "Sorts d'invocateur"));
  const sumRow = el('div', { className: 'row' });
  sumRow.style.flexWrap = 'wrap';
  for (const id of SUMMONER_IDS) {
    const def = SUMMONERS[id];
    const b = el('button', { title: def.description }, def.name);
    b.addEventListener('click', () => {
      resetPositions();
      play(synthSummoner(def, caster, mainTarget));
    });
    b.addEventListener('mouseenter', () => showAim && fxIndicators.showSummoner(def, caster.pos, mainTarget.pos));
    b.addEventListener('mouseleave', () => fxIndicators.hide());
    sumRow.append(b);
  }
  panel.append(sumRow);

  // ------------------------------------------------------------ paramètres d'URL (captures automatiques)
  // ?fxlab&cast=ilyra:W,grondak:R&every=1.5 : relance la liste en boucle ; &spam=1 : charge aléatoire
  // &at=0.6 : pas fixe de 1/60 s puis gel à t = 0.6 s (captures reproductibles, même sur un GPU lent)
  const params = new URLSearchParams(location.search);
  // slots spéciaux : AA (auto-attaque), DIE (mort/retour de la cible), RECALL (rappel du lanceur)
  const autoCasts = (params.get('cast') ?? '')
    .split(',')
    .map((x) => x.split(':'))
    .map(([c, slot]) => ({ champ: CHAMPION_LIST.find((x) => x.id === c), slot: slot ?? '' }))
    .filter((x): x is { champ: ChampionDef; slot: string } => !!x.champ && x.slot !== '');
  const every = Number(params.get('every') ?? 2);
  if (params.get('spam') === '1') spam = true;
  const freezeAt = params.has('at') ? Number(params.get('at')) : null;
  const runOne = (champ: ChampionDef, slot: string): void => {
    if (slot === 'AA') autoAttack(champ);
    else if (slot === 'DIE') toggleDeath();
    else if (slot === 'RECALL') toggleRecall();
    else if ((SLOTS as readonly string[]).includes(slot)) castAbility(champ, slot as AbilitySlot | 'P');
  };
  const runAuto = (): void => {
    autoCasts.forEach(({ champ, slot }, i) => fx.runtime.after(i * 0.15, () => runOne(champ, slot)));
    if (autoCasts.length) fx.runtime.after(every, runAuto);
  };
  runAuto();
  // &aim=sable:W : affiche en permanence l'indicateur de visée du sort (vers la cible principale)
  const [aimChamp, aimSlot] = (params.get('aim') ?? '').split(':');
  const aimDef = CHAMPION_LIST.find((c) => c.id === aimChamp)?.abilities[aimSlot as AbilitySlot];
  if (aimDef) fxIndicators.showAbility(aimDef, caster.pos, mainTarget.pos);
  // &showcase=1 : vitrine des matériaux (fresnel, givre, eau, chaleur, dissolve) au sud des mannequins
  const showcase = params.get('showcase') === '1' ? createShowcase(fx.runtime, new THREE.Vector3(center.x - 1.5, 0, center.z + 5)) : null;

  // ------------------------------------------------------------ boucle
  addEventListener('resize', () => {
    renderer.setSize(innerWidth, innerHeight, false);
    camera.aspect = innerWidth / innerHeight;
    camera.updateProjectionMatrix();
    fx.post?.setSize(innerWidth, innerHeight);
  });

  let last = performance.now();
  let fpsAcc = 0;
  let fpsFrames = 0;
  let fps = 0;
  let spamAcc = 0;
  let clock = 0;
  const simulate = (dt: number): void => {
    clock += dt;
    for (const d of dummies.values()) {
      if (d.dash) {
        d.dash.t = Math.min(1, d.dash.t + dt / d.dash.duration);
        d.pos.x = d.dash.from.x + (d.dash.to.x - d.dash.from.x) * d.dash.t;
        d.pos.y = d.dash.from.y + (d.dash.to.y - d.dash.from.y) * d.dash.t;
        if (d.dash.t >= 1) d.dash = null;
      } else if (moving && d !== caster) {
        d.pos.y = d.home.y + Math.sin(clock * 1.3 + d.id) * 200;
      }
      const vy = moving && d !== caster && !d.dash ? Math.cos(clock * 1.3 + d.id) : 0;
      d.mesh.position.set(d.pos.x * RENDER_SCALE, 0, -d.pos.y * RENDER_SCALE);
      // orientation : sens du déplacement, sinon vers sa cible
      if (d.dash) d.snap.facing = Math.atan2(d.dash.to.y - d.dash.from.y, d.dash.to.x - d.dash.from.x);
      else if (Math.abs(vy) > 0.05) d.snap.facing = vy > 0 ? Math.PI / 2 : -Math.PI / 2;
      else d.snap.facing = Math.atan2(d.lookAt.y - d.pos.y, d.lookAt.x - d.pos.x);
      d.mesh.rotation.y = d.snap.facing;
      d.snap.x = d.pos.x;
      d.snap.y = d.pos.y;
      const serverAnim = d.snap.dead ? 'death' : d.dash ? 'dash' : Math.abs(vy) > 0.05 ? 'run' : (d.snap.anim ?? 'idle');
      if (serverAnim !== d.anim) {
        d.anim = serverAnim;
        d.view.playAnim(serverAnim);
      }
      d.view.update(d.snap, dt);
    }
    if (spam) {
      spamAcc += dt;
      while (spamAcc > 0.12) {
        spamAcc -= 0.12;
        const champ = CHAMPION_LIST[Math.floor(Math.random() * CHAMPION_LIST.length)]!;
        const slot = SLOTS[1 + Math.floor(Math.random() * 4)]!;
        castAbility(champ, slot);
      }
    }
    showcase?.update(dt);
    fx.update(dt);
  };

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
      fps = fpsFrames / fpsAcc;
      fpsAcc = 0;
      fpsFrames = 0;
      const info = renderer.info;
      stats.textContent =
        `fps            ${fps.toFixed(0)}\n` +
        `effets actifs  ${fx.runtime.activeEffects}\n` +
        `projectiles    ${fx.dispatcher.inFlight}\n` +
        `particules ém. ${fx.runtime.ps.emitted}\n` +
        `draw calls     ${info.render.calls}\n` +
        `événements     ${fx.dispatcher.stats.events} (génériques ${fx.dispatcher.stats.defaults}, dédiés ${fx.dispatcher.stats.custom})`;
    }
    requestAnimationFrame(loop);
  };
  requestAnimationFrame(loop);
}
