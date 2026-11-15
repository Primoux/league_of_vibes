//! Kit de Solenne : port de `packages/server/test/champion-solenne.test.ts` (cible `tharox` au lieu
//! de `sable`).

mod common;

use common::champion::game;
use lov_shared::data::champions::get_champion;
use lov_shared::schema::CcType;
use lov_shared::types::{AbilitySlot, SummonerId, Team, Vec2};
use lov_sim::combat::buffs::has_cc;
use lov_sim::{CreateChampion, create_champion};

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-6, "{a} != {b}");
}

#[test]
fn q_blesse_et_immobilise_sa_premiere_cible() {
    let mut g = game("solenne");
    let t = g.target;
    g.cast(AbilitySlot::Q, Some(t), None);
    g.advance(0.6);
    assert!(g.damage(Some("solenne_q"), None) > 0.0);
    assert!(has_cc(g.e(t), CcType::Root));
}

#[test]
fn w_soigne_un_allie_et_la_lanceuse_selon_les_pv_manquants() {
    let mut g = game("solenne");
    let s = g.source;
    let def = get_champion("ilyra").unwrap();
    let ally = create_champion(
        &mut g.world,
        CreateChampion {
            player_id: "ally".into(),
            name: "Ally".into(),
            team: Team::Blue,
            champion_id: "ilyra".into(),
            summoners: [SummonerId::Flash, SummonerId::Heal],
            pos: Vec2 {
                x: 1100.0,
                y: 1000.0,
            },
        },
        &def.base_stats,
    );
    for id in [ally, s] {
        let e = g.e_mut(id);
        e.hp = e.stats.max_hp / 2.0;
    }
    let (before, self_before) = (g.e(ally).hp, g.e(s).hp);
    g.cast(AbilitySlot::W, Some(ally), None);
    close(g.e(ally).hp - before, 70.0 * 1.25);
    close(g.e(s).hp - self_before, 70.0 * 0.6 * 1.25);
}

#[test]
fn w_sur_soi_ne_donne_qu_un_soin() {
    let mut g = game("solenne");
    let s = g.source;
    g.e_mut(s).hp *= 0.5;
    let before = g.e(s).hp;
    g.cast(AbilitySlot::W, Some(s), None);
    close(g.e(s).hp - before, 70.0 * 1.25);
}

#[test]
fn e_protege_donne_de_la_vitesse_d_attaque_et_expire() {
    let mut g = game("solenne");
    let s = g.source;
    let speed = g.e(s).stats.attack_speed;
    g.cast(AbilitySlot::E, Some(s), None);
    assert_eq!(g.e(s).shield, 60.0);
    close(g.e(s).stats.attack_speed, speed * 1.25);
    g.advance(2.6);
    assert_eq!(g.e(s).shield, 0.0);
    assert_eq!(g.e(s).stats.attack_speed, speed);
}

#[test]
fn r_soigne_et_protege_les_allies_apres_le_delai() {
    let mut g = game("solenne");
    let (s, t) = (g.source, g.target);
    g.e_mut(s).hp -= 500.0;
    g.e_mut(t).hp -= 500.0;
    let (hp, enemy_hp) = (g.e(s).hp, g.e(t).hp);
    let pos = g.e(s).pos;
    g.cast(AbilitySlot::R, None, Some(pos));
    g.advance(0.7);
    assert_eq!(g.e(s).hp, hp);
    g.advance(0.2);
    assert!(g.e(s).hp > hp);
    assert!(g.e(s).shield > 100.0);
    assert_eq!(g.e(t).hp, enemy_hp);
    assert_eq!(g.e(t).shield, 0.0);
}
