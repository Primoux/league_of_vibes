//! Tests du système de combat : port de `packages/server/test/combat.test.ts`.

mod common;

use common::{champ, core_world, dummy, player_of, v};
use lov_shared::map::FOUNTAINS;
use lov_shared::math::dist;
use lov_shared::protocol::ClientMsg;
use lov_shared::types::{EntityKind, GameEvent, Team, TeamOrNeutral};
use lov_sim::QueuedCommand;
use lov_sim::systems::combat::{AttackConfig, attack_config};

fn count_auto_attacks(events: &[GameEvent]) -> usize {
    events
        .iter()
        .filter(|e| matches!(e, GameEvent::AutoAttack { .. }))
        .count()
}

#[test]
fn cadence_attack_speed_degats_apres_le_windup() {
    let mut s = core_world();
    let a = dummy(&mut s.world, TeamOrNeutral::Blue, v(6000.0, 3000.0), |st| {
        st.attack_speed = 2.0;
        st.ad = 50.0;
        st.attack_range = 150.0;
    });
    s.e_mut(a).data.insert(AttackConfig {
        windup: Some(0.5),
        projectile_speed: Some(0.0),
    });
    let t = dummy(&mut s.world, TeamOrNeutral::Red, v(6200.0, 3000.0), |st| {
        st.max_hp = 100000.0
    });
    s.e_mut(t).hp = 100000.0;
    s.e_mut(a).attack_target = Some(t);
    s.step(); // windup = 0.25 s
    assert_eq!(s.e(t).hp, 100000.0);
    s.run(0.25);
    assert_eq!(s.e(t).hp, 100000.0 - 50.0);
    s.run(2.0);
    // ~ 1 + 4 attaques en 2.25 s
    let hits = (100000.0 - s.e(t).hp) / 50.0;
    assert!(hits >= 4.0);
    assert!(hits <= 5.0);
    let events = s.world.events.drain();
    assert_eq!(count_auto_attacks(&events) as f64, hits);
}

#[test]
fn poursuit_une_cible_hors_de_portee_puis_attaque() {
    let mut s = core_world();
    let a = dummy(&mut s.world, TeamOrNeutral::Blue, v(6000.0, 3000.0), |st| {
        st.attack_range = 125.0;
        st.move_speed = 400.0;
    });
    let t = dummy(&mut s.world, TeamOrNeutral::Red, v(7000.0, 3000.0), |_| {});
    s.e_mut(a).attack_target = Some(t);
    s.run(3.0);
    assert!(dist(s.e(a).pos, s.e(t).pos) - s.e(a).radius - s.e(t).radius <= 125.0 + 1.0);
    assert!(s.e(t).hp < 1000.0);
}

/// TS : champion à distance `sable`, dont `getChampion('sable')` fournit `attackWindup` et
/// `projectileSpeed`. Seul Grondak (mêlée) est porté pour l'instant : on simule un champion à distance
/// absent du registre (id `sable`, portée 550). `get_champion` renvoie alors `None` et le combat applique
/// les défauts d'une entité sans définition (armement 0.3, projectile 1600 car portée > 300), que le
/// test lit via `attack_config` plutôt que dans la définition. À repasser sur la vraie définition
/// quand `sable` sera porté.
#[test]
fn distance_projectile_qui_voyage_avant_de_toucher() {
    let mut s = core_world();
    let a = champ(&mut s.world, Team::Blue, v(6000.0, 3000.0));
    {
        let e = s.e_mut(a);
        e.champ.as_mut().unwrap().champion_id = "sable".into();
        e.stats.attack_range = 550.0;
    }
    let t = dummy(&mut s.world, TeamOrNeutral::Red, v(6500.0, 3000.0), |_| {});
    let pid = player_of(&s.world, a);
    s.world.commands.push(QueuedCommand {
        player_id: pid,
        msg: ClientMsg::Attack { target_id: t },
    });
    let (windup, speed) = attack_config(s.e(a));
    assert_eq!((windup, speed), (0.3, 1600.0));
    s.run(windup / s.e(a).stats.attack_speed + 0.04);
    assert_eq!(s.projectiles.pending(), 1);
    assert_eq!(s.e(t).hp, 1000.0);
    s.run(0.3);
    assert!(s.e(t).hp < 1000.0);
    let aa = s
        .world
        .events
        .drain()
        .into_iter()
        .find(|e| matches!(e, GameEvent::AutoAttack { .. }));
    match aa {
        Some(GameEvent::AutoAttack {
            ranged, speed: sp, ..
        }) => {
            assert!(ranged);
            assert_eq!(sp, Some(speed));
        }
        other => panic!("autoAttack attendu, reçu {other:?}"),
    }
    assert_eq!(s.projectiles.pending(), 0);
}

/// Ajout Rust : un champion enregistré (Grondak, mêlée) lit son armement dans sa définition.
#[test]
fn un_champion_enregistre_lit_sa_definition() {
    let mut s = core_world();
    let a = champ(&mut s.world, Team::Blue, v(6000.0, 3000.0));
    assert_eq!(attack_config(s.e(a)), (0.3, 0.0));
    let t = dummy(&mut s.world, TeamOrNeutral::Red, v(6200.0, 3000.0), |_| {});
    s.e_mut(a).attack_target = Some(t);
    s.run(2.0);
    assert!(s.e(t).hp < 1000.0);
    assert_eq!(s.projectiles.pending(), 0);
    let aa = s
        .world
        .events
        .drain()
        .into_iter()
        .find(|e| matches!(e, GameEvent::AutoAttack { .. }));
    assert!(matches!(
        aa,
        Some(GameEvent::AutoAttack {
            ranged: false,
            speed: None,
            ..
        })
    ));
}

#[test]
fn se_deplacer_pendant_le_windup_annule_l_attaque() {
    let mut s = core_world();
    let a = champ(&mut s.world, Team::Blue, v(6000.0, 3000.0));
    let t = dummy(&mut s.world, TeamOrNeutral::Red, v(6200.0, 3000.0), |_| {});
    let pid = player_of(&s.world, a);
    s.world.commands.push(QueuedCommand {
        player_id: pid.clone(),
        msg: ClientMsg::Attack { target_id: t },
    });
    s.step();
    s.world.commands.push(QueuedCommand {
        player_id: pid,
        msg: ClientMsg::Move {
            pos: v(5000.0, 3000.0),
        },
    });
    s.run(1.0);
    assert_eq!(s.e(t).hp, 1000.0);
}

#[test]
fn attaque_deplacement_engage_la_premiere_cible_rencontree_puis_reprend() {
    let mut s = core_world();
    // le long de la voie mid (toujours dégagée, cf. shared/test/map.test.ts)
    let a = champ(&mut s.world, Team::Blue, v(4000.0, 4000.0));
    let t = dummy(&mut s.world, TeamOrNeutral::Red, v(5000.0, 5050.0), |st| {
        st.max_hp = 50.0
    });
    s.e_mut(t).hp = 50.0;
    let pid = player_of(&s.world, a);
    s.world.commands.push(QueuedCommand {
        player_id: pid,
        msg: ClientMsg::AttackMove {
            pos: v(6500.0, 6500.0),
        },
    });
    s.run(16.0);
    // Le TS lit `t.dead` sur l'objet orphelin ; ici le cadavre peut déjà avoir été retiré du monde.
    assert!(s.world.get(t).is_none_or(|e| e.dead));
    assert_eq!(s.e(a).pos, v(6500.0, 6500.0));
}

#[test]
fn crits_x1_75_et_hooks_before_attack_hit() {
    let mut s = core_world();
    let a = dummy(&mut s.world, TeamOrNeutral::Blue, v(6000.0, 3000.0), |st| {
        st.crit_chance = 1.0;
        st.ad = 100.0;
    });
    let t = dummy(&mut s.world, TeamOrNeutral::Red, v(6150.0, 3000.0), |st| {
        st.max_hp = 10000.0
    });
    s.e_mut(t).hp = 10000.0;
    s.world
        .hooks
        .before_attack_hit
        .push(Box::new(|_, ctx| ctx.bonus += 10.0));
    s.e_mut(a).attack_target = Some(t);
    s.run(0.5);
    assert_eq!(s.e(t).hp, 10000.0 - 185.0);
    let dmg = s
        .world
        .events
        .drain()
        .into_iter()
        .find(|e| matches!(e, GameEvent::Damage { .. }));
    match dmg {
        Some(GameEvent::Damage { crit, amount, .. }) => {
            assert_eq!(crit, Some(true));
            assert_eq!(amount, 185.0);
        }
        other => panic!("damage attendu, reçu {other:?}"),
    }
}

#[test]
fn ne_cible_pas_un_allie_ni_une_cible_non_ciblable() {
    let mut s = core_world();
    let a = dummy(&mut s.world, TeamOrNeutral::Blue, v(6000.0, 3000.0), |_| {});
    let ally = dummy(&mut s.world, TeamOrNeutral::Blue, v(6100.0, 3000.0), |_| {});
    s.e_mut(a).attack_target = Some(ally);
    s.run(1.0);
    assert_eq!(s.e(ally).hp, 1000.0);
    assert_eq!(s.e(a).attack_target, None);
    let e = dummy(&mut s.world, TeamOrNeutral::Red, v(6100.0, 3000.0), |_| {});
    s.world
        .hooks
        .is_targetable
        .push(Box::new(move |_, t, _| t != e));
    s.e_mut(a).attack_target = Some(e);
    s.run(1.0);
    assert_eq!(s.e(e).hp, 1000.0);
}

#[test]
fn retire_les_sbires_morts_apres_corpse_seconds_garde_les_champions() {
    let mut s = core_world();
    let a = dummy(&mut s.world, TeamOrNeutral::Blue, v(6000.0, 3000.0), |st| {
        st.ad = 5000.0
    });
    let t = dummy(&mut s.world, TeamOrNeutral::Red, v(6150.0, 3000.0), |_| {});
    s.e_mut(a).attack_target = Some(t);
    s.run(1.0);
    assert!(s.e(t).dead);
    s.run(2.5);
    assert!(s.world.get(t).is_none());
}

#[test]
fn fontaine_soigne_les_allies_et_tue_les_ennemis() {
    let mut s = core_world();
    let ally = champ(&mut s.world, Team::Blue, FOUNTAINS.blue);
    s.e_mut(ally).hp = 1.0;
    let foe = champ(
        &mut s.world,
        Team::Red,
        v(FOUNTAINS.blue.x + 100.0, FOUNTAINS.blue.y),
    );
    s.run(2.0);
    assert!(s.e(ally).hp > s.e(ally).stats.max_hp * 0.2);
    assert!(s.e(foe).dead);
}

/// Ajout Rust : un champion mort n'est jamais retiré (seuls les sbires/monstres/balises le sont) et
/// les kinds non combattants (balise) sont ignorés par l'attaquant.
#[test]
fn un_champion_mort_reste_dans_le_monde() {
    let mut s = core_world();
    let c = champ(&mut s.world, Team::Red, v(6000.0, 6000.0));
    lov_sim::combat::damage::kill(&mut s.world, c, None);
    s.run(3.0);
    assert!(s.world.get(c).is_some());
    let ward = common::dummy_kind(
        &mut s.world,
        TeamOrNeutral::Blue,
        v(100.0, 100.0),
        |_| {},
        EntityKind::Ward,
    );
    s.e_mut(ward).attack_target = Some(c);
    s.run(0.5);
    assert!(s.world.get(ward).is_some());
}
