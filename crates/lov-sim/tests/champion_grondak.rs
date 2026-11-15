//! Kit de Grondak : port de `packages/server/test/champion-grondak.test.ts`.
//!
//! Le champion cible est `tharox` (voir `common/champion.rs`) au lieu de `sable`.

mod common;

use common::champion::game;
use lov_shared::math::damage_multiplier;
use lov_shared::schema::CcType;
use lov_shared::types::{AbilitySlot, DamageType, GameEvent};
use lov_sim::combat::buffs::has_cc;
use lov_sim::combat::damage::{DamageOpts, apply_damage};

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-6, "{a} != {b}");
}

#[test]
fn q_blesse_et_ralentit_sur_sa_ligne_et_lance_la_recharge() {
    let mut g = game("grondak");
    let (s, t) = (g.source, g.target);
    assert!(g.cast(AbilitySlot::Q, Some(t), None));
    g.advance(0.3);
    let armor = g.e(t).stats.armor;
    close(
        g.damage(Some("grondak_q"), None),
        70.0 * damage_multiplier(armor),
    );
    assert!(has_cc(g.e(t), CcType::Slow));
    assert!(g.e(s).champ.as_ref().unwrap().cooldowns.q > 8.0);
}

#[test]
fn w_provoque_et_reduit_les_degats_subis_de_30_pourcent() {
    let mut g = game("grondak");
    let (s, t) = (g.source, g.target);
    g.cast(AbilitySlot::W, None, None);
    assert!(has_cc(g.e(t), CcType::Taunt));
    let value = apply_damage(
        &mut g.world,
        DamageOpts::new(Some(t), s, 100.0, DamageType::Magic),
    );
    let mr = g.e(s).stats.mr;
    close(value, 70.0 * damage_multiplier(mr));
}

#[test]
fn e_blesse_et_etourdit_seulement_a_l_arrivee() {
    let mut g = game("grondak");
    let t = g.target;
    g.e_mut(t).pos.x = 1500.0;
    g.cast(AbilitySlot::E, Some(t), None);
    assert!(!has_cc(g.e(t), CcType::Stun));
    g.advance(0.5);
    assert!(g.damage(Some("grondak_e"), None) > 0.0);
    assert!(has_cc(g.e(t), CcType::Stun));
}

#[test]
fn r_telegraphie_puis_blesse_et_projette() {
    let mut g = game("grondak");
    let t = g.target;
    g.cast(AbilitySlot::R, Some(t), None);
    g.advance(0.9);
    assert_eq!(g.damage(Some("grondak_r"), None), 0.0);
    let telegraph = g.world.events.pending.iter().any(
        |e| matches!(e, GameEvent::AreaEffect { fx_key, .. } if fx_key == "grondak_r_telegraph"),
    );
    assert!(telegraph);
    g.advance(0.2);
    assert!(g.damage(Some("grondak_r"), None) > 0.0);
    assert!(has_cc(g.e(t), CcType::Knockup));
}

#[test]
fn le_bouclier_passif_repousse_apres_huit_secondes_hors_combat() {
    let mut g = game("grondak");
    let (s, t) = (g.source, g.target);
    g.advance(8.1);
    close(g.e(s).shield, g.e(s).stats.max_hp * 0.1);
    apply_damage(
        &mut g.world,
        DamageOpts::new(Some(t), s, 10_000.0, DamageType::Physical),
    );
    assert!(g.e(s).dead);
    g.advance(9.0);
    assert_eq!(g.e(s).shield, 0.0);
}
