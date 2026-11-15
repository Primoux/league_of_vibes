//! Tests du système de vision : port de `packages/server/test/vision.test.ts`, sur `coreWorld()`.

mod common;

use common::{Sim, champ, core_world, v};
use lov_shared::map::BRUSHES;
use lov_shared::types::{DamageType, EntityId, EntityKind, Team, TeamOrNeutral, Vec2};
use lov_sim::combat::damage::{DamageOpts, apply_damage};
use lov_sim::systems::vision::{Invisible, RevealedUntil, is_visible_to};

fn sim_champ(s: &mut Sim, team: Team, pos: Vec2) -> EntityId {
    champ(&mut s.world, team, pos)
}

fn visible(s: &Sim, id: EntityId, team: TeamOrNeutral) -> bool {
    is_visible_to(s.e(id), team, true)
}

fn brush_center(i: usize) -> Vec2 {
    let b = &BRUSHES[i];
    let n = b.len() as f64;
    v(
        b.iter().map(|p| p.x).sum::<f64>() / n,
        b.iter().map(|p| p.y).sum::<f64>() / n,
    )
}

#[test]
fn rayon_de_vision_des_champions() {
    let mut s = core_world();
    sim_champ(&mut s, Team::Blue, v(6000.0, 3000.0));
    let near = sim_champ(&mut s, Team::Red, v(6000.0, 4000.0));
    let far = sim_champ(&mut s, Team::Red, v(6000.0, 5000.0));
    s.step();
    assert!(visible(&s, near, TeamOrNeutral::Blue));
    assert!(!visible(&s, far, TeamOrNeutral::Blue));
    assert!(visible(&s, far, TeamOrNeutral::Red));
}

#[test]
fn brousse_cachee_sauf_ennemi_dans_la_meme_brousse_ou_balise() {
    let mut s = core_world();
    let c = brush_center(0);
    let hidden = sim_champ(&mut s, Team::Red, c);
    let watcher = sim_champ(&mut s, Team::Blue, v(c.x + 300.0, c.y));
    s.step();
    assert!(!visible(&s, hidden, TeamOrNeutral::Blue));
    s.e_mut(watcher).pos = c;
    s.step();
    assert!(visible(&s, hidden, TeamOrNeutral::Blue));
    s.e_mut(watcher).pos = v(c.x + 3000.0, c.y);
    s.step();
    assert!(!visible(&s, hidden, TeamOrNeutral::Blue));
    common::dummy_kind(
        &mut s.world,
        TeamOrNeutral::Blue,
        v(c.x + 400.0, c.y),
        |_| {},
        EntityKind::Ward,
    );
    s.step();
    assert!(visible(&s, hidden, TeamOrNeutral::Blue));
}

#[test]
fn attaquer_depuis_une_brousse_revele_1_s() {
    let mut s = core_world();
    let c = brush_center(0);
    let hidden = sim_champ(&mut s, Team::Red, c);
    let victim = sim_champ(&mut s, Team::Blue, v(c.x + 400.0, c.y));
    s.step();
    assert!(!visible(&s, hidden, TeamOrNeutral::Blue));
    apply_damage(
        &mut s.world,
        DamageOpts::new(Some(hidden), victim, 10.0, DamageType::True),
    );
    s.step();
    assert!(visible(&s, hidden, TeamOrNeutral::Blue));
    s.run(1.2);
    assert!(!visible(&s, hidden, TeamOrNeutral::Blue));
}

#[test]
fn invisibilite_et_revelation() {
    let mut s = core_world();
    sim_champ(&mut s, Team::Blue, v(6000.0, 3000.0));
    let nyx = sim_champ(&mut s, Team::Red, v(6000.0, 3300.0));
    s.e_mut(nyx).data.insert(Invisible);
    s.step();
    assert!(!visible(&s, nyx, TeamOrNeutral::Blue));
    let until = s.world.time + 1.0;
    s.world
        .get_mut(nyx)
        .unwrap()
        .data
        .insert(RevealedUntil(until));
    s.step();
    assert!(visible(&s, nyx, TeamOrNeutral::Blue));
}

#[test]
fn on_ne_peut_pas_auto_attaquer_une_cible_invisible() {
    let mut s = core_world();
    let a = champ(&mut s.world, Team::Blue, v(6000.0, 3000.0));
    let t = champ(&mut s.world, Team::Red, v(6150.0, 3000.0));
    s.e_mut(t).data.insert(Invisible);
    s.step();
    s.e_mut(a).attack_target = Some(t);
    s.run(1.0);
    assert_eq!(s.e(t).hp, s.e(t).stats.max_hp);
}
