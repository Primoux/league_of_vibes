//! Déplacement des unités : pathfinding (A* sur grille 50 u), dashes, collisions.
//! Port de `packages/server/src/systems/movement.ts`.
//!
//! Contrat pour les autres systèmes :
//! - `entity.move_target = Some(p)` : aller vers ce point (chemin calculé ici ; `entity.path` = points restants).
//! - `entity.data.insert(DashData)` : dash prioritaire sur tout le reste (ignore pathfinding et CC de
//!   mouvement). Movement émet l'événement `Dash` au premier tick, puis supprime `DashData` à l'arrivée
//!   et appelle `on_arrive`.
//! - `entity.data.insert(Ghost)` : ignore les collisions entre unités (pas les structures).
//! - `world.hooks.can_move` : refuser le déplacement (root, stun, canalisation).
//!
//! Différences avec le TypeScript, imposées par l'emprunt Rust : les entités sont parcourues par id
//! (copie des ids dans un tampon réutilisé) ; pendant un pas de dash, `DashData` est sorti de la table de
//! l'entité (il y est remis avant l'émission de l'événement `Dash`, que les écouteurs peuvent donc voir).

use lov_shared::constants::MAP_SIZE;
use lov_shared::math::{angle_to, clamp, dist};
use lov_shared::types::{EntityId, EntityKind, GameEvent, Vec2};

use crate::nav::{NavGrid, NavSearch, nav_grid};
use crate::world::{Entity, System, World};

/// Rappel appelé une fois à l'arrivée d'un dash (`onArrive`).
pub type OnArrive = Box<dyn FnOnce(&mut World, EntityId) + Send + Sync>;

/// `entity.data.dash` : dash en cours.
pub struct DashData {
    pub to: Vec2,
    /// Unités / s.
    pub speed: f64,
    /// Ne s'arrête pas aux murs (sauts, blinks lents).
    pub ignore_walls: bool,
    /// Suit une entité (dash ciblé) : `to` est mis à jour chaque tick ; s'arrête à `stop_distance`
    /// (bord à bord).
    pub target_id: Option<EntityId>,
    pub stop_distance: Option<f64>,
    pub fx_key: Option<String>,
    pub on_arrive: Option<OnArrive>,
    /// Interne : événement `Dash` déjà émis.
    pub started: bool,
}

impl DashData {
    /// Dash simple vers `to` à `speed` u/s, arrêté par les murs, sans rappel.
    pub fn new(to: Vec2, speed: f64) -> Self {
        Self {
            to,
            speed,
            ignore_walls: false,
            target_id: None,
            stop_distance: None,
            fx_key: None,
            on_arrive: None,
            started: false,
        }
    }
}

/// `entity.data.ghost = true` : la présence de ce marqueur ignore les collisions entre unités.
#[derive(Debug, Clone, Copy, Default)]
pub struct Ghost;

/// `entity.data.move` : état privé du système.
#[derive(Debug, Default)]
struct MoveData {
    /// Destination pour laquelle `path` a été calculé.
    path_for: Option<Vec2>,
}

/// Nombre maximal de requêtes A* par tick (les autres unités avancent en ligne droite en attendant).
pub const ASTAR_BUDGET_PER_TICK: u32 = 24;

fn is_structure(k: EntityKind) -> bool {
    matches!(
        k,
        EntityKind::Tower | EntityKind::Inhibitor | EntityKind::Nexus
    )
}

fn is_colliding(k: EntityKind) -> bool {
    matches!(
        k,
        EntityKind::Champion | EntityKind::Minion | EntityKind::Monster
    )
}

/// `canMove(world, e)`.
pub fn can_move(world: &World, e: EntityId) -> bool {
    world.can_move(e)
}

/// Affecte `anim` sans allouer quand la valeur est déjà la bonne (ou réutilise le tampon).
fn set_anim(e: &mut Entity, anim: &str) {
    match &mut e.anim {
        Some(a) if a == anim => {}
        Some(a) => {
            a.clear();
            a.push_str(anim);
        }
        None => e.anim = Some(anim.to_owned()),
    }
}

/// `if (e.anim === 'run') e.anim = 'idle'`.
fn run_to_idle(e: &mut Entity) {
    if e.anim.as_deref() == Some("run") {
        set_anim(e, "idle");
    }
}

/// Tampons réutilisés d'un tick à l'autre.
struct Scratch {
    ids: Vec<EntityId>,
    movers: Vec<EntityId>,
    structures: Vec<(Vec2, f64)>,
    search: NavSearch,
}

/// Le système de mouvement (à placer après `input`, avant `combat`).
pub fn movement_system() -> System {
    let mut scratch = Scratch {
        ids: Vec::new(),
        movers: Vec::new(),
        structures: Vec::new(),
        search: nav_grid().new_search(),
    };
    System::new("movement", move |world, dt| update(world, dt, &mut scratch))
}

fn update(world: &mut World, dt: f64, sc: &mut Scratch) {
    let nav = nav_grid();
    let mut budget = ASTAR_BUDGET_PER_TICK;
    sc.movers.clear();
    world.ids_into(None, &mut sc.ids);

    for k in 0..sc.ids.len() {
        let id = sc.ids[k];
        let Some(e) = world.get(id) else { continue };
        if e.dead || is_structure(e.kind) {
            continue;
        }
        if e.data.contains::<DashData>() {
            step_dash(world, id, dt);
            continue;
        }
        let colliding = is_colliding(e.kind);
        let Some(target) = e.move_target else {
            if let Some(e) = world.get_mut(id) {
                run_to_idle(e);
            }
            if colliding {
                sc.movers.push(id);
            }
            continue;
        };
        if !can_move(world, id) {
            if let Some(e) = world.get_mut(id) {
                run_to_idle(e);
            }
            if colliding {
                sc.movers.push(id);
            }
            continue;
        }
        let Some(e) = world.get_mut(id) else { continue };
        // (Re)calcul du chemin si la destination a changé.
        let path_for = e.data.get_or_insert_with(MoveData::default).path_for;
        let stale = match (&e.path, path_for) {
            (Some(_), Some(pf)) => pf.x != target.x || pf.y != target.y,
            _ => true,
        };
        if stale {
            if nav.line_walkable(e.pos, target) && nav.walkable(target.x, target.y) {
                e.path = Some(vec![target]);
            } else if let (Some(p), Some(pf)) = (e.path.as_mut(), path_for)
                && p.len() > 1
                && dist(pf, target) < 150.0
            {
                // Cible qui bouge peu (poursuite) : on garde le chemin et on remplace la fin.
                let last = p.len() - 1;
                p[last] = nav.nearest_walkable(target);
            } else if budget > 0 {
                budget -= 1;
                let mut out = e.path.take().unwrap_or_default();
                nav.find_path_into(&mut sc.search, e.pos, target, &mut out);
                e.path = Some(out);
            } else {
                // Budget A* épuisé : l'unité attend le tick suivant sans bouger. Une ligne droite
                // traverserait les murs, et le chemin ne serait plus jamais recalculé (#309).
                if colliding {
                    sc.movers.push(id);
                }
                continue;
            }
            e.data.get_or_insert_with(MoveData::default).path_for = Some(target);
        }
        let step = e.stats.move_speed * dt;
        follow_path(e, step);
        if e.path.as_ref().is_none_or(|p| p.is_empty()) {
            e.move_target = None;
            e.path = None;
            e.data.get_or_insert_with(MoveData::default).path_for = None;
        }
        if colliding {
            sc.movers.push(id);
        }
    }

    resolve_collisions(world, &sc.movers, &mut sc.structures);
}

fn follow_path(e: &mut Entity, mut step: f64) {
    let Some(path) = e.path.as_mut() else { return };
    let mut moved = false;
    while step > 0.0 && !path.is_empty() {
        let wp = path[0];
        let d = dist(e.pos, wp);
        if d <= step {
            if d > 0.0 {
                e.facing = angle_to(e.pos, wp);
            }
            e.pos.x = wp.x;
            e.pos.y = wp.y;
            step -= d;
            path.remove(0);
            moved = true;
        } else {
            e.facing = angle_to(e.pos, wp);
            e.pos.x += ((wp.x - e.pos.x) / d) * step;
            e.pos.y += ((wp.y - e.pos.y) / d) * step;
            step = 0.0;
            moved = true;
        }
    }
    if moved {
        set_anim(e, "run");
    }
}

fn step_dash(world: &mut World, id: EntityId, dt: f64) {
    let nav = nav_grid();
    let Some(mut dash) = world.get_mut(id).and_then(|e| e.data.remove::<DashData>()) else {
        return;
    };
    // Rayon de la cible, relu à chaque pas (`world.get(dash.targetId)?.radius ?? 0`).
    let mut target_radius = 0.0;
    if let Some(tid) = dash.target_id {
        match world.get(tid) {
            Some(t) if !t.dead => {
                dash.to = t.pos;
                target_radius = t.radius;
            }
            _ => {
                end_dash(world, id, dash);
                return;
            }
        }
    }
    if !dash.started {
        dash.started = true;
        let Some(e) = world.get_mut(id) else { return };
        e.move_target = None;
        e.path = None;
        let from = e.pos;
        let ev = GameEvent::Dash {
            source: id,
            from,
            to: dash.to,
            duration: dist(from, dash.to) / dash.speed.max(1.0),
            fx_key: dash.fx_key.clone(),
        };
        // Le dash reste visible dans `data` pendant l'émission, comme en TypeScript.
        e.data.insert(dash);
        world.emit(ev);
        match world.get_mut(id).and_then(|e| e.data.remove::<DashData>()) {
            Some(d) => dash = d,
            None => return,
        }
    }
    let Some(e) = world.get_mut(id) else { return };
    set_anim(e, "dash");
    let stop_at = if dash.target_id.is_some() {
        dash.stop_distance.unwrap_or(0.0) + e.radius + target_radius
    } else {
        0.0
    };
    let d = dist(e.pos, dash.to);
    let remaining = (d - stop_at).max(0.0);
    let step = dash.speed * dt;
    if d > 0.0 {
        e.facing = angle_to(e.pos, dash.to);
    }
    let mut nx;
    let mut ny;
    let mut arrived = false;
    if remaining <= step {
        let k = if d > 0.0 { remaining / d } else { 0.0 };
        nx = e.pos.x + (dash.to.x - e.pos.x) * k;
        ny = e.pos.y + (dash.to.y - e.pos.y) * k;
        arrived = true;
    } else {
        nx = e.pos.x + ((dash.to.x - e.pos.x) / d) * step;
        ny = e.pos.y + ((dash.to.y - e.pos.y) / d) * step;
    }
    nx = clamp(nx, 0.0, MAP_SIZE);
    ny = clamp(ny, 0.0, MAP_SIZE);
    if !dash.ignore_walls && !nav.walkable(nx, ny) {
        let stop = nav.last_walkable_on_segment(e.pos, Vec2 { x: nx, y: ny });
        e.pos.x = stop.x;
        e.pos.y = stop.y;
        arrived = true;
    } else {
        e.pos.x = nx;
        e.pos.y = ny;
    }
    if arrived {
        if dash.ignore_walls && !nav.walkable(e.pos.x, e.pos.y) {
            let out = nav.nearest_walkable(e.pos);
            e.pos.x = out.x;
            e.pos.y = out.y;
        }
        end_dash(world, id, dash);
    } else {
        e.data.insert(dash);
    }
}

/// `DashData` a déjà été retiré de la table : on remet l'animation au repos puis on appelle `on_arrive`.
fn end_dash(world: &mut World, id: EntityId, mut dash: DashData) {
    if let Some(e) = world.get_mut(id) {
        set_anim(e, "idle");
    }
    if let Some(f) = dash.on_arrive.take() {
        f(world, id);
    }
}

/// Séparation douce entre unités (chacune recule de la moitié du chevauchement, pondérée : les unités à
/// l'arrêt bougent moins) et collisions dures avec les structures. Les unités `Ghost` ignorent les autres
/// unités.
fn resolve_collisions(world: &mut World, movers: &[EntityId], structures: &mut Vec<(Vec2, f64)>) {
    let nav = nav_grid();
    let n = movers.len();
    for i in 0..n {
        let Some(a) = world.get(movers[i]) else {
            continue;
        };
        if a.data.contains::<DashData>() {
            continue;
        }
        let ghost_a = a.data.contains::<Ghost>();
        for j in (i + 1)..n {
            let Some(a) = world.get(movers[i]) else {
                break;
            };
            let Some(b) = world.get(movers[j]) else {
                continue;
            };
            if b.data.contains::<DashData>() || ghost_a || b.data.contains::<Ghost>() {
                continue;
            }
            let min = (a.radius + b.radius) * 0.8;
            let dx = b.pos.x - a.pos.x;
            if dx > min || dx < -min {
                continue;
            }
            let dy = b.pos.y - a.pos.y;
            if dy > min || dy < -min {
                continue;
            }
            let d2 = dx * dx + dy * dy;
            if d2 >= min * min {
                continue;
            }
            let sq = d2.sqrt();
            let d = if sq == 0.0 { 0.01 } else { sq };
            let overlap = (min - d) * 0.5;
            let ux = if d2 == 0.0 { 1.0 } else { dx / d };
            let uy = if d2 == 0.0 { 0.0 } else { dy / d };
            // Les unités en mouvement poussent plus que celles à l'arrêt ; un champion immobile n'est
            // pas trop bousculé.
            let wa = if a.move_target.is_some() { 0.6 } else { 0.4 };
            let wb = if b.move_target.is_some() { 0.6 } else { 0.4 };
            let ka = (overlap * wa) / (wa + wb) * 2.0;
            let kb = (overlap * wb) / (wa + wb) * 2.0;
            if let Some(a) = world.get_mut(movers[i]) {
                nudge(nav, a, -ux * ka, -uy * ka);
            }
            if let Some(b) = world.get_mut(movers[j]) {
                nudge(nav, b, ux * kb, uy * kb);
            }
        }
    }
    // Structures : collision dure.
    structures.clear();
    structures.extend(
        world
            .iter()
            .filter(|s| is_structure(s.kind) && !s.dead)
            .map(|s| (s.pos, s.radius)),
    );
    for &(spos, sradius) in structures.iter() {
        for &uid in movers {
            let Some(u) = world.get_mut(uid) else {
                continue;
            };
            if u.data.contains::<DashData>() {
                continue;
            }
            let min = sradius + u.radius;
            let dx = u.pos.x - spos.x;
            let dy = u.pos.y - spos.y;
            let d2 = dx * dx + dy * dy;
            if d2 >= min * min {
                continue;
            }
            let sq = d2.sqrt();
            let d = if sq == 0.0 { 0.01 } else { sq };
            let push = min - d;
            nudge(
                nav,
                u,
                if d2 == 0.0 { push } else { (dx / d) * push },
                if d2 == 0.0 { 0.0 } else { (dy / d) * push },
            );
        }
    }
}

fn nudge(nav: &NavGrid, e: &mut Entity, dx: f64, dy: f64) {
    let nx = clamp(e.pos.x + dx, 0.0, MAP_SIZE);
    let ny = clamp(e.pos.y + dy, 0.0, MAP_SIZE);
    if nav.walkable(nx, ny) {
        e.pos.x = nx;
        e.pos.y = ny;
    } else if nav.walkable(nx, e.pos.y) {
        e.pos.x = nx;
    } else if nav.walkable(e.pos.x, ny) {
        e.pos.y = ny;
    }
}
