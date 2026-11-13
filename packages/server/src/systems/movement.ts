import { MAP_SIZE, angleTo, clamp, dist, type Vec2 } from '@lov/shared';
import { navGrid } from '../nav/grid.ts';
import type { Entity, System, World } from '../world.ts';

/**
 * Propriétaire : claude-server. Déplacement des unités : pathfinding (A* sur grille 50 u), dashes, collisions.
 *
 * Contrat pour les autres systèmes :
 * - `entity.moveTarget = {x, y}` : aller vers ce point (chemin calculé ici ; `entity.path` = points restants).
 * - `entity.data.dash = DashData` : dash prioritaire sur tout le reste (ignore pathfinding et CC de mouvement).
 *   Movement émet l'événement `dash` au premier tick, puis supprime `data.dash` à l'arrivée et appelle `onArrive`.
 * - `entity.data.ghost = true` : ignore les collisions entre unités (pas les structures).
 * - `world.hooks.canMove` : refuser le déplacement (root, stun, canalisation).
 */
export interface DashData {
  to: Vec2;
  /** Unités / s. */
  speed: number;
  /** Ne s'arrête pas aux murs (sauts, blinks lents). */
  ignoreWalls?: boolean;
  /** Suit une entité (dash ciblé) : `to` est mis à jour chaque tick ; s'arrête à `stopDistance` (bord à bord). */
  targetId?: number;
  stopDistance?: number;
  fxKey?: string;
  onArrive?: (world: World, e: Entity) => void;
  /** Interne : événement `dash` déjà émis. */
  started?: boolean;
}

interface MoveData {
  /** Destination pour laquelle `path` a été calculé. */
  pathFor?: Vec2;
}

/** Nombre maximal de requêtes A* par tick (les autres unités avancent en ligne droite en attendant). */
export const ASTAR_BUDGET_PER_TICK = 24;
/** Rayon des structures pour la collision (les unités ne les traversent pas). */
const STRUCTURE_KINDS = new Set(['tower', 'inhibitor', 'nexus']);
const COLLIDING_KINDS = new Set(['champion', 'minion', 'monster']);

const moveData = (e: Entity): MoveData => (e.data.move ??= {}) as MoveData;

export function canMove(world: World, e: Entity): boolean {
  return world.allow(world.hooks.canMove, e);
}

export const movementSystem: System = {
  name: 'movement',
  update(world, dt) {
    const nav = navGrid();
    let budget = ASTAR_BUDGET_PER_TICK;
    const movers: Entity[] = [];

    for (const e of world.entities.values()) {
      if (e.dead || STRUCTURE_KINDS.has(e.kind)) continue;
      const dash = e.data.dash as DashData | undefined;
      if (dash) {
        stepDash(world, e, dash, dt);
        continue;
      }
      if (!e.moveTarget) {
        if (e.anim === 'run') e.anim = 'idle';
        if (COLLIDING_KINDS.has(e.kind)) movers.push(e);
        continue;
      }
      if (!canMove(world, e)) {
        if (e.anim === 'run') e.anim = 'idle';
        if (COLLIDING_KINDS.has(e.kind)) movers.push(e);
        continue;
      }
      // (Re)calcul du chemin si la destination a changé.
      const md = moveData(e);
      const target = e.moveTarget;
      if (!e.path || !md.pathFor || md.pathFor.x !== target.x || md.pathFor.y !== target.y) {
        if (nav.lineWalkable(e.pos, target) && nav.walkable(target.x, target.y)) {
          e.path = [{ x: target.x, y: target.y }];
        } else if (e.path && e.path.length > 1 && md.pathFor && dist(md.pathFor, target) < 150) {
          // Cible qui bouge peu (poursuite) : on garde le chemin et on remplace la fin.
          e.path[e.path.length - 1] = nav.nearestWalkable(target);
        } else if (budget > 0) {
          budget--;
          e.path = nav.findPath(e.pos, target);
        } else {
          e.path = [nav.nearestWalkable(target)];
        }
        md.pathFor = { x: target.x, y: target.y };
      }
      followPath(e, e.stats.moveSpeed * dt);
      if (!e.path || e.path.length === 0) {
        e.moveTarget = undefined;
        e.path = undefined;
        md.pathFor = undefined;
      }
      if (COLLIDING_KINDS.has(e.kind)) movers.push(e);
    }

    resolveCollisions(world, movers);
  },
};

function followPath(e: Entity, step: number): void {
  const path = e.path;
  if (!path) return;
  let moved = false;
  while (step > 0 && path.length > 0) {
    const wp = path[0]!;
    const d = dist(e.pos, wp);
    if (d <= step) {
      if (d > 0) e.facing = angleTo(e.pos, wp);
      e.pos.x = wp.x;
      e.pos.y = wp.y;
      step -= d;
      path.shift();
      moved = true;
    } else {
      e.facing = angleTo(e.pos, wp);
      e.pos.x += ((wp.x - e.pos.x) / d) * step;
      e.pos.y += ((wp.y - e.pos.y) / d) * step;
      step = 0;
      moved = true;
    }
  }
  if (moved) e.anim = 'run';
}

function stepDash(world: World, e: Entity, dash: DashData, dt: number): void {
  const nav = navGrid();
  if (dash.targetId !== undefined) {
    const t = world.get(dash.targetId);
    if (!t || t.dead) {
      endDash(world, e, dash);
      return;
    }
    dash.to = { x: t.pos.x, y: t.pos.y };
  }
  if (!dash.started) {
    dash.started = true;
    e.moveTarget = undefined;
    e.path = undefined;
    world.events.emit({
      type: 'dash',
      source: e.id,
      from: { x: e.pos.x, y: e.pos.y },
      to: { x: dash.to.x, y: dash.to.y },
      duration: dist(e.pos, dash.to) / Math.max(1, dash.speed),
      ...(dash.fxKey ? { fxKey: dash.fxKey } : {}),
    });
  }
  e.anim = 'dash';
  const stopAt = dash.targetId !== undefined ? (dash.stopDistance ?? 0) + e.radius + (world.get(dash.targetId)?.radius ?? 0) : 0;
  const d = dist(e.pos, dash.to);
  const remaining = Math.max(0, d - stopAt);
  const step = dash.speed * dt;
  if (d > 0) e.facing = angleTo(e.pos, dash.to);
  let nx: number;
  let ny: number;
  let arrived = false;
  if (remaining <= step) {
    const k = d > 0 ? remaining / d : 0;
    nx = e.pos.x + (dash.to.x - e.pos.x) * k;
    ny = e.pos.y + (dash.to.y - e.pos.y) * k;
    arrived = true;
  } else {
    nx = e.pos.x + ((dash.to.x - e.pos.x) / d) * step;
    ny = e.pos.y + ((dash.to.y - e.pos.y) / d) * step;
  }
  nx = clamp(nx, 0, MAP_SIZE);
  ny = clamp(ny, 0, MAP_SIZE);
  if (!dash.ignoreWalls && !nav.walkable(nx, ny)) {
    const stop = nav.lastWalkableOnSegment(e.pos, { x: nx, y: ny });
    e.pos.x = stop.x;
    e.pos.y = stop.y;
    arrived = true;
  } else {
    e.pos.x = nx;
    e.pos.y = ny;
  }
  if (arrived) {
    if (dash.ignoreWalls && !nav.walkable(e.pos.x, e.pos.y)) {
      const out = nav.nearestWalkable(e.pos);
      e.pos.x = out.x;
      e.pos.y = out.y;
    }
    endDash(world, e, dash);
  }
}

function endDash(world: World, e: Entity, dash: DashData): void {
  delete e.data.dash;
  e.anim = 'idle';
  dash.onArrive?.(world, e);
}

/**
 * Séparation douce entre unités (chacune recule de la moitié du chevauchement, pondérée : les unités à l'arrêt
 * bougent moins) et collisions dures avec les structures. Les unités `data.ghost` ignorent les autres unités.
 */
function resolveCollisions(world: World, movers: Entity[]): void {
  const nav = navGrid();
  const n = movers.length;
  for (let i = 0; i < n; i++) {
    const a = movers[i]!;
    if (a.data.dash) continue;
    const ghostA = !!a.data.ghost;
    for (let j = i + 1; j < n; j++) {
      const b = movers[j]!;
      if (b.data.dash || ghostA || b.data.ghost) continue;
      const min = (a.radius + b.radius) * 0.8;
      const dx = b.pos.x - a.pos.x;
      if (dx > min || dx < -min) continue;
      const dy = b.pos.y - a.pos.y;
      if (dy > min || dy < -min) continue;
      const d2 = dx * dx + dy * dy;
      if (d2 >= min * min) continue;
      const d = Math.sqrt(d2) || 0.01;
      const overlap = (min - d) * 0.5;
      const ux = d2 === 0 ? 1 : dx / d;
      const uy = d2 === 0 ? 0 : dy / d;
      // Les unités en mouvement poussent plus que celles à l'arrêt ; un champion immobile n'est pas trop bousculé.
      const wa = a.moveTarget ? 0.6 : 0.4;
      const wb = b.moveTarget ? 0.6 : 0.4;
      const ka = (overlap * wa) / (wa + wb) * 2;
      const kb = (overlap * wb) / (wa + wb) * 2;
      nudge(nav, a, -ux * ka, -uy * ka);
      nudge(nav, b, ux * kb, uy * kb);
    }
  }
  // Structures : collision dure.
  for (const s of world.entities.values()) {
    if (!STRUCTURE_KINDS.has(s.kind) || s.dead) continue;
    for (const u of movers) {
      if (u.data.dash) continue;
      const min = s.radius + u.radius;
      const dx = u.pos.x - s.pos.x;
      const dy = u.pos.y - s.pos.y;
      const d2 = dx * dx + dy * dy;
      if (d2 >= min * min) continue;
      const d = Math.sqrt(d2) || 0.01;
      const push = min - d;
      nudge(nav, u, d2 === 0 ? push : (dx / d) * push, d2 === 0 ? 0 : (dy / d) * push);
    }
  }
}

function nudge(nav: ReturnType<typeof navGrid>, e: Entity, dx: number, dy: number): void {
  const nx = clamp(e.pos.x + dx, 0, MAP_SIZE);
  const ny = clamp(e.pos.y + dy, 0, MAP_SIZE);
  if (nav.walkable(nx, ny)) {
    e.pos.x = nx;
    e.pos.y = ny;
  } else if (nav.walkable(nx, e.pos.y)) {
    e.pos.x = nx;
  } else if (nav.walkable(e.pos.x, ny)) {
    e.pos.y = ny;
  }
}
