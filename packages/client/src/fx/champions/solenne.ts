/**
 * FX de Solenne, la Prêtresse de l'Aube (événements : server/src/abilities/champions/solenne.ts).
 *  Q  Lien lumineux  projectile solenne_q ; root (statut générique, doré)
 *  W  Bénédiction    abilityCast (targetId) puis soins (événements heal, FX génériques)
 *  E  Bouclier sacré buff de bouclier solenne_e + buff solenne_e_speed
 *  R  Aube           solenne_r_telegraph (0,5 s) puis solenne_r : soins + boucliers solenne_r
 */
import * as THREE from 'three';
import { shieldShell } from '../effects/auras.ts';
import { ProjectileFx, flash, shockwave, telegraph, timed } from '../effects/primitives.ts';
import { rng } from '../core/rng.ts';
import { registerFx } from '../registry.ts';

const DAWN = new THREE.Color(2.8, 2.3, 1.2);
const SKY = new THREE.Color(1.0, 2.3, 2.6);
const _v = new THREE.Vector3();

registerFx('solenne_q', {
  projectile(rt, e) {
    const p = new ProjectileFx(rt, rt.world(e.from), rt.world(e.to), rt.scale(e.speed), {
      color: DAWN,
      core: new THREE.Color(3, 3, 3),
      size: 0.6,
      height: 1.1,
      ribbonWidth: 0.35,
      ribbonLife: 0.35,
      trailStyle: rt.s.sparkles,
      trailDensity: 3,
      trailSize: [0.25, 0.4],
    });
    rt.add(p);
    return p;
  },
  hit(rt, e) {
    const at = rt.world(e.pos, 1);
    flash(rt, at, 2, DAWN, 0.2);
    rt.ps.emit(rt.s.sparkles, at, { count: 10, shape: 'sphere', radius: 0.4, speed: [1, 2], life: [0.4, 0.7], size: [0.3, 0.45], tint: DAWN });
  },
});

registerFx('solenne_w', {
  /** Rayon de bénédiction de Solenne vers l'allié. */
  cast(rt, e) {
    const from = new THREE.Vector3();
    if (!rt.entities.pos(e.source, from, 1.2)) return;
    rt.ps.emit(rt.s.sparkles, from, { count: 8, shape: 'sphere', radius: 0.5, life: [0.4, 0.6], size: [0.3, 0.4], tint: DAWN });
    if (e.targetId === undefined || e.targetId === e.source) return;
    const to = new THREE.Vector3();
    rt.entities.pos(e.targetId, to, 1.1);
    rt.add(new ProjectileFx(rt, from, to, 22, { color: DAWN, core: SKY, size: 0.4, height: 1.1, ribbonWidth: 0.25, ribbonLife: 0.4, targetId: e.targetId }));
  },
});

registerFx('solenne_e', {
  buff(rt, e) {
    if (e.buffId === 'solenne_e') {
      shieldShell(rt, e.target, Math.max(0.3, e.duration), `solenne_e:${e.target}`, { color: new THREE.Color(1.6, 1.3, 0.6), radius: 1.1, bands: 0.3 });
      if (rt.entities.pos(e.target, _v, 1)) flash(rt, _v, 2.2, DAWN, 0.25);
    } else if (e.buffId === 'solenne_e_speed') {
      const pos = new THREE.Vector3();
      timed(rt, Math.max(0.3, e.duration), () => {
        if (rng.next() < 0.3 && rt.entities.pos(e.target, pos, 1)) rt.ps.emit(rt.s.sparkles, pos, { count: 1, shape: 'sphere', radius: 0.5, life: 0.4, size: 0.3, tint: SKY });
      });
    }
  },
  buffEnd(rt, e) {
    if (e.buffId === 'solenne_e') rt.endAura(`solenne_e:${e.target}`);
  },
});

registerFx('solenne_r', {
  /** Colonne de lumière : grand faisceau vertical, anneau solaire, pluie d'étincelles. */
  area(rt, e) {
    const center = rt.world(e.pos);
    const r = rt.scale(e.radius);
    flash(rt, _v.set(center.x, 2, center.z), r * 0.8, DAWN, 0.45);
    shockwave(rt, center, r * 1.1, 0.6, DAWN, 0.25);
    timed(rt, 0.7, (t) => {
      const k = 1 - t;
      rt.ps.emit(rt.s.spark, center, { count: 6, shape: 'disc', radius: r * 0.5 * k + 0.3, up: [10, 16], life: [0.3, 0.5], size: [0.1, 0.18], stretch: 0.08, tint: DAWN });
      rt.ps.emit(rt.s.glow, center, { count: 1, shape: 'disc', radius: r * 0.3, up: [6, 10], life: [0.3, 0.5], size: [0.8, 1.2], tint: DAWN, intensity: 0.4 });
    });
    rt.ps.emit(rt.s.sparkles, center, { count: 30, shape: 'disc', radius: r * 0.9, up: [0.5, 2], life: [0.8, 1.3], size: [0.3, 0.5], tint: SKY });
  },
  /** Bouclier de l'aube sur chaque allié touché. */
  buff(rt, e) {
    if (e.buffId !== 'solenne_r') return;
    shieldShell(rt, e.target, Math.max(0.3, e.duration), `solenne_r:${e.target}`, { color: new THREE.Color(1.5, 1.2, 0.5), radius: 1.05, bands: 0.2 });
  },
  buffEnd(rt, e) {
    if (e.buffId === 'solenne_r') rt.endAura(`solenne_r:${e.target}`);
  },
});
/** Avant l'aube : cercle doré, motes de lumière qui descendent. */
registerFx('solenne_r_telegraph', {
  area(rt, e) {
    const center = rt.world(e.pos);
    const r = rt.scale(e.radius);
    const delay = Math.max(0.15, e.duration);
    telegraph(rt, center, r, delay, DAWN.clone().multiplyScalar(0.5), { color2: DAWN, fill: 0.12 });
    timed(rt, delay, () => {
      const a = rng.next() * Math.PI * 2;
      const d = Math.sqrt(rng.next()) * r;
      rt.ps.emit(rt.s.glow, _v.set(center.x + Math.cos(a) * d, 4, center.z + Math.sin(a) * d), { count: 1, shape: 'point', inherit: new THREE.Vector3(0, -7, 0), life: 0.5, size: 0.35, tint: DAWN });
    });
  },
});
