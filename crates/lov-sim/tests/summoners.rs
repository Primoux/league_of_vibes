//! Tests des sorts d'invocateur : port de `packages/server/test/summoners.test.ts`.
//!
//! Le champion cible est `tharox` (voir `common/champion.rs`) au lieu de `sable` : seuls sa position,
//! son équipe et son type d'entité comptent.

mod common;

use common::champion::{Game, game};
use lov_shared::schema::CcType;
use lov_shared::types::{
    DamageType, EntityId, EntityKind, SummonerId, SummonerSlot, TeamOrNeutral, Vec2,
};
use lov_sim::abilities::primitives::is_walkable;
use lov_sim::combat::buffs::{BuffOptions, Channeling, add_buff, get_buff, has_cc};
use lov_sim::combat::damage::{DamageOpts, HealOpts, apply_damage, apply_heal};
use lov_sim::systems::movement::Ghost;
use lov_sim::systems::summoners::cast_summoner;

fn p(x: f64, y: f64) -> Vec2 {
    Vec2 { x, y }
}

/// `use(g, id, target, point)` : place `id` en D puis le lance.
fn cast(g: &mut Game, id: SummonerId, target: EntityId, point: Vec2) -> bool {
    let source = g.source;
    g.e_mut(source).champ.as_mut().unwrap().summoners.d = id;
    cast_summoner(
        &mut g.world,
        source,
        SummonerSlot::D,
        Some(target),
        Some(point),
    )
}

fn cast_at_target(g: &mut Game, id: SummonerId) -> bool {
    let t = g.target;
    cast(g, id, t, p(1400.0, 1000.0))
}

fn reset_cooldown(g: &mut Game) {
    let s = g.source;
    g.e_mut(s).champ.as_mut().unwrap().cooldowns.d = 0.0;
}

#[test]
fn flash_borne_a_400_et_atterrit_hors_des_murs() {
    let mut g = game("ilyra");
    let (s, t) = (g.source, g.target);
    assert!(cast(&mut g, SummonerId::Flash, t, p(10000.0, 1000.0)));
    assert_eq!(g.e(s).pos.x, 1400.0);
    assert_eq!(g.e(s).champ.as_ref().unwrap().cooldowns.d, 300.0);
    assert!(!cast_at_target(&mut g, SummonerId::Flash));
    reset_cooldown(&mut g);
    g.e_mut(s).pos = p(1900.0, 3900.0);
    assert!(cast(&mut g, SummonerId::Flash, t, p(2300.0, 3900.0)));
    let e = g.e(s);
    assert!(is_walkable(e.pos, e.radius));
    assert!((e.pos.x - 1900.0).hypot(e.pos.y - 3900.0) <= 400.001);
}

#[test]
fn embrasement_inflige_ses_cinq_tics_et_reduit_les_soins() {
    let mut g = game("ilyra");
    let t = g.target;
    assert!(cast_at_target(&mut g, SummonerId::Ignite));
    g.e_mut(t).hp -= 100.0;
    let healed = apply_heal(&mut g.world, HealOpts::new(Some(t), t, 100.0));
    assert_eq!(healed, 60.0);
    g.advance(5.1);
    assert!((g.damage(Some("sum_ignite"), None) - 170.0).abs() < 1e-6);
}

#[test]
fn soins_rendent_des_pv_et_de_la_vitesse_barriere_expire() {
    let mut g = game("ilyra");
    let s = g.source;
    g.e_mut(s).hp -= 300.0;
    assert!(cast_at_target(&mut g, SummonerId::Heal));
    let e = g.e(s);
    assert!((e.hp - (e.stats.max_hp - 130.0)).abs() < 1e-6);
    assert!((e.stats.move_speed - 330.0 * 1.3).abs() < 1e-6);
    reset_cooldown(&mut g);
    assert!(cast_at_target(&mut g, SummonerId::Barrier));
    assert_eq!(g.e(s).shield, 225.0);
    g.advance(2.6);
    assert_eq!(g.e(s).shield, 0.0);
}

#[test]
fn fatigue_reduit_les_degats_infliges_et_ralentit() {
    let mut g = game("ilyra");
    let (s, t) = (g.source, g.target);
    assert!(cast_at_target(&mut g, SummonerId::Exhaust));
    let reduced = apply_damage(
        &mut g.world,
        DamageOpts::new(Some(t), s, 100.0, DamageType::Physical),
    );
    let armor = g.e(s).stats.armor;
    assert!((reduced - 60.0 * 100.0 / (100.0 + armor)).abs() < 1e-6);
    assert!(has_cc(g.e(t), CcType::Slow));
}

#[test]
fn chatiment_sur_sbire_ou_monstre_seulement_et_fantome() {
    let mut g = game("ilyra");
    let s = g.source;
    assert!(!cast_at_target(&mut g, SummonerId::Smite));
    let minion = g.dummy(p(1400.0, 1000.0), |_| {});
    assert!(cast(&mut g, SummonerId::Smite, minion, p(1400.0, 1000.0)));
    assert_eq!(g.damage(Some("sum_smite"), None), 600.0);
    reset_cooldown(&mut g);
    assert!(cast_at_target(&mut g, SummonerId::Ghost));
    assert!(g.e(s).data.contains::<Ghost>());
    assert!(g.e(s).stats.move_speed > 330.0);
}

#[test]
fn purge_retire_les_cc_garde_les_projections_et_donne_65_de_tenacite() {
    let mut g = game("ilyra");
    let s = g.source;
    add_buff(&mut g.world, s, "stun", BuffOptions::new(3.0));
    add_buff(&mut g.world, s, "knockup", BuffOptions::new(3.0));
    assert!(cast_at_target(&mut g, SummonerId::Cleanse));
    assert!(!has_cc(g.e(s), CcType::Stun));
    assert!(has_cc(g.e(s), CcType::Knockup));
    add_buff(&mut g.world, s, "root", BuffOptions::new(2.0));
    let remaining = get_buff(g.e(s), "root", None).unwrap().remaining;
    assert!((remaining - 0.7).abs() < 1e-6);
}

#[test]
fn teleportation_canalise_4_s_et_s_annule_sur_degats() {
    let mut g = game("ilyra");
    let (s, t) = (g.source, g.target);
    let tower = g.dummy(p(8000.0, 8000.0), |_| {});
    {
        let tw = g.e_mut(tower);
        tw.kind = EntityKind::Tower;
        tw.team = TeamOrNeutral::Blue;
    }
    assert!(cast(&mut g, SummonerId::Teleport, tower, p(1400.0, 1000.0)));
    g.advance(3.9);
    assert_eq!(g.e(s).pos.x, 1000.0);
    g.advance(0.2);
    assert!(g.e(s).pos.x > 7900.0);
    reset_cooldown(&mut g);
    assert!(cast(&mut g, SummonerId::Teleport, tower, p(1400.0, 1000.0)));
    apply_damage(
        &mut g.world,
        DamageOpts::new(Some(t), s, 1.0, DamageType::True),
    );
    g.advance(0.1);
    assert!(!g.e(s).data.contains::<Channeling>());
}

// Test ajouté : les recharges s'écoulent avec le temps de jeu.
#[test]
fn les_recharges_s_ecoulent_et_permettent_un_nouveau_lancer() {
    let mut g = game("ilyra");
    let s = g.source;
    assert!(cast_at_target(&mut g, SummonerId::Barrier));
    let cd = g.e(s).champ.as_ref().unwrap().cooldowns.d;
    g.advance(10.0);
    let after = g.e(s).champ.as_ref().unwrap().cooldowns.d;
    assert!((cd - after - 10.0).abs() < 1e-6);
    g.advance(after + 0.1);
    assert!(cast_at_target(&mut g, SummonerId::Barrier));
}
