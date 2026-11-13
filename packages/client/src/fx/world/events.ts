/**
 * FX des événements du monde : chiffres flottants (dégâts, soins, or), level-up, mort, rappel,
 * destruction de structures, objectifs, buffs de jungle et d'objectifs.
 */
import * as THREE from 'three';
import { RECALL_SECONDS, type DragonElement, type GameEvent } from '@lov/shared';
import { store } from '../../state.ts';
import type { FxRuntime } from '../core/FxRuntime.ts';
import { rng } from '../core/rng.ts';
import { flash, shockwave, timed, zone } from '../effects/primitives.ts';
import { onWorldEvent } from '../registry.ts';
import { FloatingNumbers } from './damageNumbers.ts';

const C = {
  physical: new THREE.Color(1.0, 0.5, 0.12),
  magic: new THREE.Color(0.35, 0.6, 1.0),
  true: new THREE.Color(1, 1, 1),
  taken: new THREE.Color(1.0, 0.18, 0.15),
  heal: new THREE.Color(0.3, 1.0, 0.4),
  gold: new THREE.Color(1.0, 0.82, 0.25),
  goldHdr: new THREE.Color(2.6, 2.0, 0.6),
  level: new THREE.Color(2.4, 2.0, 0.9),
  recall: new THREE.Color(0.6, 1.4, 2.6),
  soul: new THREE.Color(0.9, 1.1, 1.6),
  blue: new THREE.Color(0.45, 0.75, 2.2),
  red: new THREE.Color(2.2, 0.55, 0.35),
};
const TEAM_HDR = { blue: new THREE.Color(0.6, 1.0, 2.6), red: new THREE.Color(2.6, 0.7, 0.5) };
const ELEMENT: Record<DragonElement | 'elder', THREE.Color> = {
  infernal: new THREE.Color(2.6, 0.8, 0.2),
  ocean: new THREE.Color(0.3, 1.4, 2.4),
  mountain: new THREE.Color(1.6, 1.1, 0.6),
  cloud: new THREE.Color(1.6, 2.2, 2.4),
  hextech: new THREE.Color(0.6, 1.6, 2.6),
  chemtech: new THREE.Color(1.0, 2.4, 0.4),
  elder: new THREE.Color(1.8, 0.9, 2.6),
};

const _p = new THREE.Vector3();
const _q = new THREE.Vector3();

/** Le joueur local voit les chiffres qui le concernent (en fxlab, sans partie : tous). */
function concernsMe(...ids: (number | undefined)[]): boolean {
  const me = store.state.game?.entityId;
  return me === undefined || ids.includes(me);
}

export function installWorldFx(): void {
  const numbersByRt = new WeakMap<FxRuntime, FloatingNumbers>();
  const numbers = (rt: FxRuntime): FloatingNumbers => {
    let n = numbersByRt.get(rt);
    if (!n) {
      const fresh = new FloatingNumbers();
      rt.root.add(fresh.mesh);
      rt.add({ update: (dt) => (fresh.update(dt), true) });
      numbersByRt.set(rt, fresh);
      n = fresh;
    }
    return n;
  };

  // ---------------------------------------------------------------- chiffres
  onWorldEvent('damage', (rt, e) => {
    if (e.amount < 1 || !concernsMe(e.source, e.target)) return;
    if (!rt.entities.pos(e.target, _p, 2.1)) return;
    const me = store.state.game?.entityId;
    const color = me !== undefined && e.target === me ? C.taken : C[e.damageType];
    const big = e.crit ? 1.45 : 1;
    numbers(rt).spawn(_p, `${Math.round(e.amount)}${e.crit ? '!' : ''}`, color, 0.42 * big, e.crit ? 1.1 : 0.9);
  });
  onWorldEvent('heal', (rt, e) => {
    if (e.amount < 1 || !rt.entities.pos(e.target, _p)) return;
    rt.ps.emit(rt.s.cross, _p, { count: 5, shape: 'disc', radius: 0.45, up: [1, 2], life: [0.6, 0.9], size: [0.18, 0.28], tint: C.heal, intensity: 1.6 });
    if (concernsMe(e.source, e.target)) numbers(rt).spawn(_p.setY(2.1), `+${Math.round(e.amount)}`, C.heal, 0.36);
  });
  onWorldEvent('goldGain', (rt, e) => {
    if (!concernsMe(e.target)) return;
    if (e.pos) rt.world(e.pos, 1, _p);
    else if (!rt.entities.pos(e.target, _p, 1.6)) return;
    rt.ps.emit(rt.s.coin, _p, { count: Math.min(8, 2 + Math.round(e.amount / 20)), shape: 'sphere', radius: 0.2, speed: [1, 2.5], up: [2, 3.5], gravity: 9, life: [0.6, 0.9], size: [0.16, 0.22], spin: [-8, 8], tint: C.goldHdr });
    numbers(rt).spawn(_p.setY(_p.y + 0.6), `+${Math.round(e.amount)}`, C.gold, 0.34, 1.1);
  });
  onWorldEvent('shield', (rt, e) => {
    if (!rt.entities.pos(e.target, _p, 1)) return;
    rt.ps.emit(rt.s.sparkles, _p, { count: 4, shape: 'sphere', radius: 0.7, life: [0.4, 0.6], size: [0.3, 0.45], tint: C.soul, intensity: 1.2 });
  });

  // ---------------------------------------------------------------- progression
  onWorldEvent('levelUp', (rt, e) => {
    const pos = new THREE.Vector3();
    if (!rt.entities.pos(e.target, pos)) return;
    shockwave(rt, pos, 1.5, 0.5, C.level, 0.12);
    flash(rt, _p.set(pos.x, 1.2, pos.z), 2.6, C.level, 0.4);
    // colonne de lumière qui suit le champion
    timed(rt, 0.9, (t) => {
      if (!rt.entities.pos(e.target, _q)) return;
      rt.ps.emit(rt.s.spark, _q, { count: 3, shape: 'ring', radius: 0.55 * (1 - t * 0.5), up: [5, 8], life: [0.3, 0.5], size: [0.06, 0.1], stretch: 0.06, tint: C.level });
      rt.ps.emit(rt.s.glow, _q.setY(0.2), { count: 1, shape: 'disc', radius: 0.3, up: [3, 5], life: [0.3, 0.5], size: [0.5, 0.8], tint: C.level });
    });
  });

  // ---------------------------------------------------------------- morts
  onWorldEvent('death', (rt, e) => {
    const info = rt.entities.info(e.target);
    if (!rt.entities.pos(e.target, _p, 0.8)) return;
    if (info?.kind === 'champion') {
      // âme qui s'élève pendant la chute
      rt.after(0.7, () => {
        if (!rt.entities.pos(e.target, _q, 0.6)) return;
        rt.ps.emit(rt.s.glow, _q, { count: 10, shape: 'sphere', radius: 0.4, up: [1, 2.2], speed: [0.1, 0.4], life: [0.8, 1.3], size: [0.3, 0.6], tint: C.soul, intensity: 1.4 });
        rt.ps.emit(rt.s.sparkles, _q, { count: 6, shape: 'sphere', radius: 0.5, up: [1.5, 2.5], life: [0.8, 1.2], size: [0.3, 0.5], tint: C.soul });
      });
    } else if (info?.kind === 'minion' || info?.kind === 'monster') {
      rt.ps.emit(rt.s.dust, _p.setY(0.2), { count: 5, shape: 'disc', radius: 0.3, up: [0.3, 0.8], speed: [0.3, 0.8], life: [0.5, 0.8], size: [0.5, 0.8] });
    }
  });

  // ---------------------------------------------------------------- rappel
  onWorldEvent('recallStart', (rt, e) => {
    const key = `recall:${e.target}`;
    const pos = new THREE.Vector3();
    let t = 0;
    let acc = 0;
    let ended: false | 'done' | 'cancel' = false;
    const cancel = (): void => {
      if (!ended) ended = 'cancel';
    };
    rt.setAura(key, cancel);
    const d = rt.acquireDecal().setColor(C.recall);
    d.u.uFill.value = 0.1;
    d.u.uPattern.value = 0.8;
    rt.add({
      update(dt) {
        t += dt;
        acc += dt;
        if (!rt.entities.pos(e.target, pos)) return false;
        d.circle(pos, 1.0 + Math.sin(t * 3) * 0.05);
        d.u.uOpacity.value = ended ? Math.max(0, d.u.uOpacity.value - dt * 5) : Math.min(1, t * 3);
        if (ended) return d.u.uOpacity.value > 0;
        if (acc > 0.05) {
          acc = 0;
          const k = Math.min(1, t / RECALL_SECONDS);
          // anneau de runes qui monte et se resserre à mesure que le rappel avance
          for (let i = 0; i < 2; i++) {
            const a = t * 2 + i * Math.PI;
            _p.set(pos.x + Math.cos(a) * (0.9 - k * 0.4), 0.1 + k * 1.6, pos.z + Math.sin(a) * (0.9 - k * 0.4));
            rt.ps.emit(rt.s.rune, _p, { count: 1, shape: 'point', life: 0.4, size: 0.28, tint: C.recall });
          }
          rt.ps.emit(rt.s.spark, pos, { count: 1, shape: 'ring', radius: 0.8, up: [2, 4], life: [0.4, 0.7], size: [0.05, 0.08], stretch: 0.05, tint: C.recall });
        }
        return true;
      },
      end() {
        rt.forgetAura(key, cancel);
        rt.releaseDecal(d);
      },
    });
  });
  onWorldEvent('recallEnd', (rt, e) => {
    const pos = new THREE.Vector3();
    const had = rt.entities.pos(e.target, pos);
    rt.endAura(`recall:${e.target}`);
    if (!had) return;
    if (e.completed) {
      flash(rt, _p.set(pos.x, 1.2, pos.z), 3.2, C.recall, 0.4);
      rt.ps.emit(rt.s.spark, pos, { count: 30, shape: 'disc', radius: 0.5, up: [8, 14], life: [0.3, 0.5], size: [0.08, 0.14], stretch: 0.08, tint: C.recall });
      shockwave(rt, pos, 1.6, 0.35, C.recall, 0.1);
    } else {
      rt.ps.emit(rt.s.sparkles, pos.setY(0.6), { count: 6, shape: 'sphere', radius: 0.6, life: [0.3, 0.5], size: [0.3, 0.4], tint: C.recall, intensity: 0.7 });
    }
  });

  // ---------------------------------------------------------------- structures
  onWorldEvent('structureDestroyed', (rt, e) => {
    const pos = new THREE.Vector3();
    if (!rt.entities.pos(e.target, pos)) return;
    const team = TEAM_HDR[e.team];
    if (e.plate) {
      rt.ps.emit(rt.s.coin, _p.set(pos.x, 2.5, pos.z), { count: 10, shape: 'sphere', radius: 0.5, speed: [1.5, 3], up: [2, 4], gravity: 9, life: [0.8, 1.1], size: [0.2, 0.28], spin: [-8, 8], tint: C.goldHdr });
      debris(rt, pos, 8, 0.8, team);
      return;
    }
    const scale = e.kind === 'nexus' ? 2.2 : e.kind === 'inhibitor' ? 1.3 : 1;
    flash(rt, _p.set(pos.x, 2.5 * scale, pos.z), 6 * scale, team, 0.5);
    shockwave(rt, pos, 3.5 * scale, 0.8, team, 0.3);
    debris(rt, pos, Math.round(26 * scale), 1.6 * scale, team);
    rt.ps.emit(rt.s.smoke, pos, { count: Math.round(24 * scale), shape: 'disc', radius: 1.2 * scale, speed: [0.5, 1.5], up: [1, 3], life: [1.5, 2.5], size: [1.5, 3], drag: 0.8 });
    rt.ps.emit(rt.s.spark, _p.set(pos.x, 2, pos.z), { count: Math.round(40 * scale), shape: 'sphere', radius: 0.8, speed: [4, 10], life: [0.5, 1], size: [0.08, 0.14], stretch: 0.06, gravity: 9, tint: team });
    if (e.kind === 'nexus') {
      // le nexus explose en plusieurs temps
      for (let i = 1; i <= 3; i++)
        rt.after(i * 0.35, () => {
          _q.set(pos.x + rng.range(-1.5, 1.5), 2 + rng.range(0, 2), pos.z + rng.range(-1.5, 1.5));
          flash(rt, _q, 5, team, 0.4);
          rt.ps.emit(rt.s.spark, _q, { count: 30, shape: 'sphere', radius: 0.5, speed: [4, 9], life: [0.5, 0.9], size: [0.08, 0.14], stretch: 0.06, gravity: 9, tint: team });
        });
      zone(rt, pos, 5, 3, team, { fill: 0.15, pattern: 0.8 });
    }
  });

  // ---------------------------------------------------------------- objectifs
  onWorldEvent('objectiveTaken', (rt, e) => {
    const color = e.monster === 'elder_dragon' ? ELEMENT.elder : e.element ? ELEMENT[e.element] : TEAM_HDR[e.team];
    const pos = new THREE.Vector3();
    if (e.killer === undefined || !rt.entities.pos(e.killer, pos)) return;
    flash(rt, _p.set(pos.x, 1.5, pos.z), 4, color, 0.5);
    shockwave(rt, pos, 3, 0.7, color, 0.2);
    rt.ps.emit(rt.s.spark, pos, { count: 40, shape: 'ring', radius: 1, up: [6, 12], life: [0.4, 0.7], size: [0.08, 0.14], stretch: 0.08, tint: color });
  });
  onWorldEvent('objectiveSpawn', (rt, e) => {
    const color = e.monster === 'elder_dragon' ? ELEMENT.elder : e.element ? ELEMENT[e.element] : C.soul;
    const pos = rt.world(e.pos);
    shockwave(rt, pos, 4, 1, color, 0.3);
    rt.ps.emit(rt.s.glow, pos, { count: 20, shape: 'disc', radius: 2.5, up: [1, 3], life: [0.8, 1.4], size: [0.6, 1.2], tint: color });
  });

  // ---------------------------------------------------------------- buffs de jungle / d'objectifs
  const AURAS: Record<string, { color: THREE.Color; style: 'rune' | 'flame' | 'wisp' }> = {
    blue_buff: { color: C.blue, style: 'rune' },
    blue: { color: C.blue, style: 'rune' },
    red_buff: { color: C.red, style: 'flame' },
    red: { color: C.red, style: 'flame' },
    leviathan: { color: new THREE.Color(1.6, 0.6, 2.6), style: 'wisp' },
    leviathan_buff: { color: new THREE.Color(1.6, 0.6, 2.6), style: 'wisp' },
    elder: { color: ELEMENT.elder, style: 'flame' },
    elder_buff: { color: ELEMENT.elder, style: 'flame' },
  };
  onWorldEvent('buffApplied', (rt, e) => {
    const aura = AURAS[e.buffId];
    if (aura) return objectiveAura(rt, e, aura.color, aura.style);
    if (e.buffId === 'red_burn' || e.buffId === 'elder_burn') burn(rt, e.target, e.duration, e.buffId === 'elder_burn' ? ELEMENT.elder : C.red);
    if (e.buffId === 'elder_execute' && rt.entities.pos(e.target, _p, 1)) {
      flash(rt, _p, 4, ELEMENT.elder, 0.5);
      rt.ps.emit(rt.s.flame, _p.setY(0.2), { count: 30, shape: 'disc', radius: 0.6, up: [3, 6], life: [0.5, 0.9], size: [0.6, 1], tint: ELEMENT.elder });
    }
  });
  onWorldEvent('buffRemoved', (rt, e) => {
    if (AURAS[e.buffId]) rt.endAura(`obj:${e.buffId}:${e.target}`);
  });
}

/** Réapparition : colonne de lumière à la couleur d'équipe. */
export function respawnFx(rt: FxRuntime, id: number): void {
  const pos = new THREE.Vector3();
  if (!rt.entities.pos(id, pos)) return;
  const team = rt.entities.info(id)?.team;
  const color = team === 'red' ? TEAM_HDR.red : TEAM_HDR.blue;
  flash(rt, _p.set(pos.x, 1.2, pos.z), 3, color, 0.45);
  shockwave(rt, pos, 1.8, 0.5, color, 0.12);
  rt.ps.emit(rt.s.spark, pos, { count: 26, shape: 'disc', radius: 0.5, up: [6, 11], life: [0.3, 0.5], size: [0.07, 0.12], stretch: 0.07, tint: color });
  rt.ps.emit(rt.s.sparkles, pos, { count: 10, shape: 'disc', radius: 0.7, up: [0.5, 1.5], life: [0.6, 1], size: [0.3, 0.45], tint: color });
}

function debris(rt: FxRuntime, at: THREE.Vector3, n: number, power: number, tint: THREE.Color): void {
  const color = new THREE.Color(0.55, 0.52, 0.5).lerp(tint.clone().multiplyScalar(0.3), 0.2);
  for (let i = 0; i < n; i++) {
    const a = rng.next() * Math.PI * 2;
    rt.bursts.rockDebris.spawn({
      pos: _q.set(at.x + Math.cos(a) * 0.5, 1 + rng.next() * 2, at.z + Math.sin(a) * 0.5),
      mode: 'debris',
      size: rng.range(0.15, 0.35) * Math.sqrt(power),
      life: rng.range(1.2, 1.8),
      vel: new THREE.Vector3(Math.cos(a) * rng.range(1, 4) * power, rng.range(3, 7) * power, Math.sin(a) * rng.range(1, 4) * power),
      color,
    });
  }
}

/** Aura d'un buff de jungle/objectif : runes (bleu), flammes (rouge, ancien), volutes (Léviathan). */
function objectiveAura(rt: FxRuntime, e: Extract<GameEvent, { type: 'buffApplied' }>, color: THREE.Color, style: 'rune' | 'flame' | 'wisp'): void {
  const key = `obj:${e.buffId}:${e.target}`;
  let ended = false;
  const cancel = (): void => {
    ended = true;
  };
  rt.setAura(key, cancel);
  const pos = new THREE.Vector3();
  const until = rt.time + Math.max(1, Math.min(e.duration, 600));
  let acc = 0;
  let t = 0;
  rt.add({
    update(dt) {
      t += dt;
      acc += dt;
      if (ended || rt.time > until || !rt.entities.pos(e.target, pos)) return false;
      if (acc < 0.08) return true;
      acc = 0;
      if (style === 'rune') {
        const a = t * 2.2;
        _p.set(pos.x + Math.cos(a) * 0.6, 0.15, pos.z + Math.sin(a) * 0.6);
        rt.ps.emit(rt.s.rune, _p, { count: 1, shape: 'point', up: 0.6, life: 0.6, size: 0.22, tint: color });
      } else if (style === 'flame') {
        rt.ps.emit(rt.s.flame, pos, { count: 1, shape: 'ring', radius: 0.5, up: [0.8, 1.4], life: [0.35, 0.55], size: [0.25, 0.4], tint: color, intensity: 0.6 });
      } else {
        rt.ps.emit(rt.s.glow, _p.set(pos.x, 1.4, pos.z), { count: 1, shape: 'sphere', radius: 0.6, up: [0.3, 0.8], life: [0.6, 0.9], size: [0.3, 0.5], tint: color });
      }
      return true;
    },
    end() {
      rt.forgetAura(key, cancel);
    },
  });
}

/** Brûlure (buff rouge, Drake Ancien, Embrasement) : flammes sur la cible. */
export function burn(rt: FxRuntime, target: number, duration: number, color: THREE.Color): void {
  const pos = new THREE.Vector3();
  let acc = 0;
  timed(rt, Math.max(0.3, duration), (_t, dt) => {
    acc += dt;
    if (acc < 0.06 || !rt.entities.pos(target, pos)) return;
    acc = 0;
    rt.ps.emit(rt.s.flame, pos, { count: 2, shape: 'disc', radius: 0.35, up: [1, 2], life: [0.35, 0.6], size: [0.35, 0.6], tint: color, intensity: 1.1 });
    if (rng.next() < 0.3) rt.ps.emit(rt.s.smoke, pos.setY(1.5), { count: 1, shape: 'point', up: [0.5, 1], life: [0.6, 0.9], size: [0.3, 0.5] });
  });
}
