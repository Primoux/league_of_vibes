/**
 * FX de Nyx, l'Ombre Sans Nom (événements : server/src/abilities/champions/nyx.ts).
 *  Q  Lame du crépuscule  dégâts nyx_q sur la cible (+ marque de réinitialisation)
 *  W  Voile d'ombre       buff invisible (fxKey nyx_w)
 *  E  Pas de l'ombre      blink nyx_e derrière la cible
 *  R  Mort certaine       debuff nyx_r (3 s) puis explosion à son retrait
 */
import * as THREE from 'three';
import { flash, lightning, shockwave } from '../effects/primitives.ts';
import type { FxRuntime } from '../core/FxRuntime.ts';
import { onAbilityHit, registerFx } from '../registry.ts';

const PINK = new THREE.Color(2.8, 0.4, 1.0);
const _v = new THREE.Vector3();
const _a = new THREE.Vector3();
const _b = new THREE.Vector3();

function shadowPuff(rt: FxRuntime, at: THREE.Vector3, n: number): void {
  rt.ps.emit(rt.s.darkWisp, at, { count: n, shape: 'sphere', radius: 0.4, speed: [0.5, 1.5], up: [0.2, 0.8], life: [0.5, 0.9], size: [0.6, 1] });
  rt.ps.emit(rt.s.ember, at, { count: Math.round(n / 2), shape: 'sphere', radius: 0.4, speed: [0.5, 2], life: [0.3, 0.5], size: [0.06, 0.1], tint: PINK });
}

/** Entaille en X : deux traits roses croisés. */
function slashX(rt: FxRuntime, at: THREE.Vector3): void {
  for (const s of [-1, 1]) {
    _a.set(at.x - 0.55, at.y + 0.55 * s, at.z - 0.25 * s);
    _b.set(at.x + 0.55, at.y - 0.55 * s, at.z + 0.25 * s);
    lightning(rt, _a, _b, PINK, { width: 0.16, life: 0.22, jag: 0.02, segments: 4 });
  }
  flash(rt, at, 1.6, PINK, 0.15);
}

onAbilityHit('nyx_q', (rt, e) => {
  if (rt.entities.pos(e.target, _v, 1.1)) slashX(rt, _v);
});

registerFx('nyx_w', {
  buff(rt, e) {
    if (rt.entities.pos(e.target, _v, 0.8)) shadowPuff(rt, _v, 14);
  },
});

registerFx('nyx_e', {
  dash(rt, e) {
    const from = rt.world(e.from, 0.8);
    const to = rt.world(e.to, 0.8);
    shadowPuff(rt, from, 12);
    shadowPuff(rt, to, 10);
    flash(rt, to, 1.6, PINK, 0.2);
  },
});

registerFx('nyx_r', {
  /** Marque : sceau rose au-dessus de la tête, qui pulse de plus en plus vite. */
  buff(rt, e) {
    if (e.buffId !== 'nyx_r') return;
    const key = `nyx_r:${e.target}`;
    let ended = false;
    const cancel = (): void => {
      ended = true;
    };
    rt.setAura(key, cancel);
    const pos = new THREE.Vector3();
    const duration = Math.max(0.5, e.duration);
    let t = 0;
    let acc = 0;
    rt.add({
      update(dt) {
        t += dt;
        acc += dt;
        if (ended || t > duration + 0.5 || !rt.entities.pos(e.target, pos)) return false;
        const k = Math.min(1, t / duration);
        if (acc > 0.05) {
          acc = 0;
          rt.ps.emit(rt.s.rune, _v.set(pos.x, 2.4, pos.z), { count: 1, shape: 'point', life: 0.1, size: 0.5 + 0.15 * Math.sin(t * (6 + k * 20)), spin: 3, tint: PINK });
          rt.ps.emit(rt.s.ring, _v.set(pos.x, 0.05, pos.z), { count: 1, shape: 'point', life: 0.1, size: 1.4 - k * 0.6, tint: PINK, intensity: 0.4 + k });
        }
        return true;
      },
      end: () => rt.forgetAura(key, cancel),
    });
  },
  /** La marque explose. */
  buffEnd(rt, e) {
    if (e.buffId !== 'nyx_r') return;
    rt.endAura(`nyx_r:${e.target}`);
    const pos = new THREE.Vector3();
    if (!rt.entities.pos(e.target, pos)) return;
    flash(rt, _v.set(pos.x, 1.1, pos.z), 3.5, PINK, 0.3);
    shockwave(rt, pos, 2.2, 0.45, PINK, 0.16);
    shadowPuff(rt, _v, 20);
    rt.ps.emit(rt.s.spark, _v, { count: 30, shape: 'point', speed: [4, 9], life: [0.25, 0.45], size: [0.06, 0.1], stretch: 0.06, tint: PINK });
  },
});
