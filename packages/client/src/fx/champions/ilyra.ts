/**
 * FX d'Ilyra, la Sentinelle du Givre : glace cristalline, neige, brume froide.
 *  P  Morsure du givre — prison de glace autour de la cible étourdie, puis éclatement
 *  Q  Éclat de glace   — éclat de glace qui tournoie dans un sillage de neige, se brise à l'impact
 *  W  Nova glaciale    — runes de givre qui se remplissent, couronne de pics de glace
 *  E  Égide de givre   — flux de givre vers l'allié, bouclier de glace avec éclats en orbite
 *  R  Blizzard         — tempête de neige tournoyante, éclats qui tombent, brume glacée
 */
import * as THREE from 'three';
import { groundMist, shieldShell } from '../effects/auras.ts';
import { ProjectileFx, flash, shockwave, telegraph, timed, zone } from '../effects/primitives.ts';
import { rng } from '../core/rng.ts';
import type { FxRuntime } from '../core/FxRuntime.ts';
import { createIceMaterial } from '../shaders/materials.ts';
import { registerFx } from '../registry.ts';

let iceMat: THREE.ShaderMaterial | null = null;
const iceMaterial = (): THREE.ShaderMaterial => (iceMat ??= createIceMaterial());
const shardGeo = new THREE.OctahedronGeometry(0.16, 0);

const ICE_HDR = new THREE.Color(1.3, 2.4, 3.2);
const FROST = new THREE.Color(0.55, 0.8, 1);
const MIST = new THREE.Color(0.75, 0.88, 1);
const _v = new THREE.Vector3();
const _w = new THREE.Vector3();

function shatter(rt: FxRuntime, at: THREE.Vector3, n: number, power: number): void {
  for (let i = 0; i < n; i++) {
    const a = rng.next() * Math.PI * 2;
    rt.bursts.iceDebris.spawn({
      pos: _v.set(at.x, at.y, at.z),
      mode: 'debris',
      size: rng.range(0.1, 0.2),
      life: rng.range(0.6, 1),
      vel: _w.set(Math.cos(a) * rng.range(1, 3) * power, rng.range(1.5, 4) * power, Math.sin(a) * rng.range(1, 3) * power),
    });
  }
  rt.ps.emit(rt.s.snow, at, { count: n, shape: 'sphere', radius: 0.3, speed: [1, 3], life: [0.5, 0.9], size: [0.12, 0.22], spin: [-4, 4], drag: 2, tint: ICE_HDR });
  rt.ps.emit(rt.s.spark, at, { count: n, shape: 'point', speed: [3, 6], life: [0.2, 0.35], size: [0.04, 0.08], stretch: 0.04, tint: ICE_HDR });
}

/** Particules de neige qui convergent vers un point (incantation). */
function gather(rt: FxRuntime, at: THREE.Vector3, n: number): void {
  for (let i = 0; i < n; i++) {
    const a = rng.next() * Math.PI * 2;
    const d = rng.range(0.6, 1);
    const life = rng.range(0.25, 0.35);
    _v.set(at.x + Math.cos(a) * d, at.y + rng.range(-0.3, 0.4), at.z + Math.sin(a) * d);
    _w.set(at.x - _v.x, at.y - _v.y, at.z - _v.z).divideScalar(life);
    rt.ps.emit(rt.s.ember, _v, { count: 1, shape: 'point', life, size: [0.08, 0.14], inherit: _w, tint: ICE_HDR });
  }
}

/** Cumuls de givre par cible : nombre de flocons en orbite (mis à jour à chaque réapplication). */
const frost = new Map<number, { stacks: number; until: number }>();

registerFx('ilyra_p', {
  buff(rt, e) {
    if (e.buffId === 'stun') return icePrison(rt, e.target, e.duration);
    if (e.buffId !== 'ilyra_frost') return;
    const until = rt.time + Math.max(0.5, e.duration);
    const prev = frost.get(e.target);
    if (prev) {
      prev.stacks = e.stacks ?? prev.stacks + 1;
      prev.until = until;
      return;
    }
    const state = { stacks: e.stacks ?? 1, until };
    frost.set(e.target, state);
    const pos = new THREE.Vector3();
    let acc = 0;
    let t = 0;
    rt.add({
      update(dt) {
        t += dt;
        acc += dt;
        if (!rt.entities.pos(e.target, pos) || rt.time > state.until || state.stacks <= 0) return false;
        if (acc < 0.05) return true;
        acc = 0;
        for (let i = 0; i < state.stacks; i++) {
          const a = t * 2.5 + (i / state.stacks) * Math.PI * 2;
          _v.set(pos.x + Math.cos(a) * 0.55, 1.9, pos.z + Math.sin(a) * 0.55);
          rt.ps.emit(rt.s.snow, _v, { count: 1, shape: 'point', life: 0.12, size: 0.2, spin: 2, tint: ICE_HDR });
        }
        return true;
      },
      end() {
        if (frost.get(e.target) === state) frost.delete(e.target);
      },
    });
  },
  buffEnd(_rt, e) {
    if (e.buffId === 'ilyra_frost') {
      const s = frost.get(e.target);
      if (s) s.stacks = 0;
    }
  },
});

/** Proc du passif : la cible est figée dans la glace (étourdissement), puis la glace éclate. */
function icePrison(rt: FxRuntime, target: number, duration: number): void {
  const pos = new THREE.Vector3();
  if (!rt.entities.pos(target, pos)) return;
  const f = frost.get(target);
  if (f) f.stacks = 0; // les cumuls sont consommés
  const life = Math.max(0.6, Math.min(duration || 1, 2));
  const n = 7;
  for (let i = 0; i < n; i++) {
    const a = (i / n) * Math.PI * 2 + rng.range(-0.2, 0.2);
    _v.set(pos.x + Math.cos(a) * 0.38, 0, pos.z + Math.sin(a) * 0.38);
    rt.bursts.iceSpikes.spawn({ pos: _v, mode: 'static', size: rng.range(0.55, 0.75), life, heading: Math.atan2(Math.cos(a), Math.sin(a)) + Math.PI, tilt: -0.3, stretch: rng.range(1.6, 2.4) });
  }
  rt.bursts.iceSpikes.spawn({ pos: pos.clone(), mode: 'static', size: 0.6, life, stretch: 2.6 });
  groundMist(rt, pos, 1, MIST, 6);
  rt.after(life, () => shatter(rt, pos.clone().setY(1), 16, 1));
}

registerFx('ilyra_q', {
  cast(rt, e) {
    if (!rt.entities.pos(e.source, _v, 1.2)) return;
    gather(rt, _v, 10);
  },
  projectile(rt, e) {
    const from = rt.world(e.from);
    const to = rt.world(e.to);
    const shard = new THREE.Mesh(shardGeo, iceMaterial());
    shard.scale.set(0.8, 0.8, 2.6);
    rt.root.add(shard);
    let spin = 0;
    const p = new ProjectileFx(rt, from, to, rt.scale(e.speed), {
      color: FROST.clone().multiplyScalar(1.6),
      core: ICE_HDR,
      size: 0.55,
      height: 1.1,
      trailStyle: rt.s.snow,
      trailDensity: 6,
      trailSize: [0.1, 0.18],
      trailLife: [0.4, 0.7],
      ribbonWidth: 0.16,
      ribbonLife: 0.3,
      onStep(head, dir, dt) {
        spin += dt * 14;
        shard.position.copy(head);
        shard.lookAt(_v.copy(head).add(dir));
        shard.rotateZ(spin);
      },
      onEnd() {
        shard.removeFromParent();
      },
    });
    rt.add(p);
    return p;
  },
  hit(rt, e) {
    const at = rt.world(e.pos, 1);
    shatter(rt, at, 12, 0.8);
    flash(rt, at, 1.6, ICE_HDR, 0.2);
    shockwave(rt, at.setY(0), 0.9, 0.3, ICE_HDR, 0.08);
    groundMist(rt, at, 0.7, MIST, 4);
  },
});

registerFx('ilyra_w', {
  cast(rt, e) {
    if (!rt.entities.pos(e.source, _v, 1.4)) return;
    gather(rt, _v, 8);
  },
  /** Détonation de la nova. */
  area(rt, e) {
    nova(rt, rt.world(e.pos), rt.scale(e.radius));
  },
});

/** Pendant le délai : runes de givre qui se remplissent, flocons qui se figent au sol. */
registerFx('ilyra_w_telegraph', {
  area(rt, e) {
    const center = rt.world(e.pos);
    const r = rt.scale(e.radius);
    const delay = Math.max(0.15, e.duration);
    telegraph(rt, center, r, delay, FROST, { color2: ICE_HDR, fill: 0.16 });
    timed(rt, delay, () => {
      if (rng.next() < 0.6) rt.ps.emit(rt.s.snow, center, { count: 1, shape: 'disc', radius: r * 0.9, up: [0.1, 0.4], life: [0.4, 0.7], size: [0.12, 0.2], spin: [-2, 2], tint: ICE_HDR });
    });
  },
});

function nova(rt: FxRuntime, center: THREE.Vector3, r: number): void {
  shockwave(rt, center, r * 1.1, 0.4, ICE_HDR, 0.16);
  flash(rt, _v.set(center.x, 0.6, center.z), r * 1.2, FROST, 0.3);
  const n = 12;
  for (let i = 0; i < n; i++) {
    const a = (i / n) * Math.PI * 2 + rng.range(-0.12, 0.12);
    const d = r * rng.range(0.5, 0.85);
    _v.set(center.x + Math.cos(a) * d, 0, center.z + Math.sin(a) * d);
    rt.bursts.iceSpikes.spawn({ pos: _v, mode: 'spike', size: rng.range(0.4, 0.6), life: rng.range(0.8, 1.1), heading: Math.atan2(Math.cos(a), Math.sin(a)) + Math.PI, tilt: -0.45, stretch: rng.range(1.2, 1.8) });
  }
  shatter(rt, _v.set(center.x, 0.4, center.z), 14, 1.2);
  groundMist(rt, center, r, MIST, 12);
}

registerFx('ilyra_e', {
  /** Flux de givre vers l'allié protégé. */
  cast(rt, e) {
    const from = new THREE.Vector3();
    if (!rt.entities.pos(e.source, from, 1.2)) return;
    gather(rt, from, 6);
    if (e.targetId === undefined || e.targetId === e.source) return;
    const to = new THREE.Vector3();
    rt.entities.pos(e.targetId, to, 1.1);
    rt.add(
      new ProjectileFx(rt, from, to, 18, {
        color: FROST.clone().multiplyScalar(1.8),
        core: ICE_HDR,
        size: 0.4,
        height: 1.1,
        trailStyle: rt.s.sparkles,
        trailDensity: 5,
        trailSize: [0.2, 0.3],
        ribbonWidth: 0.22,
        ribbonLife: 0.35,
        targetId: e.targetId,
      }),
    );
  },
  /** Le bouclier lui-même (buff `ilyra_e`), jusqu'à expiration ou rupture. */
  buff(rt, e) {
    if (e.buffId !== 'ilyra_e') return;
    const orbit = new THREE.Vector3();
    shieldShell(rt, e.target, Math.max(0.3, e.duration), `ilyra_e:${e.target}`, {
      color: new THREE.Color(0.5, 0.95, 1.4),
      radius: 1.05,
      bands: 0.25,
      onStep(center, t) {
        for (let i = 0; i < 3; i++) {
          const a = t * 2.4 + (i / 3) * Math.PI * 2;
          orbit.set(center.x + Math.cos(a) * 1.05, center.y + Math.sin(t * 3 + i) * 0.3, center.z + Math.sin(a) * 1.05);
          rt.ps.emit(rt.s.snow, orbit, { count: 1, shape: 'point', life: 0.25, size: 0.18, spin: 3, tint: ICE_HDR });
        }
      },
      onEnd(center) {
        shatter(rt, center, 10, 0.7);
      },
    });
    rt.entities.pos(e.target, _v, 1);
    flash(rt, _v, 2, ICE_HDR, 0.25);
  },
  buffEnd(rt, e) {
    if (e.buffId === 'ilyra_e') rt.endAura(`ilyra_e:${e.target}`);
  },
});

registerFx('ilyra_r', {
  cast(rt, e) {
    if (!rt.entities.pos(e.source, _v, 2.4)) return;
    gather(rt, _v, 16);
    flash(rt, _v, 2.4, ICE_HDR, 0.4);
  },
  area(rt, e) {
    const center = rt.world(e.pos);
    const r = rt.scale(e.radius);
    const duration = Math.max(0.5, e.duration);
    zone(rt, center, r, duration, FROST.clone().multiplyScalar(1.3), { fill: 0.14, pattern: 0.7 });
    groundMist(rt, center, r, MIST, 14);
    const wind = new THREE.Vector3();
    let acc = 0;
    let shardAcc = 0;
    timed(rt, duration, (t, dt) => {
      const fade = Math.min(1, (1 - t) * duration * 2);
      // neige tournoyante : vitesse tangentielle selon la position (tourbillon)
      acc += dt * 90 * fade;
      while (acc >= 1) {
        acc -= 1;
        const a = rng.next() * Math.PI * 2;
        const d = Math.sqrt(rng.next()) * r;
        _v.set(center.x + Math.cos(a) * d, rng.range(0.2, 2.2), center.z + Math.sin(a) * d);
        wind.set(-Math.sin(a) * 3.5, -0.6, Math.cos(a) * 3.5);
        rt.ps.emit(rng.next() < 0.75 ? rt.s.snow : rt.s.spark, _v, { count: 1, shape: 'point', life: [0.5, 0.9], size: [0.1, 0.2], inherit: wind, spin: [-5, 5], stretch: 0.03, tint: ICE_HDR, intensity: 0.8 });
      }
      // éclats de glace qui tombent du ciel
      shardAcc += dt * 7 * fade;
      while (shardAcc >= 1) {
        shardAcc -= 1;
        const a = rng.next() * Math.PI * 2;
        const d = Math.sqrt(rng.next()) * r * 0.9;
        _v.set(center.x + Math.cos(a) * d, rng.range(3, 4.5), center.z + Math.sin(a) * d);
        rt.bursts.iceDebris.spawn({ pos: _v, mode: 'debris', size: rng.range(0.16, 0.26), life: 1.1, vel: _w.set(rng.range(-0.5, 0.5), -6, rng.range(-0.5, 0.5)) });
      }
      if (rng.next() < dt * 3) groundMist(rt, center, r * 0.8, MIST, 2);
    });
  },
});

