//! Garde-fous d'équilibrage : port de `packages/server/test/balance.test.ts`.
//!
//! Mesurés headless contre la vraie simulation (`create_systems`, la liste complète du registre) ;
//! bornes volontairement larges pour rester robustes, mais resserrées par rapport aux valeurs d'avant
//! équilibrage (tour externe 5000 PV / backdoor +200 ; sbires lanceurs 24 AD ; gromp 2050 PV / 74 AD).
//! `champion_balance.rs` porte un autre fichier (`champion-balance.test.ts`).

mod common;

use common::{game_world, v};
use lov_shared::constants::scaled;
use lov_shared::data::minions::minion;
use lov_shared::data::monsters::monster;
use lov_shared::data::structures::STRUCTURES;
use lov_shared::map::StructureLane;
use lov_shared::types::{
    EntityId, EntityKind, MinionType, MonsterType, SummonerId, Team, TeamOrNeutral, TowerTier,
};
use lov_sim::combat::buffs::recalculate_stats;
use lov_sim::systems::leveling::{grant_xp, xp_for_level};
use lov_sim::systems::minions::spawn_wave;
use lov_sim::systems::structures::structure_state;
use lov_sim::{CreateChampion, create_champion};

const DT: f64 = 1.0 / 30.0;

// ------------------------------------------------------------------------------------------------
// équilibrage — données
// ------------------------------------------------------------------------------------------------

#[test]
fn tours_pv_et_backdoor_dans_des_bornes_jouables() {
    let outer = STRUCTURES.towers.get(TowerTier::Outer);
    assert!(outer.hp <= 4000.0, "{}", outer.hp);
    assert!(outer.armor <= 35.0, "{}", outer.armor);
    const { assert!(STRUCTURES.backdoor_resistance <= 100.0) };
}

#[test]
fn sbires_degats_de_vague_contenus() {
    assert!(minion(MinionType::Caster).ad <= 20.0);
    assert!(minion(MinionType::Melee).ad <= 12.0);
}

#[test]
fn monstres_camps_clearables_tot_degats_moderes() {
    assert!(monster(MonsterType::Gromp).hp <= 1500.0);
    assert!(monster(MonsterType::Gromp).ad <= 60.0);
    assert!(monster(MonsterType::BlueBuff).hp <= 1600.0);
}

#[test]
fn monstres_xp_de_jungle_suffisante() {
    // parité avec la voie à 5 min, mesurée en simulation
    assert!(monster(MonsterType::Gromp).xp >= 250.0);
    assert!(monster(MonsterType::BlueBuff).xp >= 250.0);
    assert!(monster(MonsterType::Scuttle).xp >= 150.0);
    assert!(monster(MonsterType::Dragon).xp >= 200.0);
}

// ------------------------------------------------------------------------------------------------
// équilibrage — simulation
// ------------------------------------------------------------------------------------------------

#[test]
fn une_tour_externe_clear_une_vague_en_8_a_20_s_sans_mourir() {
    let mut s = game_world(&[]);
    s.world.time = scaled(400.0); // après la fortification
    let tower = s
        .world
        .query(Some(EntityKind::Tower))
        .find(|e| {
            e.team == TeamOrNeutral::Red
                && e.subtype.as_deref() == Some("outer")
                && structure_state(e).is_some_and(|st| st.lane == StructureLane::Mid)
        })
        .map(|e| (e.id, e.pos))
        .expect("tour externe rouge du milieu");
    let (tower_id, tp) = tower;
    spawn_wave(&mut s.world, 1);
    let wave: Vec<EntityId> = s
        .world
        .query(Some(EntityKind::Minion))
        .filter(|m| m.team == TeamOrNeutral::Blue)
        .map(|m| m.id)
        .collect();
    assert!(!wave.is_empty());
    for (i, &id) in wave.iter().enumerate() {
        let m = s.e_mut(id);
        m.pos = v(
            tp.x + 100.0 + (i % 6) as f64 * 40.0,
            tp.y + (i / 6) as f64 * 40.0,
        );
        m.attack_target = Some(tower_id);
    }
    let mut t = 0.0;
    for _ in 0..60 * 30 {
        s.step();
        t += DT;
        s.world.events.drain();
        if wave
            .iter()
            .all(|&id| s.world.get(id).is_none_or(|m| m.dead))
        {
            break;
        }
    }
    assert!(t > 8.0, "vague nettoyée en {t:.1} s");
    assert!(t < 20.0, "vague nettoyée en {t:.1} s");
    assert!(!s.e(tower_id).dead, "une vague seule ne tue pas une tour");
}

#[test]
fn un_jungler_niveau_3_avec_machette_clear_le_gromp_en_moins_de_35_s_et_survit() {
    let mut s = game_world(&[]);
    s.world.time = scaled(90.0);
    s.run(0.5);
    let camp = s
        .world
        .query(Some(EntityKind::Monster))
        .find(|m| m.subtype.as_deref() == Some("gromp") && !m.dead)
        .map(|m| m.id)
        .expect("gromp apparu");
    let near = |s: &common::Sim| {
        let p = s.e(camp).pos;
        v(p.x + 100.0, p.y)
    };
    let pos = near(&s);
    let def = lov_shared::data::champions::get_champion("tharox").expect("tharox");
    let c = create_champion(
        &mut s.world,
        CreateChampion {
            player_id: "j".into(),
            name: "J".into(),
            team: Team::Blue,
            champion_id: "tharox".into(),
            summoners: [SummonerId::Smite, SummonerId::Flash],
            pos,
        },
        &def.base_stats,
    );
    // levelUp(w, c, 3)
    grant_xp(&mut s.world, c, xp_for_level(3));
    if let Some(ch) = s.e_mut(c).champ.as_mut() {
        ch.skill_points = 0;
    }
    recalculate_stats(&mut s.world, c);
    let e = s.e_mut(c);
    e.hp = e.stats.max_hp;
    e.mana = e.stats.max_mana;
    // giveItems(w, c, ['hunter_machete'])
    if let Some(ch) = s.e_mut(c).champ.as_mut() {
        ch.items = vec![Some("hunter_machete".into()), None, None, None, None, None];
    }
    recalculate_stats(&mut s.world, c);
    let e = s.e_mut(c);
    e.hp = e.stats.max_hp;
    assert_eq!(s.e(c).champ.as_ref().map(|ch| ch.level), Some(3));

    let mut t = 0.0;
    for i in 0..40 * 30 {
        if s.e(camp).dead {
            break;
        }
        if i % 30 == 0 {
            let p = near(&s);
            let e = s.e_mut(c);
            e.attack_target = Some(camp);
            e.pos = p;
        }
        s.step();
        t += DT;
        s.world.events.drain();
    }
    assert!(s.e(camp).dead, "gromp encore en vie après {t:.1} s");
    assert!(t < 35.0, "gromp tué en {t:.1} s");
    let e = s.e(c);
    assert!(!e.dead);
    assert!(
        e.hp / e.stats.max_hp > 0.2,
        "PV restants : {:.0} %",
        100.0 * e.hp / e.stats.max_hp
    );
}
