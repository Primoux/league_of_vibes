//! Kit de Tharox : port de `packages/server/test/champion-tharox.test.ts` (cible `grondak` : Tharox
//! ne peut pas être sa propre cible de test).

mod common;

use common::champion::{Game, game_vs};
use lov_shared::schema::CcType;
use lov_shared::types::{AbilitySlot, GameEvent};
use lov_sim::abilities::passives::PassiveState;
use lov_sim::combat::buffs::{get_buff, has_cc};

fn game() -> Game {
    game_vs("tharox", "grondak", 6)
}

#[test]
fn q_touche_repousse_et_s_arrete_sur_la_premiere_unite() {
    let mut g = game();
    let t = g.target;
    g.cast(AbilitySlot::Q, Some(t), None);
    g.advance(0.6);
    assert!(g.damage(Some("tharox_q"), None) > 0.0);
    assert!(g.e(t).pos.x > 1250.0);
}

#[test]
fn w_donne_des_resistances_et_quatre_tics_d_aura() {
    let mut g = game();
    let (s, t) = (g.source, g.target);
    let armor = g.e(s).stats.armor;
    g.cast(AbilitySlot::W, None, None);
    assert!((g.e(s).stats.armor - (armor + 15.0)).abs() < 1e-6);
    g.advance(4.1);
    let ticks = g
        .world
        .events
        .pending
        .iter()
        .filter(|e| {
            matches!(e, GameEvent::Damage { ability: Some(a), target, .. } if a == "tharox_w" && *target == t)
        })
        .count();
    assert_eq!(ticks, 4);
    assert!((g.e(s).stats.armor - armor).abs() < 1e-6);
}

#[test]
fn e_blesse_et_ralentit_les_ennemis_proches() {
    let mut g = game();
    let t = g.target;
    g.cast(AbilitySlot::E, None, None);
    assert!(g.damage(Some("tharox_e"), None) > 0.0);
    assert!(has_cc(g.e(t), CcType::Slow));
}

#[test]
fn r_s_arme_puis_percute_et_etourdit_les_champions() {
    let mut g = game();
    let t = g.target;
    g.cast(AbilitySlot::R, Some(t), None);
    g.advance(0.9);
    assert_eq!(g.damage(Some("tharox_r"), None), 0.0);
    g.advance(0.4);
    assert!(g.damage(Some("tharox_r"), None) > 0.0);
    assert!(has_cc(g.e(t), CcType::Stun));
}

#[test]
fn l_elan_se_charge_en_bougeant_et_se_consomme_a_l_attaque() {
    let mut g = game();
    let (s, t) = (g.source, g.target);
    g.step();
    g.e_mut(s).pos.x += 3400.0;
    g.step();
    assert_eq!(
        g.e(s).data.get::<PassiveState>().map(|p| p.momentum),
        Some(100.0)
    );
    g.auto(s, t, false);
    assert!(g.damage(Some("tharox_p"), None) > 0.0);
    assert!(get_buff(g.e(t), "slow", None).is_some());
    let amount = g.damage(Some("tharox_p"), None);
    g.auto(s, t, false);
    assert_eq!(g.damage(Some("tharox_p"), None), amount);
}
