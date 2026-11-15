//! Tests de `world_ai::runtime` (sans équivalent TypeScript : le runtime n'y est couvert qu'à travers
//! les tests des structures, des sbires, de la jungle et des objectifs).

mod common;

use common::{dummy, dummy_kind, v};
use lov_shared::schema::{BuffKind, CcType};
use lov_shared::types::{EntityKind, GameEvent, TeamOrNeutral};
use lov_sim::World;
use lov_sim::combat::buffs::{get_buff, get_buffs};
use lov_sim::world_ai::runtime::{
    BuffOptions, RETIRE_DELAY, add_buff, attack, in_range, nearest, retire, stats, stats_with,
};

#[test]
fn stats_applique_les_valeurs_par_defaut_du_typescript() {
    let s = stats(100.0, 7.0);
    assert_eq!(
        (s.armor, s.mr, s.attack_range, s.move_speed, s.attack_speed),
        (0.0, 0.0, 125.0, 0.0, 1.0)
    );
    let s = stats_with(100.0, 7.0, 20.0, 550.0, 300.0, 0.7);
    assert_eq!(
        (s.armor, s.mr, s.attack_range, s.attack_speed),
        (20.0, 20.0, 550.0, 0.7)
    );
}

#[test]
fn add_buff_convertit_les_parametres_pve_et_rend_permanent_un_buff_de_dragon() {
    let mut w = World::new(1);
    let owner = dummy(&mut w, TeamOrNeutral::Blue, v(0.0, 0.0), |_| {});
    let target = dummy(&mut w, TeamOrNeutral::Blue, v(0.0, 0.0), |_| {});
    let mut o = BuffOptions::new(10.0);
    o.source = Some(owner);
    o.stacks = Some(2);
    o.params = vec![
        ("ad".into(), 10.0),
        ("adPercent".into(), 0.1),
        ("attackSpeed".into(), 0.2),
        ("tenacity".into(), 0.1),
    ];
    add_buff(&mut w, target, "dragon_fire", o);
    let t = w.get(target).unwrap();
    let b = get_buff(t, "dragon_fire", None).expect("source retirée : buff permanent");
    assert_eq!(b.stacks, 2);
    assert_eq!(b.def.name, "Faveur élémentaire");
    assert_eq!(b.params.persist_on_death, Some(true));
    assert_eq!(b.params.stat_mods.as_ref().unwrap().ad, Some(10.0));
    assert_eq!(b.params.stat_percent_mods.as_ref().unwrap().ad, Some(0.1));
    assert_eq!(
        b.params.stat_percent_mods.as_ref().unwrap().attack_speed,
        Some(0.2)
    );
    assert_eq!(b.params.stat_mods.as_ref().unwrap().armor, None);
    assert_eq!(b.params.tenacity, Some(0.2));
}

#[test]
fn add_buff_un_ralentissement_devient_un_cc_et_garde_sa_source() {
    let mut w = World::new(1);
    let owner = dummy(&mut w, TeamOrNeutral::Blue, v(0.0, 0.0), |_| {});
    let target = dummy(&mut w, TeamOrNeutral::Red, v(0.0, 0.0), |_| {});
    let mut o = BuffOptions::new(3.0);
    o.source = Some(owner);
    o.params = vec![("slow".into(), 0.3)];
    add_buff(&mut w, target, "grub_web", o);
    let t = w.get(target).unwrap();
    let b = get_buff(t, "grub_web", Some(owner)).unwrap();
    assert_eq!(b.def.kind, BuffKind::Cc);
    assert_eq!(b.def.cc, Some(CcType::Slow));
    assert_eq!(b.def.name, "Faveur de la Faille");
    assert_eq!(b.params.persist_on_death, Some(false));
    // Un mort ne reçoit rien.
    w.get_mut(target).unwrap().dead = true;
    let before = get_buffs(w.get(target).unwrap()).len();
    add_buff(&mut w, target, "other", BuffOptions::new(3.0));
    assert_eq!(get_buffs(w.get(target).unwrap()).len(), before);
}

#[test]
fn nearest_departage_les_egalites_par_plus_petit_id_et_ignore_les_morts() {
    let mut w = World::new(1);
    let me = dummy(&mut w, TeamOrNeutral::Blue, v(0.0, 0.0), |_| {});
    let a = dummy(&mut w, TeamOrNeutral::Red, v(100.0, 0.0), |_| {});
    let b = dummy(&mut w, TeamOrNeutral::Red, v(0.0, 100.0), |_| {});
    assert_eq!(nearest(&w, me, 200.0, |_| true), Some(a));
    w.get_mut(a).unwrap().dead = true;
    assert_eq!(nearest(&w, me, 200.0, |_| true), Some(b));
    assert_eq!(nearest(&w, me, 50.0, |_| true), None);
    assert_eq!(nearest(&w, me, 200.0, |e| e.id != b), None);
}

#[test]
fn in_range_mesure_jusquau_bord_de_la_cible() {
    let mut w = World::new(1);
    let a = dummy(&mut w, TeamOrNeutral::Blue, v(0.0, 0.0), |_| {});
    let b = dummy(&mut w, TeamOrNeutral::Red, v(140.0, 0.0), |_| {});
    // Rayon de 40 : 100 + 40 = 140.
    assert!(in_range(w.get(a).unwrap(), w.get(b).unwrap(), 100.0));
    assert!(!in_range(w.get(a).unwrap(), w.get(b).unwrap(), 99.0));
}

#[test]
fn attack_repli_approche_puis_frappe_avec_cadence() {
    let mut w = World::new(1);
    let a = dummy(&mut w, TeamOrNeutral::Blue, v(0.0, 0.0), |s| {
        s.attack_range = 100.0;
        s.ad = 10.0;
    });
    let t = dummy(&mut w, TeamOrNeutral::Red, v(500.0, 0.0), |_| {});
    assert!(!attack(&mut w, a, t, None, false));
    assert_eq!(w.get(a).unwrap().attack_target, Some(t));
    assert_eq!(w.get(a).unwrap().move_target, Some(v(500.0, 0.0)));
    w.get_mut(t).unwrap().pos = v(100.0, 0.0);
    assert!(attack(&mut w, a, t, None, false));
    assert_eq!(w.get(a).unwrap().move_target, None);
    assert_eq!(w.get(t).unwrap().hp, 990.0);
    assert!(
        w.events
            .pending
            .iter()
            .any(|e| matches!(e, GameEvent::AutoAttack { ranged: false, .. }))
    );
    // Cadence : 1 attaque par seconde.
    assert!(!attack(&mut w, a, t, None, false));
    w.time = 1.0;
    assert!(attack(&mut w, a, t, Some(5.0), false));
    assert_eq!(w.get(t).unwrap().hp, 985.0);
    // Avec le combat central, seule la cible est posée.
    w.time = 5.0;
    assert!(!attack(&mut w, a, t, None, true));
    assert_eq!(w.get(t).unwrap().hp, 985.0);
}

#[test]
fn retire_supprime_un_mort_apres_le_delai() {
    let mut w = World::new(1);
    let vivant = dummy_kind(
        &mut w,
        TeamOrNeutral::Red,
        v(0.0, 0.0),
        |_| {},
        EntityKind::Minion,
    );
    let mort = dummy(&mut w, TeamOrNeutral::Red, v(0.0, 0.0), |_| {});
    w.get_mut(mort).unwrap().dead = true;
    retire(&mut w, vivant, RETIRE_DELAY);
    retire(&mut w, mort, RETIRE_DELAY);
    assert!(w.contains(vivant) && w.contains(mort));
    w.time = RETIRE_DELAY - 0.01;
    retire(&mut w, mort, RETIRE_DELAY);
    assert!(w.contains(mort));
    w.time = RETIRE_DELAY;
    retire(&mut w, mort, RETIRE_DELAY);
    assert!(!w.contains(mort));
    assert!(w.contains(vivant));
}
