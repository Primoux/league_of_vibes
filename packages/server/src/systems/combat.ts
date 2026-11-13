import { FOUNTAINS, TEAMS, angleTo, dist, getChampion, type EntityId, type Vec2 } from '@lov/shared';
import {
  CRIT_MULTIPLIER,
  applyDamage,
  applyHeal,
  expireShields,
  respawnChampion,
} from '../combat/damage.ts';
import type { AttackContext, Entity, System, World } from '../world.ts';
import { isVisibleTo } from './vision.ts';

/**
 * Propriétaire : claude-server. Auto-attaques, poursuite, attaque-déplacement, mort/respawn, fontaine.
 *
 * Contrat pour les autres systèmes (sbires, tours, monstres utilisent aussi ce combat) :
 * - `entity.attackTarget = id` : attaque cette cible (poursuite automatique si l'entité peut bouger).
 * - `entity.data.attackConfig = { windup?: number; projectileSpeed?: number }` : surcharge du temps d'armement
 *   (fraction de la période, défaut champion.attackWindup sinon 0.3) et de la vitesse du projectile
 *   (0 = mêlée ; défaut champion.projectileSpeed, sinon 1600 si portée > 300).
 * - `entity.data.keepCorpse = true` : le combat ne retire pas l'entité morte (sinon sbires/monstres/balises
 *   sont retirés CORPSE_SECONDS après leur mort).
 * - Hooks : `canAttack`, `isTargetable`, `beforeAttackHit`, `afterAttackHit` (voir world.ts).
 */
export const CORPSE_SECONDS = 2;
/** Fontaine : soin des alliés (fraction des PV/mana max par seconde) et laser sur les ennemis (dégâts bruts / s). */
export const FOUNTAIN_HEAL_PER_SEC = 0.12;
export const FOUNTAIN_LASER_DPS = 1800;
export const FOUNTAIN_LASER_RANGE = 1200;
const FOUNTAIN_HEAL_RADIUS = 1000;
/** Distance d'acquisition d'une cible en attaque-déplacement (en plus de la portée). */
export const ATTACK_MOVE_ACQUIRE = 250;

const NON_COMBAT = new Set(['projectile', 'zone', 'ward']);
const KEEP_KINDS = new Set(['champion', 'tower', 'inhibitor', 'nexus']);

interface AttackState {
  /** Temps restant avant l'impact/lancement ; < 0 = pas d'attaque en cours. */
  windup: number;
  /** Temps restant avant de pouvoir démarrer une nouvelle attaque. */
  cooldown: number;
  /** Cible de l'attaque en cours d'armement. */
  windupTarget?: EntityId;
  crit: boolean;
  /** Poursuite en cours (moveTarget posé par le combat). */
  chasing: boolean;
  /** Destination d'attaque-déplacement. */
  attackMove?: Vec2;
}

interface AAProjectile {
  attacker: Entity;
  targetId: EntityId;
  x: number;
  y: number;
  speed: number;
  crit: boolean;
}

const projectiles = new WeakMap<World, AAProjectile[]>();

export function attackState(e: Entity): AttackState {
  return (e.data.aa ??= { windup: -1, cooldown: 0, crit: false, chasing: false }) as AttackState;
}

/** Démarre une attaque-déplacement vers `pos` (appelé par input). */
export function orderAttackMove(e: Entity, pos: Vec2): void {
  const st = attackState(e);
  st.attackMove = { x: pos.x, y: pos.y };
  e.attackTarget = undefined;
  e.moveTarget = { x: pos.x, y: pos.y };
  e.path = undefined;
}

/** Annule l'attaque en cours d'armement (déplacement, sort, CC). Le timer d'attaque est rendu. */
export function cancelAttack(e: Entity): void {
  const st = e.data.aa as AttackState | undefined;
  if (!st || st.windup < 0) return;
  st.windup = -1;
  st.cooldown = 0;
  st.windupTarget = undefined;
  if (e.anim === 'attack') e.anim = 'idle';
}

/** Réinitialise le timer d'auto-attaque (sorts « reset d'AA »). */
export function resetAttackTimer(e: Entity): void {
  attackState(e).cooldown = 0;
}

export function attackConfig(e: Entity): { windup: number; projectileSpeed: number } {
  const cfg = e.data.attackConfig as { windup?: number; projectileSpeed?: number } | undefined;
  let windup = cfg?.windup;
  let speed = cfg?.projectileSpeed;
  if (e.champ && (windup === undefined || speed === undefined)) {
    const def = getChampion(e.champ.championId);
    windup ??= def.attackWindup;
    speed ??= def.ranged ? (def.projectileSpeed ?? 2000) : 0;
  }
  return { windup: windup ?? 0.3, projectileSpeed: speed ?? (e.stats.attackRange > 300 ? 1600 : 0) };
}

/** Distance bord à bord. */
export const edgeDistance = (a: Entity, b: Entity): number => dist(a.pos, b.pos) - a.radius - b.radius;

export function isValidAttackTarget(world: World, attacker: Entity, t: Entity | undefined): t is Entity {
  if (!t || t.dead || t === attacker || t.kind === 'projectile' || t.kind === 'zone') return false;
  if (t.team === attacker.team) return false;
  if (attacker.team !== 'neutral' && !isVisibleTo(t, attacker.team)) return false;
  return world.allow(world.hooks.isTargetable, t, attacker);
}

export const combatSystem: System = {
  name: 'combat',
  update(world, dt) {
    for (const e of world.entities.values()) {
      expireShields(world, e);
      if (e.dead) {
        handleDead(world, e, dt);
        continue;
      }
      if (NON_COMBAT.has(e.kind)) continue;
      updateAttacker(world, e, dt);
    }
    updateProjectiles(world, dt);
    updateFountains(world, dt);
  },
};

function handleDead(world: World, e: Entity, dt: number): void {
  if (e.champ) {
    e.champ.respawnIn -= dt;
    if (e.champ.respawnIn <= 0) respawnChampion(world, e);
    return;
  }
  if (KEEP_KINDS.has(e.kind) || e.data.keepCorpse) return;
  const diedAt = (e.data.combat as { diedAt?: number } | undefined)?.diedAt ?? world.time;
  if (world.time - diedAt >= CORPSE_SECONDS) world.remove(e.id);
}

function updateAttacker(world: World, e: Entity, dt: number): void {
  const st = e.data.aa as AttackState | undefined;
  if (!st && e.attackTarget === undefined) return;
  const s = st ?? attackState(e);
  s.cooldown -= dt;
  if (e.anim === 'attack' && s.windup < 0 && s.cooldown <= 0) e.anim = 'idle';

  if (e.data.dash) {
    cancelAttack(e);
    return;
  }

  // Attaque-déplacement : acquisition de la cible la plus proche.
  if (e.attackTarget === undefined && s.attackMove) {
    const found = acquireTarget(world, e, e.stats.attackRange + ATTACK_MOVE_ACQUIRE);
    if (found) e.attackTarget = found.id;
    else if (!e.moveTarget) s.attackMove = undefined;
  }

  const t = world.get(e.attackTarget);
  if (!isValidAttackTarget(world, e, t)) {
    if (e.attackTarget !== undefined) dropTarget(e, s);
    if (s.windup >= 0) cancelAttack(e);
    return;
  }

  const inRange = edgeDistance(e, t) <= e.stats.attackRange;
  if (s.windup >= 0) {
    // Armement en cours : se termine même si la cible s'éloigne un peu (comme dans LoL).
    if (s.windupTarget !== t.id) {
      cancelAttack(e);
    } else {
      e.facing = angleTo(e.pos, t.pos);
      s.windup -= dt;
      if (s.windup <= 0) launch(world, e, t, s);
      return;
    }
  }

  if (!inRange) {
    if (e.stats.moveSpeed > 0) {
      s.chasing = true;
      const mt = e.moveTarget;
      if (!mt || mt.x !== t.pos.x || mt.y !== t.pos.y) e.moveTarget = { x: t.pos.x, y: t.pos.y };
    }
    return;
  }

  if (e.moveTarget) {
    e.moveTarget = undefined;
    e.path = undefined;
    s.chasing = false;
  }
  if (s.cooldown > 0 || e.stats.attackSpeed <= 0 || !world.allow(world.hooks.canAttack, e)) return;

  // Début de l'armement.
  const period = 1 / e.stats.attackSpeed;
  const cfg = attackConfig(e);
  s.cooldown = period;
  s.windup = period * cfg.windup;
  s.windupTarget = t.id;
  s.crit = e.stats.critChance > 0 && Math.random() < e.stats.critChance;
  e.anim = 'attack';
  e.facing = angleTo(e.pos, t.pos);
  s.windup -= dt;
  if (s.windup <= 0) launch(world, e, t, s);
}

function dropTarget(e: Entity, s: AttackState): void {
  e.attackTarget = undefined;
  if (s.chasing) {
    e.moveTarget = undefined;
    e.path = undefined;
    s.chasing = false;
  }
  if (s.attackMove) e.moveTarget = { x: s.attackMove.x, y: s.attackMove.y };
}

function acquireTarget(world: World, e: Entity, range: number): Entity | undefined {
  let best: Entity | undefined;
  let bestD = Infinity;
  for (const o of world.entities.values()) {
    if (o.kind === 'projectile' || o.kind === 'zone') continue;
    const d = edgeDistance(e, o);
    if (d > range || d >= bestD) continue;
    if (!isValidAttackTarget(world, e, o)) continue;
    best = o;
    bestD = d;
  }
  return best;
}

function launch(world: World, e: Entity, t: Entity, s: AttackState): void {
  s.windup = -1;
  s.windupTarget = undefined;
  const cfg = attackConfig(e);
  const ranged = cfg.projectileSpeed > 0;
  world.events.emit({
    type: 'autoAttack',
    source: e.id,
    target: t.id,
    ranged,
    ...(ranged ? { speed: cfg.projectileSpeed } : {}),
    ...(s.crit ? { crit: true } : {}),
  });
  if (ranged) {
    let list = projectiles.get(world);
    if (!list) projectiles.set(world, (list = []));
    list.push({ attacker: e, targetId: t.id, x: e.pos.x, y: e.pos.y, speed: cfg.projectileSpeed, crit: s.crit });
  } else {
    hit(world, e, t, s.crit);
  }
}

/** Résout l'impact d'une auto-attaque (hooks beforeAttackHit / afterAttackHit). Exporté pour les sorts « prochaine AA ». */
export function hit(world: World, attacker: Entity, target: Entity, crit: boolean): number {
  if (target.dead) return 0;
  const ctx: AttackContext = {
    attacker,
    target,
    amount: attacker.stats.ad,
    crit,
    critMultiplier: CRIT_MULTIPLIER,
    bonus: 0,
    damageType: 'physical',
  };
  for (const fn of world.hooks.beforeAttackHit) fn(ctx, world);
  const amount = ctx.amount * (ctx.crit ? ctx.critMultiplier : 1) + ctx.bonus;
  const dealt = applyDamage(world, { source: attacker, target, amount, type: ctx.damageType, isAutoAttack: true, crit: ctx.crit });
  for (const fn of world.hooks.afterAttackHit) fn(ctx, dealt, world);
  return dealt;
}

function updateProjectiles(world: World, dt: number): void {
  const list = projectiles.get(world);
  if (!list || list.length === 0) return;
  let w = 0;
  for (let i = 0; i < list.length; i++) {
    const p = list[i]!;
    const t = world.get(p.targetId);
    if (!t || t.dead || !world.entities.has(p.attacker.id)) continue;
    const d = Math.hypot(t.pos.x - p.x, t.pos.y - p.y);
    const step = p.speed * dt;
    if (d <= step + t.radius * 0.5) {
      hit(world, p.attacker, t, p.crit);
      continue;
    }
    p.x += ((t.pos.x - p.x) / d) * step;
    p.y += ((t.pos.y - p.y) / d) * step;
    list[w++] = p;
  }
  list.length = w;
}

/** Projectiles d'auto-attaque en vol (tests, debug). */
export function pendingAttackProjectiles(world: World): number {
  return projectiles.get(world)?.length ?? 0;
}

function updateFountains(world: World, dt: number): void {
  for (const team of TEAMS) {
    const f = FOUNTAINS[team];
    for (const e of world.entities.values()) {
      if (e.dead || e.kind !== 'champion') continue;
      const d = dist(e.pos, f);
      if (e.team === team) {
        if (d > FOUNTAIN_HEAL_RADIUS) continue;
        if (e.hp < e.stats.maxHp) applyHeal(world, { source: undefined, target: e, amount: e.stats.maxHp * FOUNTAIN_HEAL_PER_SEC * dt, silent: true });
        if (e.mana < e.stats.maxMana) e.mana = Math.min(e.stats.maxMana, e.mana + e.stats.maxMana * FOUNTAIN_HEAL_PER_SEC * dt);
      } else if (d <= FOUNTAIN_LASER_RANGE) {
        applyDamage(world, { source: undefined, target: e, amount: FOUNTAIN_LASER_DPS * dt, type: 'true', ability: 'fountain_laser' });
      }
    }
  }
}
