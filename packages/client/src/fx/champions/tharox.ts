/**
 * FX de Tharox, la Charge Sauvage (événements : server/src/abilities/champions/tharox.ts).
 *  P  Élan          buff slow (fxKey tharox_p) sur la cible de l'attaque renforcée → impact de défenses
 *  Q  Défenses      dash tharox_q, puis dash tharox_q_knockback sur la cible repoussée
 *  W  Écorce de fer buff tharox_w (résistances) + zone tharox_w qui suit Tharox (dégâts par seconde)
 *  E  Piétinement   abilityCast + dégâts tharox_e (aucune zone émise)
 *  R  Ruée sauvage  abilityCast (1 s de préparation) puis dash tharox_r
 */
import * as THREE from 'three';
import { getChampion } from '@lov/shared';
import { shieldShell } from '../effects/auras.ts';
import { dashTrail, flash, shockwave, timed } from '../effects/primitives.ts';
import type { FxColors, FxRuntime } from '../core/FxRuntime.ts';
import { rng } from '../core/rng.ts';
import { onAbilityHit, registerFx } from '../registry.ts';

const THAROX = getChampion('tharox');
const EMBER = new THREE.Color(2.6, 0.8, 0.3);
const RUST = new THREE.Color(1.2, 0.55, 0.25);
const _v = new THREE.Vector3();
const _w = new THREE.Vector3();

function dust(rt: FxRuntime, at: THREE.Vector3, n: number, r = 0.4): void {
  rt.ps.emit(rt.s.dust, at, { count: n, shape: 'disc', radius: r, up: [0.3, 1], speed: [0.5, 1.5], life: [0.6, 1], size: [0.7, 1.2], drag: 2 });
}
function embers(rt: FxRuntime, at: THREE.Vector3, n: number): void {
  rt.ps.emit(rt.s.ember, at, { count: n, shape: 'sphere', radius: 0.3, up: [1, 2.5], speed: [0.5, 1.5], life: [0.4, 0.8], size: [0.06, 0.12], gravity: 2, tint: EMBER });
}
/** Charge : traînée de poussière + braises tout le long. */
function charge(rt: FxRuntime, source: number, from: THREE.Vector3, to: THREE.Vector3, duration: number, c: FxColors, big: boolean): void {
  dashTrail(rt, source, from, to, duration, (big ? EMBER : c.accent).clone().multiplyScalar(big ? 1 : 0.6));
  const pos = new THREE.Vector3();
  let acc = 0;
  timed(rt, Math.max(0.05, duration), (_t, dt) => {
    acc += dt;
    if (acc < 0.03 || !rt.entities.pos(source, pos)) return;
    acc = 0;
    dust(rt, pos, big ? 3 : 2, 0.5);
    embers(rt, pos.setY(0.8), big ? 3 : 1);
  });
}

registerFx('tharox_p', {
  buff(rt, e) {
    if (!rt.entities.pos(e.target, _v, 1)) return;
    flash(rt, _v, 2, EMBER, 0.2);
    rt.ps.emit(rt.s.spark, _v, { count: 14, shape: 'point', speed: [3, 6], life: [0.2, 0.35], size: [0.06, 0.1], stretch: 0.05, gravity: 6, tint: EMBER });
    shockwave(rt, _v.setY(0), 1.2, 0.3, EMBER, 0.1);
  },
});

registerFx('tharox_q', {
  dash(rt, e, c) {
    charge(rt, e.source, rt.world(e.from), rt.world(e.to), e.duration, c, false);
  },
});
/** La cible repoussée glisse dans la poussière. */
registerFx('tharox_q_knockback', {
  dash(rt, e) {
    const from = rt.world(e.from);
    flash(rt, _v.copy(from).setY(1), 2, EMBER, 0.2);
    dust(rt, from, 8, 0.4);
    const pos = new THREE.Vector3();
    timed(rt, Math.max(0.05, e.duration), () => {
      if (rt.entities.pos(e.source, pos)) dust(rt, pos, 2, 0.3);
    });
  },
});

registerFx('tharox_w', {
  buff(rt, e) {
    if (e.buffId !== 'tharox_w') return;
    shieldShell(rt, e.target, Math.max(0.5, e.duration), `tharox_w:${e.target}`, { color: RUST, radius: 1.2, height: 1.05, bands: 0.5, power: 1.4 });
  },
  buffEnd(rt, e) {
    if (e.buffId === 'tharox_w') rt.endAura(`tharox_w:${e.target}`);
  },
  /** Zone qui suit Tharox : anneau de braises qui brûle autour de lui. */
  area(rt, e) {
    const r = rt.scale(e.radius);
    const d = rt.acquireDecal().setColor(EMBER.clone().multiplyScalar(0.6));
    d.u.uFill.value = 0.06;
    d.u.uPattern.value = 0.8;
    d.u.uEdge.value = 0.1;
    const pos = new THREE.Vector3();
    timed(
      rt,
      Math.max(0.3, e.duration),
      (t) => {
        if (!rt.entities.pos(e.source, pos)) return;
        d.circle(pos, r);
        d.u.uOpacity.value = Math.min(1, t * 8, (1 - t) * 6);
        if (rng.next() < 0.6) rt.ps.emit(rt.s.ember, pos, { count: 1, shape: 'ring', radius: r * 0.9, up: [0.6, 1.4], life: [0.4, 0.7], size: [0.06, 0.12], tint: EMBER });
      },
      () => rt.releaseDecal(d),
    );
  },
});
onAbilityHit('tharox_w', (rt, e) => {
  if (rt.entities.pos(e.target, _v, 0.8)) embers(rt, _v, 4);
});

registerFx('tharox_e', {
  /** Le sabot s'abat (~0,25 s dans l'animation) : onde, poussière, cailloux. */
  cast(rt, e, c) {
    const pos = new THREE.Vector3();
    if (!rt.entities.pos(e.source, pos)) return;
    const r = rt.scale(THAROX.abilities.E.radius ?? 375);
    rt.after(0.25, () => {
      rt.entities.pos(e.source, pos);
      shockwave(rt, pos, r, 0.45, c.accentHdr, 0.2);
      rt.ps.emit(rt.s.dust, pos, { count: 18, shape: 'ring', radius: r * 0.5, speed: [1.5, 3], up: [0.2, 0.8], life: [0.6, 1], size: [0.8, 1.4], drag: 2 });
      for (let i = 0; i < 8; i++) {
        const a = rng.next() * Math.PI * 2;
        rt.bursts.rockDebris.spawn({ pos: _v.set(pos.x + Math.cos(a) * 0.5, 0.2, pos.z + Math.sin(a) * 0.5), mode: 'debris', size: rng.range(0.08, 0.15), life: 0.9, vel: _w.set(Math.cos(a) * 2, rng.range(3, 5), Math.sin(a) * 2), color: new THREE.Color('#6b5d4d') });
      }
    });
  },
});
onAbilityHit('tharox_e', (rt, e) => {
  if (rt.entities.pos(e.target, _v)) dust(rt, _v, 4, 0.3);
});

registerFx('tharox_r', {
  /** Préparation : il gratte le sol, vapeur et braises. */
  cast(rt, e) {
    const pos = new THREE.Vector3();
    timed(rt, 1, () => {
      if (!rt.entities.pos(e.source, pos)) return;
      dust(rt, pos, 1, 0.5);
      if (rng.next() < 0.5) rt.ps.emit(rt.s.smoke, pos.setY(1.6), { count: 1, shape: 'point', up: [0.5, 1], speed: [0.2, 0.5], life: [0.5, 0.8], size: [0.3, 0.5] });
    });
  },
  dash(rt, e, c) {
    charge(rt, e.source, rt.world(e.from), rt.world(e.to), e.duration, c, true);
  },
});
onAbilityHit('tharox_r', (rt, e) => {
  if (!rt.entities.pos(e.target, _v, 1)) return;
  flash(rt, _v, 2.8, EMBER, 0.25);
  shockwave(rt, _v.setY(0), 1.8, 0.4, EMBER, 0.14);
  rt.shake(_v, 25, 0.3);
  embers(rt, _v.setY(1), 12);
});
