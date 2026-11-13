/**
 * Forme d'indicateur de visée d'un sort, calculée à partir de ses données (logique pure, testée).
 * Coordonnées de JEU (Vec2), comme l'input de claude-client.
 */
import type { AbilityDef, SummonerDef, Vec2 } from '@lov/shared';

export type AimShape =
  | { kind: 'line'; from: Vec2; to: Vec2; width: number }
  | { kind: 'cone'; from: Vec2; to: Vec2; halfAngle: number }
  | { kind: 'circle'; center: Vec2; radius: number };

export interface AimIndicator {
  /** Cercle de portée autour du lanceur (null = aucun). */
  range: number | null;
  aim: AimShape | null;
}

/** Point `cursor` ramené à `maxDist` de `from`. */
export function clampToRange(from: Vec2, cursor: Vec2, maxDist: number): Vec2 {
  const dx = cursor.x - from.x;
  const dy = cursor.y - from.y;
  const d = Math.hypot(dx, dy);
  if (d <= maxDist || d === 0) return { x: cursor.x, y: cursor.y };
  return { x: from.x + (dx / d) * maxDist, y: from.y + (dy / d) * maxDist };
}

/** Point à exactement `dist` de `from` dans la direction du curseur (skillshots : portée fixe). */
export function towards(from: Vec2, cursor: Vec2, dist: number): Vec2 {
  const dx = cursor.x - from.x;
  const dy = cursor.y - from.y;
  const d = Math.hypot(dx, dy) || 1;
  return { x: from.x + (dx / d) * dist, y: from.y + (dy / d) * dist };
}

const DEG = Math.PI / 180;

export function aimFor(def: Pick<AbilityDef, 'targeting' | 'range' | 'radius' | 'width' | 'params'>, caster: Vec2, cursor: Vec2): AimIndicator {
  const range = def.range;
  switch (def.targeting) {
    case 'skillshot':
    case 'global': {
      // les ultimes globaux affichent une ligne longue mais pas un cercle de 12 000 unités
      const len = Math.min(range, 3000);
      return { range: range <= 3000 ? range : null, aim: { kind: 'line', from: caster, to: towards(caster, cursor, len), width: def.width ?? 80 } };
    }
    case 'cone': {
      const deg = typeof def.params?.coneAngle === 'number' ? def.params.coneAngle : 50;
      return { range, aim: { kind: 'cone', from: caster, to: towards(caster, cursor, range), halfAngle: (deg / 2) * DEG } };
    }
    case 'ground':
      return { range, aim: { kind: 'circle', center: clampToRange(caster, cursor, range), radius: def.radius ?? 100 } };
    case 'dash': {
      const to = clampToRange(caster, cursor, range);
      if (def.radius) return { range, aim: { kind: 'circle', center: to, radius: def.radius } };
      return { range, aim: { kind: 'line', from: caster, to, width: def.width ?? 100 } };
    }
    case 'targeted':
    case 'targetedDash':
      return { range, aim: null };
    case 'self':
      return { range: null, aim: def.radius ? { kind: 'circle', center: caster, radius: def.radius } : null };
  }
}

export function aimForSummoner(def: SummonerDef, caster: Vec2, cursor: Vec2): AimIndicator {
  if (def.targeting === 'ground') return { range: def.range, aim: { kind: 'circle', center: clampToRange(caster, cursor, def.range), radius: 60 } };
  if (def.targeting === 'targeted' && def.range < 5000) return { range: def.range, aim: null };
  return { range: null, aim: null };
}
