//! Données de la carte « Faille des Vibes » (port de `map.ts`).
//!
//! Repère : origine (0,0) en bas à gauche, x vers la droite, y vers le haut. Base bleue en
//! bas-gauche, base rouge en haut-droite. La carte est symétrique par rotation de 180° autour du
//! centre (6000,6000) : `mirror(p) = (MAP_SIZE - x, MAP_SIZE - y)`. La rivière suit la diagonale
//! `x + y = MAP_SIZE`.
//!
//! Les tables calculées au chargement du module en TypeScript sont ici des `LazyLock` (même ordre
//! d'éléments que le TS) ; les petits tableaux à clés fixes sont des `const`.

use crate::constants::MAP_SIZE;
use crate::types::{Lane, MonsterType, Team, TowerTier, Vec2};
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

pub const fn mirror(p: Vec2) -> Vec2 {
    Vec2 {
        x: MAP_SIZE - p.x,
        y: MAP_SIZE - p.y,
    }
}

const fn v(x: f64, y: f64) -> Vec2 {
    Vec2 { x, y }
}

/// `Record<Team, T>`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TeamMap<T> {
    pub blue: T,
    pub red: T,
}

impl<T> TeamMap<T> {
    pub const fn get(&self, team: Team) -> &T {
        match team {
            Team::Blue => &self.blue,
            Team::Red => &self.red,
        }
    }
}

/// `Record<Lane, T>`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LaneMap<T> {
    pub top: T,
    pub mid: T,
    pub bot: T,
}

impl<T> LaneMap<T> {
    pub const fn get(&self, lane: Lane) -> &T {
        match lane {
            Lane::Top => &self.top,
            Lane::Mid => &self.mid,
            Lane::Bot => &self.bot,
        }
    }
}

pub const FOUNTAINS: TeamMap<Vec2> = TeamMap {
    blue: v(500.0, 500.0),
    red: mirror(v(500.0, 500.0)),
};
pub const NEXUS: TeamMap<Vec2> = TeamMap {
    blue: v(1500.0, 1500.0),
    red: mirror(v(1500.0, 1500.0)),
};

/// `Lane | 'base'`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StructureLane {
    Top,
    Mid,
    Bot,
    Base,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructureSpawn {
    pub team: Team,
    pub lane: StructureLane,
    pub tier: TowerTier,
    pub pos: Vec2,
}

/// La voie top bleue fait face à la voie top rouge : en miroir, bot <-> top.
const fn swap_lane(lane: StructureLane) -> StructureLane {
    match lane {
        StructureLane::Top => StructureLane::Bot,
        StructureLane::Bot => StructureLane::Top,
        other => other,
    }
}

const fn swap_voie(lane: Lane) -> Lane {
    match lane {
        Lane::Top => Lane::Bot,
        Lane::Bot => Lane::Top,
        Lane::Mid => Lane::Mid,
    }
}

const fn tower(
    lane: StructureLane,
    tier: TowerTier,
    x: f64,
    y: f64,
) -> (StructureLane, TowerTier, Vec2) {
    (lane, tier, v(x, y))
}

const BLUE_TOWERS: [(StructureLane, TowerTier, Vec2); 11] = [
    tower(StructureLane::Top, TowerTier::Outer, 1100.0, 8000.0),
    tower(StructureLane::Top, TowerTier::Inner, 1200.0, 5600.0),
    tower(StructureLane::Top, TowerTier::Inhib, 1100.0, 3500.0),
    tower(StructureLane::Mid, TowerTier::Outer, 4900.0, 4900.0),
    tower(StructureLane::Mid, TowerTier::Inner, 4000.0, 4000.0),
    tower(StructureLane::Mid, TowerTier::Inhib, 3100.0, 3100.0),
    tower(StructureLane::Bot, TowerTier::Outer, 8000.0, 1100.0),
    tower(StructureLane::Bot, TowerTier::Inner, 5600.0, 1200.0),
    tower(StructureLane::Bot, TowerTier::Inhib, 3500.0, 1100.0),
    tower(StructureLane::Base, TowerTier::Nexus, 1900.0, 1300.0),
    tower(StructureLane::Base, TowerTier::Nexus, 1300.0, 1900.0),
];

/// 22 tours : les 11 bleues, puis les 11 rouges (rotation 180°, voies top/bot échangées).
pub static TOWERS: LazyLock<Vec<StructureSpawn>> = LazyLock::new(|| {
    let blue = BLUE_TOWERS.iter().map(|&(lane, tier, pos)| StructureSpawn {
        team: Team::Blue,
        lane,
        tier,
        pos,
    });
    let red = BLUE_TOWERS.iter().map(|&(lane, tier, pos)| StructureSpawn {
        team: Team::Red,
        lane: swap_lane(lane),
        tier,
        pos: mirror(pos),
    });
    blue.chain(red).collect()
});

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InhibitorSpawn {
    pub team: Team,
    pub lane: Lane,
    pub pos: Vec2,
}

const BLUE_INHIBS: [(Lane, Vec2); 3] = [
    (Lane::Top, v(1100.0, 2900.0)),
    (Lane::Mid, v(2650.0, 2650.0)),
    (Lane::Bot, v(2900.0, 1100.0)),
];

/// 6 inhibiteurs : les 3 bleus, puis les 3 rouges.
pub static INHIBITORS: LazyLock<Vec<InhibitorSpawn>> = LazyLock::new(|| {
    let blue = BLUE_INHIBS.iter().map(|&(lane, pos)| InhibitorSpawn {
        team: Team::Blue,
        lane,
        pos,
    });
    let red = BLUE_INHIBS.iter().map(|&(lane, pos)| InhibitorSpawn {
        team: Team::Red,
        lane: swap_voie(lane),
        pos: mirror(pos),
    });
    blue.chain(red).collect()
});

/// Chemins des voies, du côté bleu vers le côté rouge. Les minions rouges les parcourent à l'envers.
pub static LANE_PATHS: LazyLock<LaneMap<Vec<Vec2>>> = LazyLock::new(|| LaneMap {
    top: vec![
        v(1300.0, 2000.0),
        v(1100.0, 9500.0),
        v(1600.0, 10400.0),
        v(2500.0, 10900.0),
        v(10000.0, 10700.0),
    ],
    mid: vec![v(2000.0, 2000.0), v(10000.0, 10000.0)],
    bot: vec![
        v(2000.0, 1300.0),
        v(9500.0, 1100.0),
        v(10400.0, 1600.0),
        v(10900.0, 2500.0),
        v(10700.0, 10000.0),
    ],
});

pub fn lane_path_for(lane: Lane, team: Team) -> Vec<Vec2> {
    let path = LANE_PATHS.get(lane);
    match team {
        Team::Blue => path.clone(),
        Team::Red => path.iter().rev().copied().collect(),
    }
}

/// `Team | 'river'`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CampSide {
    Blue,
    Red,
    River,
}

/// Petit monstre accompagnant le grand (position relative au camp).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CampSmall {
    pub monster: MonsterType,
    pub offset: Vec2,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CampSpawn {
    /// Unique, ex. `blue_gromp`.
    pub id: String,
    pub side: CampSide,
    pub monster: MonsterType,
    pub pos: Vec2,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smalls: Option<Vec<CampSmall>>,
    pub leash_radius: f64,
}

/// Réflexion sur la diagonale x = y.
const fn swap_xy(p: Vec2) -> Vec2 {
    Vec2 { x: p.y, y: p.x }
}

/// Équivalent de `Math.round` (arrondi vers +infini sur les demi-valeurs), sans `-0`.
fn js_round(x: f64) -> f64 {
    (x + 0.5).floor() + 0.0
}

/// Camp d'une jungle bleue (sans `side` ni `id`).
struct BlueCamp {
    monster: MonsterType,
    pos: Vec2,
    smalls: Option<Vec<CampSmall>>,
    leash_radius: f64,
}

/// Quadrant OUEST de la jungle bleue (entre voie top et mid, côté buff bleu).
fn west_camps() -> Vec<BlueCamp> {
    vec![
        BlueCamp {
            monster: MonsterType::BlueBuff,
            pos: v(3800.0, 6500.0),
            smalls: None,
            leash_radius: 800.0,
        },
        BlueCamp {
            monster: MonsterType::Gromp,
            pos: v(2250.0, 6900.0),
            smalls: None,
            leash_radius: 700.0,
        },
        BlueCamp {
            monster: MonsterType::Wolves,
            pos: v(3650.0, 4950.0),
            smalls: Some(vec![
                CampSmall {
                    monster: MonsterType::WolfSmall,
                    offset: v(-170.0, 130.0),
                },
                CampSmall {
                    monster: MonsterType::WolfSmall,
                    offset: v(130.0, -170.0),
                },
            ]),
            leash_radius: 700.0,
        },
    ]
}

/// Quadrant SUD : reflet du quadrant ouest (correspondance de monstres propre).
fn south_camps() -> Vec<BlueCamp> {
    west_camps()
        .into_iter()
        .map(|c| {
            let (monster, smalls) = match c.monster {
                MonsterType::BlueBuff => (MonsterType::RedBuff, None),
                MonsterType::Gromp => (
                    MonsterType::Krugs,
                    Some(vec![CampSmall {
                        monster: MonsterType::KrugSmall,
                        offset: v(180.0, -120.0),
                    }]),
                ),
                _ => (
                    MonsterType::Raptors,
                    Some(
                        (0..5)
                            .map(|i| {
                                let a = f64::from(i) * 1.257;
                                CampSmall {
                                    monster: MonsterType::RaptorSmall,
                                    offset: v(js_round(a.cos() * 220.0), js_round(a.sin() * 220.0)),
                                }
                            })
                            .collect(),
                    ),
                ),
            };
            BlueCamp {
                monster,
                pos: swap_xy(c.pos),
                smalls,
                leash_radius: c.leash_radius,
            }
        })
        .collect()
}

/// 14 camps : 6 bleus, 6 rouges (rotation 180°), puis les 2 crabes de rivière.
pub static CAMPS: LazyLock<Vec<CampSpawn>> = LazyLock::new(|| {
    let blue: Vec<BlueCamp> = west_camps().into_iter().chain(south_camps()).collect();
    let mut camps: Vec<CampSpawn> = blue
        .iter()
        .map(|c| CampSpawn {
            id: format!("blue_{}", c.monster.as_str()),
            side: CampSide::Blue,
            monster: c.monster,
            pos: c.pos,
            smalls: c.smalls.clone(),
            leash_radius: c.leash_radius,
        })
        .collect();
    camps.extend(blue.iter().map(|c| CampSpawn {
        id: format!("red_{}", c.monster.as_str()),
        side: CampSide::Red,
        monster: c.monster,
        pos: mirror(c.pos),
        smalls: c.smalls.as_ref().map(|ss| {
            ss.iter()
                .map(|s| CampSmall {
                    monster: s.monster,
                    offset: v(0.0 - s.offset.x, 0.0 - s.offset.y),
                })
                .collect()
        }),
        leash_radius: c.leash_radius,
    }));
    camps.push(CampSpawn {
        id: "scuttle_top".into(),
        side: CampSide::River,
        monster: MonsterType::Scuttle,
        pos: v(3300.0, 8700.0),
        smalls: None,
        leash_radius: 1500.0,
    });
    camps.push(CampSpawn {
        id: "scuttle_bot".into(),
        side: CampSide::River,
        monster: MonsterType::Scuttle,
        pos: v(8700.0, 3300.0),
        smalls: None,
        leash_radius: 1500.0,
    });
    camps
});

/// Fosse d'un objectif épique.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Pit {
    pub pos: Vec2,
    pub radius: f64,
}

/// Fosses des objectifs épiques : dragon côté bot, Léviathan côté top.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Pits {
    pub dragon: Pit,
    /// Larves et Colosse apparaissent aussi ici avant le Léviathan.
    pub leviathan: Pit,
}

pub const PITS: Pits = Pits {
    dragon: Pit {
        pos: v(7900.0, 4100.0),
        radius: 650.0,
    },
    leviathan: Pit {
        pos: v(4100.0, 7900.0),
        radius: 700.0,
    },
};

/// Rivière : bande centrée sur x + y = MAP_SIZE, de demi-largeur (perpendiculaire).
pub const RIVER_HALF_WIDTH: f64 = 520.0;
pub const LANE_HALF_WIDTH: f64 = 330.0;

/// Murs : polygones CONVEXES, sens anti-horaire.
pub type Polygon = Vec<Vec2>;

fn rect(x: f64, y: f64, w: f64, h: f64) -> Polygon {
    vec![v(x, y), v(x + w, y), v(x + w, y + h), v(x, y + h)]
}

fn poly(pts: &[(f64, f64)]) -> Polygon {
    ccw(pts.iter().map(|&(x, y)| v(x, y)).collect())
}

/// Aire signée : > 0 = anti-horaire.
pub fn polygon_area(p: &[Vec2]) -> f64 {
    let mut a = 0.0;
    let Some(mut j) = p.len().checked_sub(1) else {
        return 0.0;
    };
    for (i, pi) in p.iter().enumerate() {
        a += (p[j].x - pi.x) * (p[j].y + pi.y);
        j = i;
    }
    a / 2.0
}

fn ccw(mut p: Polygon) -> Polygon {
    if polygon_area(&p) < 0.0 {
        p.reverse();
    }
    p
}

/// Quadrant ouest de la jungle bleue (entre voie top et mid). Couloirs de 350 à 600 u.
fn west_walls() -> Vec<Polygon> {
    vec![
        // bordure voie top : longs murs avec entrées (y 5050-5550 et 6300-7350)
        rect(1650.0, 3350.0, 420.0, 1700.0),
        rect(1650.0, 5550.0, 380.0, 750.0),
        poly(&[
            (1650.0, 7350.0),
            (2050.0, 7550.0),
            (2100.0, 9000.0),
            (1650.0, 9300.0),
        ]),
        // mur base / loups
        poly(&[
            (2450.0, 3700.0),
            (3050.0, 3700.0),
            (3150.0, 4250.0),
            (2450.0, 4250.0),
        ]),
        // mur central ouest (entre loups, gromp et buff bleu)
        poly(&[
            (2500.0, 4950.0),
            (3150.0, 4850.0),
            (3250.0, 5700.0),
            (2650.0, 6000.0),
            (2450.0, 5600.0),
        ]),
        // au-dessus du gromp / buff bleu, vers la rivière
        poly(&[
            (2550.0, 7450.0),
            (3250.0, 7250.0),
            (3450.0, 7500.0),
            (2900.0, 8100.0),
            (2500.0, 7950.0),
        ]),
        // mur buff bleu / mid (le long de la mid)
        poly(&[
            (4250.0, 5350.0),
            (4750.0, 5850.0),
            (4550.0, 6650.0),
            (4250.0, 6800.0),
            (4050.0, 6150.0),
        ]),
    ]
}

fn west_brushes() -> Vec<Polygon> {
    vec![
        rect(420.0, 8900.0, 340.0, 950.0), // brousse de voie top (côté rivière)
        poly(&[
            (1700.0, 9550.0),
            (2150.0, 9350.0),
            (2300.0, 9650.0),
            (1850.0, 9900.0),
        ]), // tri-brousse top
        rect(2150.0, 5150.0, 300.0, 380.0), // entrée de jungle depuis la voie top
        poly(&[
            (3300.0, 4300.0),
            (3650.0, 4300.0),
            (3700.0, 4550.0),
            (3300.0, 4550.0),
        ]), // entrée des loups
        poly(&[
            (3350.0, 7000.0),
            (3750.0, 7000.0),
            (3700.0, 7300.0),
            (3350.0, 7250.0),
        ]), // derrière le buff bleu
        poly(&[
            (4850.0, 5650.0),
            (5150.0, 5600.0),
            (5150.0, 5900.0),
            (4900.0, 5950.0),
        ]), // brousse latérale mid
        poly(&[
            (2900.0, 8650.0),
            (3300.0, 8350.0),
            (3500.0, 8550.0),
            (3100.0, 8900.0),
        ]), // rivière top
        poly(&[
            (4750.0, 7300.0),
            (5100.0, 7000.0),
            (5250.0, 7150.0),
            (4900.0, 7450.0),
        ]), // rivière, entre fosse du Léviathan et mid
    ]
}

fn reflect_polys(ps: &[Polygon]) -> Vec<Polygon> {
    ps.iter()
        .map(|p| ccw(p.iter().map(|&q| swap_xy(q)).collect()))
        .collect()
}

/// Moitié bleue (ouest + reflet sud) suivie de sa rotation 180° (la rotation conserve le sens
/// anti-horaire).
fn with_mirror(blue: Vec<Polygon>) -> Vec<Polygon> {
    let rotated: Vec<Polygon> = blue
        .iter()
        .map(|p| p.iter().map(|&q| mirror(q)).collect())
        .collect();
    blue.into_iter().chain(rotated).collect()
}

/// Ensemble des murs : moitié bleue + rotation 180°.
pub static WALLS: LazyLock<Vec<Polygon>> = LazyLock::new(|| {
    let west = west_walls();
    let south = reflect_polys(&west);
    with_mirror(west.into_iter().chain(south).collect())
});

/// Brousses (cachent les unités à la vision ennemie).
pub static BRUSHES: LazyLock<Vec<Polygon>> = LazyLock::new(|| {
    let west = west_brushes();
    let south = reflect_polys(&west);
    with_mirror(west.into_iter().chain(south).collect())
});

/// Plateforme de base : zone de soin rapide / achat.
pub const BASE_RADIUS: f64 = 2600.0;

/// Point dans un polygone (règle pair-impair, mêmes opérations que le TypeScript).
pub fn point_in_polygon(p: Vec2, poly: &[Vec2]) -> bool {
    let mut inside = false;
    let Some(mut j) = poly.len().checked_sub(1) else {
        return false;
    };
    for (i, a) in poly.iter().enumerate() {
        let b = poly[j];
        if (a.y > p.y) != (b.y > p.y) && p.x < ((b.x - a.x) * (p.y - a.y)) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

pub fn is_in_wall(p: Vec2) -> bool {
    WALLS.iter().any(|w| point_in_polygon(p, w))
}

pub fn is_in_brush(p: Vec2) -> bool {
    BRUSHES.iter().any(|b| point_in_polygon(p, b))
}
