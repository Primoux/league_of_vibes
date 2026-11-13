/**
 * FX de Grondak, le Rempart de Granit : pierre, poussière, fissures de magma ambré.
 *  P  Peau de granit  — coque ambrée + plaques de roche en orbite tant que le bouclier tient
 *  Q  Poing sismique  — fissure qui court au sol, pics de roche jaillissants, impact poussiéreux
 *  W  Rempart         — rugissement (ondes de provocation) + armure de pierre pendant 3 s
 *  E  Charge de roche — ruée dans la poussière, impact étourdissant
 *  R  Effondrement    — sol qui se fissure (télégraphie), éruption de pics, débris projetés
 */
import * as THREE from 'three';
import { getChampion } from '@lov/shared';
import { shieldShell } from '../effects/auras.ts';
import { ProjectileFx, dashTrail, flash, shockwave, telegraph, timed } from '../effects/primitives.ts';
import { rng } from '../core/rng.ts';
import type { FxColors, FxRuntime } from '../core/FxRuntime.ts';
import { registerFx } from '../registry.ts';

const GRONDAK = getChampion('grondak');
const STONE = [new THREE.Color('#7a6a58'), new THREE.Color('#6b5d4d'), new THREE.Color('#8c7b66'), new THREE.Color('#5a5047')];
const stone = (): THREE.Color => STONE[Math.floor(rng.next() * STONE.length)]!;
const _v = new THREE.Vector3();
const _w = new THREE.Vector3();

/** Gerbe de débris de roche autour de `at`. */
function rockBurst(rt: FxRuntime, at: THREE.Vector3, n: number, power: number, size: [number, number]): void {
  for (let i = 0; i < n; i++) {
    const a = rng.next() * Math.PI * 2;
    const h = rng.range(0.3, 1) * power;
    rt.bursts.rockDebris.spawn({
      pos: _v.set(at.x + Math.cos(a) * 0.2, at.y + 0.2, at.z + Math.sin(a) * 0.2),
      mode: 'debris',
      size: rng.range(size[0], size[1]),
      life: rng.range(0.9, 1.4),
      vel: _w.set(Math.cos(a) * rng.range(1, 3) * power * 0.5, h * 5, Math.sin(a) * rng.range(1, 3) * power * 0.5),
      color: stone(),
    });
  }
}

/** Pic de roche qui jaillit du sol. */
function spike(rt: FxRuntime, at: THREE.Vector3, size: number, life: number, heading = rng.next() * 6.28, tilt = rng.range(-0.2, 0.2), delay = 0): void {
  rt.bursts.rockSpikes.spawn({ pos: at, mode: 'spike', size, life, heading, tilt, stretch: rng.range(1.6, 2.4), color: stone(), delay });
}

function dustRing(rt: FxRuntime, at: THREE.Vector3, radius: number, n = 10): void {
  rt.ps.emit(rt.s.dust, at, { count: n, shape: 'ring', radius, speed: [1, 2.5], up: [0.2, 0.8], life: [0.7, 1.2], size: [0.7, 1.4], drag: 2 });
}

function magmaSparks(rt: FxRuntime, at: THREE.Vector3, c: FxColors, n: number, up: [number, number] = [2, 5]): void {
  rt.ps.emit(rt.s.spark, at, { count: n, shape: 'disc', radius: 0.4, speed: [1, 3], up, life: [0.4, 0.8], size: [0.05, 0.1], stretch: 0.06, gravity: 9, tint: c.accentHdr });
}

registerFx('grondak_p', {
  buff(rt, e, c) {
    const plates: THREE.Vector3[] = [new THREE.Vector3(), new THREE.Vector3(), new THREE.Vector3()];
    let acc = 0;
    shieldShell(rt, e.target, e.duration > 0 ? e.duration : 30, `grondak_p:${e.target}`, {
      color: new THREE.Color(1.3, 0.8, 0.3),
      radius: 1.15,
      height: 1.05,
      bands: 0.25,
      onStep(center, t, dt) {
        acc += dt;
        if (acc < 0.06) return;
        acc = 0;
        // éclats de roche lumineux en orbite (particules courtes = traînée)
        plates.forEach((p, i) => {
          const a = t * 1.6 + (i / 3) * Math.PI * 2;
          p.set(center.x + Math.cos(a) * 1.05, center.y - 0.2 + Math.sin(t * 2 + i) * 0.25, center.z + Math.sin(a) * 1.05);
          rt.ps.emit(rt.s.ember, p, { count: 1, shape: 'point', life: 0.35, size: 0.16, tint: c.accentHdr });
        });
      },
      onEnd(center) {
        rockBurst(rt, center, 8, 0.6, [0.12, 0.22]);
        rt.ps.emit(rt.s.dust, center, { count: 6, shape: 'sphere', radius: 0.6, speed: [0.5, 1.5], life: [0.6, 0.9], size: [0.6, 1] });
      },
    });
    rt.entities.pos(e.target, _v, 1);
    flash(rt, _v, 2.4, c.accentHdr, 0.3);
  },
  buffEnd(rt, e) {
    rt.endAura(`grondak_p:${e.target}`);
  },
});

registerFx('grondak_q', {
  cast(rt, e, c) {
    if (!rt.entities.pos(e.source, _v)) return;
    // le poing frappe le sol devant lui (≈ 0,25 s dans l'animation)
    const target = e.targetPos ? rt.world(e.targetPos) : _v.clone();
    const dir = target.sub(_v).setY(0).normalize();
    const at = _v.clone().addScaledVector(dir, 0.7);
    rt.after(0.22, () => {
      dustRing(rt, at, 0.4, 8);
      shockwave(rt, at, 1.2, 0.35, c.accentHdr, 0.12);
      rockBurst(rt, at, 5, 0.6, [0.1, 0.18]);
    });
  },
  projectile(rt, e, c) {
    // fissure qui court au sol : pas de projectile visible, des pics jaillissent sur son passage
    const from = rt.world(e.from);
    const to = rt.world(e.to);
    const crack = rt.acquireDecal().setColor(c.accent.clone().multiplyScalar(0.9));
    crack.u.uFill.value = 0.3;
    crack.u.uEdge.value = 0.05;
    crack.u.uPattern.value = 0.8;
    const width = rt.scale(e.width) * 0.28;
    let since = 0;
    let side = 1;
    const p = new ProjectileFx(rt, from, to, rt.scale(e.speed), {
      color: c.accentHdr,
      size: 0,
      height: 0,
      ribbonWidth: 0,
      onStep(head, dir, dt) {
        crack.rect(from, head, width);
        since += rt.scale(e.speed) * dt;
        while (since > 0.42) {
          since -= 0.42;
          side = -side;
          _w.set(-dir.z, 0, dir.x).multiplyScalar(side * rng.range(0.05, 0.3));
          _v.copy(head).add(_w);
          spike(rt, _v, rng.range(0.32, 0.5), 0.9, Math.atan2(dir.x, dir.z), side * 0.35);
          rt.ps.emit(rt.s.dust, _v, { count: 2, shape: 'disc', radius: 0.2, up: [0.4, 1], speed: [0.3, 0.8], life: [0.6, 1], size: [0.6, 1] });
          magmaSparks(rt, _v, c, 2, [1, 3]);
        }
      },
      onEnd() {
        timed(rt, 0.8, (t) => (crack.u.uOpacity.value = 1 - t), () => rt.releaseDecal(crack));
      },
    });
    rt.add(p);
    return p;
  },
  hit(rt, e, c) {
    const at = rt.world(e.pos);
    rockBurst(rt, at, 7, 0.8, [0.1, 0.2]);
    rt.ps.emit(rt.s.dust, at, { count: 8, shape: 'sphere', radius: 0.4, speed: [1, 2], up: [0.5, 1.2], life: [0.6, 1], size: [0.7, 1.2], drag: 2 });
    flash(rt, _v.set(at.x, 0.9, at.z), 1.6, c.accentHdr, 0.2);
  },
});

registerFx('grondak_w', {
  /** Rugissement : ondes de provocation autour de Grondak (rayon du sort). */
  cast(rt, e, c) {
    const center = new THREE.Vector3();
    if (!rt.entities.pos(e.source, center)) return;
    const r = rt.scale(GRONDAK.abilities.W.radius ?? 325);
    flash(rt, _v.set(center.x, 1.6, center.z), 2.6, c.accentHdr, 0.35);
    for (let i = 0; i < 3; i++) rt.after(i * 0.12, () => shockwave(rt, center, r * (1 - i * 0.15), 0.5, c.accentHdr, 0.14));
    dustRing(rt, center, r * 0.6, 14);
    rockBurst(rt, center, 6, 0.5, [0.1, 0.16]);
  },
  /** Buff `grondak_w` (réduction de dégâts) : armure de pierre. Les `taunt` posés par le sort passent par les statuts. */
  buff(rt, e) {
    if (e.buffId !== 'grondak_w') return;
    let dustAcc = 0;
    shieldShell(rt, e.target, Math.max(0.5, e.duration), `grondak_w:${e.target}`, {
      color: new THREE.Color(0.9, 0.6, 0.3),
      radius: 1.2,
      height: 1.05,
      bands: 0.45,
      power: 1.6,
      onStep(cen, _t, dt) {
        dustAcc += dt * 12; // ~12 bouffées de poussière par seconde
        if (dustAcc < 1) return;
        dustAcc -= 1;
        rt.ps.emit(rt.s.dust, _w.set(cen.x, 0.1, cen.z), { count: 1, shape: 'ring', radius: 0.8, up: [0.3, 0.8], life: [0.5, 0.8], size: [0.4, 0.7] });
      },
    });
  },
  buffEnd(rt, e) {
    if (e.buffId === 'grondak_w') rt.endAura(`grondak_w:${e.target}`);
  },
});

registerFx('grondak_e', {
  /** Charge : poussière tout le long, impact à l'arrivée (l'étourdissement est un statut générique). */
  dash(rt, e, c) {
    const from = rt.world(e.from);
    const to = rt.world(e.to);
    const duration = Math.max(0.05, e.duration);
    dashTrail(rt, e.source, from, to, duration, c.accent.clone().multiplyScalar(0.7));
    rockBurst(rt, from, 5, 0.5, [0.1, 0.18]);
    const pos = new THREE.Vector3();
    timed(rt, duration, () => {
      if (!rt.entities.pos(e.source, pos)) return;
      rt.ps.emit(rt.s.dust, pos, { count: 2, shape: 'disc', radius: 0.4, up: [0.3, 1], speed: [0.5, 1.5], life: [0.6, 1], size: [0.8, 1.3], drag: 2 });
      magmaSparks(rt, pos.setY(0.5), c, 1, [0.5, 2]);
    });
    rt.after(duration, () => {
      if (!rt.entities.pos(e.source, pos)) pos.copy(to);
      shockwave(rt, pos, 1.6, 0.4, c.accentHdr, 0.14);
      rockBurst(rt, pos, 10, 1, [0.12, 0.24]);
      dustRing(rt, pos, 0.6, 12);
      flash(rt, _v.set(pos.x, 1.1, pos.z), 2.4, c.accentHdr, 0.25);
    });
  },
});

registerFx('grondak_r', {
  cast(rt, e, c) {
    if (!rt.entities.pos(e.source, _v, 2.2)) return;
    flash(rt, _v, 3, c.accentHdr, 0.5);
    magmaSparks(rt, _v.setY(1.5), c, 12, [1, 3]);
  },
  /** Détonation (0,25 s) après la télégraphie. */
  area(rt, e, c) {
    erupt(rt, rt.world(e.pos), rt.scale(e.radius), c);
  },
});

/** Télégraphie : le sol tremble, petits cailloux qui sautent, fissures qui s'illuminent. */
registerFx('grondak_r_telegraph', {
  area(rt, e, c) {
    const center = rt.world(e.pos);
    const r = rt.scale(e.radius);
    const delay = Math.max(0.2, e.duration);
    telegraph(rt, center, r, delay, c.accent.clone().multiplyScalar(0.8), { color2: c.accentHdr, fill: 0.1 });
    timed(rt, delay, (t) => {
      if (rng.next() < 0.5) {
        const a = rng.next() * Math.PI * 2;
        const d = Math.sqrt(rng.next()) * r;
        _v.set(center.x + Math.cos(a) * d, 0.05, center.z + Math.sin(a) * d);
        rt.bursts.rockDebris.spawn({ pos: _v, mode: 'debris', size: rng.range(0.06, 0.12), life: 0.5, vel: _w.set(0, rng.range(1.5, 3) * (0.5 + t), 0), color: stone() });
        if (rng.next() < 0.4) magmaSparks(rt, _v, c, 1, [0.5, 1.5]);
      }
    });
  },
});

/** Éruption de l'ultime : anneau de grands pics, débris projetés en l'air, nuage de poussière. */
function erupt(rt: FxRuntime, center: THREE.Vector3, r: number, c: FxColors): void {
  flash(rt, _v.set(center.x, 0.5, center.z), r * 1.4, c.accentHdr, 0.35);
  shockwave(rt, center, r * 1.15, 0.6, c.accentHdr, 0.25);
  const n = 14;
  for (let i = 0; i < n; i++) {
    const a = (i / n) * Math.PI * 2 + rng.range(-0.1, 0.1);
    const d = r * rng.range(0.55, 0.9);
    _v.set(center.x + Math.cos(a) * d, 0, center.z + Math.sin(a) * d);
    // inclinés vers l'extérieur
    spike(rt, _v, rng.range(0.55, 0.85), rng.range(1.2, 1.6), Math.atan2(Math.cos(a), Math.sin(a)) + Math.PI, -0.35, rng.range(0, 0.08));
  }
  for (let i = 0; i < 5; i++) {
    const a = rng.next() * Math.PI * 2;
    _v.set(center.x + Math.cos(a) * r * 0.25, 0, center.z + Math.sin(a) * r * 0.25);
    spike(rt, _v, rng.range(0.45, 0.7), 1.3, undefined, undefined, 0.05);
  }
  rockBurst(rt, center, 24, 1.6, [0.12, 0.3]);
  rt.ps.emit(rt.s.dust, center, { count: 26, shape: 'disc', radius: r * 0.9, speed: [0.5, 2], up: [0.5, 2], life: [1, 1.8], size: [1, 2], drag: 1.5 });
  magmaSparks(rt, center, c, 30, [3, 8]);
}
