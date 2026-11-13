import { BRUSHES, pointInPolygon, type Team } from '@lov/shared';
import type { Entity, System, World } from '../world.ts';

/**
 * Propriétaire : claude-server. Vision par équipe (PLAN §9).
 *
 * Contrat :
 * - Rayon de vision : `data.sightRadius` (prioritaire) sinon défaut par type (SIGHT_RADIUS). Les entités neutres
 *   et les projectiles ne donnent pas de vision.
 * - Brousses : une unité dans une brousse n'est vue que par un ennemi dans la même brousse, par une balise
 *   qui la couvre, ou si elle a attaqué depuis la brousse (révélée 1 s).
 * - `data.invisible = true` : invisible pour l'ennemi (sauf `data.revealedUntil`).
 * - `data.revealedUntil = t` : visible par tous jusqu'au temps de jeu t (ignore brousses et invisibilité).
 * - Résultat : `data.visibleTo = { blue: boolean; red: boolean }`, lire via `isVisibleTo(e, team)`.
 *   Les structures sont toujours visibles. Une entité est toujours visible par sa propre équipe.
 */
export const SIGHT_RADIUS: Partial<Record<Entity['kind'], number>> = {
  champion: 1350,
  minion: 1100,
  tower: 1350,
  inhibitor: 1000,
  nexus: 1000,
  ward: 900,
};

/** Durée pendant laquelle une unité qui inflige des dégâts depuis une brousse y est révélée. */
export const BRUSH_REVEAL_SECONDS = 1;

export interface VisibleTo {
  blue: boolean;
  red: boolean;
}

const ALWAYS_VISIBLE = new Set(['tower', 'inhibitor', 'nexus']);

/** `unknown` : réponse si la vision n'a pas encore été calculée pour cette entité (créée hors tick). */
export function isVisibleTo(e: Entity, team: Team | 'neutral', unknown = true): boolean {
  if (team === 'neutral' || e.team === team || ALWAYS_VISIBLE.has(e.kind)) return true;
  const v = e.data.visibleTo as VisibleTo | undefined;
  return v ? v[team] : unknown;
}

export function sightRadius(e: Entity): number {
  return (e.data.sightRadius as number | undefined) ?? SIGHT_RADIUS[e.kind] ?? 0;
}

/** Index de la brousse contenant le point, -1 sinon. */
export function brushAt(x: number, y: number): number {
  const p = { x, y };
  for (let i = 0; i < BRUSHES.length; i++) if (pointInPolygon(p, BRUSHES[i]!)) return i;
  return -1;
}

interface Viewer {
  x: number;
  y: number;
  r2: number;
  brush: number;
  ward: boolean;
}

const viewers: Record<Team, Viewer[]> = { blue: [], red: [] };

export const visionSystem: System = {
  name: 'vision',
  init(world) {
    // Attaquer depuis une brousse révèle brièvement l'attaquant.
    world.hooks.afterDamage.push((ctx, _dealt, w) => {
      const src = ctx.credit;
      if (!src || src.kind === 'tower' || src.team === 'neutral') return;
      if (ctx.target.team === src.team) return;
      if ((src.data.brush as number | undefined ?? -1) >= 0) src.data.brushRevealUntil = w.time + BRUSH_REVEAL_SECONDS;
    });
  },
  update(world) {
    computeVision(world);
  },
};

export function computeVision(world: World): void {
  viewers.blue.length = 0;
  viewers.red.length = 0;
  for (const e of world.entities.values()) {
    const brush = e.kind === 'champion' || e.kind === 'minion' || e.kind === 'monster' || e.kind === 'ward' ? brushAt(e.pos.x, e.pos.y) : -1;
    e.data.brush = brush;
    if (e.team === 'neutral' || (e.dead && !ALWAYS_VISIBLE.has(e.kind))) continue;
    const r = sightRadius(e);
    if (r <= 0) continue;
    viewers[e.team].push({ x: e.pos.x, y: e.pos.y, r2: r * r, brush, ward: e.kind === 'ward' });
  }
  for (const e of world.entities.values()) {
    let v = e.data.visibleTo as VisibleTo | undefined;
    if (!v) e.data.visibleTo = v = { blue: false, red: false };
    v.blue = canSee(world, 'blue', e);
    v.red = canSee(world, 'red', e);
  }
}

function canSee(world: World, team: Team, e: Entity): boolean {
  if (e.team === team || ALWAYS_VISIBLE.has(e.kind)) return true;
  if (((e.data.revealedUntil as number | undefined) ?? -Infinity) > world.time) return true;
  if (e.data.invisible) return false;
  const brush = (e.data.brush as number | undefined) ?? -1;
  const exposed = brush < 0 || ((e.data.brushRevealUntil as number | undefined) ?? -Infinity) > world.time;
  const x = e.pos.x;
  const y = e.pos.y;
  for (const vw of viewers[team]) {
    const d2 = (vw.x - x) ** 2 + (vw.y - y) ** 2;
    if (d2 > vw.r2) continue;
    if (exposed || vw.ward || vw.brush === brush) return true;
  }
  return false;
}
