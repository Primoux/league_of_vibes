//! Kit de 67 Master : port de `packages/server/test/champion-master67.test.ts` (cible `tharox` au lieu
//! de `sable`).

mod common;

use common::champion::game;
use lov_shared::data::champions::get_champion;
use lov_shared::schema::CcType;
use lov_shared::types::{AbilitySlot, DamageType, GameEvent, TeamOrNeutral, Vec2};
use lov_sim::World;
use lov_sim::combat::buffs::{BuffOptions, add_buff, get_buff, has_cc};
use lov_sim::combat::damage::{DamageOpts, apply_damage};

fn p(x: f64, y: f64) -> Vec2 {
    Vec2 { x, y }
}

fn spell(world: &mut World, source: u32, target: u32, amount: f64, ability: &str) {
    let mut opts = DamageOpts::new(Some(source), target, amount, DamageType::True);
    opts.ability = Some(ability.into());
    apply_damage(world, opts);
}

#[test]
fn est_un_champion_original_avec_un_kit_complet() {
    let def = get_champion("master67").unwrap();
    assert_eq!(def.name, "67 Master");
    assert_eq!(def.abilities.r.name, "SIX… SEVEN !");
}

#[test]
fn q_ralentit_le_premier_ennemi_sans_traverser() {
    let mut g = game("master67");
    let t = g.target;
    let behind = g.dummy(p(1500.0, 1000.0), |_| {});
    assert!(g.cast(AbilitySlot::Q, Some(t), None));
    g.advance(0.5);
    assert!(g.damage(Some("master67_q"), Some(t)) > 0.0);
    assert_eq!(g.damage(Some("master67_q"), Some(behind)), 0.0);
    assert!(has_cc(g.e(t), CcType::Slow));
}

#[test]
fn w_attend_sa_telegraphie_avant_degats_et_immobilisation() {
    let mut g = game("master67");
    let t = g.target;
    g.cast(AbilitySlot::W, Some(t), None);
    g.advance(0.8);
    assert_eq!(g.damage(Some("master67_w"), None), 0.0);
    assert!(!has_cc(g.e(t), CcType::Root));
    g.advance(0.1);
    assert!(g.damage(Some("master67_w"), None) > 0.0);
    assert!(has_cc(g.e(t), CcType::Root));
}

#[test]
fn e_respecte_sa_portee_et_son_bouclier_expire() {
    let mut g = game("master67");
    let s = g.source;
    assert!(g.cast(AbilitySlot::E, None, Some(p(1600.0, 1000.0))));
    assert!((g.e(s).pos.x - 1367.0).abs() < 1.0);
    assert_eq!(g.e(s).shield, 67.0);
    g.advance(2.7);
    assert_eq!(g.e(s).shield, 0.0);
}

#[test]
fn e_est_bloque_par_une_immobilisation() {
    let mut g = game("master67");
    let s = g.source;
    add_buff(&mut g.world, s, "root", BuffOptions::new(1.0));
    assert!(!g.cast(AbilitySlot::E, None, None));
    assert_eq!(g.e(s).pos.x, 1000.0);
}

#[test]
fn r_produit_deux_impacts_puis_la_projection() {
    let mut g = game("master67");
    let t = g.target;
    g.cast(AbilitySlot::R, None, None);
    g.advance(0.6);
    assert_eq!(g.damage(Some("master67_r"), None), 0.0);
    g.advance(0.1);
    let first = g.damage(Some("master67_r"), None);
    assert!(first > 0.0);
    assert!(has_cc(g.e(t), CcType::Slow));
    assert!(!has_cc(g.e(t), CcType::Knockup));
    g.advance(0.7);
    assert!((g.damage(Some("master67_r"), None) - first * 2.0).abs() < 1e-6);
    assert!(has_cc(g.e(t), CcType::Knockup));
    let waves: Vec<&str> = g
        .world
        .events
        .pending
        .iter()
        .filter_map(|e| match e {
            GameEvent::AreaEffect { fx_key, .. } if fx_key.starts_with("master67_r_") => {
                Some(fx_key.as_str())
            }
            _ => None,
        })
        .collect();
    assert_eq!(waves, ["master67_r_six", "master67_r_seven"]);
}

#[test]
fn r_interruptible_avant_six_et_epargne_allies_et_cibles_lointaines() {
    let mut g = game("master67");
    let (s, t) = (g.source, g.target);
    let ally = g.dummy(p(1100.0, 1000.0), |_| {});
    g.e_mut(ally).team = TeamOrNeutral::Blue;
    let far = g.dummy(p(1800.0, 1000.0), |_| {});
    g.cast(AbilitySlot::R, None, None);
    add_buff(&mut g.world, s, "stun", BuffOptions::new(1.0));
    g.advance(1.5);
    assert_eq!(g.damage(Some("master67_r"), None), 0.0);
    assert!(!has_cc(g.e(t), CcType::Knockup));
    let e = g.e_mut(s);
    e.champ.as_mut().unwrap().cooldowns.r = 0.0;
    e.mana = e.stats.max_mana;
    g.cast(AbilitySlot::R, None, None);
    g.advance(1.5);
    assert_eq!(g.damage(Some("master67_r"), Some(ally)), 0.0);
    assert_eq!(g.damage(Some("master67_r"), Some(far)), 0.0);
    assert!(g.damage(Some("master67_r"), Some(t)) > 0.0);
}

#[test]
fn la_mort_du_lanceur_annule_la_deuxieme_onde() {
    let mut g = game("master67");
    let (s, t) = (g.source, g.target);
    g.cast(AbilitySlot::R, None, None);
    g.advance(0.7);
    let first = g.damage(Some("master67_r"), None);
    apply_damage(
        &mut g.world,
        DamageOpts::new(Some(t), s, 1e6, DamageType::True),
    );
    g.advance(0.8);
    assert_eq!(g.damage(Some("master67_r"), None), first);
}

#[test]
fn le_septieme_impact_declenche_un_bonus_et_remet_le_compteur() {
    let mut g = game("master67");
    let (s, t) = (g.source, g.target);
    for i in 0..7 {
        spell(&mut g.world, s, t, 1.0, "master67_q");
        if i < 6 {
            assert_eq!(g.damage(Some("master67_p"), None), 0.0);
            g.advance(0.7);
        }
    }
    let mr = g.e(t).stats.mr;
    assert!((g.damage(Some("master67_p"), None) - 67.0 * 100.0 / (100.0 + mr)).abs() < 1e-6);
    assert!(get_buff(g.e(s), "master67_count", None).is_none());
    g.advance(0.7);
    spell(&mut g.world, s, t, 1.0, "master67_q");
    assert_eq!(
        get_buff(g.e(s), "master67_count", None).map(|b| b.stacks),
        Some(1)
    );
}

#[test]
fn un_septieme_impact_deja_letal_ne_cause_ni_degats_negatifs_ni_seconde_mort() {
    let mut g = game("master67");
    let (s, t) = (g.source, g.target);
    for _ in 0..6 {
        spell(&mut g.world, s, t, 1.0, "master67_q");
        g.advance(0.7);
    }
    g.e_mut(t).hp = 1.0;
    spell(&mut g.world, s, t, 2.0, "master67_q");
    assert!(g.e(t).dead);
    assert_eq!(g.damage(Some("master67_p"), None), 0.0);
    assert!(get_buff(g.e(s), "master67_count", None).is_none());
    assert_eq!(g.e(s).champ.as_ref().unwrap().damage_to_champions, 7.0);
    let pending = &g.world.events.pending;
    let deaths = pending
        .iter()
        .filter(|e| matches!(e, GameEvent::Death { target, .. } if *target == t))
        .count();
    assert_eq!(deaths, 1);
    assert!(
        pending
            .iter()
            .all(|e| !matches!(e, GameEvent::Damage { amount, .. } if *amount < 0.0))
    );
}

#[test]
fn une_zone_ne_compte_pas_sept_fois_et_attaques_ou_sbires_ne_comptent_pas() {
    let mut g = game("master67");
    let (s, t) = (g.source, g.target);
    for _ in 0..9 {
        spell(&mut g.world, s, t, 1.0, "master67_w");
    }
    assert_eq!(
        get_buff(g.e(s), "master67_count", None).map(|b| b.stacks),
        Some(1)
    );
    g.advance(0.7);
    let mut auto = DamageOpts::new(Some(s), t, 1.0, DamageType::True);
    auto.is_auto_attack = true;
    apply_damage(&mut g.world, auto);
    let minion = g.dummy(p(1400.0, 1000.0), |_| {});
    spell(&mut g.world, s, minion, 1.0, "master67_q");
    assert_eq!(
        get_buff(g.e(s), "master67_count", None).map(|b| b.stacks),
        Some(1)
    );
    g.advance(6.8);
    assert!(get_buff(g.e(s), "master67_count", None).is_none());
}
