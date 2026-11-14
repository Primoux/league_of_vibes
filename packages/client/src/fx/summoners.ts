/**
 * FX des 9 sorts d'invocateur, alignés sur les événements du serveur (systems/summoners.ts) :
 *  flash     dash (blink) + summonerCast         ignite   buff `ignite` (fxKey sum_ignite)
 *  heal      heal + buff sum_heal_speed           barrier  buff `sum_barrier` (bouclier)
 *  exhaust   buff `exhaust` (fxKey sum_exhaust)   teleport zone sum_teleport_channel → _arrive / _cancel
 *  smite     summonerCast (targetId)             ghost    buff sum_ghost        cleanse  summonerCast + buff
 */
import * as THREE from 'three';
import { shieldShell } from './effects/auras.ts';
import { flash, lightning, shockwave, timed } from './effects/primitives.ts';
import type { FxRuntime } from './core/FxRuntime.ts';
import { rng } from './core/rng.ts';
import { registerFx } from './registry.ts';
import { burn } from './world/events.ts';

const FLASH = new THREE.Color(2.8, 2.3, 0.9);
const IGNITE = new THREE.Color(2.6, 0.9, 0.25);
const HEAL = new THREE.Color(0.5, 2.4, 0.9);
const BARRIER = new THREE.Color(1.5, 1.2, 0.5);
const EXHAUST = new THREE.Color(1.3, 0.6, 2.2);
const TP = new THREE.Color(1.8, 0.7, 2.6);
const SMITE = new THREE.Color(2.8, 2.2, 0.8);
const GHOST = new THREE.Color(0.6, 1.8, 2.2);
const CLEANSE = new THREE.Color(1.4, 2.4, 2.8);
const _p = new THREE.Vector3();
const _q = new THREE.Vector3();

registerFx('sum_flash', {
  dash(rt, e) {
    const from = rt.world(e.from, 1);
    const to = rt.world(e.to, 1);
    flash(rt, from, 2.4, FLASH, 0.3);
    flash(rt, to, 2.8, FLASH, 0.35);
    lightning(rt, from, to, FLASH, { width: 0.3, life: 0.3, jag: 0.15, segments: 8 });
    rt.ps.emit(rt.s.spark, from, { count: 16, shape: 'line', to, radius: 0.3, speed: [0.5, 2], life: [0.2, 0.4], size: [0.06, 0.1], stretch: 0.05, tint: FLASH });
    rt.ps.emit(rt.s.sparkles, to, { count: 10, shape: 'sphere', radius: 0.5, speed: [0.5, 1.5], life: [0.3, 0.6], size: [0.3, 0.45], tint: FLASH });
    shockwave(rt, to.setY(0), 1.4, 0.3, FLASH, 0.1);
  },
  summoner() {
    /* le blink porte déjà tout le FX */
  },
});

registerFx('sum_ignite', {
  summoner(rt, e) {
    if (e.targetId === undefined || !rt.entities.pos(e.source, _p, 1.2) || !rt.entities.pos(e.targetId, _q, 1.2)) return;
    lightning(rt, _p, _q, IGNITE, { width: 0.14, life: 0.25, jag: 0.12 });
    flash(rt, _q, 1.8, IGNITE, 0.3);
  },
  buff(rt, e) {
    burn(rt, e.target, e.duration, IGNITE);
  },
});

registerFx('sum_heal', {
  summoner(rt, e) {
    if (!rt.entities.pos(e.source, _p)) return;
    shockwave(rt, _p, 2.2, 0.5, HEAL, 0.14);
  },
  buff(rt, e) {
    const pos = new THREE.Vector3();
    if (!rt.entities.pos(e.target, pos)) return;
    flash(rt, _p.set(pos.x, 1, pos.z), 2.2, HEAL, 0.3);
    rt.ps.emit(rt.s.cross, pos, { count: 10, shape: 'disc', radius: 0.6, up: [1.5, 3], life: [0.6, 1], size: [0.22, 0.32], tint: HEAL });
    timed(rt, Math.max(0.3, e.duration), () => {
      if (rng.next() < 0.5 && rt.entities.pos(e.target, pos)) rt.ps.emit(rt.s.glow, pos, { count: 1, shape: 'ring', radius: 0.4, up: [0.6, 1.2], life: [0.3, 0.5], size: [0.2, 0.3], tint: HEAL });
    });
  },
});

registerFx('sum_barrier', {
  buff(rt, e) {
    if (e.buffId !== 'sum_barrier') return;
    shieldShell(rt, e.target, Math.max(0.3, e.duration), `sum_barrier:${e.target}`, { color: BARRIER, radius: 1.15, bands: 0.35, power: 2 });
    if (rt.entities.pos(e.target, _p, 1)) flash(rt, _p, 2.4, BARRIER, 0.3);
  },
  buffEnd(rt, e) {
    if (e.buffId === 'sum_barrier') rt.endAura(`sum_barrier:${e.target}`);
  },
});

registerFx('sum_exhaust', {
  summoner(rt, e) {
    if (e.targetId === undefined || !rt.entities.pos(e.source, _p, 1.2) || !rt.entities.pos(e.targetId, _q, 1.2)) return;
    lightning(rt, _p, _q, EXHAUST, { width: 0.12, life: 0.3, jag: 0.2 });
  },
  buff(rt, e) {
    const pos = new THREE.Vector3();
    let acc = 0;
    timed(rt, Math.max(0.3, e.duration), (_t, dt) => {
      acc += dt;
      if (acc < 0.08 || !rt.entities.pos(e.target, pos)) return;
      acc = 0;
      rt.ps.emit(rt.s.darkWisp, pos.setY(1.2), { count: 1, shape: 'sphere', radius: 0.4, up: [-0.4, 0.2], life: [0.5, 0.8], size: [0.4, 0.6] });
      rt.ps.emit(rt.s.ember, pos.setY(1.6), { count: 1, shape: 'sphere', radius: 0.4, gravity: 2, life: [0.4, 0.6], size: [0.08, 0.12], tint: EXHAUST });
    });
  },
});

// ---------------------------------------------------------------- téléportation
function tpColumn(rt: FxRuntime, center: THREE.Vector3, duration: number, key: string): void {
  let ended = false;
  const cancel = (): void => {
    ended = true;
  };
  rt.setAura(key, cancel);
  const d = rt.acquireDecal().setColor(TP);
  d.circle(center, 1.1);
  d.u.uFill.value = 0.15;
  d.u.uPattern.value = 1;
  d.u.uProgress.value = 0;
  let t = 0;
  let acc = 0;
  rt.add({
    update(dt) {
      t += dt;
      acc += dt;
      d.u.uProgress.value = Math.min(1, t / duration);
      if (ended) {
        d.u.uOpacity.value -= dt * 5;
        return d.u.uOpacity.value > 0;
      }
      if (acc > 0.05) {
        acc = 0;
        const k = Math.min(1, t / duration);
        rt.ps.emit(rt.s.spark, center, { count: 1 + Math.round(k * 3), shape: 'ring', radius: 0.9, up: [3, 6 + k * 6], life: [0.3, 0.5], size: [0.05, 0.09], stretch: 0.06, tint: TP });
        rt.ps.emit(rt.s.rune, _p.set(center.x, 0.1 + k * 2, center.z), { count: 1, shape: 'point', life: 0.2, size: 1.3 - k * 0.5, spin: 2, tint: TP });
      }
      return t < duration + 1;
    },
    end() {
      rt.forgetAura(key, cancel);
      rt.releaseDecal(d);
    },
  });
}

registerFx('sum_teleport', {
  /** Canal au départ, sous le lanceur. */
  summoner(rt, e) {
    if (rt.entities.pos(e.source, _p)) tpColumn(rt, _p.clone(), 4, `tp_from:${e.source}`);
  },
  /** Arrivée : le lanceur apparaît (blink serveur). */
  dash(rt, e) {
    const to = rt.world(e.to, 1);
    flash(rt, to, 3.5, TP, 0.4);
    rt.ps.emit(rt.s.spark, to.setY(0), { count: 30, shape: 'disc', radius: 0.6, up: [6, 12], life: [0.3, 0.5], size: [0.08, 0.12], stretch: 0.07, tint: TP });
  },
});
registerFx('sum_teleport_channel', {
  /** Destination : la colonne se charge pendant la canalisation. */
  area(rt, e) {
    tpColumn(rt, rt.world(e.pos), Math.max(0.5, e.duration), `tp_to:${e.source}`);
  },
});
registerFx('sum_teleport_arrive', {
  area(rt, e) {
    rt.endAura(`tp_from:${e.source}`);
    rt.endAura(`tp_to:${e.source}`);
    shockwave(rt, rt.world(e.pos), 2, 0.4, TP, 0.14);
  },
});
registerFx('sum_teleport_cancel', {
  area(rt, e) {
    rt.endAura(`tp_from:${e.source}`);
    rt.endAura(`tp_to:${e.source}`);
    rt.ps.emit(rt.s.sparkles, rt.world(e.pos, 0.6), { count: 8, shape: 'sphere', radius: 0.6, life: [0.3, 0.5], size: [0.3, 0.4], tint: TP, intensity: 0.6 });
  },
});

registerFx('sum_smite', {
  summoner(rt, e) {
    if (e.targetId === undefined || !rt.entities.pos(e.targetId, _q)) return;
    // éclair divin qui tombe du ciel sur la cible
    lightning(rt, _p.set(_q.x + rng.range(-0.4, 0.4), 9, _q.z + rng.range(-0.4, 0.4)), _q.clone().setY(0.6), SMITE, { width: 0.45, life: 0.35, jag: 0.4, segments: 12 });
    flash(rt, _q.setY(1), 3.2, SMITE, 0.35);
    shockwave(rt, _q.setY(0), 2, 0.4, SMITE, 0.14);
    rt.ps.emit(rt.s.spark, _q, { count: 24, shape: 'point', speed: [3, 7], up: [1, 3], life: [0.3, 0.5], size: [0.07, 0.12], stretch: 0.05, gravity: 8, tint: SMITE });
  },
});

registerFx('sum_ghost', {
  buff(rt, e) {
    const pos = new THREE.Vector3();
    let acc = 0;
    timed(rt, Math.max(0.3, e.duration), (_t, dt) => {
      acc += dt;
      if (acc < 0.06 || !rt.entities.pos(e.target, pos)) return;
      acc = 0;
      rt.ps.emit(rt.s.glow, pos.setY(0.9), { count: 1, shape: 'sphere', radius: 0.35, life: [0.4, 0.6], size: [0.6, 0.9], tint: GHOST, intensity: 0.35 });
      rt.ps.emit(rt.s.swirl, pos.setY(0.05), { count: 1, shape: 'point', life: 0.35, size: 0.9, spin: 4, tint: GHOST, intensity: 0.5 });
    });
  },
});

registerFx('sum_cleanse', {
  summoner(rt, e) {
    const pos = new THREE.Vector3();
    if (!rt.entities.pos(e.source, pos)) return;
    flash(rt, _p.set(pos.x, 1, pos.z), 2.6, CLEANSE, 0.3);
    shockwave(rt, pos, 1.8, 0.4, CLEANSE, 0.1);
    timed(rt, 0.5, (t) => {
      if (!rt.entities.pos(e.source, pos)) return;
      const a = t * 18;
      _p.set(pos.x + Math.cos(a) * 0.6, 0.2 + t * 2.2, pos.z + Math.sin(a) * 0.6);
      rt.ps.emit(rt.s.sparkles, _p, { count: 1, shape: 'point', life: 0.4, size: 0.35, tint: CLEANSE });
    });
  },
});
