//! Garde-fous d'équilibrage des champions : port de `packages/server/test/champion-balance.test.ts`.
//!
//! Différences avec le TS :
//! - les `it.each` sont portés en boucles ; les duels tournent sur plusieurs fils (un duel = un monde
//!   indépendant, le résultat ne dépend pas de l'ordre) ;
//! - le rapport `LOV_BALANCE_REPORT=1` (`afterAll`) est affiché à la fin du test des duels, avec
//!   `cargo test -p lov-sim --test champion_balance -- --nocapture` ;
//! - le test des coûts prend `sable` pour cible, comme le TS (`game_vs`).

mod common;

use std::thread;

use common::champion::game_vs;
use lov_shared::constants::TICK_DT;
use lov_shared::data::champions::{CHAMPIONS, get_champion};
use lov_shared::math::dist;
use lov_shared::schema::ChampionClass;
use lov_shared::types::{AbilitySlot, Cooldowns, EntityId, Team, Vec2};
use lov_sim::abilities::engine::{CastCommand, cast_ability};
use lov_sim::combat::buffs::{can_attack, can_move, recalculate_stats};
use lov_sim::systems::leveling::{grant_xp, xp_for_level};
use lov_sim::systems::movement::DashData;
use lov_sim::systems::registry::create_systems;
use lov_sim::{CreateChampion, World, create_champion};

const SLOTS: [AbilitySlot; 4] = [
    AbilitySlot::Q,
    AbilitySlot::W,
    AbilitySlot::E,
    AbilitySlot::R,
];
/// Ordre d'essai des sorts à chaque tick du duel.
const CAST_ORDER: [AbilitySlot; 4] = [
    AbilitySlot::R,
    AbilitySlot::Q,
    AbilitySlot::W,
    AbilitySlot::E,
];

fn loadout(class: ChampionClass) -> [&'static str; 3] {
    match class {
        ChampionClass::Tank => ["ruby_crystal", "ruby_crystal", "boots"],
        ChampionClass::Fighter => ["long_sword", "ruby_crystal", "boots"],
        ChampionClass::Mage => ["amplifying_tome", "amplifying_tome", "boots"],
        ChampionClass::Marksman => ["long_sword", "long_sword", "boots"],
        ChampionClass::Assassin => ["long_sword", "long_sword", "boots"],
        ChampionClass::Support => ["amplifying_tome", "ruby_crystal", "boots"],
    }
}

/// Sorts lancés sur soi plutôt que sur l'adversaire (soins, boucliers d'allié).
fn targets_ally(id: &str, slot: AbilitySlot) -> bool {
    (id == "ilyra" && slot == AbilitySlot::E)
        || (id == "solenne" && matches!(slot, AbilitySlot::W | AbilitySlot::E))
}

fn cooldown(c: &Cooldowns, slot: AbilitySlot) -> f64 {
    match slot {
        AbilitySlot::Q => c.q,
        AbilitySlot::W => c.w,
        AbilitySlot::E => c.e,
        AbilitySlot::R => c.r,
    }
}

#[derive(Debug, Clone, PartialEq)]
struct DuelResult {
    winner: Option<String>,
    seconds: f64,
    damage: [f64; 2],
    hp: [f64; 2],
}

/// Harnais d'intégration déterministe avec les vrais systèmes (combat, déplacement, vision, kits).
/// Les équipements de classe n'ont pas de chance de critique aléatoire ; les deux IA alternent
/// l'initiative à chaque tick.
fn simulate_duel(a: &str, b: &str, level: u32) -> DuelResult {
    let mut systems = create_systems();
    let mut world = World::new(1);
    let mut champions: [EntityId; 2] = [EntityId::default(); 2];
    for (i, id) in [a, b].into_iter().enumerate() {
        let def = get_champion(id).expect("champion inconnu");
        let e = create_champion(
            &mut world,
            CreateChampion {
                player_id: format!("p{i}"),
                name: id.into(),
                team: if i == 1 { Team::Red } else { Team::Blue },
                champion_id: id.into(),
                summoners: [
                    lov_shared::types::SummonerId::Flash,
                    lov_shared::types::SummonerId::Ignite,
                ],
                pos: Vec2 {
                    x: 6000.0 + i as f64 * 400.0,
                    y: 1000.0,
                },
            },
            &def.base_stats,
        );
        grant_xp(&mut world, e, xp_for_level(level));
        let c = world.get_mut(e).and_then(|e| e.champ.as_mut()).unwrap();
        let ranks = if level == 6 {
            [3, 1, 1, 1]
        } else {
            [5, 3, 1, 2]
        };
        c.ability_levels.q = ranks[0];
        c.ability_levels.w = ranks[1];
        c.ability_levels.e = ranks[2];
        c.ability_levels.r = ranks[3];
        c.skill_points = 0;
        c.items = loadout(def.class)
            .iter()
            .map(|s| Some((*s).to_string()))
            .chain([None, None, None])
            .collect();
        recalculate_stats(&mut world, e);
        let ent = world.get_mut(e).unwrap();
        ent.hp = ent.stats.max_hp;
        ent.mana = ent.stats.max_mana;
        champions[i] = e;
    }
    world.init(&mut systems);
    let dead = |w: &World, id: EntityId| w.get(id).is_none_or(|e| e.dead);
    let mut tick = 0;
    while tick < 30 * 35 && !champions.iter().any(|&e| dead(&world, e)) {
        // Initiative alternée à chaque tick : aucun camp n'a la priorité du coup fatal en permanence.
        let order = if tick % 2 == 1 { [1, 0] } else { [0, 1] };
        for index in order {
            let (source, target) = (champions[index], champions[1 - index]);
            if dead(&world, source) || dead(&world, target) {
                continue;
            }
            let champion_id = world
                .get(source)
                .unwrap()
                .champ
                .as_ref()
                .unwrap()
                .champion_id
                .clone();
            for slot in CAST_ORDER {
                let target_pos = world.get(target).unwrap().pos;
                let command = CastCommand {
                    slot,
                    target_id: Some(if targets_ally(&champion_id, slot) {
                        source
                    } else {
                        target
                    }),
                    target_pos: Some(target_pos),
                };
                if cast_ability(&mut world, source, &command, false) {
                    break;
                }
            }
            let target_pos = world.get(target).unwrap().pos;
            let target_radius = world.get(target).unwrap().radius;
            let s = world.get_mut(source).unwrap();
            let dashing = s.data.contains::<DashData>();
            if can_attack(s) && !dashing {
                s.attack_target = Some(target);
            } else if can_move(s)
                && !dashing
                && dist(s.pos, target_pos) > s.stats.attack_range + s.radius + target_radius
            {
                s.move_target = Some(target_pos);
            }
        }
        world.step_dt(&mut systems, TICK_DT);
        // Simule les snapshots : ne pas accumuler les événements d'un long duel.
        world.events.drain();
        for &id in &champions {
            let e = world.get(id).unwrap();
            assert!(
                (e.hp + e.mana + e.stats.move_speed + e.shield).is_finite(),
                "état non fini : {a}/{b}"
            );
            assert!(
                e.hp >= 0.0 && e.mana >= -1e-6 && e.shield >= 0.0 && e.hp <= e.stats.max_hp + 1e-6,
                "ressources invalides : {a}/{b}"
            );
        }
        tick += 1;
    }
    let get = |i: usize| world.get(champions[i]).unwrap();
    DuelResult {
        winner: if get(0).dead {
            Some(b.into())
        } else if get(1).dead {
            Some(a.into())
        } else {
            None
        },
        seconds: world.time,
        damage: [0, 1].map(|i| get(i).champ.as_ref().unwrap().damage_to_champions),
        hp: [0, 1].map(|i| get(i).hp),
    }
}

#[test]
fn duels_de_classe_aux_niveaux_6_et_11() {
    let mut pairs = Vec::new();
    for (i, a) in CHAMPIONS.iter().enumerate() {
        for b in &CHAMPIONS[i + 1..] {
            for level in [6, 11] {
                pairs.push((a.id.as_str(), b.id.as_str(), level));
            }
        }
    }
    let workers = thread::available_parallelism().map_or(4, |n| n.get());
    let chunk = pairs.len().div_ceil(workers);
    let reports: Vec<(&str, &str, u32, DuelResult)> = thread::scope(|scope| {
        let handles: Vec<_> = pairs
            .chunks(chunk)
            .map(|part| {
                scope.spawn(move || {
                    part.iter()
                        .map(|&(a, b, level)| (a, b, level, simulate_duel(a, b, level)))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().expect("un duel a paniqué"))
            .collect()
    });
    for (a, b, level, duel) in &reports {
        let name = format!("{a} contre {b} au niveau {level}");
        assert!(duel.seconds > 1.0, "{name} : {duel:?}");
        assert!(duel.damage[0] + duel.damage[1] > 0.0, "{name} : {duel:?}");
        assert!(duel.damage[0] / duel.seconds < 1500.0, "{name} : {duel:?}");
        assert!(duel.damage[1] / duel.seconds < 1500.0, "{name} : {duel:?}");
        assert!(duel.hp.iter().all(|h| h.is_finite()), "{name} : {duel:?}");
    }
    if std::env::var("LOV_BALANCE_REPORT").as_deref() == Ok("1") {
        for champion in CHAMPIONS.iter() {
            let id = champion.id.as_str();
            let games: Vec<_> = reports.iter().filter(|r| r.0 == id || r.1 == id).collect();
            let wins = games
                .iter()
                .filter(|r| r.3.winner.as_deref() == Some(id))
                .count();
            let draws = games.iter().filter(|r| r.3.winner.is_none()).count();
            let max_dps = games
                .iter()
                .map(|r| r.3.damage[usize::from(r.0 != id)] / r.3.seconds)
                .fold(f64::NEG_INFINITY, f64::max)
                .round();
            println!(
                "BALANCE_REPORT {id} : {wins} victoires, {draws} nuls, {} parties, DPS max {max_dps}",
                games.len()
            );
        }
    }
}

#[test]
fn chaque_sort_consomme_une_fois_son_cout_et_son_delai() {
    for champion in CHAMPIONS.iter() {
        let id = champion.id.as_str();
        for slot in SLOTS {
            let mut g = game_vs(id, "sable", 6);
            let (source, target) = (g.source, g.target);
            let def = champion.abilities.get(slot);
            let mana = g.e(source).mana;
            let on = if targets_ally(id, slot) {
                source
            } else {
                target
            };
            assert!(g.cast(slot, Some(on), None), "{id} {slot:?} : lancement");
            let cd = cooldown(&g.e(source).champ.as_ref().unwrap().cooldowns, slot);
            assert_eq!(cd, def.cooldown[0], "{id} {slot:?} : délai");
            let spent = mana - g.e(source).mana;
            assert!(
                (spent - def.cost[0]).abs() < 0.005,
                "{id} {slot:?} : coût {spent}"
            );
            g.advance(0.1);
            let cd = cooldown(&g.e(source).champ.as_ref().unwrap().cooldowns, slot);
            assert!(
                (cd - (def.cooldown[0] - 0.1)).abs() < 0.005,
                "{id} {slot:?} : délai après 0,1 s = {cd}"
            );
        }
    }
}

#[test]
fn le_meme_duel_se_rejoue_a_l_identique() {
    assert_eq!(
        simulate_duel("nyx", "ilyra", 11),
        simulate_duel("nyx", "ilyra", 11)
    );
}
