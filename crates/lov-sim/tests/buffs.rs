//! Tests des buffs : port de `packages/server/test/buffs.test.ts`.
//!
//! `applyShield` vient de `abilities/effects.ts` (le vrai wrapper, `lov_sim::abilities::effects`) ;
//! `applyDamage` et `applyHeal` sont de simples ré-exports de `combat/damage`.

mod common;

use common::v;
use lov_shared::data::champions::get_champion;
use lov_shared::schema::CcType;
use lov_shared::types::{DamageType, EntityId, EntityKind, GameEvent, Team, TeamOrNeutral};
use lov_shared::types::{PartialStats, SummonerId};
use lov_sim::abilities::effects::{ShieldOptions, apply_shield as apply_spell_shield};
use lov_sim::combat::buffs::{
    BuffOptions, Stasis, StatModifiers, add_buff, get_buff, get_buffs, has_cc, recalculate_stats,
    register_stat_source,
};
use lov_sim::combat::damage::{DamageOpts, HealOpts, apply_damage, apply_heal};
use lov_sim::systems::buffs::buffs_system;
use lov_sim::{CreateChampion, Entity, System, World, create_champion};

/// `setup()` du TS ; `name` identifie le champion (voir le test des sources de stats).
fn setup_named(player_id: &str) -> (World, Vec<System>, EntityId) {
    let mut world = World::new(1);
    let mut systems = vec![buffs_system()];
    let def = get_champion("grondak").unwrap();
    let e = create_champion(
        &mut world,
        CreateChampion {
            player_id: player_id.into(),
            name: "Test".into(),
            team: Team::Blue,
            champion_id: "grondak".into(),
            summoners: [SummonerId::Flash, SummonerId::Ignite],
            pos: v(1000.0, 1000.0),
        },
        &def.base_stats,
    );
    // Le TS branche les hooks à la première `addBuff` ; ici `init` les branche une fois.
    world.init(&mut systems);
    (world, systems, e)
}

fn setup() -> (World, Vec<System>, EntityId) {
    setup_named("p")
}

fn opts(duration: f64, source: Option<EntityId>, amount: Option<f64>) -> BuffOptions {
    let mut o = BuffOptions::new(duration);
    o.source = source;
    o.params.amount = amount;
    o
}

/// `applyShield` de `abilities/effects.ts` (le wrapper de sorts, pas celui de `combat/damage`).
fn apply_shield_wrapped(
    world: &mut World,
    source: EntityId,
    target: EntityId,
    amount: f64,
    duration: Option<f64>,
    buff_id: &str,
) -> f64 {
    let mut o = ShieldOptions::new(source, target, amount);
    o.duration = duration;
    o.buff_id = Some(buff_id.to_owned());
    apply_spell_shield(world, o)
}

fn step(world: &mut World, systems: &mut [System]) {
    world.step(systems);
}

fn step_dt(world: &mut World, systems: &mut [System], dt: f64) {
    world.step_dt(systems, dt);
}

fn close(a: f64, b: f64) {
    // `toBeCloseTo` de vitest : 2 décimales.
    assert!((a - b).abs() < 0.005, "{a} != {b}");
}

// ---- Buffs and damage contract ----

#[test]
fn only_applies_the_strongest_slow_and_restores_base_stats_on_expiry() {
    let (mut world, mut sys, e) = setup();
    add_buff(&mut world, e, "slow", opts(1.0, Some(1), Some(0.2)));
    add_buff(&mut world, e, "slow", opts(0.5, Some(2), Some(0.5)));
    assert_eq!(world.get(e).unwrap().stats.move_speed, 335.0 * 0.5);
    step_dt(&mut world, &mut sys, 0.5);
    assert_eq!(world.get(e).unwrap().stats.move_speed, 335.0 * 0.8);
    step_dt(&mut world, &mut sys, 0.5);
    assert_eq!(world.get(e).unwrap().stats.move_speed, 335.0);
}

#[test]
fn reduces_cc_with_tenacity_except_knockups_and_suppressions() {
    let (mut world, mut sys, e) = setup();
    let mut cleanse = BuffOptions::new(3.0);
    cleanse.params.tenacity = Some(0.65);
    add_buff(&mut world, e, "cleanse", cleanse);
    add_buff(&mut world, e, "stun", BuffOptions::new(2.0));
    add_buff(&mut world, e, "knockup", BuffOptions::new(2.0));
    close(
        get_buff(world.get(e).unwrap(), "stun", None)
            .unwrap()
            .remaining,
        0.7,
    );
    assert_eq!(
        get_buff(world.get(e).unwrap(), "knockup", None)
            .unwrap()
            .remaining,
        2.0
    );
    step_dt(&mut world, &mut sys, 1.0);
    assert!(!has_cc(world.get(e).unwrap(), CcType::Stun));
    assert!(has_cc(world.get(e).unwrap(), CcType::Knockup));
}

#[test]
fn consumes_independent_shields_by_expiry_without_removing_other_shields() {
    let (mut world, mut sys, e) = setup();
    apply_shield_wrapped(&mut world, e, e, 100.0, Some(1.0), "short");
    apply_shield_wrapped(&mut world, e, e, 200.0, Some(3.0), "long");
    apply_damage(
        &mut world,
        DamageOpts::new(Some(e), e, 150.0, DamageType::True),
    );
    assert_eq!(world.get(e).unwrap().shield, 150.0);
    step_dt(&mut world, &mut sys, 1.0);
    assert_eq!(world.get(e).unwrap().shield, 150.0);
    step_dt(&mut world, &mut sys, 2.0);
    assert_eq!(world.get(e).unwrap().shield, 0.0);
    assert_eq!(world.get(e).unwrap().hp, 680.0);
}

#[test]
fn applies_grievous_wounds_mitigation_and_emits_a_single_death() {
    let (mut world, _sys, e) = setup();
    world.get_mut(e).unwrap().hp = 200.0;
    let mut g = BuffOptions::new(5.0);
    g.params.grievous = Some(0.4);
    add_buff(&mut world, e, "grievous", g);
    assert_eq!(
        apply_heal(&mut world, HealOpts::new(Some(e), e, 100.0)),
        60.0
    );
    close(
        apply_damage(
            &mut world,
            DamageOpts::new(Some(e), e, 138.0, DamageType::Physical),
        ),
        100.0,
    );
    apply_damage(
        &mut world,
        DamageOpts::new(Some(e), e, 10000.0, DamageType::True),
    );
    apply_damage(
        &mut world,
        DamageOpts::new(Some(e), e, 10000.0, DamageType::True),
    );
    let deaths = world
        .events
        .pending
        .iter()
        .filter(|ev| matches!(ev, GameEvent::Death { .. }))
        .count();
    assert_eq!(deaths, 1);
}

#[test]
fn recalculates_items_and_hooked_stats_without_accumulating_each_tick() {
    // La source de stats est globale au processus (comme le `Set` de module du TS) : elle est limitée
    // à ce champion par son `player_id` pour ne pas toucher les mondes des autres tests en parallèle.
    let (mut world, mut sys, e) = setup_named("stat-source-test");
    world.get_mut(e).unwrap().champ.as_mut().unwrap().items[0] = Some("long_sword".into());
    let handle = register_stat_source(|_, e| {
        let mine = e
            .champ
            .as_ref()
            .is_some_and(|c| c.player_id == "stat-source-test");
        mine.then(|| StatModifiers {
            flat: PartialStats {
                ad: Some(20.0),
                ..PartialStats::default()
            },
            percent: PartialStats {
                ad: Some(0.1),
                ..PartialStats::default()
            },
            tenacity: None,
        })
    });
    recalculate_stats(&mut world, e);
    let expected = (64.0 + 10.0 + 20.0) * 1.1;
    assert_eq!(world.get(e).unwrap().stats.ad, expected);
    step(&mut world, &mut sys);
    step(&mut world, &mut sys);
    assert_eq!(world.get(e).unwrap().stats.ad, expected);
    handle.unsubscribe();
}

// ---- Core-server integration ----

#[test]
fn slows_non_champion_units_without_accumulating_modifiers_or_altering_their_base_stats() {
    let (mut world, mut sys, e) = setup();
    let mut stats = world.get(e).unwrap().stats.clone();
    stats.max_hp = 100.0;
    stats.move_speed = 300.0;
    let minion = world.spawn(Entity::new(
        EntityKind::Minion,
        TeamOrNeutral::Red,
        v(2000.0, 1000.0),
        30.0,
        100.0,
        0.0,
        stats,
    ));
    add_buff(&mut world, minion, "slow", opts(1.0, None, Some(0.4)));
    assert_eq!(world.get(minion).unwrap().stats.move_speed, 180.0);
    step_dt(&mut world, &mut sys, 0.5);
    assert_eq!(world.get(minion).unwrap().stats.move_speed, 180.0);
    step_dt(&mut world, &mut sys, 0.5);
    assert_eq!(world.get(minion).unwrap().stats.move_speed, 300.0);
}

#[test]
fn registers_cc_and_stasis_with_the_server_movement_attack_targeting_hooks() {
    let (mut world, _sys, e) = setup();
    add_buff(&mut world, e, "root", BuffOptions::new(1.0));
    assert!(!world.can_move(e));
    assert!(world.can_attack(e));
    world.get_mut(e).unwrap().data.insert(Stasis);
    assert!(!world.is_targetable(e, e));
    assert_eq!(
        apply_damage(
            &mut world,
            DamageOpts::new(Some(e), e, 100.0, DamageType::True)
        ),
        0.0
    );
}

#[test]
fn allows_taunted_units_to_attack_only_their_forced_target() {
    let (mut world, mut sys, e) = setup();
    let stats = world.get(e).unwrap().stats.clone();
    let taunter = world.spawn(Entity::new(
        EntityKind::Champion,
        TeamOrNeutral::Red,
        v(1300.0, 1000.0),
        65.0,
        100.0,
        0.0,
        stats,
    ));
    let mut o = BuffOptions::new(1.0);
    o.source = Some(taunter);
    add_buff(&mut world, e, "taunt", o);
    step(&mut world, &mut sys);
    assert_eq!(world.get(e).unwrap().attack_target, Some(taunter));
    assert!(world.can_attack(e));
}

#[test]
fn keeps_an_anonymous_buff_independent_from_a_sourced_buff_with_the_same_id() {
    let (mut world, mut sys, e) = setup();
    add_buff(&mut world, e, "slow", opts(2.0, Some(42), Some(0.3)));
    add_buff(&mut world, e, "slow", opts(1.0, None, Some(0.5)));
    let slows = get_buffs(world.get(e).unwrap())
        .iter()
        .filter(|b| b.id == "slow")
        .count();
    assert_eq!(slows, 2);
    close(world.get(e).unwrap().stats.move_speed, 335.0 * 0.5);
    step_dt(&mut world, &mut sys, 1.0);
    close(world.get(e).unwrap().stats.move_speed, 335.0 * 0.7);
}
