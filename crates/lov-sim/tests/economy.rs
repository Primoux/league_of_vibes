//! Tests de l'économie, de la boutique, du rappel, des objets et des balises : port de
//! `packages/server/test/economy.test.ts`, `recall.test.ts`, `items.test.ts` et `wards.test.ts`.
//!
//! Le monde de test est celui du TS : la liste complète des systèmes, moins les systèmes de PvE
//! (`minions`, `structures`, `jungle`, `objectives`) pour la suite économique.
//!
//! Écart : le test de snapshot des balises appelle directement les hooks `selfState` et
//! `entitySnapshot` que `buildSnapshot` exécute ; la construction des snapshots vit dans `lov-server`
//! (`snapshot.rs`, testée par `lov-server/tests/snapshot.rs`), dont `lov-sim` ne dépend pas.

mod common;

use std::sync::{Arc, Mutex};

use common::{Sim, champ, champ_as, cmd, game_world, v};
use lov_shared::constants::{PASSIVE_GOLD_PER_10S, PASSIVE_GOLD_START, RECALL_SECONDS, scaled};
use lov_shared::data::items::{ITEM_LIST, item};
use lov_shared::map::FOUNTAINS;
use lov_shared::math::dist;
use lov_shared::protocol::ClientMsg;
use lov_shared::schema::ItemCategory;
use lov_shared::types::{
    AbilitySlot, ChampSnapshot, DamageType, EntityId, EntityKind, EntitySnapshot, GameEvent,
    MonsterType, SelfState, Stats, StructureKind, SummonerId, Team, TeamOrNeutral, Vec2,
    WardCharges,
};
use lov_sim::combat::buffs::Stasis;
use lov_sim::combat::damage::{DamageOpts, HealOpts, apply_damage, apply_heal};
use lov_sim::items::{EconomyState, LeviathanBuff, econ, econ_ref};
use lov_sim::systems::combat::hit;
use lov_sim::systems::economy::{
    FIRST_BLOOD_GOLD, KILL_GOLD, PLATE_GOLD, RECALL_LEVIATHAN_SECONDS, STRUCTURE_TEAM_GOLD,
    WARD_CAST_RANGE, WARD_HP, WARD_LIFETIME, WARD_MAX_CHARGES, WARD_RECHARGE, WARD_SIGHT,
    objective_gold, structure_local_gold,
};
use lov_sim::systems::movement::DashData;
use lov_sim::systems::vision::{Invisible, RevealedUntil, SightRadius};
use lov_sim::world_ai::runtime::Rewards;
use lov_sim::{Entity, EventKind};

// ------------------------------------------------------------------------------------------------
// Outils
// ------------------------------------------------------------------------------------------------

/// Monde de la suite économique : les récompenses PvE sont simulées par événements, les vraies
/// vagues, structures et camps ont leurs propres tests.
fn econ_world() -> Sim {
    game_world(&["minions", "structures", "jungle", "objectives"])
}

fn full_world() -> Sim {
    game_world(&[])
}

fn fountain(team: Team) -> Vec2 {
    let f = if team == Team::Red {
        FOUNTAINS.red
    } else {
        FOUNTAINS.blue
    };
    v(f.x, f.y)
}

fn stats_with_hp(hp: f64) -> Stats {
    let mut s = common::base_stats();
    s.ad = 60.0;
    s.max_hp = hp;
    s
}

/// `dummy(world, team, pos, kind, subtype, hp)` de `economy.test.ts`.
fn unit(
    sim: &mut Sim,
    team: TeamOrNeutral,
    pos: Vec2,
    kind: EntityKind,
    subtype: &str,
    hp: f64,
) -> EntityId {
    let mut e = Entity::new(kind, team, pos, 40.0, hp, 0.0, stats_with_hp(hp));
    e.subtype = Some(subtype.into());
    sim.world.spawn(e)
}

/// Sbire rouge de 500 PV (`dummy` de `items.test.ts`).
fn red_minion(sim: &mut Sim, pos: Vec2) -> EntityId {
    sim.world.spawn(Entity::new(
        EntityKind::Minion,
        TeamOrNeutral::Red,
        pos,
        40.0,
        500.0,
        0.0,
        stats_with_hp(500.0),
    ))
}

fn true_damage(sim: &mut Sim, source: EntityId, target: EntityId, amount: f64) -> f64 {
    apply_damage(
        &mut sim.world,
        DamageOpts::new(Some(source), target, amount, DamageType::True),
    )
}

fn gold(sim: &Sim, id: EntityId) -> f64 {
    sim.e(id).champ.as_ref().unwrap().gold
}

fn set_gold(sim: &mut Sim, id: EntityId, g: f64) {
    sim.e_mut(id).champ.as_mut().unwrap().gold = g;
}

fn items(sim: &Sim, id: EntityId) -> Vec<Option<String>> {
    sim.e(id).champ.as_ref().unwrap().items.clone()
}

fn slot_of(sim: &Sim, id: EntityId, item_id: &str) -> usize {
    items(sim, id)
        .iter()
        .position(|s| s.as_deref() == Some(item_id))
        .expect("objet absent de l'inventaire")
}

fn has(sim: &Sim, id: EntityId, item_id: &str) -> bool {
    items(sim, id).iter().any(|s| s.as_deref() == Some(item_id))
}

fn count(sim: &Sim, id: EntityId, item_id: &str) -> usize {
    items(sim, id)
        .iter()
        .filter(|s| s.as_deref() == Some(item_id))
        .count()
}

fn st(sim: &Sim, id: EntityId) -> EconomyState {
    econ_ref(sim.e(id)).cloned().unwrap_or_default()
}

fn buy(sim: &mut Sim, id: EntityId, item_id: &str) {
    cmd(
        sim,
        id,
        ClientMsg::Buy {
            item_id: item_id.into(),
        },
    );
}

fn use_slot(sim: &mut Sim, id: EntityId, slot: usize, target_pos: Option<Vec2>) {
    cmd(
        sim,
        id,
        ClientMsg::UseItem {
            slot: slot as i32,
            target_pos,
        },
    );
}

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-5, "{a} != {b}");
}

/// Mort factice : empêche le respawn automatique du système combat pendant le test.
fn play_dead(sim: &mut Sim, id: EntityId) {
    let e = sim.e_mut(id);
    e.dead = true;
    e.champ.as_mut().unwrap().respawn_in = 9999.0;
}

type Log = Arc<Mutex<Vec<GameEvent>>>;

fn listen(sim: &mut Sim, kinds: &[EventKind]) -> Log {
    let log: Log = Arc::default();
    for &k in kinds {
        let l = Arc::clone(&log);
        sim.world
            .events
            .on(k, move |_, e| l.lock().unwrap().push(e.clone()));
    }
    log
}

// ------------------------------------------------------------------------------------------------
// economy.test.ts
// ------------------------------------------------------------------------------------------------

#[test]
fn donne_500_or_de_depart() {
    let mut s = econ_world();
    let c = champ(&mut s.world, Team::Blue, v(5000.0, 5000.0));
    assert_eq!(gold(&s, c), 500.0);
}

#[test]
fn verse_l_or_passif_toutes_les_10_s() {
    let mut s = econ_world();
    let c = champ(&mut s.world, Team::Blue, v(5000.0, 5000.0));
    s.run(scaled(PASSIVE_GOLD_START) - 1.0);
    assert_eq!(gold(&s, c), 500.0);
    s.run(2.0); // passe scaled(65) = 48,75 s
    close(gold(&s, c), 500.0 + PASSIVE_GOLD_PER_10S);
    s.run(10.0);
    close(gold(&s, c), 500.0 + 2.0 * PASSIVE_GOLD_PER_10S);
}

#[test]
fn donne_l_or_du_last_hit_et_compte_le_cs() {
    let mut s = econ_world();
    let c = champ(&mut s.world, Team::Blue, v(5000.0, 5000.0));
    let m = unit(
        &mut s,
        TeamOrNeutral::Red,
        v(5050.0, 5000.0),
        EntityKind::Minion,
        "melee",
        100.0,
    );
    true_damage(&mut s, c, m, 500.0);
    assert!(s.e(m).dead);
    assert_eq!(s.e(c).champ.as_ref().unwrap().cs, 1);
    close(gold(&s, c), 500.0 + 21.0);
}

#[test]
fn donne_la_prime_de_kill_et_le_premier_sang_au_tueur() {
    let mut s = econ_world();
    let a = champ(&mut s.world, Team::Blue, v(5000.0, 5000.0));
    let b = champ(&mut s.world, Team::Red, v(5100.0, 5000.0));
    true_damage(&mut s, a, b, 100_000.0);
    assert!(s.e(b).dead);
    assert_eq!(s.e(b).champ.as_ref().unwrap().deaths, 1);
    assert_eq!(s.e(a).champ.as_ref().unwrap().kills, 1);
    close(gold(&s, a), 500.0 + KILL_GOLD + FIRST_BLOOD_GOLD);
}

// Test ajouté : le premier sang n'est versé qu'une fois par partie.
#[test]
fn le_premier_sang_n_est_donne_qu_une_fois() {
    let mut s = econ_world();
    let a = champ(&mut s.world, Team::Blue, v(5000.0, 5000.0));
    let b = champ(&mut s.world, Team::Red, v(5100.0, 5000.0));
    let c = champ(&mut s.world, Team::Red, v(5200.0, 5000.0));
    true_damage(&mut s, a, b, 100_000.0);
    true_damage(&mut s, a, c, 100_000.0);
    close(gold(&s, a), 500.0 + 2.0 * KILL_GOLD + FIRST_BLOOD_GOLD);
}

#[test]
fn partage_50_pourcent_de_la_prime_entre_les_assistants() {
    let mut s = econ_world();
    let a = champ(&mut s.world, Team::Blue, v(5000.0, 5000.0));
    let c = champ(&mut s.world, Team::Blue, v(5000.0, 5200.0));
    let b = champ(&mut s.world, Team::Red, v(5100.0, 5000.0));
    true_damage(&mut s, c, b, 10.0); // C touche B : assist
    true_damage(&mut s, a, b, 100_000.0);
    assert_eq!(s.e(c).champ.as_ref().unwrap().assists, 1);
    close(gold(&s, c), 500.0 + KILL_GOLD * 0.5);
    close(gold(&s, a), 500.0 + KILL_GOLD + FIRST_BLOOD_GOLD);
}

#[test]
fn la_prime_suit_la_serie_de_la_victime_et_ses_morts() {
    let mut s1 = econ_world();
    let a1 = champ(&mut s1.world, Team::Blue, v(5000.0, 5000.0));
    let b1 = champ(&mut s1.world, Team::Red, v(5100.0, 5000.0));
    econ(s1.e_mut(b1)).kill_streak = 3;
    true_damage(&mut s1, a1, b1, 100_000.0);
    // 300 + 150 de série + 100 de premier sang
    close(gold(&s1, a1), 500.0 + 450.0 + FIRST_BLOOD_GOLD);

    let mut s2 = econ_world();
    let a2 = champ(&mut s2.world, Team::Blue, v(5000.0, 5000.0));
    let b2 = champ(&mut s2.world, Team::Red, v(5100.0, 5000.0));
    econ(s2.e_mut(b2)).death_streak = 2;
    true_damage(&mut s2, a2, b2, 100_000.0);
    close(gold(&s2, a2), 500.0 + (KILL_GOLD - 50.0) + FIRST_BLOOD_GOLD);
}

/// #312 : un kill remet à zéro la série de morts du tueur ; sa prime n'est plus réduite.
#[test]
fn un_kill_efface_la_serie_de_morts_du_tueur() {
    let mut s = econ_world();
    let a = champ(&mut s.world, Team::Blue, v(5000.0, 5000.0));
    let b = champ(&mut s.world, Team::Red, v(5100.0, 5000.0));
    econ(s.e_mut(a)).death_streak = 4;
    true_damage(&mut s, a, b, 100_000.0);
    assert_eq!(st(&s, a).death_streak, 0);
    assert_eq!(st(&s, a).kill_streak, 1);
    let before = gold(&s, b);
    s.e_mut(b).dead = false;
    s.e_mut(b).hp = s.e(b).stats.max_hp;
    true_damage(&mut s, b, a, 100_000.0);
    close(gold(&s, b), before + KILL_GOLD); // ni réduction, ni premier sang
}

#[test]
fn donne_l_or_global_et_local_des_tours_et_des_plaques() {
    let mut s = econ_world();
    let near = champ(&mut s.world, Team::Blue, v(5000.0, 5000.0));
    let far = champ(&mut s.world, Team::Blue, v(11000.0, 11000.0));
    let tower = unit(
        &mut s,
        TeamOrNeutral::Red,
        v(5100.0, 5000.0),
        EntityKind::Tower,
        "outer",
        5000.0,
    );
    let outer = structure_local_gold("outer").unwrap();
    s.world.emit(GameEvent::StructureDestroyed {
        target: tower,
        kind: StructureKind::Tower,
        team: Team::Red,
        plate: None,
    });
    close(gold(&s, near), 500.0 + STRUCTURE_TEAM_GOLD + outer);
    close(gold(&s, far), 500.0 + STRUCTURE_TEAM_GOLD);
    s.world.emit(GameEvent::StructureDestroyed {
        target: tower,
        kind: StructureKind::Tower,
        team: Team::Red,
        plate: Some(true),
    });
    close(
        gold(&s, near),
        500.0 + STRUCTURE_TEAM_GOLD + outer + PLATE_GOLD,
    );
    close(gold(&s, far), 500.0 + STRUCTURE_TEAM_GOLD); // plaque : or local seulement
}

#[test]
fn lit_les_recompenses_posees_dans_data_rewards() {
    let mut s = econ_world();
    let c = champ(&mut s.world, Team::Blue, v(5000.0, 5000.0));
    let m = unit(
        &mut s,
        TeamOrNeutral::Red,
        v(5050.0, 5000.0),
        EntityKind::Minion,
        "cannon",
        100.0,
    );
    s.e_mut(m).data.insert(Rewards {
        gold: Some(87.0),
        xp: Some(93.0),
        last_hit_only: Some(true),
        xp_radius: Some(1400.0),
        ..Rewards::default()
    });
    true_damage(&mut s, c, m, 500.0);
    close(gold(&s, c), 500.0 + 87.0);
}

#[test]
fn reduit_de_moitie_l_or_des_camps_sans_chatiment() {
    let mut s = econ_world();
    // Flash et Embrasement : pas de Châtiment.
    let c = champ(&mut s.world, Team::Blue, v(5000.0, 5000.0));
    let rewards = Rewards {
        gold: Some(90.0),
        xp: Some(180.0),
        smite_required: Some(true),
        ..Rewards::default()
    };
    let g = unit(
        &mut s,
        TeamOrNeutral::Neutral,
        v(5050.0, 5000.0),
        EntityKind::Monster,
        "gromp",
        100.0,
    );
    s.e_mut(g).data.insert(rewards);
    true_damage(&mut s, c, g, 500.0);
    close(gold(&s, c), 500.0 + 45.0);
    s.e_mut(c).champ.as_mut().unwrap().summoners.d = SummonerId::Smite;
    let g2 = unit(
        &mut s,
        TeamOrNeutral::Neutral,
        v(5050.0, 5000.0),
        EntityKind::Monster,
        "gromp",
        100.0,
    );
    s.e_mut(g2).data.insert(rewards);
    true_damage(&mut s, c, g2, 500.0);
    close(gold(&s, c), 500.0 + 45.0 + 90.0);
}

#[test]
fn simulation_or_total_a_10_min() {
    let mut s = econ_world();
    let c = champ(&mut s.world, Team::Blue, v(5000.0, 5000.0));
    // 10 min de jeu : un last hit de sbire mêlée toutes les 30 s.
    for _ in 0..20 {
        let m = unit(
            &mut s,
            TeamOrNeutral::Red,
            v(5050.0, 5000.0),
            EntityKind::Minion,
            "melee",
            100.0,
        );
        true_damage(&mut s, c, m, 500.0);
        s.run(30.0);
    }
    // 500 de départ + passif (~56 × 20,4 après 48,75 s) + 20 × 21 de last hits
    let g = gold(&s, c);
    assert!(g > 1900.0 && g < 2300.0, "or : {g}");
    assert_eq!(s.e(c).champ.as_ref().unwrap().cs, 20);
}

#[test]
fn simulation_m4_or_a_10_20_30_min() {
    let mut s = econ_world();
    let c = champ(&mut s.world, Team::Blue, v(5000.0, 5000.0));
    let wave_interval = scaled(30.0); // 22,5 s de jeu entre deux vagues
    // Or de départ + passif + last hits : 10 min ~5,0k, 20 min ~9,6k, 30 min ~14k.
    let checkpoints = [
        (600.0, 4500.0, 5500.0),
        (1200.0, 8800.0, 10500.0),
        (1800.0, 13000.0, 15500.0),
    ];
    let mut next_wave = 0.0;
    for (target, lo, hi) in checkpoints {
        while s.world.time < target {
            if s.world.time >= next_wave {
                let n = (next_wave / wave_interval).round() as u32;
                for (dy, subtype) in [(0.0, "melee"), (50.0, "caster")] {
                    for _ in 0..3 {
                        let m = unit(
                            &mut s,
                            TeamOrNeutral::Red,
                            v(5050.0, 5000.0 + dy),
                            EntityKind::Minion,
                            subtype,
                            50.0,
                        );
                        true_damage(&mut s, c, m, 500.0);
                    }
                }
                if n % 3 == 2 {
                    let m = unit(
                        &mut s,
                        TeamOrNeutral::Red,
                        v(5050.0, 5100.0),
                        EntityKind::Minion,
                        "cannon",
                        50.0,
                    );
                    true_damage(&mut s, c, m, 500.0);
                }
                next_wave += wave_interval;
            }
            s.run(1.0);
        }
        let g = gold(&s, c);
        assert!(
            g >= lo && g <= hi,
            "à {target} s : {g} hors de [{lo}, {hi}]"
        );
    }
}

#[test]
fn n_emet_pas_d_annonce_pour_les_kills() {
    let mut s = econ_world();
    let a = champ(&mut s.world, Team::Blue, v(5000.0, 5000.0));
    let b = champ(&mut s.world, Team::Red, v(5100.0, 5000.0));
    let ann = listen(&mut s, &[EventKind::Announcement]);
    true_damage(&mut s, a, b, 100_000.0);
    assert_eq!(s.e(a).champ.as_ref().unwrap().kills, 1);
    assert!(ann.lock().unwrap().is_empty());
}

#[test]
fn donne_l_or_d_equipe_des_objectifs() {
    let mut s = econ_world();
    let a = champ(&mut s.world, Team::Blue, v(5000.0, 5000.0));
    let b = champ(&mut s.world, Team::Red, v(9000.0, 9000.0));
    s.world.emit(GameEvent::ObjectiveTaken {
        monster: MonsterType::Dragon,
        team: Team::Blue,
        killer: None,
        element: None,
    });
    close(gold(&s, a), 500.0 + objective_gold(MonsterType::Dragon));
    assert_eq!(gold(&s, b), 500.0);
}

// ------------------------------------------------------------------------------------------------
// recall.test.ts
// ------------------------------------------------------------------------------------------------

fn blue_fountain_dist(s: &Sim, id: EntityId) -> f64 {
    dist(s.e(id).pos, fountain(Team::Blue))
}

#[test]
fn rappel_canalise_8_s_puis_teleporte_a_la_fontaine() {
    let mut s = full_world();
    let c = champ(&mut s.world, Team::Blue, v(6000.0, 6000.0));
    let events = listen(&mut s, &[EventKind::RecallStart, EventKind::RecallEnd]);
    cmd(&mut s, c, ClientMsg::Recall);
    assert_eq!(st(&s, c).recall.unwrap().total, RECALL_SECONDS);
    s.run(RECALL_SECONDS - 1.0);
    assert!(st(&s, c).recall.is_some(), "pas encore terminé");
    assert!(blue_fountain_dist(&s, c) > 1000.0);
    s.run(1.5);
    assert!(st(&s, c).recall.is_none());
    assert!(blue_fountain_dist(&s, c) < 200.0);
    let events = events.lock().unwrap();
    assert_eq!(events.len(), 2);
    assert!(matches!(events[0], GameEvent::RecallStart { .. }));
    assert!(matches!(
        events[1],
        GameEvent::RecallEnd {
            completed: true,
            ..
        }
    ));
}

#[test]
fn rappel_interrompu_par_les_degats_subis() {
    let mut s = full_world();
    let c = champ(&mut s.world, Team::Blue, v(6000.0, 6000.0));
    let enemy = champ_as(&mut s.world, Team::Red, v(6100.0, 6000.0), "sable");
    let ends = listen(&mut s, &[EventKind::RecallEnd]);
    cmd(&mut s, c, ClientMsg::Recall);
    s.run(2.0);
    apply_damage(
        &mut s.world,
        DamageOpts::new(Some(enemy), c, 10.0, DamageType::Physical),
    );
    assert!(st(&s, c).recall.is_none());
    {
        let ends = ends.lock().unwrap();
        assert_eq!(ends.len(), 1);
        assert!(matches!(
            ends[0],
            GameEvent::RecallEnd {
                completed: false,
                ..
            }
        ));
    }
    s.run(RECALL_SECONDS);
    assert!(blue_fountain_dist(&s, c) > 1000.0, "pas téléporté");
}

#[test]
fn rappel_interrompu_par_un_ordre_de_deplacement() {
    let mut s = full_world();
    let c = champ(&mut s.world, Team::Blue, v(6000.0, 6000.0));
    cmd(&mut s, c, ClientMsg::Recall);
    s.run(1.0);
    cmd(
        &mut s,
        c,
        ClientMsg::Move {
            pos: v(6100.0, 6000.0),
        },
    );
    assert!(st(&s, c).recall.is_none());
}

/// #311 : Flash ou un sort interrompt le rappel ; un sort refusé (pas de rang) ne l'interrompt pas.
#[test]
fn rappel_interrompu_par_flash_ou_un_sort() {
    let mut s = full_world();
    let c = champ(&mut s.world, Team::Blue, v(6000.0, 3000.0));
    cmd(&mut s, c, ClientMsg::Recall);
    s.run(1.0);
    cmd(
        &mut s,
        c,
        ClientMsg::Summoner {
            slot: lov_shared::types::SummonerSlot::D,
            target_pos: Some(v(6300.0, 3000.0)),
            target_id: None,
        },
    );
    assert!(st(&s, c).recall.is_none(), "Flash");
    s.run(9.0);
    assert!(dist(s.e(c).pos, fountain(Team::Blue)) > 2000.0);

    let mut s = full_world();
    let c = champ(&mut s.world, Team::Blue, v(6000.0, 3000.0));
    s.e_mut(c).mana = 1000.0;
    cmd(&mut s, c, ClientMsg::Recall);
    let q = ClientMsg::Cast {
        slot: AbilitySlot::Q,
        target_pos: Some(v(6400.0, 3000.0)),
        target_id: None,
    };
    cmd(&mut s, c, q.clone()); // Q au rang 0 : refusé
    assert!(
        st(&s, c).recall.is_some(),
        "un sort refusé ne coupe pas le rappel"
    );
    s.e_mut(c).champ.as_mut().unwrap().ability_levels.q = 1;
    cmd(&mut s, c, q);
    assert!(st(&s, c).recall.is_none(), "sort lancé");
}

#[test]
fn rappel_dure_4_s_avec_le_buff_du_leviathan() {
    let mut s = full_world();
    let c = champ(&mut s.world, Team::Blue, v(6000.0, 6000.0));
    let expires_at = s.world.time + 100.0;
    s.e_mut(c).data.insert(LeviathanBuff {
        expires_at,
        recall_seconds: Some(4.0),
    });
    cmd(&mut s, c, ClientMsg::Recall);
    assert_eq!(st(&s, c).recall.unwrap().total, RECALL_LEVIATHAN_SECONDS);
    s.run(RECALL_LEVIATHAN_SECONDS + 0.5);
    assert!(st(&s, c).recall.is_none());
    assert!(blue_fountain_dist(&s, c) < 200.0);
}

#[test]
fn rappel_annule_a_la_mort_et_impossible_quand_mort() {
    let mut s = full_world();
    let c = champ(&mut s.world, Team::Blue, v(6000.0, 6000.0));
    let enemy = champ_as(&mut s.world, Team::Red, v(6100.0, 6000.0), "sable");
    play_dead(&mut s, c);
    cmd(&mut s, c, ClientMsg::Recall);
    assert!(st(&s, c).recall.is_none());
    s.e_mut(c).dead = false;
    cmd(&mut s, c, ClientMsg::Recall);
    assert!(st(&s, c).recall.is_some());
    true_damage(&mut s, enemy, c, 100_000.0);
    assert!(s.e(c).dead);
    assert!(st(&s, c).recall.is_none());
}

// ------------------------------------------------------------------------------------------------
// items.test.ts — boutique
// ------------------------------------------------------------------------------------------------

fn at_fountain(s: &mut Sim) -> EntityId {
    champ(&mut s.world, Team::Blue, fountain(Team::Blue))
}

#[test]
fn achete_un_objet_a_la_fontaine() {
    let mut s = full_world();
    let c = at_fountain(&mut s);
    let ad_before = s.e(c).stats.ad;
    let hp_before = s.e(c).stats.max_hp;
    buy(&mut s, c, "long_sword");
    assert_eq!(gold(&s, c), 150.0);
    assert_eq!(items(&s, c)[0].as_deref(), Some("long_sword"));
    s.run(0.1);
    close(s.e(c).stats.ad, ad_before + 10.0);
    assert_eq!(s.e(c).stats.max_hp, hp_before);
}

#[test]
fn refuse_l_achat_hors_fontaine_mais_l_autorise_en_mort() {
    let mut s = full_world();
    let c = champ(&mut s.world, Team::Blue, v(6000.0, 6000.0));
    buy(&mut s, c, "long_sword");
    assert_eq!(gold(&s, c), 500.0);
    assert!(items(&s, c).iter().all(Option::is_none));
    s.e_mut(c).dead = true;
    buy(&mut s, c, "long_sword");
    assert_eq!(items(&s, c)[0].as_deref(), Some("long_sword"));
}

#[test]
fn refuse_l_achat_sans_or_suffisant() {
    let mut s = full_world();
    let c = at_fountain(&mut s);
    set_gold(&mut s, c, 100.0);
    buy(&mut s, c, "long_sword");
    assert!(items(&s, c).iter().all(Option::is_none));
    assert_eq!(gold(&s, c), 100.0);
}

#[test]
fn recette_consomme_les_composants_et_deduit_leur_cout() {
    let mut s = full_world();
    let c = at_fountain(&mut s);
    set_gold(&mut s, c, 2000.0);
    let ms_base = s.e(c).stats.move_speed;
    let as_base = s.e(c).stats.attack_speed;
    buy(&mut s, c, "boots");
    assert_eq!(gold(&s, c), 1700.0);
    buy(&mut s, c, "berserker_greaves"); // 1100 - 300 (bottes possédées) = 800
    assert_eq!(gold(&s, c), 900.0);
    assert!(has(&s, c, "berserker_greaves"));
    assert!(!has(&s, c, "boots"));
    let slot = slot_of(&s, c, "berserker_greaves");
    assert_eq!(st(&s, c).paid_price[slot], Some(1100.0));
    s.run(0.1);
    close(s.e(c).stats.move_speed, ms_base + 45.0);
    close(s.e(c).stats.attack_speed, as_base + 0.35);
}

/// #307 : un composant demandé deux fois n'est déduit qu'autant de fois qu'il est possédé.
#[test]
fn recette_a_composant_double_ne_deduit_que_les_exemplaires_possedes() {
    let mut s = full_world();
    let c = at_fountain(&mut s);
    set_gold(&mut s, c, 3000.0);
    buy(&mut s, c, "long_sword"); // 3000 - 350
    buy(&mut s, c, "grand_glaive"); // 1300 - 350 (une seule épée) = 950
    assert_eq!(gold(&s, c), 3000.0 - 350.0 - 950.0);
    assert!(has(&s, c, "grand_glaive"));
    assert_eq!(count(&s, c, "long_sword"), 0);

    let mut s = full_world();
    let c = at_fountain(&mut s);
    set_gold(&mut s, c, 3000.0);
    buy(&mut s, c, "long_sword");
    buy(&mut s, c, "long_sword");
    buy(&mut s, c, "grand_glaive"); // 1300 - 2 × 350 = 600
    assert_eq!(gold(&s, c), 3000.0 - 700.0 - 600.0);
    assert_eq!(count(&s, c, "long_sword"), 0);
    let slot = slot_of(&s, c, "grand_glaive");
    assert_eq!(st(&s, c).paid_price[slot], Some(1300.0));
    // L'annulation rend bien les deux épées.
    cmd(&mut s, c, ClientMsg::UndoShop);
    assert_eq!(count(&s, c, "long_sword"), 2);
    assert_eq!(gold(&s, c), 3000.0 - 700.0);
}

#[test]
fn annuler_rembourse_l_achat_et_rend_les_composants() {
    let mut s = full_world();
    let c = at_fountain(&mut s);
    set_gold(&mut s, c, 2000.0);
    buy(&mut s, c, "boots");
    buy(&mut s, c, "berserker_greaves");
    assert_eq!(gold(&s, c), 900.0);
    cmd(&mut s, c, ClientMsg::UndoShop); // annule seulement le dernier achat (800 or)
    assert_eq!(gold(&s, c), 1700.0);
    assert!(has(&s, c, "boots"));
    assert!(!has(&s, c, "berserker_greaves"));
}

#[test]
fn vente_a_70_pourcent_et_annulation_de_la_vente() {
    let mut s = full_world();
    let c = at_fountain(&mut s);
    buy(&mut s, c, "long_sword"); // 350 -> 150
    cmd(&mut s, c, ClientMsg::Sell { slot: 0 }); // +245
    assert_eq!(gold(&s, c), 150.0 + 245.0);
    assert!(items(&s, c)[0].is_none());
    cmd(&mut s, c, ClientMsg::UndoShop);
    assert_eq!(gold(&s, c), 150.0);
    assert_eq!(items(&s, c)[0].as_deref(), Some("long_sword"));
}

#[test]
fn objet_unique_pas_deux_paires_de_bottes() {
    let mut s = full_world();
    let c = at_fountain(&mut s);
    set_gold(&mut s, c, 2000.0);
    buy(&mut s, c, "boots");
    buy(&mut s, c, "boots");
    assert_eq!(count(&s, c, "boots"), 1);
    assert_eq!(gold(&s, c), 1700.0);
}

#[test]
fn consommable_empile_les_charges_dans_un_seul_emplacement() {
    let mut s = full_world();
    let c = at_fountain(&mut s);
    set_gold(&mut s, c, 1000.0);
    buy(&mut s, c, "health_potion");
    buy(&mut s, c, "health_potion");
    assert_eq!(gold(&s, c), 900.0);
    assert_eq!(count(&s, c, "health_potion"), 1);
    assert_eq!(st(&s, c).charges[slot_of(&s, c, "health_potion")], Some(2));
}

#[test]
fn consommable_prix_cumule_annulation_et_vente() {
    let mut s = full_world();
    let c = at_fountain(&mut s);
    set_gold(&mut s, c, 1000.0);
    buy(&mut s, c, "health_potion");
    buy(&mut s, c, "health_potion");
    let slot = slot_of(&s, c, "health_potion");
    assert_eq!(st(&s, c).charges[slot], Some(2));
    cmd(&mut s, c, ClientMsg::UndoShop); // annule le 2e achat : -1 charge, +50 or
    assert_eq!(gold(&s, c), 950.0);
    assert_eq!(st(&s, c).charges[slot], Some(1));
    assert_eq!(items(&s, c)[slot].as_deref(), Some("health_potion"));
    cmd(&mut s, c, ClientMsg::UndoShop); // annule le 1er achat : emplacement vidé, +50 or
    assert_eq!(gold(&s, c), 1000.0);
    assert!(items(&s, c)[slot].is_none());
    // 2 achats puis vente : remboursement = 70 % de 100 = 70
    buy(&mut s, c, "health_potion");
    buy(&mut s, c, "health_potion");
    assert_eq!(gold(&s, c), 900.0);
    cmd(&mut s, c, ClientMsg::Sell { slot: slot as i32 });
    assert_eq!(gold(&s, c), 970.0);
}

// Tests ajoutés : corrections par rapport au TS (voir l'en-tête de `systems/economy.rs`).
#[test]
fn annuler_un_achat_empile_puis_vendre_ne_rembourse_que_le_reste() {
    let mut s = full_world();
    let c = at_fountain(&mut s);
    set_gold(&mut s, c, 1000.0);
    buy(&mut s, c, "health_potion");
    buy(&mut s, c, "health_potion");
    let slot = slot_of(&s, c, "health_potion");
    cmd(&mut s, c, ClientMsg::UndoShop);
    assert_eq!(st(&s, c).paid_price[slot], Some(50.0));
    cmd(&mut s, c, ClientMsg::Sell { slot: slot as i32 });
    assert_eq!(gold(&s, c), 950.0 + 35.0);
}

#[test]
fn annuler_la_vente_d_une_pile_rend_ses_charges() {
    let mut s = full_world();
    let c = at_fountain(&mut s);
    set_gold(&mut s, c, 1000.0);
    buy(&mut s, c, "health_potion");
    buy(&mut s, c, "health_potion");
    let slot = slot_of(&s, c, "health_potion");
    cmd(&mut s, c, ClientMsg::Sell { slot: slot as i32 });
    cmd(&mut s, c, ClientMsg::UndoShop);
    assert_eq!(items(&s, c)[slot].as_deref(), Some("health_potion"));
    assert_eq!(st(&s, c).charges[slot], Some(2));
}

#[test]
fn inventaire_plein_refuse_l_achat_au_dela_de_6_emplacements() {
    let mut s = full_world();
    let c = at_fountain(&mut s);
    set_gold(&mut s, c, 99_999.0);
    for id in [
        "long_sword",
        "amplifying_tome",
        "ruby_crystal",
        "sapphire_crystal",
        "dagger",
        "cloth_armor",
    ] {
        buy(&mut s, c, id);
    }
    assert!(items(&s, c).iter().all(Option::is_some));
    buy(&mut s, c, "negatron_cloak");
    assert!(!has(&s, c, "negatron_cloak"));
}

#[test]
fn trinket_gratuit_et_unique() {
    let mut s = full_world();
    let c = at_fountain(&mut s);
    buy(&mut s, c, "stealth_ward");
    assert_eq!(st(&s, c).trinket.as_deref(), Some("stealth_ward"));
    assert_eq!(st(&s, c).trinket_charges, 2);
    buy(&mut s, c, "stealth_ward");
    assert!(!has(&s, c, "stealth_ward"), "pas dans les 6 emplacements");
}

#[test]
fn un_gain_de_pv_max_soigne_du_delta() {
    let mut s = full_world();
    let c = at_fountain(&mut s);
    set_gold(&mut s, c, 1000.0);
    s.e_mut(c).hp = 400.0;
    let hp_max_before = s.e(c).stats.max_hp;
    buy(&mut s, c, "ruby_crystal");
    // 400 + 150 de delta de PV max dès l'achat (+ soin de fontaine)
    assert!(s.e(c).hp >= 550.0, "PV : {}", s.e(c).hp);
    s.run(0.1); // recalcul des stats par le système buffs
    close(s.e(c).stats.max_hp, hp_max_before + 150.0);
    assert!(s.e(c).hp > 500.0);
}

// ------------------------------------------------------------------------------------------------
// items.test.ts — passifs et actifs
// ------------------------------------------------------------------------------------------------

#[test]
fn potion_soigne_sur_15_s() {
    let mut s = full_world();
    let c = at_fountain(&mut s);
    set_gold(&mut s, c, 1000.0);
    buy(&mut s, c, "health_potion");
    // Hors fontaine : pas de soin de fontaine parasite.
    s.e_mut(c).pos = v(6000.0, 6000.0);
    s.e_mut(c).hp = 400.0;
    let slot = slot_of(&s, c, "health_potion");
    use_slot(&mut s, c, slot, None);
    assert!(st(&s, c).hot.is_some());
    s.run(15.0);
    // Soin complet de la potion (120) + régénération passive pendant le test.
    let hp = s.e(c).hp;
    assert!(hp > 515.0 && hp < 580.0, "PV : {hp}");
    assert!(
        items(&s, c)[slot].is_none(),
        "plus de charges : emplacement vidé"
    );
}

/// #315 : anti-soin de l'objet et des buffs (Ignite) ne se cumulent pas, seul le plus fort compte.
#[test]
fn anti_soins_objet_et_buff_ne_se_cumulent_pas() {
    use lov_sim::combat::buffs::{BuffOptions, add_buff};
    let heal_with = |buff: Option<f64>| {
        let mut s = econ_world();
        let e = champ(&mut s.world, Team::Blue, v(5000.0, 5000.0));
        s.e_mut(e).hp = 100.0;
        let now = s.world.time;
        econ(s.e_mut(e)).grievous_until = now + 3.0;
        if let Some(g) = buff {
            let mut o = BuffOptions::new(5.0);
            o.params.grievous = Some(g);
            add_buff(&mut s.world, e, "ignite", o);
        }
        apply_heal(&mut s.world, HealOpts::new(Some(e), e, 100.0))
    };
    close(heal_with(None), 60.0); // objet seul : 40 %
    close(heal_with(Some(0.4)), 60.0); // Ignite 40 % + objet 40 % : 40 %
    close(heal_with(Some(0.2)), 60.0); // objet plus fort
    close(heal_with(Some(0.6)), 40.0); // buff plus fort
}

#[test]
fn hache_du_bourreau_blessure_grave_reduit_les_soins_de_40_pourcent() {
    let mut s = full_world();
    let c = champ(&mut s.world, Team::Blue, v(5000.0, 5000.0));
    set_gold(&mut s, c, 5000.0);
    s.e_mut(c).pos = fountain(Team::Blue);
    buy(&mut s, c, "hache_du_bourreau");
    let e = red_minion(&mut s, v(5100.0, 5000.0));
    apply_damage(
        &mut s.world,
        DamageOpts::new(Some(c), e, 50.0, DamageType::Physical),
    );
    assert!(st(&s, e).grievous_until > s.world.time);
    s.e_mut(e).hp = 100.0;
    let healed = apply_heal(&mut s.world, HealOpts::new(Some(c), e, 100.0));
    close(healed, 60.0);
}

#[test]
fn sablier_stase_de_2_5_s_invulnerable() {
    let mut s = full_world();
    let c = at_fountain(&mut s);
    set_gold(&mut s, c, 5000.0);
    buy(&mut s, c, "sablier_eternel");
    let slot = slot_of(&s, c, "sablier_eternel");
    use_slot(&mut s, c, slot, None);
    assert!(st(&s, c).stase_until > s.world.time);
    assert!(s.e(c).data.contains::<Stasis>());
    let hp_before = s.e(c).hp;
    let src = red_minion(&mut s, v(0.0, 0.0));
    true_damage(&mut s, src, c, 99_999.0);
    assert!(!s.e(c).dead);
    assert_eq!(s.e(c).hp, hp_before, "dégâts annulés par la stase");
    s.run(3.0);
    assert!(!s.e(c).data.contains::<Stasis>(), "stase levée");
}

#[test]
fn carapace_du_roc_renvoie_10_pourcent_des_degats() {
    let mut s = full_world();
    let c = at_fountain(&mut s);
    set_gold(&mut s, c, 5000.0);
    buy(&mut s, c, "carapace_du_roc");
    let e = red_minion(&mut s, v(5100.0, 5000.0));
    let hp_before = s.e(e).hp;
    apply_damage(
        &mut s.world,
        DamageOpts::new(Some(e), c, 200.0, DamageType::Magic),
    );
    assert!(s.e(e).hp < hp_before, "dégâts renvoyés");
}

#[test]
fn vengeance_pas_de_boucle_entre_deux_porteurs() {
    let mut s = full_world();
    let a = champ(&mut s.world, Team::Blue, v(5000.0, 5000.0));
    set_gold(&mut s, a, 9999.0);
    play_dead(&mut s, a);
    buy(&mut s, a, "carapace_du_roc");
    s.e_mut(a).dead = false;
    let b = champ_as(&mut s.world, Team::Red, v(5100.0, 5000.0), "sable");
    set_gold(&mut s, b, 9999.0);
    play_dead(&mut s, b);
    buy(&mut s, b, "carapace_du_roc");
    s.e_mut(b).dead = false;
    let (hp_a, hp_b) = (s.e(a).hp, s.e(b).hp);
    apply_damage(
        &mut s.world,
        DamageOpts::new(Some(a), b, 100.0, DamageType::Magic),
    );
    // Un seul niveau de renvoi : b renvoie à a, et ce renvoi ne renvoie pas.
    assert!(s.e(b).hp < hp_b);
    assert!(s.e(a).hp < hp_a);
    assert!(s.e(a).hp > hp_a - 30.0, "renvoi borné");
}

#[test]
fn lame_spectrale_dash_vers_la_position_visee() {
    let mut s = full_world();
    let c = champ(&mut s.world, Team::Blue, v(5000.0, 5000.0));
    set_gold(&mut s, c, 5000.0);
    play_dead(&mut s, c); // achat autorisé en mort
    buy(&mut s, c, "lame_spectrale");
    s.e_mut(c).dead = false;
    let slot = slot_of(&s, c, "lame_spectrale");
    use_slot(&mut s, c, slot, Some(v(5400.0, 5000.0)));
    assert!(s.e(c).data.contains::<DashData>());
    s.run(1.0);
    assert!(s.e(c).pos.x > 5100.0);
    assert!(
        !s.e(c).data.contains::<DashData>(),
        "dash consommé à l'arrivée"
    );
}

/// #314 : l'AA renforcée reste physique, le bonus part en dégâts magiques séparés ; 1,5 s de délai.
#[test]
fn sort_lame_bonus_magique_separe_et_delai() {
    let mut s = full_world();
    let c = champ_as(&mut s.world, Team::Blue, fountain(Team::Blue), "ilyra");
    set_gold(&mut s, c, 5000.0);
    buy(&mut s, c, "sceptre_de_lorage");
    s.step(); // stats de l'objet recalculées au tick suivant l'achat
    let target = red_minion(&mut s, v(600.0, 600.0));
    let cast = |s: &mut Sim| {
        s.world.emit(GameEvent::AbilityCast {
            source: c,
            champion_id: "ilyra".into(),
            slot: AbilitySlot::Q,
            ability_id: "ilyra_q".into(),
            target_pos: None,
            target_id: None,
            fx_key: None,
        })
    };
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&seen);
    s.world.events.on(EventKind::Damage, move |_, ev| {
        if let GameEvent::Damage {
            damage_type,
            ability,
            ..
        } = ev
        {
            log.lock().unwrap().push((*damage_type, ability.clone()));
        }
    });
    cast(&mut s);
    hit(&mut s.world, c, target, false);
    let got = seen.lock().unwrap().clone();
    assert_eq!(got[0].0, DamageType::Physical, "{got:?}");
    assert_eq!(
        got[1],
        (DamageType::Magic, Some("item_spellblade".into())),
        "{got:?}"
    );
    // Délai : un sort juste après ne réarme pas, un sort 1,5 s plus tard si.
    cast(&mut s);
    assert!(!st(&s, c).spellblade_ready);
    s.run(1.6);
    cast(&mut s);
    assert!(st(&s, c).spellblade_ready);
}

#[test]
fn sceptre_de_l_orage_sort_lame() {
    let mut s = full_world();
    let c = champ_as(&mut s.world, Team::Blue, fountain(Team::Blue), "ilyra");
    set_gold(&mut s, c, 5000.0);
    buy(&mut s, c, "sceptre_de_lorage");
    let target = red_minion(&mut s, v(600.0, 600.0));
    assert!(!st(&s, c).spellblade_ready);
    s.world.emit(GameEvent::AbilityCast {
        source: c,
        champion_id: "ilyra".into(),
        slot: AbilitySlot::Q,
        ability_id: "ilyra_q".into(),
        target_pos: None,
        target_id: None,
        fx_key: None,
    });
    assert!(st(&s, c).spellblade_ready);
    hit(&mut s.world, c, target, false);
    assert!(!st(&s, c).spellblade_ready);
}

#[test]
fn donnees_des_objets_coherentes() {
    let ids: Vec<&str> = ITEM_LIST.iter().map(|i| i.id.as_str()).collect();
    let mut uniq = ids.clone();
    uniq.sort_unstable();
    uniq.dedup();
    assert_eq!(uniq.len(), ids.len(), "ids uniques");
    let mut icons: Vec<&str> = ITEM_LIST.iter().map(|i| i.icon.as_str()).collect();
    icons.sort_unstable();
    icons.dedup();
    assert_eq!(icons.len(), ITEM_LIST.len(), "icônes uniques");
    for it in ITEM_LIST.iter() {
        assert!(it.name.chars().count() > 2);
        assert!(it.description.chars().count() > 5);
        let comp_cost: f64 = it
            .builds_from
            .iter()
            .map(|c| item(c).expect("composant inconnu").cost)
            .sum();
        assert!(
            it.cost >= comp_cost,
            "{} : coût de combinaison négatif",
            it.id
        );
        if it.category == ItemCategory::Legendary {
            assert!(
                (2000.0..=3400.0).contains(&it.cost),
                "{} : {}",
                it.id,
                it.cost
            );
        }
    }
    for it in ITEM_LIST
        .iter()
        .filter(|i| i.category == ItemCategory::Boots)
    {
        assert_eq!(it.unique, Some(true));
    }
    assert_eq!(item("colossus_eye").unwrap().shop, Some(false));
    assert_eq!(item("stealth_ward").unwrap().cost, 0.0);
    assert!(ITEM_LIST.len() >= 95);
    for cat in [
        ItemCategory::Starter,
        ItemCategory::Consumable,
        ItemCategory::Boots,
        ItemCategory::Basic,
        ItemCategory::Legendary,
        ItemCategory::Trinket,
    ] {
        assert!(ITEM_LIST.iter().any(|i| i.category == cat));
    }
}

// ------------------------------------------------------------------------------------------------
// wards.test.ts
// ------------------------------------------------------------------------------------------------

fn wards(s: &Sim) -> Vec<EntityId> {
    s.world
        .query(Some(EntityKind::Ward))
        .map(|w| w.id)
        .collect()
}

fn place(s: &mut Sim, id: EntityId, x: f64, y: f64) {
    cmd(s, id, ClientMsg::PlaceWard { pos: v(x, y) });
}

#[test]
fn pose_une_balise_consomme_une_charge_2_max() {
    let mut s = full_world();
    let c = at_fountain(&mut s);
    buy(&mut s, c, "stealth_ward");
    assert_eq!(st(&s, c).trinket_charges, WARD_MAX_CHARGES);
    place(&mut s, c, 600.0, 600.0);
    assert_eq!(wards(&s).len(), 1);
    assert_eq!(st(&s, c).trinket_charges, 1);
    assert!(s.e(wards(&s)[0]).data.contains::<Invisible>());
    place(&mut s, c, 700.0, 600.0);
    assert_eq!(wards(&s).len(), 2);
    assert_eq!(st(&s, c).trinket_charges, 0);
    place(&mut s, c, 800.0, 600.0);
    assert_eq!(wards(&s).len(), 2, "plus de charge");
}

#[test]
fn refuse_la_pose_sans_trinket_ou_hors_de_portee() {
    let mut s = full_world();
    let c = at_fountain(&mut s);
    place(&mut s, c, 600.0, 600.0);
    assert!(wards(&s).is_empty());
    buy(&mut s, c, "stealth_ward");
    let f = fountain(Team::Blue);
    place(&mut s, c, f.x + WARD_CAST_RANGE + 500.0, f.y);
    assert!(wards(&s).is_empty());
}

#[test]
fn recharge_une_charge_toutes_les_90_s() {
    let mut s = full_world();
    let c = at_fountain(&mut s);
    buy(&mut s, c, "stealth_ward");
    place(&mut s, c, 600.0, 600.0);
    place(&mut s, c, 700.0, 600.0);
    assert_eq!(st(&s, c).trinket_charges, 0);
    s.run(WARD_RECHARGE + 1.0);
    assert_eq!(st(&s, c).trinket_charges, 1);
}

#[test]
fn la_balise_expire_apres_90_s() {
    let mut s = full_world();
    let c = at_fountain(&mut s);
    buy(&mut s, c, "stealth_ward");
    place(&mut s, c, 600.0, 600.0);
    assert_eq!(wards(&s).len(), 1);
    s.run(WARD_LIFETIME - 1.0);
    assert_eq!(wards(&s).len(), 1);
    s.run(4.0); // expiration à 90 s + 2 s de cadavre (combat)
    assert!(wards(&s).is_empty());
}

#[test]
fn donne_la_vision_et_se_revele_si_un_ennemi_approche() {
    let mut s = full_world();
    let c = at_fountain(&mut s);
    buy(&mut s, c, "stealth_ward");
    s.e_mut(c).pos = v(3000.0, 3000.0);
    place(&mut s, c, 3100.0, 3000.0);
    let ward = wards(&s)[0];
    assert_eq!(
        s.e(ward).data.get::<SightRadius>().map(|r| r.0),
        Some(WARD_SIGHT)
    );
    let enemy = champ(
        &mut s.world,
        Team::Red,
        v(3000.0 + WARD_SIGHT - 50.0, 3000.0),
    );
    s.run(0.1);
    let revealed = s.e(ward).data.get::<RevealedUntil>().map(|r| r.0);
    assert!(revealed.is_some_and(|t| t > s.world.time - 1.0));
    assert!(s.e(enemy).pos.x <= 3000.0 + WARD_SIGHT);
}

#[test]
fn snapshot_items_a_7_entrees_et_charges_de_balise() {
    let mut s = full_world();
    let c = at_fountain(&mut s);
    buy(&mut s, c, "stealth_ward");
    let ent = s.e(c);
    let ch = ent.champ.as_ref().unwrap();
    let mut snap = EntitySnapshot {
        id: c,
        kind: ent.kind,
        team: ent.team,
        x: ent.pos.x,
        y: ent.pos.y,
        facing: ent.facing,
        hp: ent.hp,
        max_hp: ent.stats.max_hp,
        mana: None,
        max_mana: None,
        shield: None,
        dead: None,
        subtype: None,
        owner_id: None,
        level: None,
        anim: None,
        buffs: None,
        champ: Some(ChampSnapshot {
            player_id: ch.player_id.clone(),
            name: ch.name.clone(),
            kills: 0,
            deaths: 0,
            assists: 0,
            cs: 0,
            items: ch.items.clone(),
            respawn_in: None,
        }),
    };
    s.world.apply_entity_snapshot(ent, &mut snap);
    let items = snap.champ.unwrap().items;
    assert_eq!(items.len(), 7);
    assert_eq!(items[6].as_deref(), Some("stealth_ward"));
    let mut state = SelfState {
        entity_id: c,
        gold: ch.gold,
        xp: ch.xp,
        xp_to_next: 0.0,
        level: ch.level,
        skill_points: ch.skill_points,
        ability_levels: ch.ability_levels.clone(),
        cooldowns: ch.cooldowns.clone(),
        summoners: ch.summoners.clone(),
        stats: ent.stats.clone(),
        recalling: None,
        can_shop: false,
        ward_charges: None,
        item_charges: None,
        item_cooldowns: None,
    };
    s.world.apply_self_state(ent, &mut state);
    assert!(state.can_shop);
    assert_eq!(
        state.ward_charges,
        Some(WardCharges {
            charges: 2,
            cooldown_remaining: 0.0
        })
    );
}

#[test]
fn est_detruite_en_3_auto_attaques() {
    let mut s = full_world();
    let c = at_fountain(&mut s);
    buy(&mut s, c, "stealth_ward");
    s.e_mut(c).pos = v(3000.0, 3000.0);
    place(&mut s, c, 3100.0, 3000.0);
    let ward = wards(&s)[0];
    assert_eq!(s.e(ward).hp, WARD_HP);
    let enemy = champ(&mut s.world, Team::Red, v(3150.0, 3000.0));
    s.run(0.1); // révèle la balise
    hit(&mut s.world, enemy, ward, false);
    assert_eq!(s.e(ward).hp, WARD_HP - 1.0);
    hit(&mut s.world, enemy, ward, false);
    hit(&mut s.world, enemy, ward, false);
    assert!(s.e(ward).dead);
}
