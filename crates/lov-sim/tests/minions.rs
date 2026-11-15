//! Tests des sbires : port de `packages/server/test/minions.test.ts`.
//!
//! Le test d'endurance « une partie sans joueurs reste défendable durant trente minutes simulées »
//! utilise en TypeScript `createSystems()` (la liste complète). Le registre `systems/registry.rs`
//! n'existe pas encore et plusieurs systèmes (abilities, summoners, leveling, economy, jungle,
//! objectives) ne sont pas portés : la version Rust fait tourner les systèmes portés, dans l'ordre
//! canonique (voir son commentaire), et devra être rebranchée sur le registre.

mod common;

use common::{champ, v};
use lov_shared::constants::{TIMERS, scaled};
use lov_shared::types::{DamageType, EntityId, EntityKind, Lane, Team, TeamOrNeutral};
use lov_sim::combat::damage::{DamageOpts, apply_damage as core_damage};
use lov_sim::systems::buffs::buffs_system;
use lov_sim::systems::combat::combat_system;
use lov_sim::systems::input::input_system;
use lov_sim::systems::minions::{MinionState, minions_system, spawn_wave};
use lov_sim::systems::movement::movement_system;
use lov_sim::systems::structures::structures_system;
use lov_sim::systems::vision::vision_system;
use lov_sim::world_ai::runtime::{DamageRequest, apply_damage, stats};
use lov_sim::{Entity, System, World};

struct Sim {
    world: World,
    systems: Vec<System>,
}

impl Sim {
    fn step(&mut self) {
        self.world.step(&mut self.systems);
    }

    fn step_dt(&mut self, dt: f64) {
        self.world.step_dt(&mut self.systems, dt);
    }
}

/// `setup(structures)` : mouvement et sbires, plus les structures si demandé (sans combat central).
fn setup(structures: bool) -> Sim {
    let mut systems = vec![movement_system(), minions_system(false)];
    if structures {
        systems.push(structures_system(false));
    }
    let mut world = World::new(1);
    world.init(&mut systems);
    Sim { world, systems }
}

fn minions_where(world: &World, f: impl Fn(&Entity) -> bool) -> Vec<EntityId> {
    world
        .query(Some(EntityKind::Minion))
        .filter(|e| f(e))
        .map(|e| e.id)
        .collect()
}

fn lane_of(e: &Entity) -> Option<Lane> {
    e.data.get::<MinionState>().map(|m| m.lane)
}

fn cannons(world: &World) -> usize {
    minions_where(world, |e| e.subtype.as_deref() == Some("cannon")).len()
}

#[test]
fn respecte_les_timers_les_trois_voies_et_la_composition_canon() {
    let mut s = setup(false);
    s.step_dt(scaled(TIMERS.minion_first_wave) - 0.1);
    assert_eq!(s.world.len(), 0);
    s.step_dt(0.1);
    assert_eq!(s.world.len(), 36);
    spawn_wave(&mut s.world, 3);
    assert_eq!(cannons(&s.world), 6);
    s.world.time = scaled(TIMERS.cannon_late_at);
    spawn_wave(&mut s.world, 4);
    assert_eq!(cannons(&s.world), 12);
}

#[test]
fn ajoute_un_super_sbire_uniquement_dans_la_voie_dun_inhibiteur_ennemi_detruit() {
    let mut s = setup(true);
    let inhibitor = s
        .world
        .query(Some(EntityKind::Inhibitor))
        .find(|e| e.team == TeamOrNeutral::Red)
        .unwrap()
        .id;
    s.world.get_mut(inhibitor).unwrap().dead = true;
    spawn_wave(&mut s.world, 1);
    let supers = minions_where(&s.world, |e| e.subtype.as_deref() == Some("super"));
    assert_eq!(supers.len(), 1);
    assert_eq!(s.world.get(supers[0]).unwrap().team, TeamOrNeutral::Blue);
}

#[test]
fn conserve_la_symetrie_de_la_voie_mid_et_engage_pres_du_milieu() {
    let mut s = setup(false);
    spawn_wave(&mut s.world, 1);
    let mut met = false;
    for _ in 0..1200 {
        s.step();
        s.world.events.drain();
        let first_mid = |team: TeamOrNeutral| {
            s.world
                .query(Some(EntityKind::Minion))
                .find(|e| !e.dead && e.team == team && lane_of(e) == Some(Lane::Mid))
        };
        let (blue, red) = (
            first_mid(TeamOrNeutral::Blue),
            first_mid(TeamOrNeutral::Red),
        );
        if let (Some(blue), Some(red)) = (blue, red)
            && blue.attack_target.is_some()
            && red.attack_target.is_some()
        {
            // `toBeCloseTo(12000, 0)` : écart inférieur à 0,5.
            assert!(
                (blue.pos.x + red.pos.x - 12000.0).abs() < 0.5,
                "{}",
                blue.pos.x + red.pos.x
            );
            assert!(blue.pos.x > 5400.0);
            met = true;
            break;
        }
    }
    assert!(met);
}

#[test]
fn reagit_a_une_agression_de_champion_meme_avec_une_larvelette_alliee_proche() {
    let mut s = setup(false);
    spawn_wave(&mut s.world, 1);
    let defender = minions_where(&s.world, |e| {
        e.team == TeamOrNeutral::Blue && lane_of(e) == Some(Lane::Mid)
    })[0];
    s.world.get_mut(defender).unwrap().pos = v(6000.0, 6000.0);
    let ally = champ(&mut s.world, Team::Blue, v(6100.0, 6100.0));
    let aggressor = champ(&mut s.world, Team::Red, v(6400.0, 6000.0));
    // La larvelette n'a pas d'état de sbire : le système l'ignore.
    let mut grubling = Entity::new(
        EntityKind::Minion,
        TeamOrNeutral::Blue,
        v(6050.0, 6000.0),
        30.0,
        250.0,
        0.0,
        stats(250.0, 25.0),
    );
    grubling.subtype = Some("grubling".to_owned());
    s.world.spawn(grubling);
    let mut enemy = Entity::new(
        EntityKind::Minion,
        TeamOrNeutral::Red,
        v(6200.0, 6000.0),
        30.0,
        500.0,
        0.0,
        stats(500.0, 10.0),
    );
    enemy.subtype = Some("melee".to_owned());
    s.world.spawn(enemy);
    apply_damage(
        &mut s.world,
        DamageRequest {
            source: aggressor,
            target: ally,
            amount: 1.0,
            damage_type: DamageType::True,
            ability: None,
        },
    );
    s.step();
    assert_eq!(
        s.world.get(defender).unwrap().attack_target,
        Some(aggressor)
    );
}

#[test]
fn une_tour_defend_une_vague_seule_et_une_partie_vide_ne_se_termine_pas() {
    let mut s = setup(true);
    spawn_wave(&mut s.world, 1);
    for _ in 0..10 * 180 {
        s.step_dt(0.1);
        s.world.events.drain();
    }
    assert_eq!(s.world.winner, None);
    assert!(s.world.query(Some(EntityKind::Tower)).all(|e| !e.dead));
    let dead = s
        .world
        .query(Some(EntityKind::Minion))
        .filter(|e| e.dead)
        .count();
    assert!(dead < 50, "{dead}");
}

// Validation d'intégration contre le vrai pathfinding, les projectiles et le combat.

#[test]
fn ne_double_pas_les_attaques_quand_le_combat_central_est_present() {
    let mut systems = vec![combat_system(), structures_system(true)];
    let mut world = World::new(1);
    world.init(&mut systems);
    let tower = world
        .query(Some(EntityKind::Tower))
        .find(|e| e.team == TeamOrNeutral::Blue && e.subtype.as_deref() == Some("outer"))
        .unwrap();
    let tpos = tower.pos;
    let mut m = Entity::new(
        EntityKind::Minion,
        TeamOrNeutral::Red,
        v(tpos.x + 200.0, tpos.y),
        30.0,
        1000.0,
        0.0,
        stats(1000.0, 0.0),
    );
    m.subtype = Some("melee".to_owned());
    let minion = world.spawn(m);
    world.step(&mut systems);
    assert_eq!(world.get(minion).unwrap().hp, 1000.0);
    for _ in 0..20 {
        world.step(&mut systems);
    }
    assert_eq!(world.get(minion).unwrap().hp, 550.0);
    let inner = world
        .query(Some(EntityKind::Tower))
        .find(|e| e.team == TeamOrNeutral::Blue && e.subtype.as_deref() == Some("inner"))
        .unwrap()
        .id;
    core_damage(
        &mut world,
        DamageOpts::new(Some(minion), inner, 10000.0, DamageType::True),
    );
    assert!(!world.get(inner).unwrap().dead);
}

/// `une partie sans joueurs reste défendable durant trente minutes simulées`. Systèmes portés
/// seulement, dans l'ordre de `systems/index.ts` (input, buffs, movement, combat, minions, structures,
/// vision) : abilities, summoners, jungle, objectives, leveling et economy ne sont pas encore portés.
/// Même scénario que le TS : 3600 pas de 0,5 s, aucune partie ne doit se terminer.
#[test]
fn une_partie_sans_joueurs_reste_defendable_durant_trente_minutes_simulees() {
    let mut systems = vec![
        input_system(),
        buffs_system(),
        movement_system(),
        combat_system(),
        minions_system(true),
        structures_system(true),
        vision_system(),
    ];
    let mut world = World::new(1);
    world.init(&mut systems);
    for _ in 0..3600 {
        world.step_dt(&mut systems, 0.5);
        world.events.drain();
    }
    assert_eq!(world.time, 1800.0);
    assert_eq!(world.winner, None);
    assert!(world.query(Some(EntityKind::Nexus)).all(|e| !e.dead));
    assert!(world.len() < 1000, "{}", world.len());
    // Le pas de 0,5 s accélère seulement ce scénario d'endurance ; le tick réel est celui de
    // minions.performance.test.ts (non porté, voir le rapport).
}
