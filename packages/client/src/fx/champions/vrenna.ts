/**
 * FX de Vrenna, la Lance des Steppes (événements : server/src/abilities/champions/vrenna.ts, passives.ts).
 *  P  Saignée          buff vrenna_bleed (cumuls) sur la cible ; vrenna_p_speed sur Vrenna à 5 cumuls
 *  Q  Coup de lance    projectile vrenna_q ; la pointe attire : dash vrenna_q_pull sur la cible
 *  W  Cri de guerre    buff vrenna_w + soin
 *  E  Bond des steppes dash vrenna_e, dégâts de zone à l'atterrissage
 *  R  Lance céleste    dash vrenna_r sur un champion, projection à l'arrivée
 */
import * as THREE from 'three';
import { ProjectileFx, dashTrail, flash, shockwave, timed } from '../effects/primitives.ts';
import type { FxRuntime } from '../core/FxRuntime.ts';
import { rng } from '../core/rng.ts';
import { registerFx } from '../registry.ts';

const GOLD = new THREE.Color(2.8, 2.1, 0.5);
const WAR = new THREE.Color(2.4, 0.5, 0.3);
const _v = new THREE.Vector3();

/** Saignement par cible : nombre de cumuls (rafraîchi à chaque réapplication). */
const bleeds = new Map<number, { stacks: number; until: number }>();

registerFx('vrenna_p', {
  buff(rt, e) {
    if (e.buffId === 'vrenna_p_speed') {
      const pos = new THREE.Vector3();
      timed(rt, Math.max(0.3, e.duration), () => {
        if (rng.next() < 0.4 && rt.entities.pos(e.target, pos)) rt.ps.emit(rt.s.ember, pos, { count: 1, shape: 'ring', radius: 0.4, up: [1, 2], life: [0.3, 0.5], size: [0.06, 0.1], tint: WAR });
      });
      return;
    }
    if (e.buffId !== 'vrenna_bleed') return;
    const until = rt.time + Math.max(0.3, e.duration);
    const prev = bleeds.get(e.target);
    if (prev) {
      prev.stacks = e.stacks ?? prev.stacks + 1;
      prev.until = until;
      return;
    }
    const state = { stacks: e.stacks ?? 1, until };
    bleeds.set(e.target, state);
    const pos = new THREE.Vector3();
    let acc = 0;
    rt.add({
      update(dt) {
        acc += dt * (2 + state.stacks * 2.5);
        if (rt.time > state.until || state.stacks <= 0 || !rt.entities.pos(e.target, pos)) return false;
        while (acc >= 1) {
          acc -= 1;
          rt.ps.emit(rt.s.blood, pos.setY(rng.range(0.6, 1.4)), { count: 1, shape: 'sphere', radius: 0.25, speed: [0.3, 0.8], gravity: 8, life: [0.35, 0.55], size: [0.06, 0.1] });
        }
        return true;
      },
      end() {
        if (bleeds.get(e.target) === state) bleeds.delete(e.target);
      },
    });
  },
  buffEnd(_rt, e) {
    if (e.buffId === 'vrenna_bleed') {
      const s = bleeds.get(e.target);
      if (s) s.stacks = 0;
    }
  },
});

registerFx('vrenna_q', {
  /** Estoc : la lance file, pointe dorée, sillage rouge. */
  projectile(rt, e) {
    const p = new ProjectileFx(rt, rt.world(e.from), rt.world(e.to), rt.scale(e.speed), {
      color: WAR,
      core: GOLD,
      size: 0.35,
      height: 1.1,
      ribbonWidth: 0.2,
      ribbonLife: 0.18,
      trailStyle: rt.s.spark,
      trailDensity: 4,
      trailSize: [0.05, 0.08],
      trailLife: [0.15, 0.25],
    });
    rt.add(p);
    return p;
  },
  hit(rt, e) {
    const at = rt.world(e.pos, 1);
    flash(rt, at, 1.4, GOLD, 0.15);
    rt.ps.emit(rt.s.blood, at, { count: 8, shape: 'point', speed: [1.5, 3], gravity: 9, life: [0.3, 0.5], size: [0.06, 0.09] });
  },
});
/** La cible est arrachée vers Vrenna. */
registerFx('vrenna_q_pull', {
  dash(rt, e) {
    dashTrail(rt, e.source, rt.world(e.from), rt.world(e.to), Math.max(0.05, e.duration), WAR.clone().multiplyScalar(0.5), true);
  },
});

registerFx('vrenna_w', {
  cast(rt, e) {
    const pos = new THREE.Vector3();
    if (!rt.entities.pos(e.source, pos)) return;
    for (let i = 0; i < 2; i++) rt.after(i * 0.12, () => shockwave(rt, pos, 2.2 - i * 0.5, 0.45, WAR, 0.12));
    flash(rt, _v.set(pos.x, 1.6, pos.z), 2.4, WAR, 0.3);
  },
  buff(rt, e) {
    if (e.buffId !== 'vrenna_w') return;
    const pos = new THREE.Vector3();
    let acc = 0;
    timed(rt, Math.max(0.3, e.duration), (_t, dt) => {
      acc += dt;
      if (acc < 0.05 || !rt.entities.pos(e.target, pos)) return;
      acc = 0;
      rt.ps.emit(rt.s.flame, pos, { count: 1, shape: 'ring', radius: 0.45, up: [0.8, 1.5], life: [0.3, 0.5], size: [0.2, 0.35], tint: WAR, intensity: 0.6 });
    });
  },
});

registerFx('vrenna_e', {
  dash(rt, e) {
    const from = rt.world(e.from);
    const to = rt.world(e.to);
    const duration = Math.max(0.05, e.duration);
    rt.ps.emit(rt.s.dust, from, { count: 8, shape: 'disc', radius: 0.4, up: [0.5, 1.2], speed: [0.5, 1.5], life: [0.6, 0.9], size: [0.6, 1.1] });
    dashTrail(rt, e.source, from, to, duration, WAR.clone().multiplyScalar(0.6), false);
    // atterrissage : la lance frappe le sol
    rt.after(duration, () => {
      const pos = new THREE.Vector3();
      if (!rt.entities.pos(e.source, pos)) pos.copy(to);
      shockwave(rt, pos, 2.5, 0.4, WAR, 0.16);
      rt.ps.emit(rt.s.dust, pos, { count: 14, shape: 'ring', radius: 0.6, speed: [1.5, 3], up: [0.3, 1], life: [0.6, 1], size: [0.8, 1.3], drag: 2 });
      flash(rt, _v.set(pos.x, 0.6, pos.z), 2, GOLD, 0.2);
    });
  },
});

registerFx('vrenna_r', {
  /** Bond vers le ciel puis la lance s'abat : traînée dorée, impact projetant. */
  dash(rt, e) {
    const from = rt.world(e.from);
    const to = rt.world(e.to);
    const duration = Math.max(0.05, e.duration);
    dashTrail(rt, e.source, from, to, duration, GOLD, false);
    const pos = new THREE.Vector3();
    timed(rt, duration, (t) => {
      if (!rt.entities.pos(e.source, pos)) return;
      rt.ps.emit(rt.s.glow, pos.setY(1 + Math.sin(t * Math.PI) * 1.5), { count: 2, shape: 'point', life: 0.25, size: 0.6, tint: GOLD });
    });
    rt.after(duration, () => impactR(rt, e.source, to));
  },
});

function impactR(rt: FxRuntime, source: number, fallback: THREE.Vector3): void {
  const pos = new THREE.Vector3();
  if (!rt.entities.pos(source, pos)) pos.copy(fallback);
  flash(rt, _v.set(pos.x, 1, pos.z), 3.5, GOLD, 0.3);
  shockwave(rt, pos, 2.6, 0.5, GOLD, 0.2);
  rt.ps.emit(rt.s.spark, pos, { count: 30, shape: 'disc', radius: 0.5, up: [3, 8], speed: [1, 3], life: [0.3, 0.6], size: [0.07, 0.12], stretch: 0.06, gravity: 6, tint: GOLD });
  rt.ps.emit(rt.s.dust, pos, { count: 12, shape: 'ring', radius: 0.6, speed: [1.5, 3], up: [0.3, 1], life: [0.7, 1.1], size: [0.9, 1.4], drag: 2 });
}
