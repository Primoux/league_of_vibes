/**
 * Picking d'entités : intersection rayon / cylindres verticaux simplifiés (hitbox). Propriétaire : claude-client.
 * Tout est en coordonnées monde Three.js. Fonctions pures, sans allocation.
 */
import type { EntitySnapshot } from '@lov/shared';
import { S } from './coords.ts';

export interface Ray3 {
  ox: number;
  oy: number;
  oz: number;
  dx: number;
  dy: number;
  dz: number;
}

/**
 * Distance le long du rayon jusqu'au cylindre vertical (axe en (cx, cz), de y=0 à y=h, rayon r), ou -1.
 * Gère l'entrée par le flanc et par le couvercle supérieur.
 */
export function rayCylinder(ray: Ray3, cx: number, cz: number, r: number, h: number): number {
  const ox = ray.ox - cx;
  const oz = ray.oz - cz;
  const a = ray.dx * ray.dx + ray.dz * ray.dz;
  let best = -1;
  if (a > 1e-12) {
    const b = 2 * (ox * ray.dx + oz * ray.dz);
    const c = ox * ox + oz * oz - r * r;
    const disc = b * b - 4 * a * c;
    if (disc >= 0) {
      const sq = Math.sqrt(disc);
      const t = (-b - sq) / (2 * a);
      if (t >= 0) {
        const y = ray.oy + ray.dy * t;
        if (y >= 0 && y <= h) best = t;
      }
    }
  }
  // couvercle supérieur
  if (Math.abs(ray.dy) > 1e-12) {
    const t = (h - ray.oy) / ray.dy;
    if (t >= 0 && (best < 0 || t < best)) {
      const x = ox + ray.dx * t;
      const z = oz + ray.dz * t;
      if (x * x + z * z <= r * r) best = t;
    }
  }
  return best;
}

const MONSTER_HITBOX: Record<string, [number, number]> = {
  dragon: [260, 300],
  elder_dragon: [300, 340],
  leviathan: [380, 420],
  herald: [260, 320],
  blue_buff: [150, 220],
  red_buff: [150, 220],
  gromp: [130, 160],
  wolves: [110, 140],
  raptors: [100, 130],
  krugs: [130, 160],
  scuttle: [110, 110],
  grub: [80, 90],
};

/** Hitbox [rayon, hauteur] en unités de jeu, généreuse pour faciliter le clic. */
export function hitboxFor(e: EntitySnapshot): [number, number] | null {
  switch (e.kind) {
    case 'champion':
      return [80, 200];
    case 'minion':
      return e.subtype === 'super' || e.subtype === 'cannon' ? [75, 140] : [55, 110];
    case 'tower':
      return [140, 520];
    case 'inhibitor':
      return [170, 200];
    case 'nexus':
      return [260, 320];
    case 'monster':
      return MONSTER_HITBOX[e.subtype ?? ''] ?? [60, 90];
    case 'ward':
      return [45, 90];
    default:
      return null; // projectiles, zones : non cliquables
  }
}

/** Hitbox en unités monde. */
export function worldHitbox(e: EntitySnapshot): [number, number] | null {
  const hb = hitboxFor(e);
  return hb ? [hb[0] * S, hb[1] * S] : null;
}
