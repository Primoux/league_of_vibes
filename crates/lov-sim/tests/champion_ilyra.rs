//! Kit d'Ilyra : port de `packages/server/test/champion-ilyra.test.ts` (cible `tharox` au lieu de
//! `sable`).

mod common;

use common::champion::game;
use lov_shared::schema::CcType;
use lov_shared::types::{AbilitySlot, DamageType, GameEvent};
use lov_sim::combat::buffs::{get_buff, has_cc};
use lov_sim::combat::damage::{DamageOpts, apply_damage};

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-6, "{a} != {b}");
}

#[test]
fn q_touche_le_premier_ennemi_et_pose_givre_et_ralentissement() {
    let mut g = game("ilyra");
    let (s, t) = (g.source, g.target);
    g.cast(AbilitySlot::Q, Some(t), None);
    g.advance(0.5);
    assert!(g.damage(Some("ilyra_q"), None) > 0.0);
    assert!(has_cc(g.e(t), CcType::Slow));
    assert_eq!(
        get_buff(g.e(t), "ilyra_frost", Some(s)).map(|b| b.stacks),
        Some(1)
    );
}

#[test]
fn w_explose_apres_son_telegraphe_avec_un_ralentissement_plus_fort() {
    let mut g = game("ilyra");
    let t = g.target;
    // Le TS attend 330 × 0,5 (vitesse de Sable) : on part de la vitesse de la cible.
    let speed = g.e(t).stats.move_speed;
    g.cast(AbilitySlot::W, Some(t), None);
    g.advance(0.8);
    assert_eq!(g.damage(Some("ilyra_w"), None), 0.0);
    g.advance(0.1);
    assert!(g.damage(Some("ilyra_w"), None) > 0.0);
    close(g.e(t).stats.move_speed, speed * 0.5);
}

#[test]
fn e_protege_un_allie_ralentit_l_attaquant_et_expire() {
    let mut g = game("ilyra");
    let (s, t) = (g.source, g.target);
    g.cast(AbilitySlot::E, Some(s), None);
    assert_eq!(g.e(s).shield, 70.0);
    apply_damage(
        &mut g.world,
        DamageOpts::new(Some(t), s, 10.0, DamageType::Physical),
    );
    assert!(has_cc(g.e(t), CcType::Slow));
    g.advance(2.6);
    assert_eq!(g.e(s).shield, 0.0);
}

#[test]
fn r_inflige_quatre_tics_et_le_passif_une_seule_fois() {
    let mut g = game("ilyra");
    let (s, t) = (g.source, g.target);
    g.cast(AbilitySlot::R, Some(t), None);
    g.advance(4.3);
    let hits = g
        .world
        .events
        .pending
        .iter()
        .filter(|e| matches!(e, GameEvent::Damage { ability: Some(a), .. } if a == "ilyra_r"))
        .count();
    assert_eq!(hits, 4);
    let stuns = g
        .world
        .events
        .pending
        .iter()
        .filter(|e| matches!(e, GameEvent::BuffApplied { buff_id, .. } if buff_id == "stun"))
        .count();
    assert_eq!(stuns, 1);
    assert!(get_buff(g.e(t), "ilyra_frost_immunity", Some(s)).is_some());
}

#[test]
fn garde_le_ralentissement_de_w_quand_q_touche_aussi() {
    let mut g = game("ilyra");
    let t = g.target;
    let speed = g.e(t).stats.move_speed;
    g.cast(AbilitySlot::W, Some(t), None);
    g.advance(0.9);
    close(g.e(t).stats.move_speed, speed * 0.5);
    g.cast(AbilitySlot::Q, Some(t), None);
    g.advance(0.5);
    close(g.e(t).stats.move_speed, speed * 0.5);
}
