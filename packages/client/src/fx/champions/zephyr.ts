/**
 * FX de Zephyr, l'Arcaniste Instable (événements : server/src/abilities/champions/zephyr.ts).
 *  Q  Orbe boomerang    projectile zephyr_q (aller), puis zephyr_q_return (retour, tête chercheuse)
 *  W  Brèche            blink zephyr_w ; à l'origine : zephyr_w_explosion_telegraph puis zephyr_w_explosion
 *  E  Chaîne arcanique  debuff zephyr_e (lien) sur la cible ; à la fin : root (statut générique)
 *  R  Barrage arcanique rayons zephyr_r (0,5 s d'incantation, plusieurs charges)
 */
import * as THREE from 'three';
import { ProjectileFx, flash, lightning, shockwave, telegraph, timed } from '../effects/primitives.ts';
import type { FxRuntime } from '../core/FxRuntime.ts';
import { rng } from '../core/rng.ts';
import { registerFx, type Ev } from '../registry.ts';

const VIOLET = new THREE.Color(1.8, 0.8, 2.8);
const CYAN = new THREE.Color(0.6, 2.4, 2.8);
const _v = new THREE.Vector3();
const _w = new THREE.Vector3();

function arcaneBurst(rt: FxRuntime, at: THREE.Vector3, scale: number): void {
  flash(rt, at, 2 * scale, VIOLET, 0.25);
  rt.ps.emit(rt.s.sparkles, at, { count: Math.round(10 * scale), shape: 'sphere', radius: 0.3 * scale, speed: [1, 3], life: [0.3, 0.6], size: [0.25, 0.4], tint: CYAN });
  rt.ps.emit(rt.s.spark, at, { count: Math.round(14 * scale), shape: 'point', speed: [3, 6], life: [0.2, 0.35], size: [0.05, 0.09], stretch: 0.05, tint: VIOLET });
}

function orb(rt: FxRuntime, e: Ev<'projectileSpawn'>, color: THREE.Color, core: THREE.Color, homingTo?: number): ProjectileFx {
  let spin = 0;
  const p = new ProjectileFx(rt, rt.world(e.from), rt.world(e.to), rt.scale(e.speed), {
    color,
    core,
    size: 0.7,
    height: 1.1,
    ribbonWidth: 0.3,
    ribbonLife: 0.25,
    targetId: homingTo,
    onStep(head, _dir, dt) {
      spin += dt * 12;
      // deux étincelles en orbite autour de l'orbe
      for (let i = 0; i < 2; i++) {
        const a = spin + i * Math.PI;
        _w.set(head.x + Math.cos(a) * 0.3, head.y + Math.sin(a) * 0.3, head.z);
        rt.ps.emit(rt.s.glow, _w, { count: 1, shape: 'point', life: 0.2, size: 0.18, tint: core });
      }
    },
  });
  rt.add(p);
  return p;
}

registerFx('zephyr_q', {
  projectile(rt, e) {
    return orb(rt, e, VIOLET, CYAN);
  },
  hit(rt, e) {
    arcaneBurst(rt, rt.world(e.pos, 1), 0.8);
  },
});
registerFx('zephyr_q_return', {
  projectile(rt, e) {
    return orb(rt, e, CYAN, VIOLET, e.source);
  },
  hit(rt, e) {
    arcaneBurst(rt, rt.world(e.pos, 1), 0.8);
  },
});

registerFx('zephyr_w', {
  /** Blink : déchirure au départ, apparition à l'arrivée. */
  dash(rt, e) {
    const from = rt.world(e.from, 1);
    const to = rt.world(e.to, 1);
    arcaneBurst(rt, from, 1);
    arcaneBurst(rt, to, 0.8);
    lightning(rt, from, to, VIOLET, { width: 0.12, life: 0.25, jag: 0.25 });
  },
});
/** La brèche laissée derrière crépite pendant le délai. */
registerFx('zephyr_w_explosion_telegraph', {
  area(rt, e) {
    const center = rt.world(e.pos);
    const r = rt.scale(e.radius);
    const delay = Math.max(0.2, e.duration);
    telegraph(rt, center, r, delay, VIOLET.clone().multiplyScalar(0.5), { color2: VIOLET, fill: 0.12 });
    let acc = 0;
    timed(rt, delay, (t, dt) => {
      acc += dt;
      if (acc < 0.07) return;
      acc = 0;
      const a = rng.next() * Math.PI * 2;
      _v.set(center.x, 1, center.z);
      _w.set(center.x + Math.cos(a) * r * 0.6, 0.4 + rng.next(), center.z + Math.sin(a) * r * 0.6);
      lightning(rt, _v, _w, rng.next() < 0.5 ? VIOLET : CYAN, { width: 0.06, life: 0.12, jag: 0.15, segments: 5 });
      rt.ps.emit(rt.s.glow, _v, { count: 1, shape: 'point', life: 0.15, size: 0.8 + t, tint: VIOLET });
    });
  },
});
registerFx('zephyr_w_explosion', {
  area(rt, e) {
    const center = rt.world(e.pos);
    const r = rt.scale(e.radius);
    shockwave(rt, center, r * 1.1, 0.45, VIOLET, 0.16);
    arcaneBurst(rt, _v.set(center.x, 0.8, center.z), 1.6);
  },
});

registerFx('zephyr_e', {
  /** Chaîne arcanique : un éclair relie Zephyr et la cible tant que le lien tient. */
  buff(rt, e) {
    if (e.buffId !== 'zephyr_e' || e.source === undefined) return;
    const source = e.source;
    const key = `zephyr_e:${e.target}`;
    let ended = false;
    const cancel = (): void => {
      ended = true;
    };
    rt.setAura(key, cancel);
    const a = new THREE.Vector3();
    const b = new THREE.Vector3();
    let acc = 0;
    const until = rt.time + Math.max(0.3, e.duration);
    rt.add({
      update(dt) {
        acc += dt;
        if (ended || rt.time > until || !rt.entities.pos(source, a, 1.1) || !rt.entities.pos(e.target, b, 1.1)) return false;
        if (acc >= 0.06) {
          acc = 0;
          lightning(rt, a, b, VIOLET, { width: 0.1, life: 0.1, jag: 0.18, segments: 8 });
          rt.ps.emit(rt.s.sparkles, b, { count: 1, shape: 'sphere', radius: 0.4, life: 0.3, size: 0.3, tint: CYAN });
        }
        return true;
      },
      end() {
        rt.forgetAura(key, cancel);
      },
    });
  },
  buffEnd(rt, e) {
    if (e.buffId !== 'zephyr_e') return;
    rt.endAura(`zephyr_e:${e.target}`);
    if (rt.entities.pos(e.target, _v, 1)) arcaneBurst(rt, _v, 1.2);
  },
});

registerFx('zephyr_r', {
  /** Charge entre les mains pendant l'incantation. */
  cast(rt, e) {
    const pos = new THREE.Vector3();
    timed(rt, 0.5, (t) => {
      if (!rt.entities.pos(e.source, pos, 1.3)) return;
      rt.ps.emit(rt.s.glow, pos, { count: 2, shape: 'sphere', radius: 0.8 * (1 - t), speed: [0, 0.2], life: 0.15, size: 0.3 + t * 0.6, tint: CYAN });
    });
  },
  /** Rayon : faisceau épais et très rapide. */
  projectile(rt, e) {
    const p = new ProjectileFx(rt, rt.world(e.from), rt.world(e.to), rt.scale(e.speed), {
      color: VIOLET,
      core: CYAN,
      size: 1.2,
      height: 1.2,
      ribbonWidth: 0.7,
      ribbonLife: 0.4,
      trailStyle: rt.s.sparkles,
      trailDensity: 3,
      trailSize: [0.3, 0.5],
      trailLife: [0.3, 0.5],
    });
    rt.add(p);
    return p;
  },
  hit(rt, e) {
    const at = rt.world(e.pos, 1);
    arcaneBurst(rt, at, 2);
    shockwave(rt, at.setY(0), 2, 0.4, CYAN, 0.14);
  },
});
