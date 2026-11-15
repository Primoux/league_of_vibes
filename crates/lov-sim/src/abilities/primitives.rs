//! Primitives de sorts : port de `packages/server/src/abilities/primitives.ts`.
//!
//! Projectiles, zones, dashes, blinks, tâches différées, zones d'impact différé, liens, cônes. Tout
//! l'état est dans le monde, jamais global :
//!
//! | TypeScript (`WeakMap<World, Runtime>`)   | Rust                                                           |
//! |------------------------------------------|----------------------------------------------------------------|
//! | `runtime.tasks`                          | file `TaskQueue` dans `data` de l'entité **hôte** de la tâche  |
//! | `runtime.projectiles`                    | `Projectile` dans `data` de l'entité projectile                |
//! | `runtime.zones`                          | `Zone` dans `data` de l'entité zone                            |
//! | `runtime.dashes`                         | `AbilityDash` dans `data` de l'entité qui fonce                |
//!
//! Écarts imposés par Rust, à connaître en traduisant un kit :
//! - les entités sont désignées par `EntityId` et les rappels reçoivent `&mut World` ;
//! - `schedule` prend l'**hôte** de la tâche (en pratique `ctx.source`) : la tâche vit dans ses `data`
//!   et disparaît si l'hôte quitte le monde (les champions n'en sortent jamais) ;
//! - les rappels sont `FnMut/FnOnce + Send + 'static` (ils capturent des ids, des `Copy` ou des `Arc`) ;
//! - le moteur suppose que le système `movement` avance les dashes (`startDash` TS prenait l'autre
//!   branche si `movementSystem` était absent du monde ; les systèmes ne sont pas stockés dans `World`).
//!
//! Ordre d'exécution (identique au TS) : tâches échues, projectiles, zones, dashes. À instant égal,
//! la tâche planifiée en dernier s'exécute en premier (le TS parcourt sa liste à l'envers avant un tri
//! stable) : un compteur monotone (`SEQ`), seul état global du module, ne sert qu'à reproduire cet ordre.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use lov_shared::constants::MAP_SIZE;
use lov_shared::map::{WALLS, point_in_polygon};
use lov_shared::math::{clamp, dist, dist2, move_towards};
use lov_shared::schema::CcType;
use lov_shared::types::{EntityId, EntityKind, GameEvent, TeamOrNeutral, Vec2};

use crate::combat::buffs::{Revealed, Stasis, can_act, has_cc};
use crate::systems::movement::DashData;
use crate::systems::vision::{Invisible, RevealedUntil, is_visible_to};
use crate::world::{Entity, World};

/// Propriétaire d'un projectile ou d'une zone (`data.ownerId` du TS ; distinct de `damage::Owner`,
/// que le TS ne posait pas sur ces entités).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OwnerId(pub EntityId);

/// Conteneur `Sync` pour des rappels `Send` rangés dans `entity.data` (qui exige `Send + Sync`).
/// On ne l'utilise que par propriété ou `&mut` : le verrou n'est jamais pris.
struct SyncCell<T>(Mutex<T>);

impl<T> SyncCell<T> {
    fn new(v: T) -> Self {
        Self(Mutex::new(v))
    }

    fn get(&mut self) -> &mut T {
        self.0.get_mut().unwrap_or_else(|p| p.into_inner())
    }
}

static SEQ: AtomicU64 = AtomicU64::new(1);

fn next_seq() -> u64 {
    SEQ.fetch_add(1, Ordering::Relaxed)
}

// ------------------------------------------------------------------------------------------------
// Requêtes
// ------------------------------------------------------------------------------------------------

pub fn is_unit(e: &Entity) -> bool {
    matches!(
        e.kind,
        EntityKind::Champion | EntityKind::Minion | EntityKind::Monster
    )
}

fn is_enemy_of(source: EntityId, team: TeamOrNeutral, target: &Entity) -> bool {
    !target.dead
        && target.hp > 0.0
        && source != target.id
        && team != target.team
        && is_unit(target)
        && !target.data.contains::<Stasis>()
}

/// `isEnemy(source, target)`.
pub fn is_enemy(source: &Entity, target: &Entity) -> bool {
    is_enemy_of(source.id, source.team, target)
}

/// `isVisibleTarget(world, source, target)`.
pub fn is_visible_target(world: &World, source: &Entity, target: &Entity) -> bool {
    if source.team == target.team {
        return true;
    }
    let revealed_until = target
        .data
        .get::<RevealedUntil>()
        .map_or(f64::NEG_INFINITY, |r| r.0);
    if revealed_until > world.time || target.data.contains::<Revealed>() {
        return true;
    }
    if target.data.contains::<Invisible>() {
        return false;
    }
    is_visible_to(target, source.team, true)
}

/// `validPoint(p)` : présent et fini.
pub fn valid_point(p: Option<Vec2>) -> bool {
    p.is_some_and(|p| p.x.is_finite() && p.y.is_finite())
}

/// Vecteur unitaire de `from` vers `to` ; `fallback` (angle) si les points sont confondus.
pub fn direction(from: Vec2, to: Vec2, fallback: f64) -> Vec2 {
    let d = dist(from, to);
    if d < 1e-6 {
        Vec2 {
            x: fallback.cos(),
            y: fallback.sin(),
        }
    } else {
        Vec2 {
            x: (to.x - from.x) / d,
            y: (to.y - from.y) / d,
        }
    }
}

/// Point à `range` au plus de `from` vers `to`, borné à la carte.
pub fn endpoint(from: Vec2, to: Vec2, range: f64) -> Vec2 {
    let p = move_towards(from, to, range);
    Vec2 {
        x: clamp(p.x, 0.0, MAP_SIZE),
        y: clamp(p.y, 0.0, MAP_SIZE),
    }
}

/// Ennemis de `source` dont le corps touche le disque (`pos`, `radius`), dans l'ordre des entités,
/// ajoutés à `out` (vidé d'abord). Évite l'allocation d'`enemies_in_radius`.
pub fn enemies_in_radius_into(
    world: &World,
    source: EntityId,
    pos: Vec2,
    radius: f64,
    out: &mut Vec<EntityId>,
) {
    out.clear();
    let Some(src) = world.get(source) else { return };
    for e in world.iter() {
        let reach = radius + e.radius;
        if is_enemy(src, e) && dist2(e.pos, pos) <= reach * reach {
            out.push(e.id);
        }
    }
}

/// `enemiesInRadius(world, source, pos, radius)`.
pub fn enemies_in_radius(world: &World, source: EntityId, pos: Vec2, radius: f64) -> Vec<EntityId> {
    let mut out = Vec::new();
    enemies_in_radius_into(world, source, pos, radius, &mut out);
    out
}

/// `alliesInRadius(world, source, pos, radius)` : champions vivants de l'équipe de `source`.
pub fn allies_in_radius(world: &World, source: EntityId, pos: Vec2, radius: f64) -> Vec<EntityId> {
    let mut out = Vec::new();
    let Some(src) = world.get(source) else {
        return out;
    };
    for e in world.query(Some(EntityKind::Champion)) {
        let reach = radius + e.radius;
        if !e.dead && e.team == src.team && dist2(e.pos, pos) <= reach * reach {
            out.push(e.id);
        }
    }
    out
}

/// Premier point d'intersection d'un segment balayé avec un disque, origine comprise (`t` dans [0, 1]).
pub fn segment_hit(from: Vec2, to: Vec2, pos: Vec2, radius: f64) -> Option<f64> {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    let (ox, oy) = (from.x - pos.x, from.y - pos.y);
    let c = ox * ox + oy * oy - radius * radius;
    if c <= 0.0 {
        return Some(0.0);
    }
    let a = dx * dx + dy * dy;
    if a < 1e-12 {
        return None;
    }
    let b = 2.0 * (ox * dx + oy * dy);
    let discriminant = b * b - 4.0 * a * c;
    if discriminant < 0.0 {
        return None;
    }
    let t = (-b - discriminant.sqrt()) / (2.0 * a);
    (0.0..=1.0).contains(&t).then_some(t)
}

// ------------------------------------------------------------------------------------------------
// Tâches différées
// ------------------------------------------------------------------------------------------------

struct Task {
    at: f64,
    seq: u64,
    run: Box<dyn FnOnce(&mut World) + Send>,
    cancelled: Arc<AtomicBool>,
    cancel_on_death: bool,
}

#[derive(Default)]
struct TaskQueue {
    tasks: Vec<Task>,
}

/// Le `() => void` renvoyé par `schedule` : annule la tâche si elle n'a pas encore commencé.
#[derive(Debug, Clone)]
pub struct TaskHandle(Arc<AtomicBool>);

impl TaskHandle {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

/// `schedule(world, delay, fn)` : exécute `f` dans `delay` secondes (au plus tôt au prochain tick).
/// `host` est l'entité qui porte la tâche (voir l'en-tête du module) ; si elle n'existe pas, la tâche
/// est annulée d'emblée.
pub fn schedule(
    world: &mut World,
    host: EntityId,
    delay: f64,
    f: impl FnOnce(&mut World) + Send + 'static,
) -> TaskHandle {
    schedule_with(world, host, delay, false, f)
}

/// `schedule(world, delay, fn, owner, cancelOnDeath)` : `host` est aussi le propriétaire ; avec
/// `cancel_on_death`, la tâche est abandonnée si `host` meurt avant l'échéance.
pub fn schedule_with(
    world: &mut World,
    host: EntityId,
    delay: f64,
    cancel_on_death: bool,
    f: impl FnOnce(&mut World) + Send + 'static,
) -> TaskHandle {
    let cancelled = Arc::new(AtomicBool::new(false));
    let at = world.time + delay.max(0.0);
    match world.get_mut(host) {
        Some(e) => e
            .data
            .get_or_insert_with(|| SyncCell::new(TaskQueue::default()))
            .get()
            .tasks
            .push(Task {
                at,
                seq: next_seq(),
                run: Box::new(f),
                cancelled: Arc::clone(&cancelled),
                cancel_on_death,
            }),
        None => cancelled.store(true, Ordering::Relaxed),
    }
    TaskHandle(cancelled)
}

// ------------------------------------------------------------------------------------------------
// Projectiles
// ------------------------------------------------------------------------------------------------

pub type ProjectileHitFn = Box<dyn FnMut(&mut World, EntityId, EntityId) + Send>;
pub type ProjectileEndFn = Box<dyn FnOnce(&mut World, Vec2) + Send>;

pub struct ProjectileOptions {
    pub source: EntityId,
    /// Défaut : position de `source`.
    pub from: Option<Vec2>,
    pub to: Vec2,
    pub speed: f64,
    pub width: f64,
    pub fx_key: String,
    pub pierce: bool,
    pub stop_on_champion: bool,
    pub only_champions: bool,
    /// Projectile à tête chercheuse qui ne touche que cette cible et s'arrête si elle disparaît.
    pub target: Option<EntityId>,
    /// Le point d'arrivée suit cette entité tant qu'elle existe.
    pub homing_to: Option<EntityId>,
    /// `(cible, projectile)`.
    pub on_hit: ProjectileHitFn,
    pub on_end: Option<ProjectileEndFn>,
}

impl ProjectileOptions {
    /// Projectile simple (sans perçage ni cible) ; les drapeaux se règlent sur les champs.
    pub fn new(
        source: EntityId,
        to: Vec2,
        speed: f64,
        width: f64,
        fx_key: impl Into<String>,
        on_hit: impl FnMut(&mut World, EntityId, EntityId) + Send + 'static,
    ) -> Self {
        Self {
            source,
            from: None,
            to,
            speed,
            width,
            fx_key: fx_key.into(),
            pierce: false,
            stop_on_champion: false,
            only_champions: false,
            target: None,
            homing_to: None,
            on_hit: Box::new(on_hit),
            on_end: None,
        }
    }
}

struct Projectile {
    source: EntityId,
    team: TeamOrNeutral,
    speed: f64,
    width: f64,
    fx_key: String,
    pierce: bool,
    stop_on_champion: bool,
    only_champions: bool,
    target: Option<EntityId>,
    homing_to: Option<EntityId>,
    on_hit: ProjectileHitFn,
    on_end: Option<ProjectileEndFn>,
    hit: Vec<EntityId>,
    end: Vec2,
    finished: bool,
}

/// Crée un projectile et émet `projectileSpawn`. `None` seulement si `source` n'existe plus.
pub fn spawn_projectile(world: &mut World, options: ProjectileOptions) -> Option<EntityId> {
    let src = world.get(options.source)?;
    let from = options.from.unwrap_or(src.pos);
    let team = src.team;
    let mut e = Entity::new(
        EntityKind::Projectile,
        team,
        from,
        options.width / 2.0,
        1.0,
        0.0,
        src.stats.clone(),
    );
    e.subtype = Some(options.fx_key.clone());
    e.data.insert(OwnerId(options.source));
    let id = world.spawn(e);
    let p = Projectile {
        source: options.source,
        team,
        speed: options.speed,
        width: options.width,
        fx_key: options.fx_key.clone(),
        pierce: options.pierce,
        stop_on_champion: options.stop_on_champion,
        only_champions: options.only_champions,
        target: options.target,
        homing_to: options.homing_to,
        on_hit: options.on_hit,
        on_end: options.on_end,
        hit: Vec::new(),
        end: options.to,
        finished: false,
    };
    if let Some(e) = world.get_mut(id) {
        e.data.insert(SyncCell::new(p));
    }
    world.emit(GameEvent::ProjectileSpawn {
        id,
        source: options.source,
        fx_key: options.fx_key,
        from,
        to: options.to,
        speed: options.speed,
        width: options.width,
    });
    Some(id)
}

fn finish_projectile(world: &mut World, id: EntityId, p: &mut Projectile, pos: Vec2) {
    if p.finished {
        return;
    }
    p.finished = true;
    world.remove(id);
    if let Some(f) = p.on_end.take() {
        f(world, pos);
    }
}

/// Fait avancer un projectile ; renvoie vrai s'il est terminé (retiré du monde).
fn step_projectile(
    world: &mut World,
    id: EntityId,
    dt: f64,
    p: &mut Projectile,
    hits: &mut Vec<(f64, EntityId)>,
) -> bool {
    let Some(here) = world.get(id).map(|e| e.pos) else {
        return true;
    };
    if let Some(t) = p.target {
        match world.get(t) {
            Some(te) if !te.dead => p.end = te.pos,
            _ => {
                finish_projectile(world, id, p, here);
                return true;
            }
        }
    }
    if let Some(h) = p.homing_to
        && let Some(he) = world.get(h)
    {
        p.end = he.pos;
    }
    let from = here;
    let to = move_towards(from, p.end, p.speed * dt);
    hits.clear();
    for target in world.iter() {
        if !is_enemy_of(p.source, p.team, target)
            || p.hit.contains(&target.id)
            || (p.only_champions && target.champ.is_none())
            || p.target.is_some_and(|t| t != target.id)
        {
            continue;
        }
        if let Some(t) = segment_hit(from, to, target.pos, p.width / 2.0 + target.radius) {
            hits.push((t, target.id));
        }
    }
    hits.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let mut stopped: Option<Vec2> = None;
    for &(t, target) in hits.iter() {
        p.hit.push(target);
        let pos = Vec2 {
            x: from.x + (to.x - from.x) * t,
            y: from.y + (to.y - from.y) * t,
        };
        let target_is_champion = world.get(target).is_some_and(|e| e.champ.is_some());
        world.emit(GameEvent::ProjectileHit {
            id,
            target: Some(target),
            pos,
            fx_key: p.fx_key.clone(),
        });
        (p.on_hit)(world, target, id);
        if (!p.pierce && !p.stop_on_champion) || (p.stop_on_champion && target_is_champion) {
            stopped = Some(pos);
            break;
        }
    }
    let new_pos = stopped.unwrap_or(to);
    if let Some(e) = world.get_mut(id) {
        e.pos = new_pos;
    }
    if stopped.is_some() || dist2(to, p.end) < 1e-6 {
        finish_projectile(world, id, p, new_pos);
        return true;
    }
    false
}

// ------------------------------------------------------------------------------------------------
// Zones
// ------------------------------------------------------------------------------------------------

pub type ZoneTickFn = Box<dyn FnMut(&mut World, EntityId, f64) + Send>;
pub type ZoneEndFn = Box<dyn FnOnce(&mut World) + Send>;

pub struct ZoneOptions {
    pub source: EntityId,
    pub pos: Vec2,
    pub radius: f64,
    pub duration: f64,
    /// Défaut 1 s.
    pub interval: Option<f64>,
    pub fx_key: String,
    /// La zone suit cette entité et disparaît avec elle.
    pub follow: Option<EntityId>,
    pub invisible: bool,
    /// `(zone, secondes écoulées depuis la création)`.
    pub on_tick: ZoneTickFn,
    pub on_end: Option<ZoneEndFn>,
}

impl ZoneOptions {
    pub fn new(
        source: EntityId,
        pos: Vec2,
        radius: f64,
        duration: f64,
        fx_key: impl Into<String>,
        on_tick: impl FnMut(&mut World, EntityId, f64) + Send + 'static,
    ) -> Self {
        Self {
            source,
            pos,
            radius,
            duration,
            interval: None,
            fx_key: fx_key.into(),
            follow: None,
            invisible: false,
            on_tick: Box::new(on_tick),
            on_end: None,
        }
    }
}

struct Zone {
    follow: Option<EntityId>,
    interval: Option<f64>,
    start: f64,
    next: f64,
    expires: f64,
    /// `None` pendant que le rappel s'exécute (il est sorti pour recevoir `&mut World`).
    on_tick: Option<ZoneTickFn>,
    on_end: Option<ZoneEndFn>,
}

/// Crée une zone et émet `areaEffect`. `None` seulement si `source` n'existe plus.
pub fn spawn_zone(world: &mut World, options: ZoneOptions) -> Option<EntityId> {
    let src = world.get(options.source)?;
    let mut e = Entity::new(
        EntityKind::Zone,
        src.team,
        options.pos,
        options.radius,
        1.0,
        0.0,
        src.stats.clone(),
    );
    e.subtype = Some(options.fx_key.clone());
    e.data.insert(OwnerId(options.source));
    if options.invisible {
        e.data.insert(Invisible);
    }
    let id = world.spawn(e);
    let zone = Zone {
        follow: options.follow,
        interval: options.interval,
        start: world.time,
        next: world.time + options.interval.unwrap_or(1.0),
        expires: world.time + options.duration,
        on_tick: Some(options.on_tick),
        on_end: options.on_end,
    };
    if let Some(e) = world.get_mut(id) {
        e.data.insert(SyncCell::new(zone));
    }
    world.emit(GameEvent::AreaEffect {
        source: options.source,
        fx_key: options.fx_key,
        pos: options.pos,
        radius: options.radius,
        duration: options.duration,
    });
    Some(id)
}

/// Retire une zone (sans effet si ce n'en est pas une) puis appelle son `on_end`.
pub fn remove_zone(world: &mut World, id: EntityId) {
    let Some(mut zone) = world
        .get_mut(id)
        .and_then(|e| e.data.remove::<SyncCell<Zone>>())
    else {
        return;
    };
    world.remove(id);
    if let Some(f) = zone.get().on_end.take() {
        f(world);
    }
}

/// Copie des champs d'une zone nécessaires à la boucle (le rappel, lui, sort de la zone).
fn zone_times(world: &mut World, id: EntityId) -> Option<(f64, f64, f64)> {
    let z = world.get_mut(id)?.data.get_mut::<SyncCell<Zone>>()?.get();
    Some((z.next, z.expires, z.start))
}

fn update_zone(world: &mut World, id: EntityId) {
    let Some(follow) = world
        .get_mut(id)
        .and_then(|e| e.data.get_mut::<SyncCell<Zone>>())
        .map(|z| z.get().follow)
    else {
        return;
    };
    if let Some(f) = follow {
        match world.get(f) {
            Some(fe) if !fe.dead => {
                let pos = fe.pos;
                if let Some(e) = world.get_mut(id) {
                    e.pos = pos;
                }
            }
            _ => {
                remove_zone(world, id);
                return;
            }
        }
    }
    loop {
        let Some((next, expires, start)) = zone_times(world, id) else {
            return;
        };
        if next > world.time.min(expires) + 1e-8 {
            break;
        }
        let Some(mut on_tick) = world
            .get_mut(id)
            .and_then(|e| e.data.get_mut::<SyncCell<Zone>>())
            .and_then(|z| z.get().on_tick.take())
        else {
            return;
        };
        on_tick(world, id, next - start);
        // La zone a pu être retirée par son propre rappel : alors il n'y a plus rien à avancer.
        let Some(z) = world
            .get_mut(id)
            .and_then(|e| e.data.get_mut::<SyncCell<Zone>>())
        else {
            return;
        };
        let z = z.get();
        z.on_tick = Some(on_tick);
        z.next += (1.0f64 / 30.0).max(z.interval.unwrap_or(1.0));
    }
    if zone_times(world, id).is_some_and(|(_, expires, _)| world.time + 1e-8 >= expires) {
        remove_zone(world, id);
    }
}

// ------------------------------------------------------------------------------------------------
// Dashes, blinks
// ------------------------------------------------------------------------------------------------

pub type DashHitFn = Box<dyn FnMut(&mut World, EntityId) + Send>;
pub type DashEndFn = Box<dyn FnOnce(&mut World) + Send>;

pub struct DashOptions {
    pub fx_key: String,
    pub speed: f64,
    /// Largeur de collision (défaut : le diamètre du lanceur).
    pub width: Option<f64>,
    pub stop_on_first: bool,
    /// Ignore les contrôles (projection d'une cible…).
    pub unstoppable: bool,
    pub cross_walls: bool,
    pub target: Option<EntityId>,
    pub on_hit: Option<DashHitFn>,
    pub on_end: Option<DashEndFn>,
}

impl DashOptions {
    pub fn new(fx_key: impl Into<String>, speed: f64) -> Self {
        Self {
            fx_key: fx_key.into(),
            speed,
            width: None,
            stop_on_first: false,
            unstoppable: false,
            cross_walls: false,
            target: None,
            on_hit: None,
            on_end: None,
        }
    }
}

struct AbilityDash {
    team: TeamOrNeutral,
    seq: u64,
    previous: Vec2,
    hit: Vec<EntityId>,
    width: Option<f64>,
    stop_on_first: bool,
    unstoppable: bool,
    on_hit: Option<DashHitFn>,
    on_end: Option<DashEndFn>,
}

/// Déplacement radius-aware sur les arêtes exactes des murs (pas de grille).
pub fn is_walkable(pos: Vec2, radius: f64) -> bool {
    if pos.x < radius || pos.y < radius || pos.x > MAP_SIZE - radius || pos.y > MAP_SIZE - radius {
        return false;
    }
    for wall in WALLS.iter() {
        if point_in_polygon(pos, wall) {
            return false;
        }
        for i in 0..wall.len() {
            let a = wall[i];
            let b = wall[(i + 1) % wall.len()];
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let length2 = dx * dx + dy * dy;
            let t = if length2 != 0.0 {
                clamp(
                    ((pos.x - a.x) * dx + (pos.y - a.y) * dy) / length2,
                    0.0,
                    1.0,
                )
            } else {
                0.0
            };
            let nearest = Vec2 {
                x: a.x + t * dx,
                y: a.y + t * dy,
            };
            if dist2(pos, nearest) < radius * radius {
                return false;
            }
        }
    }
    true
}

/// Atterrissage valide le plus proche dans la portée ; peut traverser un mur, jamais s'y poser.
pub fn valid_landing(from: Vec2, desired: Vec2, range: f64, radius: f64) -> Vec2 {
    let point = endpoint(from, desired, range);
    if is_walkable(point, radius) {
        return point;
    }
    let mut r = 5.0;
    while r <= range * 2.0 + radius {
        let count = 16.0f64.max((2.0 * std::f64::consts::PI * r / 10.0).ceil()) as u32;
        for i in 0..count {
            let angle = f64::from(i) / f64::from(count) * 2.0 * std::f64::consts::PI;
            let candidate = Vec2 {
                x: point.x + angle.cos() * r,
                y: point.y + angle.sin() * r,
            };
            if dist2(candidate, from) <= range * range + 1e-6 && is_walkable(candidate, radius) {
                return candidate;
            }
        }
        r += 5.0;
    }
    from
}

/// Téléportation immédiate vers le point valide le plus proche (émet `dash` de durée 0).
pub fn blink(world: &mut World, source: EntityId, point: Vec2, range: f64, fx_key: &str) {
    let Some(e) = world.get_mut(source) else {
        return;
    };
    let from = e.pos;
    let to = valid_landing(from, point, range, e.radius);
    e.pos = to;
    e.move_target = None;
    e.path = None;
    world.emit(GameEvent::Dash {
        source,
        from,
        to,
        duration: 0.0,
        fx_key: Some(fx_key.to_owned()),
    });
}

/// Lance un dash : `DashData` est avancé par le système `movement`, qui émet l'événement `dash`.
pub fn start_dash(world: &mut World, source: EntityId, desired: Vec2, options: DashOptions) {
    let Some(src) = world.get(source) else { return };
    let (from, radius, team) = (src.pos, src.radius, src.team);
    let mut to = valid_landing(from, desired, dist(from, desired), radius);
    if !options.cross_walls {
        let length = dist(from, to);
        let mut step = 10.0;
        while step <= length {
            let point = move_towards(from, to, step);
            if !is_walkable(point, radius) {
                to = move_towards(from, to, (step - 10.0f64).max(0.0));
                break;
            }
            step += 10.0;
        }
    }
    let mut data = DashData::new(to, options.speed);
    data.ignore_walls = options.cross_walls;
    data.target_id = options.target;
    data.stop_distance = options.target.map(|_| 0.0);
    data.fx_key = Some(options.fx_key);
    data.on_arrive = Some(Box::new(|world, id| {
        resolve_dash_hits(world, id);
        finish_dash(world, id);
    }));
    let Some(e) = world.get_mut(source) else {
        return;
    };
    // Un dash déjà suivi garde sa place dans l'ordre de traitement (le `Map.set` du TS).
    let seq = e
        .data
        .get_mut::<SyncCell<AbilityDash>>()
        .map_or_else(next_seq, |d| d.get().seq);
    e.data.insert(SyncCell::new(AbilityDash {
        team,
        seq,
        previous: from,
        hit: Vec::new(),
        width: options.width,
        stop_on_first: options.stop_on_first,
        unstoppable: options.unstoppable,
        on_hit: options.on_hit,
        on_end: options.on_end,
    }));
    e.data.insert(data);
    e.move_target = None;
    e.path = None;
    e.anim = Some("dash".to_owned());
}

/// Termine un dash dont l'état a déjà été sorti de `data` : `DashData`, animation, `on_end`.
fn finish_dash_owned(world: &mut World, id: EntityId, dash: &mut AbilityDash) {
    if let Some(e) = world.get_mut(id) {
        e.data.remove::<DashData>();
        if !e.dead {
            e.anim = Some("idle".to_owned());
        }
    }
    if let Some(f) = dash.on_end.take() {
        f(world);
    }
}

fn finish_dash(world: &mut World, id: EntityId) {
    let Some(mut cell) = world
        .get_mut(id)
        .and_then(|e| e.data.remove::<SyncCell<AbilityDash>>())
    else {
        return;
    };
    finish_dash_owned(world, id, cell.get());
}

fn resolve_dash_hits(world: &mut World, id: EntityId) {
    let Some(mut cell) = world
        .get_mut(id)
        .and_then(|e| e.data.remove::<SyncCell<AbilityDash>>())
    else {
        return;
    };
    let dash = cell.get();
    if dash.on_hit.is_none() {
        if let Some(e) = world.get_mut(id) {
            e.data.insert(cell);
        }
        return;
    }
    let Some(e) = world.get(id) else { return };
    let (from, to) = (dash.previous, e.pos);
    let reach = dash.width.unwrap_or(e.radius * 2.0) / 2.0;
    let mut hits: Vec<(f64, EntityId)> = Vec::new();
    for target in world.iter() {
        if !is_enemy_of(id, dash.team, target) || dash.hit.contains(&target.id) {
            continue;
        }
        if let Some(t) = segment_hit(from, to, target.pos, reach + target.radius) {
            hits.push((t, target.id));
        }
    }
    hits.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let mut finished = false;
    for (t, target) in hits {
        dash.hit.push(target);
        if dash.stop_on_first
            && let Some(e) = world.get_mut(id)
        {
            e.pos = Vec2 {
                x: from.x + (to.x - from.x) * t,
                y: from.y + (to.y - from.y) * t,
            };
        }
        if let Some(f) = dash.on_hit.as_mut() {
            f(world, target);
        }
        if dash.stop_on_first {
            finished = true;
            break;
        }
    }
    if finished {
        finish_dash_owned(world, id, dash);
        return;
    }
    if let Some(e) = world.get_mut(id) {
        dash.previous = e.pos;
        // Un rappel a pu lancer un nouveau dash pour ce lanceur : il prime sur l'ancien.
        if !e.data.contains::<SyncCell<AbilityDash>>() {
            e.data.insert(cell);
        }
    }
}

// ------------------------------------------------------------------------------------------------
// Zones d'impact différé, liens, cônes
// ------------------------------------------------------------------------------------------------

/// Télégraphe immédiat (`<fx_key>_telegraph`), puis `on_impact` après `delay` secondes (porté par `source`).
pub fn delayed_area(
    world: &mut World,
    source: EntityId,
    pos: Vec2,
    radius: f64,
    delay: f64,
    fx_key: &str,
    on_impact: impl FnOnce(&mut World) + Send + 'static,
) {
    world.emit(GameEvent::AreaEffect {
        source,
        pos,
        radius,
        duration: delay,
        fx_key: format!("{fx_key}_telegraph"),
    });
    let fx_key = fx_key.to_owned();
    schedule(world, source, delay, move |world| {
        world.emit(GameEvent::AreaEffect {
            source,
            pos,
            radius,
            duration: 0.25,
            fx_key,
        });
        on_impact(world);
    });
}

/// Cône devant `source` vers `toward` : appelle `hit` pour chaque ennemi touché.
pub fn cone(
    world: &mut World,
    source: EntityId,
    toward: Vec2,
    range: f64,
    angle_degrees: f64,
    mut hit: impl FnMut(&mut World, EntityId),
) {
    let Some(src) = world.get(source) else { return };
    let (pos, facing) = (src.pos, src.facing);
    let dir = direction(pos, toward, facing);
    let cos = (angle_degrees * std::f64::consts::PI / 360.0).cos();
    for target in enemies_in_radius(world, source, pos, range) {
        let Some(t) = world.get(target) else { continue };
        let d = dist(pos, t.pos);
        let rel = direction(pos, t.pos, 0.0);
        if d <= t.radius || dir.x * rel.x + dir.y * rel.y >= cos - t.radius / d.max(1.0) {
            hit(world, target);
        }
    }
}

type TetherFn = Box<dyn FnOnce(&mut World) + Send>;

fn tether_check(
    world: &mut World,
    source: EntityId,
    target: EntityId,
    start: f64,
    duration: f64,
    break_range: f64,
    on_complete: TetherFn,
) {
    let alive = |id: EntityId| world.get(id).filter(|e| !e.dead).map(|e| e.pos);
    let (Some(a), Some(b)) = (alive(source), alive(target)) else {
        return;
    };
    if dist2(a, b) > break_range * break_range {
        return;
    }
    if world.time + 1e-8 >= start + duration {
        on_complete(world);
    } else {
        let delay = (1.0f64 / 30.0).min(start + duration - world.time);
        schedule(world, source, delay, move |world| {
            tether_check(
                world,
                source,
                target,
                start,
                duration,
                break_range,
                on_complete,
            );
        });
    }
}

/// Lien entre `source` et `target` : `on_complete` s'exécute si le lien tient `duration` secondes
/// (il casse si l'un meurt ou si la distance dépasse `break_range`).
pub fn tether(
    world: &mut World,
    source: EntityId,
    target: EntityId,
    duration: f64,
    break_range: f64,
    on_complete: impl FnOnce(&mut World) + Send + 'static,
) {
    let start = world.time;
    let on_complete: TetherFn = Box::new(on_complete);
    schedule(world, source, duration.min(1.0 / 30.0), move |world| {
        tether_check(
            world,
            source,
            target,
            start,
            duration,
            break_range,
            on_complete,
        );
    });
}

// ------------------------------------------------------------------------------------------------
// Tick
// ------------------------------------------------------------------------------------------------

/// Tampons réutilisés d'un tick à l'autre.
#[derive(Default)]
pub struct PrimitivesScratch {
    ids: Vec<EntityId>,
    due: Vec<Task>,
    hits: Vec<(f64, EntityId)>,
    dashes: Vec<(u64, EntityId)>,
}

/// `updatePrimitives(world, dt)`.
pub fn update_primitives(world: &mut World, dt: f64, sc: &mut PrimitivesScratch) {
    run_tasks(world, sc);
    update_projectiles(world, dt, sc);
    update_zones(world, sc);
    update_dashes(world, sc);
}

fn run_tasks(world: &mut World, sc: &mut PrimitivesScratch) {
    // Les tâches échues sortent des files avant l'appel des rappels : le travail planifié par un
    // rappel survit à ce tick.
    sc.ids.clear();
    for e in world.iter() {
        if e.data.contains::<SyncCell<TaskQueue>>() {
            sc.ids.push(e.id);
        }
    }
    let time = world.time;
    for k in 0..sc.ids.len() {
        let Some(e) = world.get_mut(sc.ids[k]) else {
            continue;
        };
        let owner_dead = e.dead;
        let Some(queue) = e.data.get_mut::<SyncCell<TaskQueue>>() else {
            continue;
        };
        let tasks = &mut queue.get().tasks;
        let mut i = 0;
        while i < tasks.len() {
            let t = &tasks[i];
            if t.cancelled.load(Ordering::Relaxed) || (t.cancel_on_death && owner_dead) {
                tasks.swap_remove(i);
            } else if t.at <= time + 1e-8 {
                sc.due.push(tasks.swap_remove(i));
            } else {
                i += 1;
            }
        }
    }
    // Même instant : la tâche planifiée en dernier d'abord (voir l'en-tête du module).
    sc.due
        .sort_by(|a, b| a.at.total_cmp(&b.at).then(b.seq.cmp(&a.seq)));
    let mut due = std::mem::take(&mut sc.due);
    for task in due.drain(..) {
        if !task.cancelled.load(Ordering::Relaxed) {
            (task.run)(world);
        }
    }
    sc.due = due;
}

fn update_projectiles(world: &mut World, dt: f64, sc: &mut PrimitivesScratch) {
    // Un projectile créé pendant ce tick (retour d'un boomerang) ne bouge qu'au suivant.
    sc.ids.clear();
    for e in world.iter() {
        if e.data.contains::<SyncCell<Projectile>>() {
            sc.ids.push(e.id);
        }
    }
    for k in 0..sc.ids.len() {
        let id = sc.ids[k];
        let Some(mut cell) = world
            .get_mut(id)
            .and_then(|e| e.data.remove::<SyncCell<Projectile>>())
        else {
            continue;
        };
        let done = step_projectile(world, id, dt, cell.get(), &mut sc.hits);
        if !done && let Some(e) = world.get_mut(id) {
            e.data.insert(cell);
        }
    }
}

fn update_zones(world: &mut World, sc: &mut PrimitivesScratch) {
    sc.ids.clear();
    for e in world.iter() {
        if e.data.contains::<SyncCell<Zone>>() {
            sc.ids.push(e.id);
        }
    }
    for &id in &sc.ids {
        update_zone(world, id);
    }
}

fn update_dashes(world: &mut World, sc: &mut PrimitivesScratch) {
    sc.dashes.clear();
    for e in world.iter_mut() {
        if let Some(d) = e.data.get_mut::<SyncCell<AbilityDash>>() {
            sc.dashes.push((d.get().seq, e.id));
        }
    }
    sc.dashes.sort_unstable();
    for &(_, id) in &sc.dashes {
        let stopped = {
            let Some(e) = world.get_mut(id) else { continue };
            let unstoppable = e
                .data
                .get_mut::<SyncCell<AbilityDash>>()
                .is_some_and(|d| d.get().unstoppable);
            e.dead
                || !e.data.contains::<DashData>()
                || (!unstoppable && (!can_act(e) || has_cc(e, CcType::Root)))
        };
        if stopped {
            if let Some(e) = world.get_mut(id) {
                e.data.remove::<SyncCell<AbilityDash>>();
                e.data.remove::<DashData>();
                if !e.dead {
                    e.anim = Some("idle".to_owned());
                }
            }
            continue;
        }
        resolve_dash_hits(world, id);
    }
}
