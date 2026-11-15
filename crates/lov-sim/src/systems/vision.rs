//! Vision par équipe (PLAN §9). Port de `packages/server/src/systems/vision.ts`.
//!
//! Contrat :
//! - Rayon de vision : `SightRadius` dans `entity.data` (prioritaire) sinon défaut par type
//!   (`default_sight_radius`). Les entités neutres et les projectiles ne donnent pas de vision.
//! - Brousses : une unité dans une brousse n'est vue que par un ennemi dans la même brousse, par une
//!   balise qui la couvre, ou si elle a attaqué depuis la brousse (révélée 1 s).
//! - `Invisible` : invisible pour l'ennemi (sauf `RevealedUntil`).
//! - `RevealedUntil(t)` : visible par tous jusqu'au temps de jeu `t` (ignore brousses et invisibilité).
//! - Résultat : `VisibleTo { blue, red }` dans `entity.data`, à lire via `is_visible_to`. Les structures
//!   sont toujours visibles. Une entité est toujours visible par sa propre équipe.
//!
//! Les clés `data.sightRadius`, `data.invisible`, `data.revealedUntil`, `data.brush`,
//! `data.brushRevealUntil` et `data.visibleTo` du TypeScript deviennent les types de données ci-dessous.

use lov_shared::map::{BRUSHES, point_in_polygon};
use lov_shared::types::{EntityKind, Team, TeamOrNeutral, Vec2};

use crate::world::{Entity, System, World};

/// Durée pendant laquelle une unité qui inflige des dégâts depuis une brousse y est révélée.
pub const BRUSH_REVEAL_SECONDS: f64 = 1.0;

/// `data.sightRadius` : rayon de vision propre à l'entité (prioritaire sur le défaut du type).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SightRadius(pub f64);

/// `data.invisible = true` : la présence de ce marqueur rend l'entité invisible pour l'ennemi.
#[derive(Debug, Clone, Copy, Default)]
pub struct Invisible;

/// `data.revealedUntil` : visible par tous jusqu'à ce temps de jeu.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RevealedUntil(pub f64);

/// `data.brush` : index de la brousse contenant l'entité, -1 sinon (recalculé à chaque tick).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Brush(pub i32);

/// `data.brushRevealUntil` : l'unité a attaqué depuis une brousse, révélée jusqu'à ce temps de jeu.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BrushRevealUntil(pub f64);

/// `data.visibleTo` : résultat du calcul de vision, par équipe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct VisibleTo {
    pub blue: bool,
    pub red: bool,
}

impl VisibleTo {
    fn get(&self, team: Team) -> bool {
        match team {
            Team::Blue => self.blue,
            Team::Red => self.red,
        }
    }
}

/// `SIGHT_RADIUS[kind]` (`None` : pas de vision par défaut).
pub fn default_sight_radius(kind: EntityKind) -> Option<f64> {
    match kind {
        EntityKind::Champion => Some(1350.0),
        EntityKind::Minion => Some(1100.0),
        EntityKind::Tower => Some(1350.0),
        EntityKind::Inhibitor => Some(1000.0),
        EntityKind::Nexus => Some(1000.0),
        EntityKind::Ward => Some(900.0),
        _ => None,
    }
}

fn always_visible(k: EntityKind) -> bool {
    matches!(
        k,
        EntityKind::Tower | EntityKind::Inhibitor | EntityKind::Nexus
    )
}

fn team_of(t: Team) -> TeamOrNeutral {
    match t {
        Team::Blue => TeamOrNeutral::Blue,
        Team::Red => TeamOrNeutral::Red,
    }
}

/// `isVisibleTo(e, team, unknown)`. `unknown` : réponse si la vision n'a pas encore été calculée pour
/// cette entité (créée hors tick) ; le TypeScript le fixe à `true` par défaut.
pub fn is_visible_to(e: &Entity, team: TeamOrNeutral, unknown: bool) -> bool {
    let t = match team {
        TeamOrNeutral::Neutral => return true,
        TeamOrNeutral::Blue => Team::Blue,
        TeamOrNeutral::Red => Team::Red,
    };
    if e.team == team || always_visible(e.kind) {
        return true;
    }
    match e.data.get::<VisibleTo>() {
        Some(v) => v.get(t),
        None => unknown,
    }
}

/// `sightRadius(e)`.
pub fn sight_radius(e: &Entity) -> f64 {
    e.data
        .get::<SightRadius>()
        .map(|r| r.0)
        .or_else(|| default_sight_radius(e.kind))
        .unwrap_or(0.0)
}

/// Index de la brousse contenant le point, -1 sinon.
pub fn brush_at(x: f64, y: f64) -> i32 {
    let p = Vec2 { x, y };
    for (i, b) in BRUSHES.iter().enumerate() {
        if point_in_polygon(p, b) {
            return i as i32;
        }
    }
    -1
}

struct Viewer {
    x: f64,
    y: f64,
    r2: f64,
    brush: i32,
    ward: bool,
}

/// Le système de vision (à placer après `combat`).
pub fn vision_system() -> System {
    // Tampons réutilisés d'un tick à l'autre : spectateurs [bleu, rouge].
    let mut viewers: [Vec<Viewer>; 2] = [Vec::new(), Vec::new()];
    System::new("vision", move |world, _dt| {
        compute_vision_with(world, &mut viewers)
    })
    .with_init(|world| {
        // Attaquer depuis une brousse révèle brièvement l'attaquant.
        world.hooks.after_damage.push(Box::new(|w, ctx, _dealt| {
            let Some(src_id) = ctx.credit else { return };
            let Some(src) = w.get(src_id) else { return };
            if src.kind == EntityKind::Tower || src.team == TeamOrNeutral::Neutral {
                return;
            }
            let Some(target) = w.get(ctx.target) else {
                return;
            };
            if target.team == src.team {
                return;
            }
            if src.data.get::<Brush>().map_or(-1, |b| b.0) >= 0 {
                let until = w.time + BRUSH_REVEAL_SECONDS;
                if let Some(src) = w.get_mut(src_id) {
                    src.data.insert(BrushRevealUntil(until));
                }
            }
        }));
    })
}

/// `computeVision(world)` avec des tampons neufs (usage hors système).
pub fn compute_vision(world: &mut World) {
    let mut viewers: [Vec<Viewer>; 2] = [Vec::new(), Vec::new()];
    compute_vision_with(world, &mut viewers);
}

fn compute_vision_with(world: &mut World, viewers: &mut [Vec<Viewer>; 2]) {
    viewers[0].clear();
    viewers[1].clear();
    for e in world.iter_mut() {
        let brush = if matches!(
            e.kind,
            EntityKind::Champion | EntityKind::Minion | EntityKind::Monster | EntityKind::Ward
        ) {
            brush_at(e.pos.x, e.pos.y)
        } else {
            -1
        };
        e.data.insert(Brush(brush));
        let slot = match e.team {
            TeamOrNeutral::Neutral => continue,
            TeamOrNeutral::Blue => 0,
            TeamOrNeutral::Red => 1,
        };
        if e.dead && !always_visible(e.kind) {
            continue;
        }
        let r = sight_radius(e);
        if r <= 0.0 {
            continue;
        }
        viewers[slot].push(Viewer {
            x: e.pos.x,
            y: e.pos.y,
            r2: r * r,
            brush,
            ward: e.kind == EntityKind::Ward,
        });
    }
    let time = world.time;
    for e in world.iter_mut() {
        let blue = can_see(time, &viewers[0], Team::Blue, e);
        let red = can_see(time, &viewers[1], Team::Red, e);
        let v = e.data.get_or_insert_with(VisibleTo::default);
        v.blue = blue;
        v.red = red;
    }
}

fn can_see(time: f64, viewers: &[Viewer], team: Team, e: &Entity) -> bool {
    if e.team == team_of(team) || always_visible(e.kind) {
        return true;
    }
    let revealed = e
        .data
        .get::<RevealedUntil>()
        .map_or(f64::NEG_INFINITY, |r| r.0);
    if revealed > time {
        return true;
    }
    if e.data.contains::<Invisible>() {
        return false;
    }
    let brush = e.data.get::<Brush>().map_or(-1, |b| b.0);
    let brush_reveal = e
        .data
        .get::<BrushRevealUntil>()
        .map_or(f64::NEG_INFINITY, |r| r.0);
    let exposed = brush < 0 || brush_reveal > time;
    let x = e.pos.x;
    let y = e.pos.y;
    for vw in viewers {
        let d2 = (vw.x - x).powi(2) + (vw.y - y).powi(2);
        if d2 > vw.r2 {
            continue;
        }
        if exposed || vw.ward || vw.brush == brush {
            return true;
        }
    }
    false
}
