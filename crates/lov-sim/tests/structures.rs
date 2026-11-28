//! Tests des structures : port de `packages/server/test/structures.test.ts` et
//! `structures.bugs.test.ts`. Le monde n'a ici que le système des structures, sans combat central
//! (`core_combat = false`) : les tours tirent par le repli de `world_ai::runtime::attack`.

mod common;

use common::{champ, v};
use lov_shared::constants::{TIMERS, scaled};
use lov_shared::data::structures::STRUCTURES;
use lov_shared::map::StructureLane;
use lov_shared::types::{DamageType, EntityId, EntityKind, GameEvent, Team, TeamOrNeutral};
use lov_sim::combat::damage::{DamageOpts, apply_damage as core_damage};
use lov_sim::systems::structures::{is_structure_attackable, structure_state, structures_system};
use lov_sim::world_ai::runtime::{DamageRequest, apply_damage, stats};
use lov_sim::{Entity, System, World};

struct Setup {
    world: World,
    systems: Vec<System>,
    champion: EntityId,
    tower: EntityId,
}

impl Setup {
    fn step(&mut self) {
        self.world.step(&mut self.systems);
    }

    fn step_dt(&mut self, dt: f64) {
        self.world.step_dt(&mut self.systems, dt);
    }
}

fn find_structure(
    world: &World,
    kind: EntityKind,
    team: TeamOrNeutral,
    subtype: &str,
    lane: StructureLane,
) -> EntityId {
    world
        .query(Some(kind))
        .find(|e| {
            e.team == team
                && e.subtype.as_deref() == Some(subtype)
                && structure_state(e).unwrap().lane == lane
        })
        .unwrap()
        .id
}

fn setup() -> Setup {
    let mut systems = vec![structures_system(false)];
    let mut world = World::new(1);
    world.init(&mut systems);
    let champion = champ(&mut world, Team::Red, v(4900.0, 4900.0));
    let tower = find_structure(
        &world,
        EntityKind::Tower,
        TeamOrNeutral::Blue,
        "outer",
        StructureLane::Mid,
    );
    Setup {
        world,
        systems,
        champion,
        tower,
    }
}

fn hit(world: &mut World, source: EntityId, target: EntityId, amount: f64) {
    apply_damage(
        world,
        DamageRequest {
            source,
            target,
            amount,
            damage_type: DamageType::True,
            ability: None,
        },
    );
}

fn count(world: &World, kind: EntityKind) -> usize {
    world.query(Some(kind)).count()
}

fn plate_events(world: &World) -> usize {
    world
        .events
        .pending
        .iter()
        .filter(|e| {
            matches!(
                e,
                GameEvent::StructureDestroyed {
                    plate: Some(true),
                    ..
                }
            )
        })
        .count()
}

#[test]
fn initialise_22_tours_six_inhibiteurs_et_deux_nexus() {
    let s = setup();
    assert_eq!(count(&s.world, EntityKind::Tower), 22);
    assert_eq!(count(&s.world, EntityKind::Inhibitor), 6);
    assert_eq!(count(&s.world, EntityKind::Nexus), 2);
}

#[test]
fn bloque_les_degats_hors_ordre_et_fortifie_les_tours_externes() {
    let mut s = setup();
    let (champion, tower) = (s.champion, s.tower);
    let hp0 = s.world.get(tower).unwrap().hp;
    hit(&mut s.world, champion, tower, 100.0);
    // Fortification -50 % avant 3:00.
    assert_eq!(s.world.get(tower).unwrap().hp, hp0 - 50.0);
    let inner = find_structure(
        &s.world,
        EntityKind::Tower,
        TeamOrNeutral::Blue,
        "inner",
        StructureLane::Mid,
    );
    hit(&mut s.world, champion, inner, 10000.0);
    let i = s.world.get(inner).unwrap();
    assert_eq!(i.hp, i.stats.max_hp);
    s.world.time = scaled(180.0);
    hit(&mut s.world, champion, tower, 10000.0);
    assert!(is_structure_attackable(&s.world, inner));
}

#[test]
fn ne_donne_une_plaque_quau_seuil_y_compris_plusieurs_sur_un_meme_coup() {
    let mut s = setup();
    let (champion, tower) = (s.champion, s.tower);
    s.world.time = scaled(180.0);
    let seuil = s.world.get(tower).unwrap().stats.max_hp / 5.0;
    let plates = |w: &World| structure_state(w.get(tower).unwrap()).unwrap().plates;
    hit(&mut s.world, champion, tower, 1.0);
    assert_eq!(plates(&s.world), 5);
    hit(&mut s.world, champion, tower, seuil + 1.0);
    assert_eq!(plates(&s.world), 4);
    hit(&mut s.world, champion, tower, 2.0 * seuil);
    assert_eq!(plates(&s.world), 2);
    assert_eq!(plate_events(&s.world), 3);
    s.world.time = scaled(TIMERS.tower_plates_until);
    s.step();
    assert_eq!(plates(&s.world), 0);
}

#[test]
fn ajoute_puis_retire_la_resistance_anti_backdoor() {
    let mut s = setup();
    let tower = s.tower;
    s.step();
    assert_eq!(
        s.world.get(tower).unwrap().stats.armor,
        STRUCTURES.towers.outer.armor + STRUCTURES.backdoor_resistance
    );
    let pos = s.world.get(tower).unwrap().pos;
    s.world.spawn(Entity::new(
        EntityKind::Minion,
        TeamOrNeutral::Red,
        pos,
        30.0,
        500.0,
        0.0,
        stats(500.0, 10.0),
    ));
    s.step();
    assert_eq!(
        s.world.get(tower).unwrap().stats.armor,
        STRUCTURES.towers.outer.armor
    );
}

#[test]
fn privilegie_un_sbire_puis_le_champion_qui_agresse_un_allie() {
    let mut s = setup();
    let (champion, tower) = (s.champion, s.tower);
    let tpos = s.world.get(tower).unwrap().pos;
    let mut m = Entity::new(
        EntityKind::Minion,
        TeamOrNeutral::Red,
        v(tpos.x + 200.0, tpos.y),
        30.0,
        1000.0,
        0.0,
        stats(1000.0, 10.0),
    );
    m.subtype = Some("melee".to_owned());
    let minion = s.world.spawn(m);
    s.step();
    assert_eq!(s.world.get(tower).unwrap().attack_target, Some(minion));
    let ally = champ(&mut s.world, Team::Blue, tpos);
    hit(&mut s.world, champion, ally, 1.0);
    s.step();
    assert_eq!(s.world.get(tower).unwrap().attack_target, Some(champion));
}

#[test]
fn augmente_les_tirs_successifs_contre_un_champion_et_reinitialise_a_la_perte_de_cible() {
    let mut s = setup();
    let (champion, tower) = (s.champion, s.tower);
    {
        let c = s.world.get_mut(champion).unwrap();
        c.hp = 10000.0;
        c.stats.max_hp = 10000.0;
    }
    s.step();
    let first = 10000.0 - s.world.get(champion).unwrap().hp;
    s.step_dt(1.3);
    let second = 10000.0 - s.world.get(champion).unwrap().hp - first;
    // `toBeCloseTo(1.4)` : deux décimales.
    assert!((second / first - 1.4).abs() < 0.005, "{}", second / first);
    s.world.get_mut(champion).unwrap().pos = v(6000.0, 6000.0);
    s.step();
    assert_eq!(
        structure_state(s.world.get(tower).unwrap()).unwrap().ramp,
        0.0
    );
}

#[test]
fn reanime_un_inhibiteur_apres_cinq_minutes_scalees_et_valide_la_victoire_seulement_au_nexus_expose()
 {
    let mut s = setup();
    let champion = s.champion;
    s.world.time = 300.0;
    let blue_mid_towers: Vec<EntityId> = s
        .world
        .query(Some(EntityKind::Tower))
        .filter(|e| {
            e.team == TeamOrNeutral::Blue && structure_state(e).unwrap().lane == StructureLane::Mid
        })
        .map(|e| e.id)
        .collect();
    for id in blue_mid_towers {
        hit(&mut s.world, champion, id, 10000.0);
    }
    let inhibitor = find_structure(
        &s.world,
        EntityKind::Inhibitor,
        TeamOrNeutral::Blue,
        "inhibitor",
        StructureLane::Mid,
    );
    hit(&mut s.world, champion, inhibitor, 10000.0);
    assert!(s.world.get(inhibitor).unwrap().dead);
    s.step_dt(scaled(TIMERS.inhibitor_respawn) - 0.1);
    assert!(s.world.get(inhibitor).unwrap().dead);
    s.step_dt(0.1);
    assert!(!s.world.get(inhibitor).unwrap().dead);
    hit(&mut s.world, champion, inhibitor, 10000.0);
    let nexus = s
        .world
        .query(Some(EntityKind::Nexus))
        .find(|e| e.team == TeamOrNeutral::Blue)
        .unwrap()
        .id;
    hit(&mut s.world, champion, nexus, 10000.0);
    assert_eq!(s.world.winner, None);
    let nexus_towers: Vec<EntityId> = s
        .world
        .query(Some(EntityKind::Tower))
        .filter(|e| e.team == TeamOrNeutral::Blue && e.subtype.as_deref() == Some("nexus"))
        .map(|e| e.id)
        .collect();
    for id in nexus_towers {
        hit(&mut s.world, champion, id, 10000.0);
    }
    hit(&mut s.world, champion, nexus, 10000.0);
    assert_eq!(s.world.winner, Some(Team::Red));
}

// ---- structures.bugs.test.ts ----

struct Bugs {
    world: World,
    systems: Vec<System>,
    tower: EntityId,
    attacker: EntityId,
}

fn setup_bugs() -> Bugs {
    let mut systems = vec![structures_system(false)];
    let mut world = World::new(1);
    world.init(&mut systems);
    world.time = 200.0;
    let tower = find_structure(
        &world,
        EntityKind::Tower,
        TeamOrNeutral::Blue,
        "outer",
        // Première tour externe bleue du TS (`find` sans voie) : voie haute.
        StructureLane::Top,
    );
    let tpos = world.get(tower).unwrap().pos;
    let attacker = champ(&mut world, Team::Red, v(tpos.x + 300.0, tpos.y));
    Bugs {
        world,
        systems,
        tower,
        attacker,
    }
}

#[test]
fn donne_une_plaque_franchie_par_des_degats_positifs_arrondis_a_zero() {
    let mut s = setup_bugs();
    let (tower, attacker) = (s.tower, s.attacker);
    // Juste au-dessus du seuil de la 4e plaque.
    let max = s.world.get(tower).unwrap().stats.max_hp;
    s.world.get_mut(tower).unwrap().hp = (max / 5.0) * 4.0 + 0.05;
    core_damage(
        &mut s.world,
        DamageOpts::new(Some(attacker), tower, 0.1, DamageType::True),
    );
    assert_eq!(
        structure_state(s.world.get(tower).unwrap()).unwrap().plates,
        4
    );
    assert_eq!(plate_events(&s.world), 1);
}

#[test]
fn protege_un_allie_agresse_meme_si_les_degats_sont_inferieurs_a_une_unite() {
    let mut s = setup_bugs();
    let (tower, attacker) = (s.tower, s.attacker);
    let tpos = s.world.get(tower).unwrap().pos;
    let ally = champ(&mut s.world, Team::Blue, v(tpos.x + 100.0, tpos.y));
    let mut m = Entity::new(
        EntityKind::Minion,
        TeamOrNeutral::Red,
        v(tpos.x + 150.0, tpos.y),
        30.0,
        1000.0,
        0.0,
        stats(1000.0, 10.0),
    );
    m.subtype = Some("melee".to_owned());
    let minion = s.world.spawn(m);
    s.world.step(&mut s.systems);
    assert_eq!(s.world.get(tower).unwrap().attack_target, Some(minion));
    core_damage(
        &mut s.world,
        DamageOpts::new(Some(attacker), ally, 0.1, DamageType::True),
    );
    s.world.step(&mut s.systems);
    assert_eq!(s.world.get(tower).unwrap().attack_target, Some(attacker));
}

/// #313 : un champion invisible proche ne bloque plus la tour, qui tire sur le champion visible.
#[test]
fn une_tour_ignore_le_champion_invisible_et_tire_sur_le_visible() {
    use lov_shared::map::TOWERS;
    use lov_shared::types::TowerTier;
    use lov_sim::combat::buffs::{BuffOptions, add_buff};
    let mut sim = common::game_world(&["minions", "jungle", "objectives"]);
    let t = TOWERS
        .iter()
        .find(|t| t.team == Team::Red && t.tier == TowerTier::Outer)
        .unwrap();
    let hidden = sim.champ(Team::Blue, v(t.pos.x + 250.0, t.pos.y));
    let visible = sim.champ(Team::Blue, v(t.pos.x - 550.0, t.pos.y));
    add_buff(&mut sim.world, hidden, "invisible", BuffOptions::new(30.0));
    let (hp_hidden, hp_visible) = (sim.e(hidden).hp, sim.e(visible).hp);
    sim.run(4.0);
    assert!(
        sim.e(visible).hp < hp_visible,
        "la tour doit tirer sur le champion visible"
    );
    assert_eq!(sim.e(hidden).hp, hp_hidden, "ni sur l'invisible");
}
