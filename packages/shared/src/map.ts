/**
 * Données de la carte « Faille des Vibes ». Propriétaire : claude-client (géométrie), utilisée par le serveur
 * (pathfinding, spawns) et le client (rendu, minimap).
 *
 * Repère : origine (0,0) en bas à gauche, x vers la droite, y vers le haut. Base bleue en bas-gauche,
 * base rouge en haut-droite. La carte est symétrique par rotation de 180° autour du centre (6000,6000) :
 * mirror(p) = (MAP_SIZE - x, MAP_SIZE - y). La rivière suit la diagonale x + y = MAP_SIZE.
 *
 * VERSION BOOTSTRAP : positions cohérentes mais murs/brousses provisoires — à affiner par claude-client.
 */
import { MAP_SIZE } from './constants.ts';
import type { Lane, MonsterType, Team, TowerTier, Vec2 } from './types.ts';

export const mirror = (p: Vec2): Vec2 => ({ x: MAP_SIZE - p.x, y: MAP_SIZE - p.y });
const v = (x: number, y: number): Vec2 => ({ x, y });

export const FOUNTAINS: Record<Team, Vec2> = { blue: v(500, 500), red: mirror(v(500, 500)) };
export const NEXUS: Record<Team, Vec2> = { blue: v(1500, 1500), red: mirror(v(1500, 1500)) };

export interface StructureSpawn {
  team: Team;
  lane: Lane | 'base';
  tier: TowerTier;
  pos: Vec2;
}

const BLUE_TOWERS: Omit<StructureSpawn, 'team'>[] = [
  { lane: 'top', tier: 'outer', pos: v(1100, 8000) },
  { lane: 'top', tier: 'inner', pos: v(1200, 5600) },
  { lane: 'top', tier: 'inhib', pos: v(1100, 3500) },
  { lane: 'mid', tier: 'outer', pos: v(4900, 4900) },
  { lane: 'mid', tier: 'inner', pos: v(4000, 4000) },
  { lane: 'mid', tier: 'inhib', pos: v(3100, 3100) },
  { lane: 'bot', tier: 'outer', pos: v(8000, 1100) },
  { lane: 'bot', tier: 'inner', pos: v(5600, 1200) },
  { lane: 'bot', tier: 'inhib', pos: v(3500, 1100) },
  { lane: 'base', tier: 'nexus', pos: v(1900, 1300) },
  { lane: 'base', tier: 'nexus', pos: v(1300, 1900) },
];

export const TOWERS: StructureSpawn[] = [
  ...BLUE_TOWERS.map((t) => ({ ...t, team: 'blue' as const })),
  ...BLUE_TOWERS.map((t) => ({
    ...t,
    team: 'red' as const,
    // la voie top bleue fait face à la voie top rouge : en miroir, bot <-> top
    lane: t.lane === 'top' ? ('bot' as const) : t.lane === 'bot' ? ('top' as const) : t.lane,
    pos: mirror(t.pos),
  })),
];

const BLUE_INHIBS: { lane: Lane; pos: Vec2 }[] = [
  { lane: 'top', pos: v(1100, 2900) },
  { lane: 'mid', pos: v(2650, 2650) },
  { lane: 'bot', pos: v(2900, 1100) },
];
export const INHIBITORS: { team: Team; lane: Lane; pos: Vec2 }[] = [
  ...BLUE_INHIBS.map((i) => ({ ...i, team: 'blue' as const })),
  ...BLUE_INHIBS.map((i) => ({
    team: 'red' as const,
    lane: i.lane === 'top' ? ('bot' as const) : i.lane === 'bot' ? ('top' as const) : i.lane,
    pos: mirror(i.pos),
  })),
];

/** Chemins des voies, du côté bleu vers le côté rouge. Les minions rouges les parcourent à l'envers. */
export const LANE_PATHS: Record<Lane, Vec2[]> = {
  top: [v(1300, 2000), v(1100, 9500), v(1600, 10400), v(2500, 10900), v(10000, 10700)],
  mid: [v(2000, 2000), v(10000, 10000)],
  bot: [v(2000, 1300), v(9500, 1100), v(10400, 1600), v(10900, 2500), v(10700, 10000)],
};
export const lanePathFor = (lane: Lane, team: Team): Vec2[] =>
  team === 'blue' ? LANE_PATHS[lane] : [...LANE_PATHS[lane]].reverse();

export interface CampSpawn {
  id: string; // unique, ex. 'blue_gromp'
  side: Team | 'river';
  monster: MonsterType;
  pos: Vec2;
  /** Petits monstres accompagnant le grand (positions relatives au camp). */
  smalls?: { monster: MonsterType; offset: Vec2 }[];
  leashRadius: number;
}

const BLUE_CAMPS: Omit<CampSpawn, 'side' | 'id'>[] = [
  { monster: 'blue_buff', pos: v(3700, 6100), leashRadius: 800 },
  { monster: 'gromp', pos: v(2200, 6700), leashRadius: 700 },
  { monster: 'wolves', pos: v(3600, 4900), smalls: [{ monster: 'wolf_small', offset: v(150, 150) }, { monster: 'wolf_small', offset: v(-150, 150) }], leashRadius: 700 },
  { monster: 'red_buff', pos: v(6200, 3600), leashRadius: 800 },
  { monster: 'raptors', pos: v(6400, 4700), smalls: [0, 1, 2, 3, 4].map((i) => ({ monster: 'raptor_small' as const, offset: v(Math.cos(i * 1.25) * 220, Math.sin(i * 1.25) * 220) })), leashRadius: 700 },
  { monster: 'krugs', pos: v(7900, 2300), smalls: [{ monster: 'krug_small', offset: v(200, -100) }], leashRadius: 700 },
];

export const CAMPS: CampSpawn[] = [
  ...BLUE_CAMPS.map((c) => ({ ...c, side: 'blue' as const, id: `blue_${c.monster}` })),
  ...BLUE_CAMPS.map((c) => ({ ...c, side: 'red' as const, id: `red_${c.monster}`, pos: mirror(c.pos) })),
  { id: 'scuttle_top', side: 'river', monster: 'scuttle', pos: v(3400, 8600), leashRadius: 1500 },
  { id: 'scuttle_bot', side: 'river', monster: 'scuttle', pos: v(8600, 3400), leashRadius: 1500 },
];

/** Fosses des objectifs épiques. Fosse du dragon côté bot (rivière bas-droite), fosse du Léviathan côté top. */
export const PITS = {
  dragon: { pos: v(7900, 4100), radius: 650 },
  leviathan: { pos: v(4100, 7900), radius: 700 }, // larves et Colosse apparaissent aussi ici avant le Léviathan
} as const;

/** Murs : polygones convexes (sens anti-horaire). PROVISOIRE — claude-client les affine. */
export type Polygon = Vec2[];
const rect = (x: number, y: number, w: number, h: number): Polygon => [v(x, y), v(x + w, y), v(x + w, y + h), v(x, y + h)];

const BLUE_WALLS: Polygon[] = [
  rect(2000, 3600, 900, 700), // entre base et loups
  rect(2600, 5300, 600, 900), // entre gromp et buff bleu
  rect(4300, 5200, 800, 600), // jungle top, près de mid
  rect(4700, 2100, 700, 900), // jungle bot, près du buff rouge
  rect(6900, 3000, 600, 700), // entre raptors et krugs
  rect(5300, 3900, 600, 500), // près de mid
  rect(6600, 1800, 800, 400), // derrière krugs
  rect(1800, 7600, 600, 1200), // voie top / rivière
  rect(8800, 2000, 1200, 600), // voie bot / rivière
];
export const WALLS: Polygon[] = [...BLUE_WALLS, ...BLUE_WALLS.map((p) => p.map(mirror).reverse())];

/** Brousses (cachent les unités à la vision ennemie). PROVISOIRE. */
export const BRUSHES: Polygon[] = (() => {
  const blue: Polygon[] = [
    rect(500, 9000, 300, 900), // brousse voie top
    rect(9000, 500, 900, 300), // brousse voie bot
    rect(5300, 5800, 400, 250), // brousse mid (gauche)
    rect(3000, 7300, 500, 300), // rivière top
  ];
  return [...blue, ...blue.map((p) => p.map(mirror).reverse())];
})();

/** Plateforme de base : zone de soin rapide / achat. */
export const BASE_RADIUS = 2600;

export function pointInPolygon(p: Vec2, poly: Polygon): boolean {
  let inside = false;
  for (let i = 0, j = poly.length - 1; i < poly.length; j = i++) {
    const a = poly[i]!;
    const b = poly[j]!;
    if (a.y > p.y !== b.y > p.y && p.x < ((b.x - a.x) * (p.y - a.y)) / (b.y - a.y) + a.x) inside = !inside;
  }
  return inside;
}

export const isInWall = (p: Vec2): boolean => WALLS.some((w) => pointInPolygon(p, w));
export const isInBrush = (p: Vec2): boolean => BRUSHES.some((b) => pointInPolygon(p, b));
