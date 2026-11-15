//! Kit de Mélenchon : port de `packages/server/test/champion-melenchon.test.ts` (cible `tharox` au
//! lieu de `sable`).

mod common;

use common::champion::{game, game_vs};
use lov_shared::data::champions::get_champion;
use lov_shared::schema::CcType;
use lov_shared::types::{AbilitySlot, DamageType, GameEvent, TeamOrNeutral, Vec2};
use lov_sim::World;
use lov_sim::abilities::engine::{CastCommand, cast_ability};
use lov_sim::combat::buffs::{BuffOptions, SourceMatch, add_buff, get_buff, has_cc, remove_buff};
use lov_sim::combat::damage::{DamageOpts, apply_damage};

fn p(x: f64, y: f64) -> Vec2 {
    Vec2 { x, y }
}

fn spell(world: &mut World, source: u32, target: u32, ability: &str) {
    let mut opts = DamageOpts::new(Some(source), target, 1.0, DamageType::True);
    opts.ability = Some(ability.into());
    apply_damage(world, opts);
}

#[test]
fn onzieme_champion_jouable_avec_son_ultime() {
    let def = get_champion("melenchon").unwrap();
    assert_eq!(def.abilities.r.name, "La République, c’est moi !");
    assert!(def.lore.contains("satirique"));
}

#[test]
fn q_ralentit_seulement_le_premier_ennemi() {
    let mut g = game("melenchon");
    let (s, t) = (g.source, g.target);
    let behind = g.dummy(p(1600.0, 1000.0), |_| {});
    assert!(g.cast(AbilitySlot::Q, Some(t), None));
    g.advance(0.6);
    assert!(g.damage(Some("melenchon_q"), Some(t)) > 0.0);
    assert_eq!(g.damage(Some("melenchon_q"), Some(behind)), 0.0);
    assert!(has_cc(g.e(t), CcType::Slow));
    assert!(g.e(s).champ.as_ref().unwrap().cooldowns.q > 6.0);
}

#[test]
fn w_annonce_sa_motion_avant_de_blesser_et_reduire_au_silence() {
    let mut g = game("melenchon");
    let t = g.target;
    g.cast(AbilitySlot::W, Some(t), None);
    g.advance(0.7);
    assert_eq!(g.damage(Some("melenchon_w"), None), 0.0);
    let telegraph = g.world.events.pending.iter().any(
        |e| matches!(e, GameEvent::AreaEffect { fx_key, .. } if fx_key == "melenchon_w_telegraph"),
    );
    assert!(telegraph);
    g.advance(0.1);
    assert!(g.damage(Some("melenchon_w"), None) > 0.0);
    assert!(has_cc(g.e(t), CcType::Silence));
}

#[test]
fn e_laisse_un_hologramme_au_depart_et_protege_le_lanceur() {
    let mut g = game("melenchon");
    let (s, t) = (g.source, g.target);
    g.e_mut(t).pos.x = 1100.0;
    g.cast(AbilitySlot::E, None, Some(p(1400.0, 1000.0)));
    assert_eq!(g.e(s).pos.x, 1400.0);
    assert_eq!(g.e(s).shield, 45.0);
    g.advance(1.1);
    assert_eq!(g.damage(Some("melenchon_e"), None), 0.0);
    g.advance(0.2);
    assert!(g.damage(Some("melenchon_e"), None) > 0.0);
    g.advance(1.0);
    assert_eq!(g.e(s).shield, 0.0);
}

#[test]
fn r_provoque_et_donne_un_bouclier_de_quatre_secondes() {
    let mut g = game("melenchon");
    let (s, t) = (g.source, g.target);
    let expected = g.e(s).stats.max_hp * 0.2;
    g.cast(AbilitySlot::R, None, None);
    g.advance(0.5);
    assert_eq!(g.damage(Some("melenchon_r"), None), 0.0);
    g.advance(0.2);
    assert!(g.damage(Some("melenchon_r"), None) > 0.0);
    assert!(has_cc(g.e(t), CcType::Taunt));
    assert!((g.e(s).shield - expected).abs() < 1e-6);
    assert!(get_buff(g.e(s), "melenchon_r", None).is_some());
    g.advance(4.1);
    assert_eq!(g.e(s).shield, 0.0);
}

#[test]
fn r_est_interruptible_pendant_son_incantation() {
    let mut g = game("melenchon");
    let (s, t) = (g.source, g.target);
    g.cast(AbilitySlot::R, None, None);
    add_buff(&mut g.world, s, "stun", BuffOptions::new(1.0));
    g.advance(1.0);
    assert_eq!(g.damage(Some("melenchon_r"), None), 0.0);
    assert_eq!(g.e(s).shield, 0.0);
    assert!(!has_cc(g.e(t), CcType::Taunt));
}

#[test]
fn r_exclut_les_allies_et_les_ennemis_hors_rayon() {
    let mut g = game("melenchon");
    let t = g.target;
    let outside = g.dummy(p(1700.0, 1000.0), |_| {});
    let ally = g.dummy(p(1100.0, 1000.0), |_| {});
    g.e_mut(ally).team = TeamOrNeutral::Blue;
    g.cast(AbilitySlot::R, None, None);
    g.advance(0.7);
    assert!(g.damage(Some("melenchon_r"), Some(t)) > 0.0);
    assert_eq!(g.damage(Some("melenchon_r"), Some(outside)), 0.0);
    assert_eq!(g.damage(Some("melenchon_r"), Some(ally)), 0.0);
    assert!(!has_cc(g.e(ally), CcType::Taunt));
}

#[test]
fn l_indignation_expire() {
    let mut g = game("melenchon");
    let (s, t) = (g.source, g.target);
    spell(&mut g.world, s, t, "melenchon_q");
    g.advance(6.1);
    assert!(get_buff(g.e(s), "melenchon_indignation", None).is_none());
    spell(&mut g.world, s, t, "melenchon_q");
    assert_eq!(
        get_buff(g.e(s), "melenchon_indignation", None).map(|b| b.stacks),
        Some(1)
    );
    assert_eq!(g.e(s).shield, 0.0);
}

#[test]
fn trois_sorts_espaces_declenchent_l_indignation() {
    let mut g = game("melenchon");
    let (s, t) = (g.source, g.target);
    for i in 0..3 {
        spell(&mut g.world, s, t, "melenchon_q");
        if i < 2 {
            g.advance(0.55);
        }
    }
    assert!(get_buff(g.e(s), "melenchon_indignation", None).is_none());
    let level = f64::from(g.e(s).champ.as_ref().unwrap().level);
    assert!((g.e(s).shield - (35.0 + level * 5.0)).abs() < 1e-6);
    assert!((g.e(s).stats.move_speed - 325.0 * 1.18).abs() < 1e-6);
    g.advance(2.1);
    assert_eq!(g.e(s).shield, 0.0);
    assert_eq!(g.e(s).stats.move_speed, 325.0);
}

#[test]
fn ni_une_zone_ni_les_attaques_ni_les_sbires_ne_cumulent() {
    let mut g = game("melenchon");
    let (s, t) = (g.source, g.target);
    for _ in 0..5 {
        spell(&mut g.world, s, t, "melenchon_r");
    }
    assert_eq!(
        get_buff(g.e(s), "melenchon_indignation", None).map(|b| b.stacks),
        Some(1)
    );
    g.advance(0.55);
    let mut auto = DamageOpts::new(Some(s), t, 1.0, DamageType::True);
    auto.is_auto_attack = true;
    apply_damage(&mut g.world, auto);
    let minion = g.dummy(p(1400.0, 1000.0), |_| {});
    spell(&mut g.world, s, minion, "melenchon_q");
    assert_eq!(
        get_buff(g.e(s), "melenchon_indignation", None).map(|b| b.stacks),
        Some(1)
    );
}

#[test]
fn refuse_un_hologramme_immobilise_et_un_ultime_verrouille() {
    let mut g = game_vs("melenchon", "tharox", 1);
    let s = g.source;
    assert!(!g.cast(AbilitySlot::R, None, None));
    add_buff(&mut g.world, s, "root", BuffOptions::new(1.0));
    assert!(!g.cast(AbilitySlot::E, None, None));
    remove_buff(&mut g.world, s, "root", SourceMatch::Any);
    let command = CastCommand {
        slot: AbilitySlot::E,
        target_id: None,
        target_pos: Some(p(f64::NAN, 1000.0)),
    };
    assert!(!cast_ability(&mut g.world, s, &command, true));
}
