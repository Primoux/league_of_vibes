//! Kit de Kaelen : port de `packages/server/test/champion-kaelen.test.ts` (cible `tharox` au lieu
//! de `sable`).

mod common;

use common::champion::game;
use lov_shared::schema::CcType;
use lov_shared::types::{AbilitySlot, DamageType, GameEvent, PartialStats};
use lov_sim::combat::buffs::{StatModifiers, get_buff, has_cc, register_stat_source};
use lov_sim::combat::damage::{DamageOpts, apply_damage};

/// Marque l'entité qui reçoit les +25 AD du test du E.
struct BonusAdForTest;

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-6, "{a} != {b}");
}

#[test]
fn q_permet_exactement_trois_lancers_et_projette_au_troisieme() {
    let mut g = game("kaelen");
    let (s, t) = (g.source, g.target);
    for i in 0..3 {
        assert!(g.cast(AbilitySlot::Q, Some(t), None));
        g.advance(0.35);
        if i < 2 {
            assert!(!has_cc(g.e(t), CcType::Knockup));
        }
    }
    assert!(has_cc(g.e(t), CcType::Knockup));
    assert!(!g.cast(AbilitySlot::Q, Some(t), None));
    let hits = g
        .world
        .events
        .pending
        .iter()
        .filter(|e| matches!(e, GameEvent::Damage { ability: Some(a), .. } if a == "kaelen_q"))
        .count();
    assert_eq!(hits, 3);
    assert_eq!(g.e(s).mana, 0.0);
}

#[test]
fn w_bloque_un_coup_de_champion_et_etourdit_un_attaquant_de_melee() {
    let mut g = game("kaelen");
    let (s, t) = (g.source, g.target);
    g.cast(AbilitySlot::W, None, None);
    let blocked = apply_damage(
        &mut g.world,
        DamageOpts::new(Some(t), s, 100.0, DamageType::Physical),
    );
    assert_eq!(blocked, 0.0);
    assert!(has_cc(g.e(t), CcType::Stun));
    assert!(get_buff(g.e(s), "kaelen_w", None).is_none());
    let next = apply_damage(
        &mut g.world,
        DamageOpts::new(Some(t), s, 100.0, DamageType::Physical),
    );
    assert!(next > 0.0);
}

#[test]
fn e_bondit_et_son_bouclier_suit_l_ad_bonus() {
    let mut g = game("kaelen");
    let (s, t) = (g.source, g.target);
    // Le registre des sources de stats est global au processus et les tests tournent en parallèle :
    // la source ne s'applique qu'à l'entité marquée par ce test, jamais au Kaelen d'un autre test.
    g.e_mut(s).data.insert(BonusAdForTest);
    let off = register_stat_source(|_, e| {
        e.data.contains::<BonusAdForTest>().then(|| StatModifiers {
            flat: PartialStats {
                ad: Some(25.0),
                ..PartialStats::default()
            },
            ..StatModifiers::default()
        })
    });
    g.step();
    g.cast(AbilitySlot::E, Some(t), None);
    close(g.e(s).shield, 85.0);
    g.advance(0.4);
    let x = g.e(s).pos.x;
    assert!(x > 1100.0 && x <= 1250.0, "x = {x}");
    g.advance(2.0);
    assert_eq!(g.e(s).shield, 0.0);
    off.unsubscribe();
}

#[test]
fn r_frappe_plus_fort_un_champion_blesse() {
    let mut full = game("kaelen");
    let t = full.target;
    full.cast(AbilitySlot::R, Some(t), None);
    full.advance(0.5);
    let mut wounded = game("kaelen");
    let t = wounded.target;
    wounded.e_mut(t).hp *= 0.5;
    wounded.cast(AbilitySlot::R, Some(t), None);
    wounded.advance(0.5);
    close(
        wounded.damage(Some("kaelen_r"), None),
        full.damage(Some("kaelen_r"), None) * 1.5,
    );
}

#[test]
fn la_troisieme_auto_attaque_frappe_plus_fort_et_soigne() {
    let mut g = game("kaelen");
    let (s, t) = (g.source, g.target);
    g.e_mut(s).hp -= 300.0;
    let first = g.auto(s, t, false);
    g.auto(s, t, false);
    let before = g.e(s).hp;
    let third = g.auto(s, t, false);
    close(third, first * 1.4);
    close(g.e(s).hp - before, third * 0.3);
}
