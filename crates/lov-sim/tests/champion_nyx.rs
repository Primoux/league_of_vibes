//! Kit de Nyx : port de `packages/server/test/champion-nyx.test.ts` (cible `tharox` au lieu de
//! `sable`).

mod common;

use common::champion::game;
use lov_shared::schema::CcType;
use lov_shared::types::{AbilitySlot, DamageType, TeamOrNeutral, Vec2};
use lov_sim::combat::buffs::{get_buff, has_cc};
use lov_sim::combat::damage::{DamageOpts, apply_damage};
use lov_sim::systems::vision::Invisible;

#[test]
fn q_coute_de_l_energie_et_se_recharge_sur_un_coup_mortel() {
    let mut g = game("nyx");
    let (s, t) = (g.source, g.target);
    g.e_mut(t).hp = 1.0;
    g.cast(AbilitySlot::Q, Some(t), None);
    assert!(g.e(t).dead);
    assert_eq!(g.e(s).champ.as_ref().unwrap().cooldowns.q, 0.0);
    assert_eq!(g.e(s).mana, 160.0);
}

#[test]
fn q_se_recharge_aussi_si_une_autre_source_tue_sa_victime() {
    let mut g = game("nyx");
    let (s, t) = (g.source, g.target);
    g.cast(AbilitySlot::Q, Some(t), None);
    apply_damage(
        &mut g.world,
        DamageOpts::new(Some(t), t, 10_000.0, DamageType::True),
    );
    assert_eq!(g.e(s).champ.as_ref().unwrap().cooldowns.q, 0.0);
}

#[test]
fn w_l_invisibilite_cesse_a_l_attaque_ou_au_sort() {
    let mut g = game("nyx");
    let (s, t) = (g.source, g.target);
    g.cast(AbilitySlot::W, None, None);
    assert!(g.e(s).data.contains::<Invisible>());
    g.auto(s, t, false);
    assert!(!g.e(s).data.contains::<Invisible>());
    let e = g.e_mut(s);
    e.champ.as_mut().unwrap().cooldowns.w = 0.0;
    e.mana = 200.0;
    g.cast(AbilitySlot::W, None, None);
    g.cast(AbilitySlot::Q, Some(t), None);
    assert!(!g.e(s).data.contains::<Invisible>());
}

#[test]
fn e_saute_derriere_un_ennemi_et_le_ralentit_sans_blesser_un_allie() {
    let mut g = game("nyx");
    let (s, t) = (g.source, g.target);
    g.cast(AbilitySlot::E, Some(t), None);
    assert!(g.e(s).pos.x < g.e(t).pos.x);
    assert!(g.damage(Some("nyx_e"), None) > 0.0);
    assert!(has_cc(g.e(t), CcType::Slow));
    let ally = g.dummy(
        Vec2 {
            x: 1500.0,
            y: 1000.0,
        },
        |_| {},
    );
    g.e_mut(ally).team = TeamOrNeutral::Blue;
    let e = g.e_mut(s);
    e.champ.as_mut().unwrap().cooldowns.e = 0.0;
    e.mana = 200.0;
    assert!(g.cast(AbilitySlot::E, Some(ally), None));
    assert_eq!(g.damage(Some("nyx_e"), Some(ally)), 0.0);
}

#[test]
fn r_enregistre_les_degats_reels_et_detone_apres_trois_secondes() {
    let mut g = game("nyx");
    let (s, t) = (g.source, g.target);
    g.cast(AbilitySlot::R, Some(t), None);
    let mut opts = DamageOpts::new(Some(t), t, 100.0, DamageType::True);
    opts.ability = Some("test".into());
    apply_damage(&mut g.world, opts);
    let stored = get_buff(g.e(t), "nyx_r", Some(s)).and_then(|b| b.params.number("stored"));
    assert_eq!(stored, Some(100.0));
    g.advance(2.9);
    assert_eq!(g.damage(Some("nyx_r"), None), 0.0);
    g.advance(0.2);
    let armor = g.e(t).stats.armor;
    let expected = (100.0 + 100.0 * 0.3) * 1.2 * 100.0 / (100.0 + armor);
    assert!((g.damage(Some("nyx_r"), None) - expected).abs() < 1e-6);
    assert!(get_buff(g.e(t), "nyx_r", None).is_none());
}

#[test]
fn le_passif_amplifie_seulement_contre_une_cible_isolee() {
    let mut isolated = game("nyx");
    let (s, t) = (isolated.source, isolated.target);
    let bonus = isolated.auto(s, t, false);
    let mut grouped = game("nyx");
    let (s, t) = (grouped.source, grouped.target);
    grouped.dummy(
        Vec2 {
            x: 1300.0,
            y: 1000.0,
        },
        |_| {},
    );
    let normal = grouped.auto(s, t, false);
    assert!((bonus - normal * 1.2).abs() < 1e-6);
}
