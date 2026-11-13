/**
 * FX de Sable, la Tireuse des Dunes (événements : server/src/abilities/champions/sable.ts, passives.ts).
 *  P  Œil du désert      buff sable_p_speed (vitesse après un coup sur un champion)
 *  Q  Tir explosif       buff sable_q (arme chargée) ; zone sable_q_explosion à l'impact de l'attaque
 *  W  Volée              7 projectiles sable_w en cône
 *  E  Roulade            dash sable_e + buff sable_e (critique garanti)
 *  R  Balle du jugement  abilityCast (1 s de visée) puis projectile sable_r global
 */
import * as THREE from 'three';
import { ProjectileFx, flash, shockwave, timed } from '../effects/primitives.ts';
import type { FxRuntime } from '../core/FxRuntime.ts';
import { registerFx, type Ev } from '../registry.ts';

const FIRE = new THREE.Color(2.8, 1.2, 0.3);
const MUZZLE = new THREE.Color(3, 2.2, 1);
const LASER = new THREE.Color(2.6, 0.3, 0.2);
const _v = new THREE.Vector3();

/** Lueur persistante sur l'arme (tant que le buff tient). */
function charged(rt: FxRuntime, e: Ev<'buffApplied'>, color: THREE.Color): void {
  const key = `${e.buffId}:${e.target}`;
  let ended = false;
  const cancel = (): void => {
    ended = true;
  };
  rt.setAura(key, cancel);
  const pos = new THREE.Vector3();
  const until = rt.time + Math.max(0.3, e.duration);
  let acc = 0;
  rt.add({
    update(dt) {
      acc += dt;
      if (ended || rt.time > until || !rt.entities.pos(e.target, pos, 1.2)) return false;
      if (acc > 0.05) {
        acc = 0;
        rt.ps.emit(rt.s.ember, pos, { count: 1, shape: 'sphere', radius: 0.35, up: [0.3, 0.8], life: [0.3, 0.5], size: [0.06, 0.1], tint: color });
      }
      return true;
    },
    end: () => rt.forgetAura(key, cancel),
  });
}

function muzzleFlash(rt: FxRuntime, source: number, toward: THREE.Vector3 | null, scale: number): void {
  if (!rt.entities.pos(source, _v, 1.25)) return;
  if (toward) _v.addScaledVector(toward.clone().setY(1.25).sub(_v).setY(0).normalize(), 0.7);
  flash(rt, _v, 1.4 * scale, MUZZLE, 0.12);
  rt.ps.emit(rt.s.smoke, _v, { count: Math.round(3 * scale), shape: 'sphere', radius: 0.1, up: [0.3, 0.6], speed: [0.2, 0.6], life: [0.5, 0.8], size: [0.3, 0.5] });
}

function tracer(rt: FxRuntime, e: Ev<'projectileSpawn'>, size: number, ribbon: number, life: number): ProjectileFx {
  const p = new ProjectileFx(rt, rt.world(e.from), rt.world(e.to), rt.scale(e.speed), {
    color: FIRE,
    core: MUZZLE,
    size,
    height: 1.2,
    ribbonWidth: ribbon,
    ribbonLife: life,
  });
  rt.add(p);
  return p;
}

registerFx('sable_p', {
  buff(rt, e) {
    const pos = new THREE.Vector3();
    let acc = 0;
    timed(rt, Math.max(0.3, e.duration), (_t, dt) => {
      acc += dt;
      if (acc < 0.06 || !rt.entities.pos(e.target, pos)) return;
      acc = 0;
      rt.ps.emit(rt.s.dust, pos, { count: 1, shape: 'disc', radius: 0.25, up: [0.2, 0.5], life: [0.4, 0.6], size: [0.4, 0.6] });
    });
  },
});

registerFx('sable_q', {
  buff(rt, e) {
    if (e.buffId === 'sable_q') charged(rt, e, FIRE);
  },
  buffEnd(rt, e) {
    if (e.buffId === 'sable_q') rt.endAura(`sable_q:${e.target}`);
  },
});
registerFx('sable_q_explosion', {
  area(rt, e) {
    const center = rt.world(e.pos);
    const r = rt.scale(e.radius);
    flash(rt, _v.set(center.x, 0.8, center.z), r * 1.2, FIRE, 0.22);
    shockwave(rt, center, r * 1.1, 0.35, FIRE, 0.14);
    rt.ps.emit(rt.s.flame, center, { count: 16, shape: 'disc', radius: r * 0.6, up: [1, 2.5], speed: [0.5, 1.5], life: [0.35, 0.6], size: [0.4, 0.7], tint: FIRE });
    rt.ps.emit(rt.s.smoke, center, { count: 8, shape: 'disc', radius: r * 0.5, up: [0.5, 1.2], life: [0.8, 1.2], size: [0.6, 1] });
  },
});

registerFx('sable_w', {
  cast(rt, e) {
    muzzleFlash(rt, e.source, e.targetPos ? rt.world(e.targetPos) : null, 1.6);
  },
  projectile(rt, e) {
    return tracer(rt, e, 0.22, 0.07, 0.12);
  },
  hit(rt, e) {
    const at = rt.world(e.pos, 1);
    rt.ps.emit(rt.s.spark, at, { count: 6, shape: 'point', speed: [2, 4], life: [0.12, 0.2], size: [0.04, 0.07], stretch: 0.04, tint: FIRE });
  },
});

registerFx('sable_e', {
  dash(rt, e) {
    const pos = new THREE.Vector3();
    timed(rt, Math.max(0.05, e.duration), () => {
      if (rt.entities.pos(e.source, pos)) rt.ps.emit(rt.s.dust, pos, { count: 2, shape: 'disc', radius: 0.3, up: [0.3, 0.8], speed: [0.3, 1], life: [0.5, 0.8], size: [0.6, 1] });
    });
  },
  buff(rt, e) {
    if (e.buffId === 'sable_e') charged(rt, e, MUZZLE);
  },
  buffEnd(rt, e) {
    if (e.buffId === 'sable_e') rt.endAura(`sable_e:${e.target}`);
  },
});

registerFx('sable_r', {
  /** Visée : un laser rouge se stabilise vers la cible pendant 1 s. */
  cast(rt, e) {
    if (!e.targetPos) return;
    const from = new THREE.Vector3();
    const to = rt.world(e.targetPos);
    const d = rt.acquireDecal().setColor(LASER);
    d.u.uFill.value = 0.8;
    d.u.uEdge.value = 0.02;
    timed(
      rt,
      1,
      (t) => {
        if (!rt.entities.pos(e.source, from)) return;
        const dir = to.clone().sub(from).setY(0).normalize();
        d.rect(from, _v.copy(from).addScaledVector(dir, 30), 0.05 + (1 - t) * 0.1);
        d.u.uOpacity.value = 0.3 + 0.7 * t * (0.7 + 0.3 * Math.sin(t * 40));
      },
      () => rt.releaseDecal(d),
    );
  },
  projectile(rt, e) {
    muzzleFlash(rt, e.source, rt.world(e.to), 1.8);
    return tracer(rt, e, 0.9, 0.35, 0.6);
  },
  hit(rt, e) {
    const at = rt.world(e.pos, 1);
    flash(rt, at, 3, FIRE, 0.3);
    shockwave(rt, at.clone().setY(0), 2, 0.4, FIRE, 0.14);
    rt.shake(at, 20, 0.25);
    rt.ps.emit(rt.s.spark, at, { count: 30, shape: 'point', speed: [4, 9], life: [0.25, 0.45], size: [0.07, 0.12], stretch: 0.06, gravity: 6, tint: FIRE });
    for (let i = 0; i < 3; i++) rt.after(i * 0.05, () => rt.ps.emit(rt.s.smoke, at, { count: 3, shape: 'sphere', radius: 0.3, up: [0.4, 1], life: [0.8, 1.2], size: [0.5, 0.9] }));
  },
});
