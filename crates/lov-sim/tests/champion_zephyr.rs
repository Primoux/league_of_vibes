//! Kit de Zephyr : port de `packages/server/test/champion-zephyr.test.ts` (cible `tharox` au lieu
//! de `sable`).

mod common;

use common::champion::game;
use lov_shared::data::champions::get_champion;
use lov_shared::schema::CcType;
use lov_shared::types::{AbilitySlot, DamageType, GameEvent, Vec2};
use lov_sim::combat::buffs::has_cc;

fn p(x: f64, y: f64) -> Vec2 {
    Vec2 { x, y }
}

#[test]
fn q_touche_a_l_aller_puis_au_retour_en_degats_bruts() {
    let mut g = game("zephyr");
    let t = g.target;
    g.cast(AbilitySlot::Q, Some(t), None);
    g.advance(1.7);
    let hits: Vec<(DamageType, f64)> = g
        .world
        .events
        .pending
        .iter()
        .filter_map(|e| match e {
            GameEvent::Damage {
                ability: Some(a),
                damage_type,
                amount,
                ..
            } if a == "zephyr_q" => Some((*damage_type, *amount)),
            _ => None,
        })
        .collect();
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].0, DamageType::Magic);
    assert_eq!(hits[1], (DamageType::True, 60.0));
}

#[test]
fn w_saute_et_explose_au_point_de_depart() {
    let mut g = game("zephyr");
    let (s, t) = (g.source, g.target);
    g.e_mut(t).pos.x = 1100.0;
    g.cast(AbilitySlot::W, None, Some(p(1450.0, 1000.0)));
    assert_eq!(g.e(s).pos.x, 1450.0);
    g.advance(0.9);
    assert_eq!(g.damage(Some("zephyr_w"), None), 0.0);
    g.advance(0.2);
    assert!(g.damage(Some("zephyr_w"), None) > 0.0);
}

#[test]
fn e_immobilise_seulement_si_le_lien_tient() {
    let mut g = game("zephyr");
    let t = g.target;
    g.cast(AbilitySlot::E, Some(t), None);
    g.advance(1.6);
    assert!(has_cc(g.e(t), CcType::Root));
    assert!(g.damage(Some("zephyr_e"), None) > 0.0);

    let mut broken = game("zephyr");
    let t = broken.target;
    broken.cast(AbilitySlot::E, Some(t), None);
    broken.e_mut(t).pos.x = 4000.0;
    broken.advance(2.0);
    assert_eq!(broken.damage(Some("zephyr_e"), None), 0.0);
}

#[test]
fn r_donne_exactement_trois_rayons_pour_un_seul_cout() {
    let mut g = game("zephyr");
    let (s, t) = (g.source, g.target);
    let mana = g.e(s).mana;
    for _ in 0..3 {
        assert!(g.cast(AbilitySlot::R, Some(t), None));
        g.advance(0.8);
    }
    assert!(!g.cast(AbilitySlot::R, Some(t), None));
    assert!((g.e(s).mana - (mana - 100.0)).abs() < 1e-6);
    let rays = g
        .world
        .events
        .pending
        .iter()
        .filter(|e| matches!(e, GameEvent::Damage { ability: Some(a), .. } if a == "zephyr_r"))
        .count();
    assert_eq!(rays, 3);
}

#[test]
fn amplifie_le_sort_qui_suit_quatre_lancers_et_rend_du_mana() {
    let mut g = game("zephyr");
    let (s, t) = (g.source, g.target);
    for _ in 0..4 {
        let e = g.e_mut(s);
        e.champ.as_mut().unwrap().cooldowns.w = 0.0;
        e.mana = e.stats.max_mana;
        let pos = e.pos;
        g.cast(AbilitySlot::W, None, Some(pos));
    }
    g.e_mut(s).mana = 100.0;
    g.cast(AbilitySlot::Q, Some(t), None);
    g.advance(0.5);
    let q_cost = get_champion("zephyr").unwrap().abilities.q.cost[0];
    assert_eq!(g.e(s).mana, 100.0 - q_cost + 30.0);
    let mr = g.e(t).stats.mr;
    assert!((g.damage(Some("zephyr_q"), None) - 60.0 * 1.25 * 100.0 / (100.0 + mr)).abs() < 1e-6);
}
