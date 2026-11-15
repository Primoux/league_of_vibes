//! Kit de Corvin : port de `packages/server/test/champion-corvin.test.ts` (cible `tharox` au lieu
//! de `sable`).

mod common;

use common::champion::game;
use lov_shared::schema::CcType;
use lov_shared::types::{AbilitySlot, EntityKind, GameEvent, Vec2};
use lov_sim::combat::buffs::{Revealed, get_buff, has_cc};
use lov_sim::systems::vision::Invisible;

fn zones(g: &common::champion::Game) -> Vec<u32> {
    g.world
        .query(Some(EntityKind::Zone))
        .map(|z| z.id)
        .collect()
}

#[test]
fn q_traverse_et_marque_chaque_unite() {
    let mut g = game("corvin");
    let (s, t) = (g.source, g.target);
    let extra = g.dummy(
        Vec2 {
            x: 1600.0,
            y: 1000.0,
        },
        |_| {},
    );
    g.cast(AbilitySlot::Q, Some(t), None);
    g.advance(0.8);
    assert!(g.damage(Some("corvin_q"), Some(extra)) > 0.0);
    assert_eq!(
        get_buff(g.e(t), "corvin_marks", Some(s)).map(|b| b.stacks),
        Some(1)
    );
}

#[test]
fn w_immobilise_et_revele_le_premier_ennemi_puis_disparait() {
    let mut g = game("corvin");
    let t = g.target;
    g.cast(AbilitySlot::W, Some(t), None);
    let z = zones(&g);
    assert!(g.e(z[0]).data.contains::<Invisible>());
    g.advance(0.1);
    assert!(has_cc(g.e(t), CcType::Root));
    assert!(g.e(t).data.contains::<Revealed>());
    assert!(zones(&g).is_empty());
}

#[test]
fn garde_seulement_les_trois_pieges_les_plus_recents() {
    let mut g = game("corvin");
    let s = g.source;
    for i in 0..4 {
        g.e_mut(s).champ.as_mut().unwrap().cooldowns.w = 0.0;
        g.cast(
            AbilitySlot::W,
            None,
            Some(Vec2 {
                x: 1000.0 + f64::from(i) * 50.0,
                y: 1500.0,
            }),
        );
    }
    assert_eq!(zones(&g).len(), 3);
}

#[test]
fn e_bondit_a_l_oppose_de_la_visee_et_accelere() {
    let mut g = game("corvin");
    let (s, t) = (g.source, g.target);
    g.cast(AbilitySlot::E, Some(t), None);
    g.advance(0.5);
    assert_eq!(g.e(s).pos.x, 575.0);
    assert!((g.e(s).stats.move_speed - 330.0 * 1.3).abs() < 1e-6);
}

#[test]
fn r_inflige_trois_tics_revele_et_reduit_les_soins() {
    let mut g = game("corvin");
    let t = g.target;
    g.cast(AbilitySlot::R, Some(t), None);
    g.advance(3.3);
    let ticks = g
        .world
        .events
        .pending
        .iter()
        .filter(|e| matches!(e, GameEvent::Damage { ability: Some(a), .. } if a == "corvin_r"))
        .count();
    assert_eq!(ticks, 3);
    assert!(get_buff(g.e(t), "grievous", None).is_some());
    assert!(g.e(t).data.contains::<Revealed>());
}

#[test]
fn trois_marques_infligent_six_pourcent_des_pv_max_en_magique() {
    let mut g = game("corvin");
    let (s, t) = (g.source, g.target);
    for _ in 0..3 {
        g.auto(s, t, false);
    }
    let (max_hp, mr) = (g.e(t).stats.max_hp, g.e(t).stats.mr);
    assert!((g.damage(Some("corvin_p"), None) - max_hp * 0.06 * 100.0 / (100.0 + mr)).abs() < 1e-6);
    assert!(get_buff(g.e(t), "corvin_marks", None).is_none());
}
