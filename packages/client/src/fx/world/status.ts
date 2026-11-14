/**
 * Visuels génériques des statuts (CC, anti-soin, invisibilité…), choisis par `buffId` et colorés par la fxKey
 * du sort qui les a posés (une immobilisation de Solenne est dorée, celle de Zephyr violette).
 * Ils s'ajoutent aux FX propres au sort. Une réapplication prolonge l'effet au lieu de le recréer.
 */
import * as THREE from 'three';
import type { FxColors, FxRuntime } from '../core/FxRuntime.ts';
import { rng } from '../core/rng.ts';
import { flash, shockwave } from '../effects/primitives.ts';
import { onWorldEvent, type Ev } from '../registry.ts';

interface StatusFx {
  /** Hauteur de référence (au-dessus de la tête = 2.3). */
  start?(rt: FxRuntime, pos: THREE.Vector3, c: FxColors, e: Ev<'buffApplied'>): void;
  /** Appelé ~20 fois par seconde tant que le statut est actif. */
  tick?(rt: FxRuntime, pos: THREE.Vector3, t: number, c: FxColors): void;
  end?(rt: FxRuntime, pos: THREE.Vector3, c: FxColors): void;
}

const STAR = new THREE.Color(2.4, 2.1, 0.8);
const TAUNT = new THREE.Color(2.6, 0.7, 0.3);
const SILENCE = new THREE.Color(1.6, 0.8, 2.4);
const GRIEVOUS = new THREE.Color(1.8, 0.15, 0.2);
const DARK = new THREE.Color(0.4, 0.3, 0.5);
const _p = new THREE.Vector3();

const STATUS: Record<string, StatusFx> = {
  stun: {
    tick(rt, pos, t) {
      for (let i = 0; i < 3; i++) {
        const a = t * 5 + (i / 3) * Math.PI * 2;
        _p.set(pos.x + Math.cos(a) * 0.45, 2.3, pos.z + Math.sin(a) * 0.45);
        rt.ps.emit(rt.s.star, _p, { count: 1, shape: 'point', life: 0.12, size: 0.24, tint: STAR });
      }
    },
  },
  suppress: {
    tick(rt, pos, t, c) {
      _p.set(pos.x, 0.9 + Math.sin(t * 6) * 0.3, pos.z);
      rt.ps.emit(rt.s.ring, _p, { count: 1, shape: 'point', life: 0.3, size: 1.6, tint: c.vividHdr });
    },
  },
  root: {
    start(rt, pos, c) {
      shockwave(rt, pos, 0.9, 0.25, c.vividHdr, 0.12);
    },
    tick(rt, pos, _t, c) {
      // liens qui remontent le long des jambes
      rt.ps.emit(rt.s.shard, pos, { count: 2, shape: 'ring', radius: 0.45, up: [0.8, 1.6], speed: [-0.6, -0.3], life: [0.3, 0.45], size: [0.12, 0.2], tint: c.vividHdr });
      rt.ps.emit(rt.s.ring, _p.set(pos.x, 0.06, pos.z), { count: 1, shape: 'point', life: 0.15, size: 1.1, tint: c.vivid });
    },
  },
  slow: {
    tick(rt, pos, _t, c) {
      if (rng.next() < 0.5) rt.ps.emit(rt.s.ember, pos, { count: 1, shape: 'disc', radius: 0.35, up: [0.2, 0.5], life: [0.4, 0.6], size: [0.08, 0.14], gravity: -0.5, tint: c.vivid, intensity: 0.8 });
    },
  },
  knockup: {
    start(rt, pos, c) {
      rt.ps.emit(rt.s.dust, pos, { count: 8, shape: 'ring', radius: 0.4, speed: [1, 2], up: [0.4, 1], life: [0.5, 0.8], size: [0.6, 1], drag: 2 });
      rt.ps.emit(rt.s.swirl, _p.set(pos.x, 0.3, pos.z), { count: 2, shape: 'point', life: 0.4, size: [1.2, 1.6], spin: 8, tint: c.vivid });
    },
  },
  taunt: {
    tick(rt, pos, t) {
      if (Math.sin(t * 20) > 0.3) rt.ps.emit(rt.s.spark, _p.set(pos.x, 2.3, pos.z), { count: 1, shape: 'cone', angle: 0.6, speed: [1.5, 2.5], life: [0.15, 0.25], size: [0.06, 0.1], stretch: 0.05, tint: TAUNT });
    },
  },
  fear: {
    tick(rt, pos) {
      rt.ps.emit(rt.s.darkWisp, _p.set(pos.x, 1.8, pos.z), { count: 1, shape: 'sphere', radius: 0.3, up: [0.4, 0.8], life: [0.5, 0.8], size: [0.4, 0.6] });
    },
  },
  silence: {
    tick(rt, pos, t) {
      const a = t * 3;
      _p.set(pos.x + Math.cos(a) * 0.35, 2.25, pos.z + Math.sin(a) * 0.35);
      rt.ps.emit(rt.s.rune, _p, { count: 1, shape: 'point', life: 0.2, size: 0.3, tint: SILENCE });
    },
  },
  blind: {
    tick(rt, pos) {
      rt.ps.emit(rt.s.darkWisp, _p.set(pos.x, 1.9, pos.z), { count: 1, shape: 'sphere', radius: 0.2, life: [0.4, 0.6], size: [0.3, 0.5] });
    },
  },
  grievous: {
    tick(rt, pos) {
      if (rng.next() < 0.35) rt.ps.emit(rt.s.cross, pos, { count: 1, shape: 'disc', radius: 0.3, up: [0.6, 1], life: [0.5, 0.7], size: [0.18, 0.24], tint: GRIEVOUS });
    },
  },
  invisible: {
    start(rt, pos) {
      rt.ps.emit(rt.s.darkWisp, _p.set(pos.x, 0.8, pos.z), { count: 12, shape: 'sphere', radius: 0.5, speed: [0.5, 1.2], life: [0.5, 0.9], size: [0.6, 1] });
    },
    end(rt, pos) {
      rt.ps.emit(rt.s.darkWisp, _p.set(pos.x, 0.8, pos.z), { count: 10, shape: 'sphere', radius: 0.5, speed: [0.5, 1.2], life: [0.5, 0.9], size: [0.6, 1] });
      flash(rt, _p, 1.2, DARK, 0.2);
    },
  },
  revealed: {
    start(rt, pos) {
      shockwave(rt, pos, 1.2, 0.35, GRIEVOUS, 0.06);
    },
  },
};

/** Catégorie d'un buff : `ilyra_q_slow` → slow, `stun` → stun… */
export function statusOf(buffId: string): string | null {
  if (buffId in STATUS) return buffId;
  if (buffId.endsWith('_slow') || buffId === 'exhaust') return 'slow';
  return null;
}

interface Active {
  until: number;
  removed: boolean;
}

export function installStatusFx(): void {
  const active = new Map<string, Active>();
  onWorldEvent('buffApplied', (rt, e) => {
    const kind = statusOf(e.buffId);
    if (!kind) return;
    const fx = STATUS[kind]!;
    const key = `${kind}:${e.target}`;
    const until = rt.time + Math.max(0.15, Math.min(e.duration, 60));
    const prev = active.get(key);
    if (prev && !prev.removed) {
      prev.until = Math.max(prev.until, until); // réapplication : on prolonge
      return;
    }
    const state: Active = { until, removed: false };
    active.set(key, state);
    const c = rt.colors(e.fxKey ?? e.buffId);
    const pos = new THREE.Vector3();
    if (!rt.entities.pos(e.target, pos)) return;
    fx.start?.(rt, pos, c, e);
    let t = 0;
    let acc = 0;
    rt.add({
      update(dt) {
        t += dt;
        acc += dt;
        if (!rt.entities.pos(e.target, pos)) return false;
        if (fx.tick && acc >= 0.05) {
          acc = 0;
          fx.tick(rt, pos, t, c);
        }
        return !state.removed && rt.time < state.until;
      },
      end() {
        if (active.get(key) === state) active.delete(key);
        fx.end?.(rt, pos, c);
      },
    });
  });
  onWorldEvent('buffRemoved', (_rt, e) => {
    const kind = statusOf(e.buffId);
    if (!kind) return;
    const s = active.get(`${kind}:${e.target}`);
    if (s) s.removed = true;
  });
}
