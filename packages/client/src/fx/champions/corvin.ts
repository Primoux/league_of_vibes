/**
 * FX de Corvin, l'Archer des Corbeaux (événements : server/src/abilities/champions/corvin.ts, passives.ts).
 *  P  Marque du corbeau  buff corvin_marks (cumuls, fxKey corvin_p) ; la 3e marque est consommée
 *  Q  Flèche perçante    projectile corvin_q
 *  W  Piège à griffes    zone corvin_w (60 s) ; déclenché : root + revealed (fxKey corvin_w) sur la victime
 *  E  Envol              dash corvin_e (en arrière) + buff corvin_e (vitesse)
 *  R  Nuée               zone corvin_r (3 s)
 */
import * as THREE from 'three';
import { ProjectileFx, flash, shockwave, timed, zone } from '../effects/primitives.ts';
import type { FxRuntime } from '../core/FxRuntime.ts';
import { rng } from '../core/rng.ts';
import { registerFx } from '../registry.ts';

const GREEN = new THREE.Color(0.8, 2.6, 0.7);
const INK = new THREE.Color(0.12, 0.12, 0.18);
const _v = new THREE.Vector3();
const _w = new THREE.Vector3();

function feathers(rt: FxRuntime, at: THREE.Vector3, n: number, power = 1): void {
  rt.ps.emit(rt.s.feather, at, { count: n, shape: 'sphere', radius: 0.4, speed: [0.8 * power, 2.5 * power], up: [0.5, 1.5], gravity: 1.2, drag: 1.5, life: [0.8, 1.4], size: [0.22, 0.34], spin: [-3, 3], tint: INK });
}

/** Marques par cible (nombre de cumuls). */
const marks = new Map<number, { stacks: number; until: number }>();

registerFx('corvin_p', {
  buff(rt, e) {
    if (e.buffId !== 'corvin_marks') return;
    const until = rt.time + Math.max(0.3, e.duration);
    const prev = marks.get(e.target);
    if (prev) {
      prev.stacks = e.stacks ?? prev.stacks + 1;
      prev.until = until;
      return;
    }
    const state = { stacks: e.stacks ?? 1, until };
    marks.set(e.target, state);
    const pos = new THREE.Vector3();
    let acc = 0;
    let t = 0;
    rt.add({
      update(dt) {
        t += dt;
        acc += dt;
        if (rt.time > state.until || state.stacks <= 0 || !rt.entities.pos(e.target, pos)) return false;
        if (acc < 0.05) return true;
        acc = 0;
        for (let i = 0; i < state.stacks; i++) {
          const a = t * 2 + (i / Math.max(1, state.stacks)) * Math.PI * 2;
          rt.ps.emit(rt.s.rune, _v.set(pos.x + Math.cos(a) * 0.5, 2.1, pos.z + Math.sin(a) * 0.5), { count: 1, shape: 'point', life: 0.12, size: 0.24, tint: GREEN });
        }
        return true;
      },
      end() {
        if (marks.get(e.target) === state) marks.delete(e.target);
      },
    });
  },
  buffEnd(rt, e) {
    if (e.buffId !== 'corvin_marks') return;
    const s = marks.get(e.target);
    // marques consommées (3e coup) : explosion de plumes vertes
    if (s && s.stacks >= 2 && rt.entities.pos(e.target, _v, 1)) {
      flash(rt, _v, 2.2, GREEN, 0.25);
      feathers(rt, _v, 10, 1.4);
    }
    if (s) s.stacks = 0;
  },
});

registerFx('corvin_q', {
  projectile(rt, e) {
    const p = new ProjectileFx(rt, rt.world(e.from), rt.world(e.to), rt.scale(e.speed), {
      color: GREEN,
      core: new THREE.Color(2.4, 3, 2.4),
      size: 0.4,
      height: 1.2,
      ribbonWidth: 0.12,
      ribbonLife: 0.25,
      trailStyle: rt.s.feather,
      trailDensity: 1.5,
      trailSize: [0.15, 0.22],
      trailLife: [0.5, 0.8],
    });
    rt.add(p);
    return p;
  },
  hit(rt, e) {
    const at = rt.world(e.pos, 1);
    rt.ps.emit(rt.s.spark, at, { count: 10, shape: 'point', speed: [2, 5], life: [0.15, 0.3], size: [0.05, 0.08], stretch: 0.05, tint: GREEN });
    feathers(rt, at, 3);
  },
});

// ---------------------------------------------------------------- pièges
interface Trap {
  center: THREE.Vector3;
  end: () => void;
}
const traps = new Set<Trap>();

registerFx('corvin_w', {
  /** Piège posé : griffes discrètes au sol (visible des alliés seulement : le serveur filtre la zone). */
  area(rt, e) {
    const center = rt.world(e.pos);
    const r = rt.scale(e.radius);
    const d = rt.acquireDecal().setColor(GREEN.clone().multiplyScalar(0.35));
    d.circle(center, r);
    d.u.uFill.value = 0.1;
    d.u.uEdge.value = 0.04;
    d.u.uPattern.value = 0.6;
    let done = false;
    const trap: Trap = { center, end: () => (done = true) };
    traps.add(trap);
    feathers(rt, center.clone().setY(0.3), 4, 0.6);
    let t = 0;
    rt.add({
      update(dt) {
        t += dt;
        d.u.uOpacity.value = Math.min(1, t * 4) * (0.6 + 0.4 * Math.sin(t * 2));
        return !done && t < Math.max(1, e.duration);
      },
      end() {
        traps.delete(trap);
        rt.releaseDecal(d);
      },
    });
  },
  /** Déclenché : la victime est immobilisée et révélée → le piège le plus proche se referme. */
  buff(rt, e) {
    if (e.buffId !== 'root' || !rt.entities.pos(e.target, _v)) return;
    let best: Trap | null = null;
    let bestD = 4;
    for (const t of traps) {
      const d = t.center.distanceToSquared(_v);
      if (d < bestD) {
        bestD = d;
        best = t;
      }
    }
    best?.end();
    shockwave(rt, _v, 1.2, 0.3, GREEN, 0.1);
    for (let i = 0; i < 6; i++) {
      const a = (i / 6) * Math.PI * 2;
      rt.bursts.rockSpikes.spawn({ pos: _w.set(_v.x + Math.cos(a) * 0.45, 0, _v.z + Math.sin(a) * 0.45), mode: 'spike', size: 0.35, life: 1.2, heading: Math.atan2(Math.cos(a), Math.sin(a)), tilt: 0.6, stretch: 1.4, color: new THREE.Color(0.25, 0.25, 0.3) });
    }
  },
});

registerFx('corvin_e', {
  dash(rt, e) {
    const from = rt.world(e.from, 0.8);
    flash(rt, from, 1.6, GREEN, 0.2);
    feathers(rt, from, 16, 1.5);
    const pos = new THREE.Vector3();
    timed(rt, Math.max(0.05, e.duration), () => {
      if (rt.entities.pos(e.source, pos, 0.8)) feathers(rt, pos, 1, 0.5);
    });
  },
  buff(rt, e) {
    if (e.buffId !== 'corvin_e') return;
    const pos = new THREE.Vector3();
    let acc = 0;
    timed(rt, Math.max(0.3, e.duration), (_t, dt) => {
      acc += dt;
      if (acc < 0.12 || !rt.entities.pos(e.target, pos, 0.3)) return;
      acc = 0;
      feathers(rt, pos, 1, 0.3);
    });
  },
});

registerFx('corvin_r', {
  /** Nuée : corbeaux (plumes) qui tournoient, ombres, éclats verts. */
  area(rt, e) {
    const center = rt.world(e.pos);
    const r = rt.scale(e.radius);
    const duration = Math.max(0.5, e.duration);
    zone(rt, center, r, duration, GREEN.clone().multiplyScalar(0.5), { fill: 0.08, pattern: 0.4 });
    const vel = new THREE.Vector3();
    let acc = 0;
    timed(rt, duration, (t, dt) => {
      const fade = Math.min(1, (1 - t) * duration * 2);
      acc += dt * 60 * fade;
      while (acc >= 1) {
        acc -= 1;
        const a = rng.next() * Math.PI * 2;
        const d = r * rng.range(0.3, 1);
        _v.set(center.x + Math.cos(a) * d, rng.range(0.8, 2.6), center.z + Math.sin(a) * d);
        vel.set(-Math.sin(a) * 5, rng.range(-0.5, 0.5), Math.cos(a) * 5);
        rt.ps.emit(rt.s.feather, _v, { count: 1, shape: 'point', inherit: vel, life: [0.4, 0.7], size: [0.3, 0.45], spin: [-6, 6], tint: INK });
        if (rng.next() < 0.15) rt.ps.emit(rt.s.ember, _v, { count: 1, shape: 'point', inherit: vel, life: 0.3, size: 0.08, tint: GREEN });
      }
      if (rng.next() < dt * 6) rt.ps.emit(rt.s.darkWisp, center, { count: 1, shape: 'disc', radius: r * 0.8, up: [0.2, 0.6], life: [0.8, 1.2], size: [0.8, 1.3] });
    });
  },
});
