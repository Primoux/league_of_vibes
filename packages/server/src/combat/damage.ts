/**
 * API de dégâts / soins / boucliers. Propriétaire : claude-server.
 * TOUS les dégâts du jeu passent par `applyDamage` (sorts, AA, tours, monstres, fontaine, DoT...).
 *
 * Extension sans éditer ce fichier : `world.hooks.beforeDamage`, `afterDamage`, `beforeHeal`, `preventDeath`,
 * `bounty`, `respawnTime` (voir WorldHooks dans world.ts).
 */
import { FOUNTAINS, damageMultiplier, type DamageType, type EntityId } from '@lov/shared';
import type { DamageContext, Entity, HealContext, World } from '../world.ts';

/** Fenêtre (s) pendant laquelle un champion ayant infligé des dégâts compte pour un assist. */
export const ASSIST_WINDOW = 10;
/** Multiplicateur des coups critiques. */
export const CRIT_MULTIPLIER = 1.75;

interface ShieldInstance {
  amount: number;
  /** Temps de jeu (s) d'expiration ; Infinity = permanent jusqu'à consommation. */
  until: number;
  source?: EntityId;
  id?: string;
}

/** État de combat namespacé (`entity.data.combat`), géré par claude-server. Lecture libre pour les autres. */
export interface CombatData {
  /** Dernier temps (s) où l'entité a subi / infligé des dégâts (hors combat, rappel, régénération...). */
  lastDamagedAt: number;
  lastDealtAt: number;
  /** Dernière fois que l'entité a été touchée par un champion ennemi, par id de champion. */
  championDamagers: Map<EntityId, number>;
  shields: ShieldInstance[];
  /** Mort : temps de jeu de la mort. */
  diedAt?: number;
}

export function combatData(e: Entity): CombatData {
  let c = e.data.combat as CombatData | undefined;
  if (!c) {
    c = { lastDamagedAt: -Infinity, lastDealtAt: -Infinity, championDamagers: new Map(), shields: [] };
    e.data.combat = c;
  }
  return c;
}

/** Secondes depuis le dernier échange de dégâts (subis ou infligés). */
export function timeSinceCombat(world: World, e: Entity): number {
  const c = combatData(e);
  return world.time - Math.max(c.lastDamagedAt, c.lastDealtAt);
}

/** Entité créditée pour une source : suit `data.owner` (EntityId) pour les invocations, zones, projectiles. */
export function creditOf(world: World, source: Entity | undefined): Entity | undefined {
  let cur = source;
  for (let i = 0; i < 4 && cur; i++) {
    const owner = cur.data.owner as EntityId | undefined;
    if (owner === undefined) return cur;
    const next = world.get(owner);
    if (!next) return cur;
    cur = next;
  }
  return cur;
}

export interface DamageOpts {
  source: Entity | EntityId | undefined;
  target: Entity;
  amount: number;
  type: DamageType;
  /** Id du sort (ex. 'ilyra_q') : transmis dans l'événement `damage`. */
  ability?: string;
  isAutoAttack?: boolean;
  crit?: boolean;
  /** N'émet pas d'événement `damage` (DoT très fréquents agrégés ailleurs). Défaut false. */
  silent?: boolean;
}

const resolve = (world: World, s: Entity | EntityId | undefined): Entity | undefined =>
  typeof s === 'number' ? world.get(s) : s;

/**
 * Applique des dégâts : hooks beforeDamage → résistances/pénétration → boucliers → PV → lifesteal (AA) → mort.
 * Retourne les dégâts effectivement infligés (bouclier absorbé inclus).
 */
export function applyDamage(world: World, opts: DamageOpts): number {
  const target = opts.target;
  if (target.dead || opts.amount <= 0 || world.entities.get(target.id) !== target) return 0;
  const source = resolve(world, opts.source);
  const credit = creditOf(world, source);
  const ctx: DamageContext = {
    source,
    credit,
    target,
    amount: opts.amount,
    type: opts.type,
    ability: opts.ability,
    isAutoAttack: !!opts.isAutoAttack,
    crit: !!opts.crit,
    flatPen: 0,
    percentPen: 0,
    multiplier: 1,
  };
  for (const fn of world.hooks.beforeDamage) fn(ctx, world);
  if (ctx.amount <= 0 || ctx.multiplier <= 0) return 0;

  let amount = ctx.amount;
  if (ctx.type !== 'true') {
    const physical = ctx.type === 'physical';
    const resist = physical ? target.stats.armor : target.stats.mr;
    const flat = (physical ? (credit?.stats.armorPen ?? 0) : (credit?.stats.magicPen ?? 0)) + ctx.flatPen;
    amount *= damageMultiplier(effectiveResist(resist, ctx.percentPen, flat));
  }
  amount *= ctx.multiplier;
  if (amount <= 0) return 0;

  // Boucliers d'abord (les plus proches de l'expiration en premier).
  const cd = combatData(target);
  let remaining = amount;
  if (target.shield > 0 && cd.shields.length) {
    cd.shields.sort((a, b) => a.until - b.until);
    for (const s of cd.shields) {
      const absorbed = Math.min(s.amount, remaining);
      s.amount -= absorbed;
      remaining -= absorbed;
      if (remaining <= 0) break;
    }
    cd.shields = cd.shields.filter((s) => s.amount > 0);
    target.shield = sumShields(cd.shields);
  }
  const hpLoss = Math.min(target.hp, remaining);
  target.hp -= remaining;
  const dealt = amount - remaining + hpLoss;

  cd.lastDamagedAt = world.time;
  if (credit) combatData(credit).lastDealtAt = world.time;
  if (credit?.champ && credit.team !== target.team) {
    if (target.kind === 'champion') {
      cd.championDamagers.set(credit.id, world.time);
      credit.champ.damageToChampions += dealt;
    }
  }

  if (!opts.silent) {
    world.events.emit({
      type: 'damage',
      source: credit?.id ?? source?.id ?? 0,
      target: target.id,
      amount: Math.round(dealt),
      damageType: ctx.type,
      ...(ctx.crit ? { crit: true } : {}),
      ...(ctx.ability ? { ability: ctx.ability } : {}),
    });
  }

  if (ctx.isAutoAttack && credit && credit.stats.lifesteal > 0 && !credit.dead) {
    applyHeal(world, { source: credit, target: credit, amount: dealt * credit.stats.lifesteal });
  }

  for (const fn of world.hooks.afterDamage) fn(ctx, dealt, world);

  if (target.hp <= 0 && !target.dead) {
    let prevented = false;
    for (const fn of world.hooks.preventDeath) if (fn(target, credit, world)) prevented = true;
    if (prevented && target.hp > 0) return dealt;
    kill(world, target, credit);
  }
  return dealt;
}

/** Résistance effective : % de pénétration puis pénétration fixe, la fixe ne descend pas sous 0. */
export function effectiveResist(resist: number, percentPen: number, flatPen: number): number {
  if (resist <= 0) return resist;
  const afterPercent = resist * (1 - Math.min(1, Math.max(0, percentPen)));
  return Math.max(0, afterPercent - flatPen);
}

/** Dégâts après résistances, sans effet de bord (prévisualisation, tests, IA). */
export function previewDamage(source: Entity | undefined, target: Entity, amount: number, type: DamageType): number {
  if (type === 'true') return amount;
  const physical = type === 'physical';
  const resist = physical ? target.stats.armor : target.stats.mr;
  const flat = physical ? (source?.stats.armorPen ?? 0) : (source?.stats.magicPen ?? 0);
  return amount * damageMultiplier(effectiveResist(resist, 0, flat));
}

export interface HealOpts {
  source: Entity | EntityId | undefined;
  target: Entity;
  amount: number;
  silent?: boolean;
}

/** Soigne (plafonné aux PV max). Retourne le soin effectif. Hooks : beforeHeal (anti-soin, amplification). */
export function applyHeal(world: World, opts: HealOpts): number {
  const target = opts.target;
  if (target.dead || opts.amount <= 0) return 0;
  const ctx: HealContext = { source: resolve(world, opts.source), target, amount: opts.amount };
  for (const fn of world.hooks.beforeHeal) fn(ctx, world);
  const healed = Math.max(0, Math.min(ctx.amount, target.stats.maxHp - target.hp));
  if (healed <= 0) return 0;
  target.hp += healed;
  if (!opts.silent && healed >= 1) {
    world.events.emit({ type: 'heal', source: ctx.source?.id ?? target.id, target: target.id, amount: Math.round(healed) });
  }
  return healed;
}

export interface ShieldOpts {
  source: Entity | EntityId | undefined;
  target: Entity;
  amount: number;
  /** Durée en secondes (défaut : jusqu'à consommation). */
  duration?: number;
  /** Id du bouclier : un nouveau bouclier avec le même id remplace l'ancien (ex. 'grondak_p'). */
  id?: string;
}

/** Ajoute un bouclier. `entity.shield` est la somme des boucliers actifs (expirés par le système combat). */
export function applyShield(world: World, opts: ShieldOpts): number {
  const target = opts.target;
  if (target.dead || opts.amount <= 0) return 0;
  const source = resolve(world, opts.source);
  const cd = combatData(target);
  if (opts.id) cd.shields = cd.shields.filter((s) => s.id !== opts.id);
  cd.shields.push({
    amount: opts.amount,
    until: opts.duration !== undefined ? world.time + opts.duration : Infinity,
    source: source?.id,
    id: opts.id,
  });
  target.shield = sumShields(cd.shields);
  world.events.emit({ type: 'shield', source: source?.id ?? target.id, target: target.id, amount: Math.round(opts.amount) });
  return opts.amount;
}

/** Retire un bouclier par id (ou tous si id absent). */
export function removeShield(target: Entity, id?: string): void {
  const cd = combatData(target);
  cd.shields = id ? cd.shields.filter((s) => s.id !== id) : [];
  target.shield = sumShields(cd.shields);
}

/** Expire les boucliers échus (appelé chaque tick par le système combat). */
export function expireShields(world: World, e: Entity): void {
  const cd = e.data.combat as CombatData | undefined;
  if (!cd || cd.shields.length === 0) {
    if (e.shield !== 0 && !cd) e.shield = 0;
    return;
  }
  let changed = false;
  for (const s of cd.shields) if (s.until <= world.time) changed = true;
  if (changed) {
    cd.shields = cd.shields.filter((s) => s.until > world.time);
    e.shield = sumShields(cd.shields);
  }
}

function sumShields(list: ShieldInstance[]): number {
  let t = 0;
  for (const s of list) t += s.amount;
  return t;
}

/** Durée de mort par défaut : 6 + 2,5 × niveau, +2 % par minute au-delà de 15:00 (max +50 %). */
export function defaultRespawnTime(level: number, gameTime: number): number {
  const base = 6 + level * 2.5;
  const lateFactor = Math.min(0.5, Math.max(0, (gameTime - 15 * 60) / 60) * 0.02);
  return base * (1 + lateFactor);
}

/** Assistants d'un kill : champions ennemis de la victime ayant infligé des dégâts dans les ASSIST_WINDOW secondes. */
export function assistsFor(world: World, victim: Entity, killer: Entity | undefined): Entity[] {
  const out: Entity[] = [];
  const cd = victim.data.combat as CombatData | undefined;
  if (!cd) return out;
  for (const [id, t] of cd.championDamagers) {
    if (world.time - t > ASSIST_WINDOW || id === killer?.id) continue;
    const e = world.get(id);
    if (e?.champ && e.team !== victim.team) out.push(e);
  }
  return out;
}

/** Donne un crédit d'assist à `helper` sur `victim` (CC, soins d'un allié qui combat...). */
export function markAssist(world: World, helper: Entity, victim: Entity): void {
  if (helper.champ && helper.team !== victim.team) combatData(victim).championDamagers.set(helper.id, world.time);
}

/**
 * Tue une entité (sans passer par les dégâts : exécutions, laser de fontaine, despawn...).
 * Champion : K/D/A, timer de respawn. Émet `death` (killer = champion crédité si un champion a touché la victime
 * dans les 10 dernières secondes et que le coup final vient d'une unité non-champion).
 */
export function kill(world: World, target: Entity, killer?: Entity): void {
  if (target.dead) return;
  target.hp = 0;
  target.dead = true;
  target.anim = 'death';
  target.moveTarget = undefined;
  target.path = undefined;
  target.attackTarget = undefined;
  delete target.data.dash;
  removeShield(target);
  const cd = combatData(target);
  cd.diedAt = world.time;

  let credited = killer;
  if (target.champ && !killer?.champ) {
    // Exécuté par une tour / un sbire : le dernier champion ennemi à l'avoir touché récupère le kill.
    let best: Entity | undefined;
    let bestT = -Infinity;
    for (const [id, t] of cd.championDamagers) {
      if (world.time - t > ASSIST_WINDOW || t <= bestT) continue;
      const e = world.get(id);
      if (e?.champ && e.team !== target.team) {
        best = e;
        bestT = t;
      }
    }
    if (best) credited = best;
  }

  const assists = target.champ ? assistsFor(world, target, credited) : [];
  let bounty: number | undefined;
  for (const fn of world.hooks.bounty) {
    bounty = fn(target, credited, assists, world);
    if (bounty !== undefined) break;
  }

  if (target.champ) {
    target.champ.deaths++;
    let respawn: number | undefined;
    for (const fn of world.hooks.respawnTime) {
      respawn = fn(target, world);
      if (respawn !== undefined) break;
    }
    target.champ.respawnIn = respawn ?? defaultRespawnTime(target.champ.level, world.time);
    if (credited?.champ && credited.team !== target.team) credited.champ.kills++;
    for (const a of assists) a.champ!.assists++;
    cd.championDamagers.clear();
  }

  world.events.emit({
    type: 'death',
    target: target.id,
    ...(credited ? { killer: credited.id } : {}),
    ...(assists.length ? { assists: assists.map((a) => a.id) } : {}),
    ...(bounty !== undefined ? { bounty } : {}),
  });
}

/** Ressuscite un champion à sa fontaine (PV/mana pleins). Émet `respawn`. */
export function respawnChampion(world: World, e: Entity): void {
  const f = FOUNTAINS[e.team === 'red' ? 'red' : 'blue'];
  e.dead = false;
  e.hp = e.stats.maxHp;
  e.mana = e.stats.maxMana;
  e.pos = { x: f.x, y: f.y };
  e.anim = 'idle';
  e.moveTarget = undefined;
  e.path = undefined;
  e.attackTarget = undefined;
  if (e.champ) e.champ.respawnIn = 0;
  const cd = combatData(e);
  cd.diedAt = undefined;
  cd.lastDamagedAt = -Infinity;
  cd.lastDealtAt = -Infinity;
  world.events.emit({ type: 'respawn', target: e.id, pos: { x: f.x, y: f.y } });
}
