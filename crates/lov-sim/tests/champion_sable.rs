//! Kit de Sable : port de `packages/server/test/champion-sable.test.ts` (cible `tharox` : Sable ne
//! peut pas être sa propre cible de test).

mod common;

use common::champion::{Game, game_vs};
use lov_shared::schema::CcType;
use lov_shared::types::{AbilitySlot, EntityKind, GameEvent, Vec2};
use lov_sim::combat::buffs::{get_buff, has_cc};

fn game() -> Game {
    game_vs("sable", "tharox", 6)
}

fn p(x: f64, y: f64) -> Vec2 {
    Vec2 { x, y }
}

#[test]
fn q_renforce_une_attaque_avec_degats_de_zone() {
    let mut g = game();
    let (s, t) = (g.source, g.target);
    let nearby = g.dummy(p(1300.0, 1000.0), |_| {});
    g.cast(AbilitySlot::Q, None, None);
    g.auto(s, t, false);
    assert!(g.damage(Some("sable_q"), Some(t)) > 0.0);
    assert!(g.damage(Some("sable_q"), Some(nearby)) > 0.0);
    let dealt = g.damage(Some("sable_q"), None);
    g.auto(s, t, false);
    assert_eq!(g.damage(Some("sable_q"), None), dealt);
}

#[test]
fn w_tire_sept_balles_mais_touche_chaque_ennemi_une_fois() {
    let mut g = game();
    let t = g.target;
    g.cast(AbilitySlot::W, Some(t), None);
    g.advance(0.8);
    let pending = &g.world.events.pending;
    let spawned = pending
        .iter()
        .filter(|e| matches!(e, GameEvent::ProjectileSpawn { fx_key, .. } if fx_key == "sable_w"))
        .count();
    assert_eq!(spawned, 7);
    let hits = pending
        .iter()
        .filter(|e| matches!(e, GameEvent::Damage { ability: Some(a), .. } if a == "sable_w"))
        .count();
    assert_eq!(hits, 1);
    assert!(has_cc(g.e(t), CcType::Slow));
}

#[test]
fn e_roule_et_rend_la_prochaine_attaque_critique_renforcee() {
    let mut g = game();
    let (s, t) = (g.source, g.target);
    g.cast(AbilitySlot::E, None, Some(p(900.0, 1000.0)));
    g.advance(0.2);
    let amount = g.auto(s, t, false);
    let (ad, armor) = (g.e(s).stats.ad, g.e(t).stats.armor);
    assert!((amount - ad * 1.75 * 1.1 * 100.0 / (100.0 + armor)).abs() < 1e-6);
    assert!(get_buff(g.e(s), "sable_e", None).is_none());
    let crit = g.world.events.pending.iter().any(|e| {
        matches!(
            e,
            GameEvent::Damage {
                crit: Some(true),
                ..
            }
        )
    });
    assert!(crit);
}

#[test]
fn r_traverse_les_sbires_et_s_arrete_au_premier_champion() {
    let mut g = game();
    let t = g.target;
    g.e_mut(t).pos.x = 1600.0;
    let minion = g.dummy(p(1300.0, 1000.0), |_| {});
    g.cast(AbilitySlot::R, Some(t), None);
    g.advance(0.9);
    assert_eq!(g.damage(Some("sable_r"), None), 0.0);
    g.advance(0.5);
    assert!(g.damage(Some("sable_r"), Some(minion)) > 0.0);
    assert!(g.damage(Some("sable_r"), Some(t)) > 0.0);
    assert_eq!(g.world.query(Some(EntityKind::Projectile)).count(), 0);
}

#[test]
fn le_passif_donne_de_la_vitesse_en_touchant_un_champion() {
    let mut g = game();
    let (s, t) = (g.source, g.target);
    let speed = g.e(s).stats.move_speed;
    g.auto(s, t, false);
    assert!((g.e(s).stats.move_speed - speed * 1.15).abs() < 1e-6);
    g.advance(1.1);
    assert_eq!(g.e(s).stats.move_speed, speed);
}
