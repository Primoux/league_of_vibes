/**
 * FX de Kaelen, la Lame de l'Aube (événements : server/src/abilities/champions/kaelen.ts).
 *  Q  Taille ascendante  dash kaelen_q (1re/2e) ou kaelen_q_3 (3e, projette) ; dégâts en fin de dash
 *  W  Parade             buff kaelen_w (fenêtre) ; contre : buff stun (fxKey kaelen_w) sur l'attaquant
 *  E  Fente              buff de bouclier kaelen_e + dash kaelen_e
 *  R  Jugement solaire   abilityCast (0,4 s) puis dégâts kaelen_r sur la cible
 */
import * as THREE from 'three';
import { shieldShell } from '../effects/auras.ts';
import { dashTrail, flash, shockwave, timed } from '../effects/primitives.ts';
import type { FxRuntime } from '../core/FxRuntime.ts';
import { onAbilityHit, registerFx, type Ev } from '../registry.ts';

const SUN = new THREE.Color(2.8, 2.2, 1.0);
const LIGHT = new THREE.Color(2.4, 2.4, 2.6);
const _v = new THREE.Vector3();
const _w = new THREE.Vector3();

/** Croissant d'étincelles devant `at` dans la direction `dir` (taille d'épée). */
function crescent(rt: FxRuntime, at: THREE.Vector3, dir: THREE.Vector3, radius: number, color: THREE.Color, n = 20): void {
  const base = Math.atan2(dir.z, dir.x);
  for (let i = 0; i < n; i++) {
    const a = base - 1.1 + (2.2 * i) / (n - 1);
    _w.set(at.x + Math.cos(a) * radius, 1 + Math.sin(i * 0.7) * 0.1, at.z + Math.sin(a) * radius);
    rt.ps.emit(rt.s.spark, _w, { count: 1, shape: 'point', speed: [0.5, 1], inherit: _v.set(-Math.sin(a) * 4, 0, Math.cos(a) * 4), life: [0.15, 0.25], size: [0.08, 0.12], stretch: 0.05, tint: color });
  }
}

function qDash(rt: FxRuntime, e: Ev<'dash'>, third: boolean): void {
  const from = rt.world(e.from);
  const to = rt.world(e.to);
  const dir = to.clone().sub(from).setY(0).normalize();
  const duration = Math.max(0.05, e.duration);
  dashTrail(rt, e.source, from, to, duration, SUN.clone().multiplyScalar(0.6), false);
  rt.after(duration, () => {
    const pos = new THREE.Vector3();
    if (!rt.entities.pos(e.source, pos)) pos.copy(to);
    crescent(rt, pos, dir, 1.1, SUN, third ? 30 : 20);
    if (third) {
      // 3e frappe : tourbillon ascendant qui projette
      shockwave(rt, pos, 2.2, 0.4, SUN, 0.16);
      timed(rt, 0.4, (t) => {
        const a = t * 25;
        _w.set(pos.x + Math.cos(a) * 0.9, 0.2 + t * 2.5, pos.z + Math.sin(a) * 0.9);
        rt.ps.emit(rt.s.glow, _w, { count: 2, shape: 'point', life: 0.3, size: 0.4, tint: SUN });
      });
    }
  });
}

registerFx('kaelen_q', {
  dash(rt, e) {
    qDash(rt, e, false);
  },
});
registerFx('kaelen_q_3', {
  dash(rt, e) {
    qDash(rt, e, true);
  },
});

registerFx('kaelen_w', {
  buff(rt, e) {
    if (e.buffId === 'kaelen_w') {
      shieldShell(rt, e.target, Math.max(0.3, e.duration), `kaelen_w:${e.target}`, { color: SUN.clone().multiplyScalar(0.6), radius: 1.05, bands: 0.6, power: 3 });
      return;
    }
    // contre réussi : l'attaquant est étourdi
    if (e.buffId === 'stun' && rt.entities.pos(e.target, _v, 1)) {
      flash(rt, _v, 2.6, SUN, 0.3);
      rt.ps.emit(rt.s.spark, _v, { count: 20, shape: 'point', speed: [3, 7], life: [0.2, 0.35], size: [0.07, 0.12], stretch: 0.05, tint: SUN });
      if (e.source !== undefined) rt.endAura(`kaelen_w:${e.source}`);
    }
  },
  buffEnd(rt, e) {
    if (e.buffId === 'kaelen_w') rt.endAura(`kaelen_w:${e.target}`);
  },
});

registerFx('kaelen_e', {
  dash(rt, e) {
    dashTrail(rt, e.source, rt.world(e.from), rt.world(e.to), Math.max(0.05, e.duration), LIGHT.clone().multiplyScalar(0.6), false);
  },
  buff(rt, e) {
    if (e.buffId !== 'kaelen_e') return;
    shieldShell(rt, e.target, Math.max(0.3, e.duration), `kaelen_e:${e.target}`, { color: new THREE.Color(1.3, 1.2, 0.8), radius: 1.05, bands: 0.2 });
  },
  buffEnd(rt, e) {
    if (e.buffId === 'kaelen_e') rt.endAura(`kaelen_e:${e.target}`);
  },
});

registerFx('kaelen_r', {
  /** Une lame de lumière se forme au-dessus de la cible pendant l'incantation. */
  cast(rt, e) {
    if (e.targetId === undefined) return;
    const target = e.targetId;
    const pos = new THREE.Vector3();
    timed(rt, 0.4, (t) => {
      if (!rt.entities.pos(target, pos)) return;
      const h = 5 - t * 3.5;
      rt.ps.emit(rt.s.spark, _v.set(pos.x, h, pos.z), { count: 3, shape: 'sphere', radius: 0.15, life: 0.15, size: [0.12, 0.2], stretch: 0.1, inherit: _w.set(0, -14, 0), tint: SUN });
      rt.ps.emit(rt.s.glow, _v, { count: 1, shape: 'point', life: 0.12, size: 0.9 + t, tint: SUN });
    });
  },
});
onAbilityHit('kaelen_r', (rt, e) => {
  const pos = new THREE.Vector3();
  if (!rt.entities.pos(e.target, pos)) return;
  flash(rt, _v.set(pos.x, 1, pos.z), 4, SUN, 0.35);
  shockwave(rt, pos, 2.4, 0.5, SUN, 0.2);
  rt.ps.emit(rt.s.spark, pos, { count: 40, shape: 'disc', radius: 0.4, up: [4, 10], speed: [1, 3], life: [0.3, 0.6], size: [0.08, 0.14], stretch: 0.07, tint: SUN });
  rt.ps.emit(rt.s.glow, pos, { count: 8, shape: 'disc', radius: 0.3, up: [6, 10], life: [0.3, 0.5], size: [0.6, 1], tint: SUN });
});
