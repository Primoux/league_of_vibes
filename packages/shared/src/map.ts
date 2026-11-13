/**
 * Données de la carte « Faille des Vibes ». Propriétaire : claude-client (géométrie), utilisée par le serveur
 * (pathfinding, spawns) et le client (rendu, minimap).
 *
 * Repère : origine (0,0) en bas à gauche, x vers la droite, y vers le haut. Base bleue en bas-gauche,
 * base rouge en haut-droite. La carte est symétrique par rotation de 180° autour du centre (6000,6000) :
 * mirror(p) = (MAP_SIZE - x, MAP_SIZE - y). La rivière suit la diagonale x + y = MAP_SIZE.
 *
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

/**
 * Réflexion sur la diagonale x = y. La jungle bleue est composée de deux quadrants : OUEST (entre la voie top
 * et la mid, côté buff bleu) et SUD (entre la mid et la voie bot, côté buff rouge). Le quadrant sud est le
 * reflet du quadrant ouest, ce qui équilibre les deux moitiés de jungle ; l'équipe rouge est obtenue par
 * rotation 180° (`mirror`).
 */
const swapXY = (p: Vec2): Vec2 => ({ x: p.y, y: p.x });

type BlueCamp = Omit<CampSpawn, 'side' | 'id'>;
const WEST_CAMPS: BlueCamp[] = [
  { monster: 'blue_buff', pos: v(3800, 6500), leashRadius: 800 },
  { monster: 'gromp', pos: v(2250, 6900), leashRadius: 700 },
  { monster: 'wolves', pos: v(3650, 4950), smalls: [{ monster: 'wolf_small', offset: v(-170, 130) }, { monster: 'wolf_small', offset: v(130, -170) }], leashRadius: 700 },
];
/** Correspondance quadrant ouest -> sud. */
const SOUTH_OF: Partial<Record<MonsterType, MonsterType>> = { blue_buff: 'red_buff', gromp: 'krugs', wolves: 'raptors', wolf_small: 'raptor_small' };
const SOUTH_CAMPS: BlueCamp[] = WEST_CAMPS.map((c) => ({
  monster: SOUTH_OF[c.monster]!,
  pos: swapXY(c.pos),
  leashRadius: c.leashRadius,
  smalls:
    c.monster === 'wolves'
      ? [0, 1, 2, 3, 4].map((i) => ({ monster: 'raptor_small' as const, offset: v(Math.round(Math.cos(i * 1.257) * 220), Math.round(Math.sin(i * 1.257) * 220)) }))
      : c.monster === 'gromp'
        ? [{ monster: 'krug_small' as const, offset: v(180, -120) }]
        : undefined,
}));
const BLUE_CAMPS: BlueCamp[] = [...WEST_CAMPS, ...SOUTH_CAMPS];

export const CAMPS: CampSpawn[] = [
  ...BLUE_CAMPS.map((c) => ({ ...c, side: 'blue' as const, id: `blue_${c.monster}` })),
  ...BLUE_CAMPS.map((c) => ({ ...c, side: 'red' as const, id: `red_${c.monster}`, pos: mirror(c.pos), smalls: c.smalls?.map((s) => ({ ...s, offset: v(-s.offset.x, -s.offset.y) })) })),
  { id: 'scuttle_top', side: 'river', monster: 'scuttle', pos: v(3300, 8700), leashRadius: 1500 },
  { id: 'scuttle_bot', side: 'river', monster: 'scuttle', pos: v(8700, 3300), leashRadius: 1500 },
];

/** Fosses des objectifs épiques. Fosse du dragon côté bot (rivière bas-droite), fosse du Léviathan côté top. */
export const PITS = {
  dragon: { pos: v(7900, 4100), radius: 650 },
  leviathan: { pos: v(4100, 7900), radius: 700 }, // larves et Colosse apparaissent aussi ici avant le Léviathan
} as const;

/** Rivière : bande centrée sur x + y = MAP_SIZE, de demi-largeur RIVER_HALF_WIDTH (perpendiculaire). */
export const RIVER_HALF_WIDTH = 520;
export const LANE_HALF_WIDTH = 330;

/** Murs : polygones CONVEXES, sens anti-horaire. */
export type Polygon = Vec2[];
const rect = (x: number, y: number, w: number, h: number): Polygon => [v(x, y), v(x + w, y), v(x + w, y + h), v(x, y + h)];
const poly = (...pts: [number, number][]): Polygon => ccw(pts.map(([x, y]) => v(x, y)));

export function polygonArea(p: Polygon): number {
  let a = 0;
  for (let i = 0, j = p.length - 1; i < p.length; j = i++) a += (p[j]!.x - p[i]!.x) * (p[j]!.y + p[i]!.y);
  return a / 2; // > 0 : anti-horaire
}
function ccw(p: Polygon): Polygon {
  return polygonArea(p) < 0 ? [...p].reverse() : p;
}

/** Quadrant ouest de la jungle bleue (entre voie top et mid). Couloirs de 350 à 600 u. */
const WEST_WALLS: Polygon[] = [
  // bordure voie top : longs murs avec entrées (y 5050-5550 et 6300-7350)
  rect(1650, 3350, 420, 1700),
  rect(1650, 5550, 380, 750),
  poly([1650, 7350], [2050, 7550], [2100, 9000], [1650, 9300]),
  // mur base / loups
  poly([2450, 3700], [3050, 3700], [3150, 4250], [2450, 4250]),
  // mur central ouest (entre loups, gromp et buff bleu)
  poly([2500, 4950], [3150, 4850], [3250, 5700], [2650, 6000], [2450, 5600]),
  // au-dessus du gromp / buff bleu, vers la rivière
  poly([2550, 7450], [3250, 7250], [3450, 7500], [2900, 8100], [2500, 7950]),
  // mur buff bleu / mid (le long de la mid)
  poly([4250, 5350], [4750, 5850], [4550, 6650], [4250, 6800], [4050, 6150]),
];

const WEST_BRUSHES: Polygon[] = [
  rect(420, 8900, 340, 950), // brousse de voie top (côté rivière)
  poly([1700, 9550], [2150, 9350], [2300, 9650], [1850, 9900]), // tri-brousse top
  rect(2150, 5150, 300, 380), // entrée de jungle depuis la voie top
  poly([3300, 4300], [3650, 4300], [3700, 4550], [3300, 4550]), // entrée des loups
  poly([3350, 7000], [3750, 7000], [3700, 7300], [3350, 7250]), // derrière le buff bleu
  poly([4850, 5650], [5150, 5600], [5150, 5900], [4900, 5950]), // brousse latérale mid
  poly([2900, 8650], [3300, 8350], [3500, 8550], [3100, 8900]), // rivière top
  poly([4750, 7300], [5100, 7000], [5250, 7150], [4900, 7450]), // rivière, entre la fosse du Léviathan et la mid
];

const reflectPolys = (ps: Polygon[]): Polygon[] => ps.map((p) => ccw(p.map(swapXY)));
const BLUE_WALLS: Polygon[] = [...WEST_WALLS, ...reflectPolys(WEST_WALLS)];
/** Ensemble des murs : moitié bleue + rotation 180° (la rotation conserve le sens anti-horaire). */
export const WALLS: Polygon[] = [...BLUE_WALLS, ...BLUE_WALLS.map((p) => p.map(mirror))];

/** Brousses (cachent les unités à la vision ennemie). */
export const BRUSHES: Polygon[] = (() => {
  const blue = [...WEST_BRUSHES, ...reflectPolys(WEST_BRUSHES)];
  return [...blue, ...blue.map((p) => p.map(mirror))];
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
