/**
 * FX génériques, aux couleurs de la palette du sort. Ils s'appliquent à toute fxKey sans handler dédié,
 * pour que chaque sort ait dès le départ une anticipation, un projectile, un impact et une zone lisibles.
 */
import * as THREE from 'three';
import type { AllHandlers } from './dispatcher.ts';
import { ProjectileFx, dashTrail, flash, shockwave, telegraph, zone } from './effects/primitives.ts';
import type { FxColors, FxRuntime } from './core/FxRuntime.ts';
import { isTelegraphKey } from './keys.ts';

const _p = new THREE.Vector3();
const _q = new THREE.Vector3();

/** Petite explosion colorée : éclat + étincelles + onde. Réutilisée par plusieurs FX. */
export function impactBurst(rt: FxRuntime, at: THREE.Vector3, c: FxColors, scale = 1): void {
  _q.set(at.x, 1, at.z);
  flash(rt, _q, 1.6 * scale, c.accentHdr, 0.22);
  rt.ps.emit(rt.s.spark, _q, { count: 14 * scale, shape: 'point', speed: [3, 7], life: [0.2, 0.4], size: [0.06, 0.12], stretch: 0.05, gravity: 6, tint: c.vividHdr });
  rt.ps.emit(rt.s.glowPop, _q, { count: 4, shape: 'sphere', radius: 0.2, speed: [0.3, 1], life: [0.25, 0.4], size: [0.6 * scale, 1.1 * scale], tint: c.vivid, intensity: 1.6 });
  _q.y = 0;
  shockwave(rt, _q, 1.1 * scale, 0.35, c.vividHdr, 0.1);
}

/** Explosion de zone (fin d'une télégraphie). */
export function areaBurst(rt: FxRuntime, center: THREE.Vector3, radius: number, c: FxColors): void {
  shockwave(rt, center, radius * 1.05, 0.45, c.vividHdr, 0.18);
  _q.set(center.x, 0.3, center.z);
  rt.ps.emit(rt.s.glowPop, _q, { count: 10, shape: 'disc', radius: radius * 0.7, speed: [0.5, 1.5], up: [1, 2.5], life: [0.3, 0.6], size: [0.6, 1.2], tint: c.vivid, intensity: 1.8 });
  rt.ps.emit(rt.s.spark, _q, { count: 24, shape: 'disc', radius: radius * 0.6, speed: [2, 5], up: [2, 6], life: [0.3, 0.6], size: [0.06, 0.12], stretch: 0.05, gravity: 9, tint: c.accentHdr });
  rt.ps.emit(rt.s.dust, center, { count: 8, shape: 'ring', radius: radius * 0.8, speed: [0.5, 1.2], up: [0.2, 0.6], life: [0.6, 1], size: [0.8, 1.4] });
}

export const defaultFx: AllHandlers = {
  cast(rt, e, c) {
    if (!rt.entities.pos(e.source, _p)) return;
    shockwave(rt, _p, 0.9, 0.3, c.vividHdr, 0.08);
    _p.y = 0.2;
    rt.ps.emit(rt.s.sparkles, _p, { count: 6, shape: 'disc', radius: 0.5, up: [1.5, 3], life: [0.35, 0.6], size: [0.25, 0.45], tint: c.accent, intensity: 1.5 });
  },

  projectile(rt, e, c) {
    const from = rt.world(e.from, 0, new THREE.Vector3());
    const to = rt.world(e.to, 0, new THREE.Vector3());
    const size = Math.max(0.45, rt.scale(e.width) * 1.4);
    const p = new ProjectileFx(rt, from, to, rt.scale(e.speed), {
      color: c.vividHdr,
      core: c.accentHdr,
      size,
      height: 1,
      trailStyle: rt.s.glow,
      trailDensity: 10,
      trailSize: [size * 0.3, size * 0.55],
      ribbonWidth: size * 0.5,
    });
    rt.add(p);
    return p;
  },

  hit(rt, e, c) {
    impactBurst(rt, rt.world(e.pos, 0, _p), c, 0.8);
  },

  area(rt, e, c) {
    const center = rt.world(e.pos, 0, new THREE.Vector3());
    const radius = rt.scale(e.radius);
    // zone retardée : le serveur émet d'abord `<clé>_telegraph` (durée = délai), puis `<clé>` à la détonation
    if (isTelegraphKey(e.fxKey)) {
      telegraph(rt, center, radius, Math.max(0.1, e.duration), c.vivid, { color2: c.vividHdr });
      return;
    }
    if (e.duration <= 0.35) return areaBurst(rt, center, radius, c);
    // zone persistante
    zone(rt, center, radius, e.duration, c.vividHdr, {
      fill: 0.12,
      pattern: 0.5,
      emit: { style: rt.s.sparkles, rate: 18 * radius, size: [0.25, 0.45], life: [0.5, 0.9], up: [0.4, 1.2], tint: c.accent, intensity: 1.4 },
    });
  },

  dash(rt, e, c) {
    const from = rt.world(e.from, 0, new THREE.Vector3());
    const to = rt.world(e.to, 0, new THREE.Vector3());
    if (e.duration < 0.08) {
      // blink : disparition / apparition
      for (const p of [from, to]) {
        _q.set(p.x, 1, p.z);
        flash(rt, _q, 2, c.vividHdr, 0.3);
        rt.ps.emit(rt.s.sparkles, _q, { count: 10, shape: 'sphere', radius: 0.5, speed: [0.5, 1.5], life: [0.3, 0.6], size: [0.25, 0.4], tint: c.accent, intensity: 1.6 });
      }
      return;
    }
    rt.ps.emit(rt.s.dust, from, { count: 8, shape: 'disc', radius: 0.4, speed: [0.5, 1.5], up: [0.3, 0.8], life: [0.5, 0.8], size: [0.6, 1.1] });
    dashTrail(rt, e.source, from, to, e.duration, c.vividHdr);
    rt.after(e.duration, () => shockwave(rt, to, 1.2, 0.3, c.vividHdr, 0.1));
  },

  buff() {
    /* Buffs génériques : rien par défaut (trop de bruit). Les CC/buffs notables ont des FX dédiés (M3). */
  },

  buffEnd() {},

  summoner(rt, e, c) {
    const from = rt.world(e.from, 0, new THREE.Vector3());
    _q.set(from.x, 1, from.z);
    flash(rt, _q, 2.2, c.vividHdr, 0.3);
    shockwave(rt, from, 1.3, 0.35, c.vividHdr);
    if (e.to) {
      const to = rt.world(e.to, 0, _p);
      _q.set(to.x, 1, to.z);
      flash(rt, _q, 2.2, c.accentHdr, 0.3);
    }
  },
};
