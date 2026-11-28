//! Tests du système de mouvement : port du bloc `movement` de `packages/server/test/nav.test.ts`.
//!
//! Le monde de test ne contient que le système de mouvement (`coreWorld()` du TS y ajoute `input`,
//! `combat` et `vision`, pas encore portés). Le système `input` ne sert, dans ces tests, qu'à traduire
//! la commande `move` en `move_target` : on pose donc `move_target` directement, ce qui exerce le même
//! code de mouvement.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use lov_shared::map::{WALLS, is_in_wall};
use lov_shared::math::dist;
use lov_shared::types::{
    EntityId, EntityKind, GameEvent, Stats, SummonerId, Team, TeamOrNeutral, Vec2,
};
use lov_sim::nav::nav_grid;
use lov_sim::systems::movement::{DashData, Ghost, movement_system};
use lov_sim::{CreateChampion, Entity, System, World, create_champion};

fn v(x: f64, y: f64) -> Vec2 {
    Vec2 { x, y }
}

/// Stats de base de grondak (`packages/shared/src/data/champions/grondak.ts`), le champion de `champ()`.
fn stats() -> Stats {
    Stats {
        max_hp: 680.0,
        hp_regen: 0.0,
        max_mana: 0.0,
        mana_regen: 0.0,
        ad: 64.0,
        ap: 0.0,
        armor: 38.0,
        mr: 32.0,
        attack_speed: 0.62,
        attack_range: 150.0,
        move_speed: 335.0,
        crit_chance: 0.0,
        ability_haste: 0.0,
        lifesteal: 0.0,
        armor_pen: 0.0,
        magic_pen: 0.0,
    }
}

struct Sim {
    world: World,
    systems: [System; 1],
    n: u32,
}

impl Sim {
    fn new() -> Self {
        let mut systems = [movement_system()];
        let mut world = World::new(1);
        world.init(&mut systems);
        Self {
            world,
            systems,
            n: 0,
        }
    }

    fn step(&mut self) {
        self.world.step(&mut self.systems);
    }

    /// `run(world, seconds)` du TS : `round(seconds * 30)` ticks.
    fn run(&mut self, seconds: f64) {
        for _ in 0..(seconds * 30.0).round() as u32 {
            self.step();
        }
    }

    fn champ(&mut self, team: Team, pos: Vec2) -> EntityId {
        self.n += 1;
        let opts = CreateChampion {
            player_id: format!("p{}", self.n),
            name: "T".into(),
            team,
            champion_id: "grondak".into(),
            summoners: [SummonerId::Flash, SummonerId::Ignite],
            pos,
        };
        create_champion(&mut self.world, opts, &stats())
    }

    fn pos(&self, id: EntityId) -> Vec2 {
        self.world.get(id).unwrap().pos
    }
}

/// Boîte englobante d'un mur (les tests ne dépendent pas de la géométrie exacte de map).
fn wall_box(p: &[Vec2]) -> (f64, f64, f64, f64) {
    let f = |g: fn(&Vec2) -> f64, init: f64, m: fn(f64, f64) -> f64| p.iter().map(g).fold(init, m);
    (
        f(|q| q.x, f64::INFINITY, f64::min),
        f(|q| q.x, f64::NEG_INFINITY, f64::max),
        f(|q| q.y, f64::INFINITY, f64::min),
        f(|q| q.y, f64::NEG_INFINITY, f64::max),
    )
}

#[test]
fn un_champion_contourne_un_mur_de_la_carte_pour_atteindre_sa_destination() {
    let mut s = Sim::new();
    let (min_x, max_x, min_y, max_y) = wall_box(&WALLS[0]);
    let from = v(min_x - 250.0, (min_y + max_y) / 2.0);
    let to = v(max_x + 200.0, from.y);
    let nav = nav_grid();
    assert!(nav.walkable(from.x, from.y) && nav.walkable(to.x, to.y));
    let c = s.champ(Team::Blue, from);
    s.world.get_mut(c).unwrap().move_target = Some(to);
    let mut crossed = false;
    for _ in 0..30 * 15 {
        s.step();
        let p = s.pos(c);
        if !nav.walkable(p.x, p.y) {
            crossed = true;
        }
    }
    assert!(!crossed);
    // `toBeCloseTo(to.x, 0)` : |écart| < 0,5.
    assert!((s.pos(c).x - to.x).abs() < 0.5, "x = {}", s.pos(c).x);
}

#[test]
fn dash_atteint_la_destination_emet_dash_appelle_on_arrive_et_s_arrete_aux_murs() {
    let mut s = Sim::new();
    let c = s.champ(Team::Blue, v(6000.0, 3000.0));
    let arrived = Arc::new(AtomicUsize::new(0));
    let a = arrived.clone();
    let mut dash = DashData::new(v(6400.0, 3000.0), 1200.0);
    dash.on_arrive = Some(Box::new(move |_, _| {
        a.fetch_add(1, Ordering::SeqCst);
    }));
    s.world.get_mut(c).unwrap().data.insert(dash);
    s.run(0.5);
    assert_eq!(s.pos(c), v(6400.0, 3000.0));
    assert_eq!(arrived.load(Ordering::SeqCst), 1);
    assert!(!s.world.get(c).unwrap().data.contains::<DashData>());
    let dashes = s
        .world
        .events
        .drain()
        .into_iter()
        .filter(|e| matches!(e, GameEvent::Dash { .. }))
        .count();
    assert_eq!(dashes, 1);

    let (min_x, max_x, min_y, max_y) = wall_box(&WALLS[0]);
    let y = (min_y + max_y) / 2.0;
    let d = s.champ(Team::Blue, v(min_x - 250.0, y));
    s.world
        .get_mut(d)
        .unwrap()
        .data
        .insert(DashData::new(v(max_x + 300.0, y), 1500.0));
    s.run(1.0);
    let p = s.pos(d);
    assert!(p.x < min_x);
    assert!(nav_grid().walkable(p.x, p.y));
}

#[test]
fn collisions_deux_unites_ne_se_superposent_pas_ghost_les_ignore() {
    let mut s = Sim::new();
    let a = s.champ(Team::Blue, v(6000.0, 3000.0));
    let b = s.champ(Team::Blue, v(6001.0, 3000.0));
    s.run(1.0);
    assert!(dist(s.pos(a), s.pos(b)) > 80.0);
    let c = s.champ(Team::Blue, v(7000.0, 3000.0));
    let d = s.champ(Team::Blue, v(7001.0, 3000.0));
    s.world.get_mut(c).unwrap().data.insert(Ghost);
    s.run(1.0);
    assert!(dist(s.pos(c), s.pos(d)) < 5.0);
}

#[test]
fn les_structures_bloquent_les_champions() {
    let mut s = Sim::new();
    let mut tower_stats = stats();
    tower_stats.move_speed = 0.0;
    let tower = s.world.spawn(Entity::new(
        EntityKind::Tower,
        TeamOrNeutral::Red,
        v(6000.0, 3000.0),
        120.0,
        5000.0,
        0.0,
        tower_stats,
    ));
    let c = s.champ(Team::Blue, v(5500.0, 3000.0));
    s.world.get_mut(c).unwrap().move_target = Some(v(6500.0, 3000.0));
    let mut min_d = f64::INFINITY;
    for _ in 0..90 {
        s.step();
        min_d = min_d.min(dist(s.pos(c), s.pos(tower)));
    }
    let tower_radius = s.world.get(tower).unwrap().radius;
    let champ_radius = s.world.get(c).unwrap().radius;
    assert!(
        min_d > tower_radius + champ_radius - 15.0,
        "min_d = {min_d}"
    );
}

/// #309 : au-delà du budget A* d'un tick, les unités attendent leur chemin au lieu de foncer en ligne
/// droite à travers les murs.
#[test]
fn au_dela_du_budget_astar_aucune_unite_ne_traverse_un_mur() {
    let mut s = Sim::new();
    let nav = nav_grid();
    let mut pairs = Vec::new();
    let mut y = 1500.0;
    while pairs.len() < 60 && y < 10500.0 {
        let mut x = 1500.0;
        while pairs.len() < 60 && x < 10500.0 {
            let (a, b) = (v(x, y), v(x + 1200.0, y + 900.0));
            if nav.walkable(a.x, a.y) && nav.walkable(b.x, b.y) && !nav.line_walkable(a, b) {
                pairs.push((a, b));
            }
            x += 397.0;
        }
        y += 431.0;
    }
    assert_eq!(
        pairs.len(),
        60,
        "plus de deux fois le budget de requêtes dans le même tick"
    );
    let ids: Vec<EntityId> = pairs
        .iter()
        .map(|&(a, b)| {
            let mut e = Entity::new(
                EntityKind::Monster,
                TeamOrNeutral::Neutral,
                a,
                40.0,
                100.0,
                0.0,
                stats(),
            );
            e.data.insert(Ghost);
            e.move_target = Some(b);
            s.world.spawn(e)
        })
        .collect();
    for _ in 0..300 {
        s.step();
        for &id in &ids {
            let p = s.pos(id);
            assert!(
                !is_in_wall(p),
                "unité {id} dans un mur en ({:.0}, {:.0})",
                p.x,
                p.y
            );
        }
    }
    let arrived = ids
        .iter()
        .zip(&pairs)
        .filter(|(id, (_, b))| dist(s.pos(**id), *b) < 60.0)
        .count();
    assert_eq!(arrived, 60, "toutes les unités finissent par arriver");
}
