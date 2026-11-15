//! Partie complète avec le registre `create_systems` : tous les systèmes ensemble, sans panique ni
//! blocage, et dans l'ordre du TypeScript.

mod common;

use common::{champ_as, v};
use lov_shared::data::champions::CHAMPIONS;
use lov_shared::types::{EntityKind, Team};
use lov_sim::World;
use lov_sim::systems::registry::create_systems;

#[test]
fn ordre_des_systemes_identique_au_typescript() {
    let names: Vec<&str> = create_systems().iter().map(|s| s.name).collect();
    assert_eq!(
        names,
        [
            "input",
            "buffs",
            "summoners",
            "abilities",
            "movement",
            "combat",
            "minions",
            "structures",
            "jungle",
            "objectives",
            "leveling",
            "economy",
            "vision",
        ]
    );
}

#[test]
fn quinze_minutes_de_partie_sans_panique_ni_blocage() {
    let mut systems = create_systems();
    let mut world = World::new(42);
    for (i, def) in CHAMPIONS.iter().cycle().take(8).enumerate() {
        let team = if i < 4 { Team::Blue } else { Team::Red };
        let pos = if i < 4 {
            v(6000.0, 5600.0)
        } else {
            v(6000.0, 6400.0)
        };
        champ_as(&mut world, team, pos, &def.id);
    }
    world.init(&mut systems);
    for _ in 0..15 * 60 * 30 {
        world.step(&mut systems);
        world.events.drain();
    }
    assert!((world.time - 900.0).abs() < 1e-6);
    let monsters = world.query(Some(EntityKind::Monster)).count();
    let minions = world.query(Some(EntityKind::Minion)).count();
    assert!(monsters > 0, "les camps et objectifs sont apparus");
    assert!(minions > 0, "les vagues de sbires sont en jeu");
}
