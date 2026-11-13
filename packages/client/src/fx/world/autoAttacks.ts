/**
 * FX des auto-attaques (événement `autoAttack`, émis au lancement : impact pour la mêlée, départ du projectile
 * à distance). Variantes par champion via `registerAutoAttackFx`, sinon FX générique aux couleurs du champion.
 */
import * as THREE from 'three';
import { CHAMPIONS, type EntitySnapshot } from '@lov/shared';
import { ProjectileFx, flash, lightning, shockwave } from '../effects/primitives.ts';
import type { DragonElement } from '@lov/shared';
import type { FxColors, FxRuntime } from '../core/FxRuntime.ts';
import { rng } from '../core/rng.ts';
import { onWorldEvent, type Ev } from '../registry.ts';

export interface AutoAttackFx {
  /** Tir à distance : renvoie le projectile (tête chercheuse vers la cible). */
  ranged?(rt: FxRuntime, e: Ev<'autoAttack'>, from: THREE.Vector3, c: FxColors): void;
  /** Coup au corps à corps (au moment de l'impact). */
  melee?(rt: FxRuntime, e: Ev<'autoAttack'>, at: THREE.Vector3, c: FxColors): void;
}

const custom = new Map<string, AutoAttackFx>();
export function registerAutoAttackFx(championId: string, fx: AutoAttackFx): void {
  custom.set(championId, fx);
}

const TEAM = { blue: new THREE.Color(0.5, 0.8, 2.4), red: new THREE.Color(2.4, 0.6, 0.5), neutral: new THREE.Color(2, 1.6, 0.6) };
const _from = new THREE.Vector3();
const _at = new THREE.Vector3();

/** Projectile générique à tête chercheuse ; `onHit` à l'arrivée. */
export function homingShot(
  rt: FxRuntime,
  e: Ev<'autoAttack'>,
  from: THREE.Vector3,
  color: THREE.Color,
  core: THREE.Color,
  size: number,
  onHit: (at: THREE.Vector3) => void,
): void {
  rt.entities.pos(e.target, _at, 1);
  rt.add(
    new ProjectileFx(rt, from, _at, rt.scale(e.speed ?? 1600), {
      color,
      core,
      size,
      height: from.y,
      trailStyle: rt.s.glow,
      trailDensity: 6,
      trailSize: [size * 0.25, size * 0.45],
      trailLife: [0.15, 0.25],
      ribbonWidth: size * 0.35,
      ribbonLife: 0.15,
      targetId: e.target,
      onEnd: (head) => onHit(head),
    }),
  );
}

/**
 * Impact d'auto-attaque à distance : volontairement sobre (plusieurs par seconde en combat).
 * Un empilement de halos HDR sature en blanc sous le bloom.
 */
function aaImpact(rt: FxRuntime, at: THREE.Vector3, c: FxColors, crit: boolean): void {
  flash(rt, at, crit ? 1.1 : 0.7, c.vivid, 0.14);
  rt.ps.emit(rt.s.spark, at, { count: crit ? 10 : 5, shape: 'point', speed: [2, 4], life: [0.12, 0.22], size: [0.04, 0.07], stretch: 0.04, tint: c.vividHdr });
  if (crit) shockwave(rt, _at.set(at.x, 0, at.z), 1, 0.25, c.vivid, 0.08);
}

function genericRanged(rt: FxRuntime, e: Ev<'autoAttack'>, from: THREE.Vector3, c: FxColors): void {
  homingShot(rt, e, from, c.vivid, c.accent, e.crit ? 0.45 : 0.3, (at) => aaImpact(rt, at, c, !!e.crit));
}

function genericMelee(rt: FxRuntime, e: Ev<'autoAttack'>, at: THREE.Vector3, c: FxColors): void {
  const k = e.crit ? 1.5 : 1;
  _at.set(at.x, 1, at.z);
  flash(rt, _at, 1.1 * k, c.vividHdr, 0.15);
  rt.ps.emit(rt.s.spark, _at, { count: Math.round(8 * k), shape: 'point', speed: [3, 6], life: [0.15, 0.3], size: [0.05, 0.09], stretch: 0.04, gravity: 6, tint: c.accentHdr });
  if (e.crit) shockwave(rt, at, 1.2, 0.3, c.vividHdr, 0.1);
}

function structureOrMinion(rt: FxRuntime, e: Ev<'autoAttack'>, src: EntitySnapshot): void {
  const team = TEAM[src.team];
  if (!e.ranged) {
    if (!rt.entities.pos(e.target, _at, 0.7)) return;
    rt.ps.emit(rt.s.spark, _at, { count: 4, shape: 'point', speed: [2, 4], life: [0.12, 0.2], size: [0.04, 0.07], stretch: 0.03, tint: team });
    return;
  }
  const tower = src.kind === 'tower';
  if (!rt.entities.pos(e.source, _from, tower ? 4.8 : 0.9)) return;
  homingShot(rt, e, _from.clone(), team, team, tower ? 0.9 : 0.3, (at) => {
    flash(rt, at, tower ? 1.8 : 0.7, team, 0.15);
    if (tower) rt.ps.emit(rt.s.spark, at, { count: 10, shape: 'point', speed: [2, 5], life: [0.2, 0.35], size: [0.05, 0.1], stretch: 0.04, tint: team });
  });
}

// ---------------------------------------------------------------- monstres épiques
/** Élément du drake courant (comme le renderer : appris par `objectiveSpawn`). */
let dragonElement: DragonElement | 'elder' = 'infernal';

const BREATH: Record<DragonElement | 'elder', { color: THREE.Color; style: 'flame' | 'glow' | 'dust' | 'sparkles' | 'smoke' }> = {
  infernal: { color: new THREE.Color(2.6, 0.9, 0.25), style: 'flame' },
  ocean: { color: new THREE.Color(0.3, 1.3, 2.6), style: 'glow' },
  mountain: { color: new THREE.Color(0.7, 0.55, 0.4), style: 'dust' },
  cloud: { color: new THREE.Color(1.8, 2.3, 2.6), style: 'sparkles' },
  hextech: { color: new THREE.Color(0.5, 1.8, 2.8), style: 'sparkles' },
  chemtech: { color: new THREE.Color(0.7, 2.2, 0.3), style: 'smoke' },
  elder: { color: new THREE.Color(1.8, 0.8, 2.8), style: 'flame' },
};

const _dir = new THREE.Vector3();
const _vel = new THREE.Vector3();

/** Souffle de drake : jet de particules élémentaires de la gueule vers la cible, impact coloré. */
function breath(rt: FxRuntime, e: Ev<'autoAttack'>, element: DragonElement | 'elder'): void {
  const b = BREATH[element];
  if (!rt.entities.pos(e.source, _from, 1.8) || !rt.entities.pos(e.target, _at, 0.9)) return;
  _dir.copy(_at).sub(_from);
  const len = _dir.length();
  _dir.divideScalar(len || 1);
  _from.addScaledVector(_dir, 1.2); // gueule, devant le corps
  const speed = Math.max(6, len * 2.5);
  _vel.copy(_dir).multiplyScalar(speed);
  const style = rt.s[b.style];
  rt.ps.emit(style, _from, { count: 26, shape: 'cone', dir: _dir, angle: 0.18, speed: [speed * 0.8, speed * 1.1], life: [len / speed, (len / speed) * 1.2], size: [0.3, 0.6], drag: 0.5, tint: b.color });
  if (element === 'hextech') lightning(rt, _from, _at, b.color, { width: 0.16, life: 0.2, jag: 0.25 });
  const at = _at.clone();
  rt.after(len / speed, () => {
    flash(rt, at, 1.8, b.color, 0.2);
    rt.ps.emit(style, at, { count: 10, shape: 'sphere', radius: 0.3, speed: [1, 2.5], life: [0.3, 0.6], size: [0.3, 0.6], tint: b.color });
    if (element === 'mountain') rt.ps.emit(rt.s.debris, at, { count: 6, shape: 'sphere', radius: 0.2, speed: [2, 4], up: [1, 3], gravity: 9, life: [0.6, 0.9], size: [0.12, 0.2], spin: [-6, 6], tint: b.color });
  });
}

/** Coup lourd d'un monstre épique (Léviathan, Colosse) : onde de choc et poussière. */
function heavyHit(rt: FxRuntime, e: Ev<'autoAttack'>, color: THREE.Color): void {
  if (!rt.entities.pos(e.target, _at)) return;
  shockwave(rt, _at, 1.8, 0.4, color, 0.14);
  flash(rt, _at.setY(1), 2, color, 0.2);
  rt.ps.emit(rt.s.dust, _at.setY(0.1), { count: 8, shape: 'ring', radius: 0.5, speed: [1, 2], up: [0.3, 0.8], life: [0.6, 0.9], size: [0.6, 1] });
}
const LEVIATHAN = new THREE.Color(1.6, 0.6, 2.6);
const HERALD = new THREE.Color(1.2, 0.7, 2.4);

function monster(rt: FxRuntime, e: Ev<'autoAttack'>, src: EntitySnapshot): boolean {
  switch (src.subtype) {
    case 'dragon':
      breath(rt, e, dragonElement === 'elder' ? 'infernal' : dragonElement);
      return true;
    case 'elder_dragon':
      breath(rt, e, 'elder');
      return true;
    case 'leviathan':
      heavyHit(rt, e, LEVIATHAN);
      return true;
    case 'herald':
      heavyHit(rt, e, HERALD);
      return true;
    default:
      return false;
  }
}

export function installAutoAttackFx(): void {
  onWorldEvent('objectiveSpawn', (_rt, e) => {
    if (e.monster === 'dragon' && e.element) dragonElement = e.element;
  });
  onWorldEvent('autoAttack', (rt, e) => {
    const src = rt.entities.info(e.source);
    const champId = src?.subtype && CHAMPIONS[src.subtype] ? src.subtype : null;
    if (!champId) {
      if (src && !(src.kind === 'monster' && monster(rt, e, src))) structureOrMinion(rt, e, src);
      return;
    }
    const c = rt.colors(`${champId}_aa`);
    const fx = custom.get(champId);
    if (e.ranged) {
      if (!rt.entities.pos(e.source, _from, 1.25)) return;
      // départ légèrement devant le champion, du côté de la cible
      if (rt.entities.pos(e.target, _at, 1.25)) _from.addScaledVector(_at.sub(_from).setY(0).normalize(), 0.45);
      (fx?.ranged ?? genericRanged)(rt, e, _from.clone(), c);
    } else {
      if (!rt.entities.pos(e.target, _at)) return;
      (fx?.melee ?? genericMelee)(rt, e, _at.clone(), c);
    }
  });
}

// ---------------------------------------------------------------- variantes
const STONE = new THREE.Color('#7a6a58');
registerAutoAttackFx('grondak', {
  melee(rt, e, at, c) {
    genericMelee(rt, e, at, c);
    rt.ps.emit(rt.s.dust, at, { count: 4, shape: 'disc', radius: 0.3, up: [0.3, 0.8], speed: [0.5, 1.2], life: [0.5, 0.8], size: [0.5, 0.8] });
    for (let i = 0; i < 3; i++) {
      const a = rng.next() * Math.PI * 2;
      rt.bursts.rockDebris.spawn({ pos: at.clone().setY(0.4), mode: 'debris', size: rng.range(0.07, 0.12), life: 0.8, vel: new THREE.Vector3(Math.cos(a) * 1.5, 3, Math.sin(a) * 1.5), color: STONE });
    }
  },
});

const ICE_HDR = new THREE.Color(1.3, 2.4, 3.2);
registerAutoAttackFx('ilyra', {
  ranged(rt, e, from) {
    homingShot(rt, e, from, new THREE.Color(0.7, 1.4, 2), ICE_HDR, e.crit ? 0.5 : 0.36, (at) => {
      rt.ps.emit(rt.s.snow, at, { count: 6, shape: 'sphere', radius: 0.15, speed: [1, 2.5], life: [0.35, 0.6], size: [0.1, 0.16], spin: [-4, 4], drag: 2, tint: ICE_HDR });
      flash(rt, at, 0.9, ICE_HDR, 0.15);
    });
  },
});
