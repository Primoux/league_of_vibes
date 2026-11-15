//! Tests de `combat/damage` : port de `packages/server/test/damage.test.ts`, sur `coreWorld()`
//! (le système `combat` réel expire les boucliers et décompte la réapparition).

mod common;

use common::{Sim, core_world, v};
use lov_shared::types::{DamageType, EntityId, EntityKind, GameEvent, Team, TeamOrNeutral};
use lov_sim::combat::damage::{
    DamageOpts, HealOpts, Owner, ShieldOpts, apply_damage, apply_heal, apply_shield,
    default_respawn_time,
};

fn close(a: f64, b: f64) {
    // `toBeCloseTo` de vitest : 2 décimales.
    assert!((a - b).abs() < 0.005, "{a} != {b}");
}

/// Extension locale de `Sim` : un coup de dégâts simple.
trait DmgExt {
    fn dmg(
        &mut self,
        source: Option<EntityId>,
        target: EntityId,
        amount: f64,
        t: DamageType,
    ) -> f64;
}

impl DmgExt for Sim {
    fn dmg(
        &mut self,
        source: Option<EntityId>,
        target: EntityId,
        amount: f64,
        t: DamageType,
    ) -> f64 {
        apply_damage(&mut self.world, DamageOpts::new(source, target, amount, t))
    }
}

#[test]
fn applique_armure_et_rm_les_degats_bruts_ignorent_les_resistances() {
    let mut s = core_world();
    let t = s.dummy(TeamOrNeutral::Red, v(6000.0, 6000.0), |st| {
        st.armor = 100.0;
        st.mr = 50.0;
    });
    close(s.dmg(None, t, 100.0, DamageType::Physical), 50.0);
    close(s.dmg(None, t, 150.0, DamageType::Magic), 100.0);
    close(s.dmg(None, t, 100.0, DamageType::True), 100.0);
    close(s.e(t).hp, 1000.0 - 250.0);
    let evs = s
        .world
        .events
        .drain()
        .into_iter()
        .filter(|e| matches!(e, GameEvent::Damage { .. }))
        .count();
    assert_eq!(evs, 3);
}

#[test]
fn applique_la_penetration_fixe_sans_descendre_sous_0() {
    let mut s = core_world();
    let src = s.dummy(TeamOrNeutral::Blue, v(0.0, 0.0), |st| {
        st.armor_pen = 50.0;
        st.magic_pen = 200.0;
    });
    let t = s.dummy(TeamOrNeutral::Red, v(6000.0, 6000.0), |st| {
        st.armor = 150.0;
        st.mr = 50.0;
    });
    close(s.dmg(Some(src), t, 200.0, DamageType::Physical), 100.0);
    close(s.dmg(Some(src), t, 100.0, DamageType::Magic), 100.0);
}

#[test]
fn les_resistances_negatives_amplifient_les_degats() {
    let mut s = core_world();
    let t = s.dummy(TeamOrNeutral::Red, v(6000.0, 6000.0), |st| {
        st.armor = -100.0
    });
    close(s.dmg(None, t, 100.0, DamageType::Physical), 150.0);
}

#[test]
fn consomme_les_boucliers_avant_les_pv_et_les_fait_expirer() {
    let mut s = core_world();
    let t = s.dummy(TeamOrNeutral::Red, v(6000.0, 6000.0), |_| {});
    let mut o = ShieldOpts::new(Some(t), t, 80.0);
    o.duration = Some(2.0);
    apply_shield(&mut s.world, o);
    apply_shield(&mut s.world, ShieldOpts::new(Some(t), t, 50.0));
    assert_eq!(s.e(t).shield, 130.0);
    let dealt = s.dmg(None, t, 100.0, DamageType::True);
    assert_eq!(dealt, 100.0);
    assert_eq!(s.e(t).hp, 1000.0); // 80 (expire en premier) + 20 sur le permanent
    assert_eq!(s.e(t).shield, 30.0);
    let mut o = ShieldOpts::new(Some(t), t, 40.0);
    o.duration = Some(1.0);
    apply_shield(&mut s.world, o);
    s.run(1.1);
    assert_eq!(s.e(t).shield, 30.0);
}

#[test]
fn tue_emet_death_avec_killer_et_assists_met_a_jour_kda() {
    let mut s = core_world();
    let a = s.champ(Team::Blue, v(6000.0, 3000.0));
    let b = s.champ(Team::Blue, v(6100.0, 3000.0));
    let vic = s.champ(Team::Red, v(6000.0, 3200.0));
    s.world
        .hooks
        .bounty
        .push(Box::new(|_, _, _, _| Some(300.0)));
    s.dmg(Some(b), vic, 100.0, DamageType::True);
    s.dmg(Some(a), vic, 1e6, DamageType::True);
    assert!(s.e(vic).dead);
    assert_eq!(s.e(vic).champ.as_ref().unwrap().deaths, 1);
    assert_eq!(s.e(a).champ.as_ref().unwrap().kills, 1);
    assert_eq!(s.e(b).champ.as_ref().unwrap().assists, 1);
    assert!(s.e(a).champ.as_ref().unwrap().damage_to_champions > 500.0);
    let death = s
        .world
        .events
        .drain()
        .into_iter()
        .find(|e| matches!(e, GameEvent::Death { .. }));
    assert_eq!(
        death,
        Some(GameEvent::Death {
            target: vic,
            killer: Some(a),
            assists: Some(vec![b]),
            bounty: Some(300.0),
        })
    );
    close(
        s.e(vic).champ.as_ref().unwrap().respawn_in,
        default_respawn_time(1, s.world.time),
    );
}

#[test]
fn credite_le_kill_au_champion_si_une_tour_porte_le_coup_final() {
    let mut s = core_world();
    let a = s.champ(Team::Blue, v(6000.0, 3000.0));
    let tower = s.dummy_kind(
        TeamOrNeutral::Blue,
        v(6000.0, 2500.0),
        |_| {},
        EntityKind::Tower,
    );
    let vic = s.champ(Team::Red, v(6000.0, 3200.0));
    s.dmg(Some(a), vic, 100.0, DamageType::True);
    s.dmg(Some(tower), vic, 1e6, DamageType::True);
    assert_eq!(s.e(a).champ.as_ref().unwrap().kills, 1);
    let death = s
        .world
        .events
        .drain()
        .into_iter()
        .find(|e| matches!(e, GameEvent::Death { .. }));
    assert!(matches!(
        death,
        Some(GameEvent::Death { killer, .. }) if killer == Some(a)
    ));
}

#[test]
fn respawn_a_la_fontaine_apres_le_timer() {
    let mut s = core_world();
    let vic = s.champ(Team::Red, v(6000.0, 6000.0));
    s.dmg(None, vic, 1e6, DamageType::True);
    assert!(s.e(vic).dead);
    let respawn_in = s.e(vic).champ.as_ref().unwrap().respawn_in;
    s.run(respawn_in + 0.1);
    assert!(!s.e(vic).dead);
    assert_eq!(s.e(vic).hp, s.e(vic).stats.max_hp);
    assert!(s.e(vic).pos.x > 11000.0);
    assert!(
        s.world
            .events
            .drain()
            .iter()
            .any(|e| matches!(e, GameEvent::Respawn { .. }))
    );
}

#[test]
fn hooks_before_damage_reduction_et_prevent_death() {
    let mut s = core_world();
    let t = s.dummy(TeamOrNeutral::Red, v(6000.0, 6000.0), |_| {});
    s.world
        .hooks
        .before_damage
        .push(Box::new(|_, ctx| ctx.multiplier *= 0.5));
    assert_eq!(s.dmg(None, t, 100.0, DamageType::True), 50.0);
    s.world.hooks.prevent_death.push(Box::new(|w, id, _| {
        if let Some(e) = w.get_mut(id) {
            e.hp = 1.0;
        }
        true
    }));
    s.dmg(None, t, 1e6, DamageType::True);
    assert!(!s.e(t).dead);
    assert_eq!(s.e(t).hp, 1.0);
}

#[test]
fn soins_plafonnes_aux_pv_max_lifesteal_sur_aa_uniquement() {
    let mut s = core_world();
    let src = s.dummy(TeamOrNeutral::Blue, v(0.0, 0.0), |st| st.lifesteal = 0.5);
    let t = s.dummy(TeamOrNeutral::Red, v(6000.0, 6000.0), |_| {});
    s.world.get_mut(src).unwrap().hp = 500.0;
    assert_eq!(
        apply_heal(&mut s.world, HealOpts::new(Some(src), src, 2000.0)),
        500.0
    );
    s.world.get_mut(src).unwrap().hp = 500.0;
    s.dmg(Some(src), t, 100.0, DamageType::Physical);
    assert_eq!(s.e(src).hp, 500.0);
    let mut o = DamageOpts::new(Some(src), t, 100.0, DamageType::Physical);
    o.is_auto_attack = true;
    apply_damage(&mut s.world, o);
    assert_eq!(s.e(src).hp, 550.0);
}

#[test]
fn credite_le_proprietaire_data_owner() {
    let mut s = core_world();
    let a = s.champ(Team::Blue, v(6000.0, 3000.0));
    let pet = s.dummy(TeamOrNeutral::Blue, v(6000.0, 3000.0), |_| {});
    s.world.get_mut(pet).unwrap().data.insert(Owner(a));
    let vic = s.champ(Team::Red, v(6000.0, 3200.0));
    s.dmg(Some(pet), vic, 1e6, DamageType::True);
    assert_eq!(s.e(a).champ.as_ref().unwrap().kills, 1);
}
