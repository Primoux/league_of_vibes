//! Kit de Vrenna : port de `packages/server/test/champion-vrenna.test.ts` (cible `tharox` au lieu
//! de `sable`).

mod common;

use common::champion::game;
use lov_shared::schema::CcType;
use lov_shared::types::{AbilitySlot, GameEvent};
use lov_sim::combat::buffs::{get_buff, has_cc};

#[test]
fn q_traverse_et_attire_les_cibles_touchees_par_la_pointe() {
    let mut g = game("vrenna");
    let t = g.target;
    g.e_mut(t).pos.x = 1550.0;
    g.cast(AbilitySlot::Q, Some(t), None);
    g.advance(0.9);
    assert!(g.damage(Some("vrenna_q"), None) > 0.0);
    assert!(g.e(t).pos.x < 1550.0);
}

#[test]
fn w_donne_de_la_vitesse_d_attaque_et_soigne_huit_pourcent_des_pv_manquants() {
    let mut g = game("vrenna");
    let s = g.source;
    let speed = g.e(s).stats.attack_speed;
    g.e_mut(s).hp -= 500.0;
    g.cast(AbilitySlot::W, None, None);
    let e = g.e(s);
    assert!((e.hp - (e.stats.max_hp - 460.0)).abs() < 1e-6);
    assert!((e.stats.attack_speed - speed * 1.3).abs() < 1e-6);
}

#[test]
fn e_blesse_et_ralentit_a_l_atterrissage() {
    let mut g = game("vrenna");
    let t = g.target;
    g.cast(AbilitySlot::E, Some(t), None);
    assert_eq!(g.damage(Some("vrenna_e"), None), 0.0);
    g.advance(0.5);
    assert!(g.damage(Some("vrenna_e"), None) > 0.0);
    assert!(has_cc(g.e(t), CcType::Slow));
}

#[test]
fn r_exige_un_champion_et_projette_a_l_impact() {
    let mut g = game("vrenna");
    let t = g.target;
    g.cast(AbilitySlot::R, Some(t), None);
    g.advance(0.3);
    assert!(g.damage(Some("vrenna_r"), None) > 0.0);
    assert!(has_cc(g.e(t), CcType::Knockup));
}

#[test]
fn le_saignement_cumule_jusqu_a_cinq_donne_de_la_vitesse_et_tique() {
    let mut g = game("vrenna");
    let (s, t) = (g.source, g.target);
    for _ in 0..5 {
        g.auto(s, t, false);
    }
    assert_eq!(
        get_buff(g.e(t), "vrenna_bleed", Some(s)).map(|b| b.stacks),
        Some(5)
    );
    assert!(get_buff(g.e(s), "vrenna_p_speed", None).is_some());
    g.advance(3.1);
    let ticks = g
        .world
        .events
        .pending
        .iter()
        .filter(|e| matches!(e, GameEvent::Damage { ability: Some(a), .. } if a == "vrenna_p"))
        .count();
    assert_eq!(ticks, 3);
    assert!(get_buff(g.e(t), "vrenna_bleed", None).is_none());
}
