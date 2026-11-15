//! Tests du moteur de sorts : port de `packages/server/test/abilities.test.ts`.
//!
//! Les six tests du TS sont portés avec les mêmes assertions et seuils. Ils utilisent `game('ilyra')`
//! (et, pour la file d'attente, `grondak`), mais aucun n'exerce une règle de kit : ils éprouvent le
//! moteur (validation, balayage, collisions, zones, liens, files d'attente). Les kits de champions ne
//! sont pas portés ; ces tests enregistrent donc des kits de test (`setup`) :
//!
//! - `ilyra` : la fiche de `packages/shared/src/data/champions/ilyra.ts` (statistiques de base, croissance
//!   par niveau, et pour chaque sort id, emplacement, ciblage, portée, largeur, vitesse, temps
//!   d'incantation, recharge, coût, dégâts, clé FX, paramètres ; les textes, la palette et la fiche de
//!   lore ne servent à rien ici et sont omis) enregistrée par `register_champion_def`, car `lov-shared`
//!   ne la contient pas encore ; son `Q` est celui d'`ilyra.ts` (`line` + `hit` + ralentissement), `E` est
//!   ciblé sur un allié, W et R ne font rien (aucun test ne les lance) ;
//! - `grondak` : la fiche est celle de `lov-shared` ; son kit ne fait rien (le `E` de Grondak lance un
//!   dash que ce test n'observe pas : il vérifie la file d'attente, la dépense et la recharge).
//!   Quand les kits arrivent, ces enregistrements sont remplacés par les vrais.
//!
//! Le champion cible est `tharox` au lieu de `sable` (voir `tests/common/champion.rs`).
//!
//! Limite : `combat/buffs` (déjà porté) lit les fiches dans `lov-shared` seulement ; les stats d'Ilyra
//! ne montent donc pas de niveau ici (elles restent celles du niveau 1) et ne sont recalculées qu'en
//! présence d'un buff (voir le test de la hâte). Aucun test n'en dépend.

mod common;

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, Once};

use common::champion::{Game, game};
use common::v;
use lov_shared::protocol::ClientMsg;
use lov_shared::schema::{
    AbilityDamage, AbilityDef, AbilityDefSlot, AbilityParam, CcType, ChampionClass, ChampionDef,
    Palette, Resource, Scaling, Targeting,
};
use lov_shared::types::{
    AbilityMap, AbilitySlot, DamageType, EntityId, EntityKind, PartialStats, Role, Stats,
};
use lov_sim::abilities::engine::ability_state;
use lov_sim::abilities::prelude::*;
use lov_sim::abilities::registry::{register_champion_def, register_kit};
use lov_sim::combat::buffs::{BuffOptions, Casting, StatModifiers, add_buff, register_stat_source};
use lov_sim::combat::damage::{DamageOpts, apply_damage};

// ---- Fiches et kits de test ----

fn params(
    list: &[(&str, AbilityParam)],
) -> Option<std::collections::BTreeMap<String, AbilityParam>> {
    Some(
        list.iter()
            .map(|(k, p)| ((*k).to_owned(), p.clone()))
            .collect(),
    )
}

fn base_ability(id: &str, slot: AbilityDefSlot, targeting: Targeting) -> AbilityDef {
    AbilityDef {
        id: id.into(),
        slot,
        name: id.into(),
        description: String::new(),
        targeting,
        max_rank: match slot {
            AbilityDefSlot::R => 3,
            AbilityDefSlot::P => 0,
            _ => 5,
        },
        cooldown: vec![0.0],
        cost: vec![0.0],
        range: 0.0,
        radius: None,
        width: None,
        speed: None,
        cast_time: None,
        damage: None,
        fx_key: id.into(),
        params: None,
    }
}

fn magic(base: &[f64], ap: f64) -> Option<AbilityDamage> {
    Some(AbilityDamage {
        scaling: Scaling {
            base: base.to_vec(),
            ad: None,
            bonus_ad: None,
            ap: Some(ap),
            max_hp: None,
            bonus_hp: None,
            armor: None,
            target_max_hp: None,
            target_missing_hp: None,
        },
        damage_type: DamageType::Magic,
    })
}

fn one(v: f64) -> AbilityParam {
    AbilityParam::One(v)
}

/// `packages/shared/src/data/champions/ilyra.ts`.
fn ilyra_def() -> ChampionDef {
    use AbilityDefSlot::{E, P, Q, R, W};
    ChampionDef {
        id: "ilyra".into(),
        name: "Ilyra".into(),
        title: "la Sentinelle du Givre".into(),
        class: ChampionClass::Mage,
        roles: vec![Role::Mid],
        difficulty: 2,
        ranged: true,
        resource: Resource::Mana,
        base_stats: Stats {
            max_hp: 560.0,
            hp_regen: 8.0,
            max_mana: 420.0,
            mana_regen: 9.0,
            ad: 52.0,
            ap: 0.0,
            armor: 22.0,
            mr: 30.0,
            attack_speed: 0.64,
            attack_range: 550.0,
            move_speed: 330.0,
            crit_chance: 0.0,
            ability_haste: 0.0,
            lifesteal: 0.0,
            armor_pen: 0.0,
            magic_pen: 0.0,
        },
        per_level: PartialStats {
            max_hp: Some(95.0),
            max_mana: Some(30.0),
            mana_regen: Some(0.8),
            ad: Some(3.0),
            armor: Some(4.4),
            mr: Some(1.3),
            attack_speed: Some(0.015),
            ..PartialStats::default()
        },
        attack_windup: 0.3,
        projectile_speed: Some(1600.0),
        passive: AbilityDef {
            params: params(&[
                ("stacks", one(3.0)),
                ("markDuration", one(4.0)),
                ("stun", one(1.0)),
                ("perTargetCooldown", one(8.0)),
            ]),
            ..base_ability("ilyra_p", P, Targeting::SelfCast)
        },
        abilities: AbilityMap {
            q: AbilityDef {
                range: 950.0,
                width: Some(100.0),
                speed: Some(1700.0),
                cast_time: Some(0.25),
                cooldown: vec![6.0, 5.5, 5.0, 4.5, 4.0],
                cost: vec![50.0, 55.0, 60.0, 65.0, 70.0],
                damage: magic(&[80.0, 120.0, 160.0, 200.0, 240.0], 0.7),
                params: params(&[("slow", one(0.3)), ("slowDuration", one(1.5))]),
                ..base_ability("ilyra_q", Q, Targeting::Skillshot)
            },
            w: AbilityDef {
                range: 850.0,
                radius: Some(280.0),
                cast_time: Some(0.25),
                cooldown: vec![10.0, 9.5, 9.0, 8.5, 8.0],
                cost: vec![70.0],
                damage: magic(&[70.0, 110.0, 150.0, 190.0, 230.0], 0.6),
                params: params(&[
                    ("delay", one(0.6)),
                    ("slow", one(0.5)),
                    ("slowDuration", one(2.0)),
                ]),
                ..base_ability("ilyra_w", W, Targeting::Ground)
            },
            e: AbilityDef {
                range: 700.0,
                cooldown: vec![16.0, 15.0, 14.0, 13.0, 12.0],
                cost: vec![60.0],
                params: params(&[
                    (
                        "shield",
                        AbilityParam::Many(vec![70.0, 105.0, 140.0, 175.0, 210.0]),
                    ),
                    ("shieldAp", one(0.5)),
                    ("duration", one(2.5)),
                    ("slow", one(0.2)),
                    ("slowDuration", one(1.0)),
                ]),
                ..base_ability("ilyra_e", E, Targeting::Targeted)
            },
            r: AbilityDef {
                range: 900.0,
                radius: Some(500.0),
                cast_time: Some(0.25),
                cooldown: vec![120.0, 100.0, 80.0],
                cost: vec![100.0],
                damage: magic(&[60.0, 90.0, 120.0], 0.25),
                params: params(&[
                    ("duration", one(4.0)),
                    ("tickRate", one(1.0)),
                    ("maxSlow", one(0.6)),
                ]),
                ..base_ability("ilyra_r", R, Targeting::Ground)
            },
        },
        palette: Palette {
            primary: "#8fd3ff".into(),
            secondary: "#1e3a5f".into(),
            accent: "#e9fbff".into(),
        },
        lore: String::new(),
    }
}

/// `Q` d'`ilyra.ts` : `line(ctx, target => { hit; cc slow })`.
fn ilyra_q(world: &mut World, ctx: &CastContext) {
    let ctx = *ctx;
    line(world, &ctx, LineOptions::default(), move |world, target| {
        hit(world, &ctx, target);
        cc(
            world,
            &ctx,
            target,
            CcType::Slow,
            param(&ctx, "slowDuration"),
            Some(param(&ctx, "slow")),
        );
    });
}

fn nothing(_: &mut World, _: &CastContext) {}

// Données et crochets du kit de test d'Ilyra, pour les tests propres au portage Rust (crochets de kit).
/// Marque une cible dont les dégâts sont divisés par deux par le crochet installé par le kit.
#[derive(Debug, Clone, Copy)]
struct Halved;
/// Nombre de `update_champion` reçus.
#[derive(Debug, Clone, Copy, Default)]
struct ChampionTicks(u32);
/// Nombre de `update_entity` reçus.
#[derive(Debug, Clone, Copy, Default)]
struct EntityTicks(u32);
/// Nombre de `reset_dead` reçus.
#[derive(Debug, Clone, Copy, Default)]
struct DeadTicks(u32);

fn install_halving(world: &mut World) {
    world.hooks.before_damage.push(Box::new(|world, ctx| {
        if world
            .get(ctx.target)
            .is_some_and(|t| t.data.contains::<Halved>())
        {
            ctx.multiplier *= 0.5;
        }
    }));
}

fn count_champion(world: &mut World, id: EntityId, _dt: f64) {
    if let Some(e) = world.get_mut(id) {
        e.data.get_or_insert_with(ChampionTicks::default).0 += 1;
    }
}

fn count_entity(world: &mut World, id: EntityId, _dt: f64) {
    if let Some(e) = world.get_mut(id) {
        e.data.get_or_insert_with(EntityTicks::default).0 += 1;
    }
}

fn count_dead(world: &mut World, id: EntityId) {
    if let Some(e) = world.get_mut(id) {
        e.data.get_or_insert_with(DeadTicks::default).0 += 1;
    }
}

static SETUP: Once = Once::new();

/// Enregistre les kits et la fiche de test, une fois par processus.
fn setup() {
    SETUP.call_once(|| {
        register_champion_def(ilyra_def());
        register_kit(
            Kit::new("ilyra", [ilyra_q, nothing, nothing, nothing])
                .with_targeting([
                    TargetRule::Enemy,
                    TargetRule::Enemy,
                    TargetRule::AllyChampion,
                    TargetRule::Enemy,
                ])
                .with_install(install_halving)
                .with_update_champion(count_champion)
                .with_update_entity(count_entity)
                .with_reset_dead(count_dead),
        );
        register_kit(Kit::new("grondak", [nothing; 4]));
    });
}

fn new_game(id: &str) -> Game {
    setup();
    game(id)
}

fn count(g: &Game, kind: EntityKind) -> usize {
    g.world.query(Some(kind)).count()
}

type Seen = Arc<Mutex<Vec<EntityId>>>;

fn recorder() -> (Seen, Seen) {
    let a = Arc::new(Mutex::new(Vec::new()));
    (Arc::clone(&a), a)
}

fn seen_of(list: &Seen) -> Vec<EntityId> {
    list.lock().map(|l| l.clone()).unwrap_or_default()
}

// ---- Ability engine ----

#[test]
fn validates_ranks_resources_cc_finite_positions_and_cooldowns() {
    let mut g = new_game("ilyra");
    let (source, target) = (g.source, g.target);
    g.e_mut(source).champ.as_mut().unwrap().ability_levels.q = 0;
    assert!(!g.cast(AbilitySlot::Q, Some(target), None));
    g.e_mut(source).champ.as_mut().unwrap().ability_levels.q = 1;
    g.e_mut(source).mana = 0.0;
    assert!(!g.cast(AbilitySlot::Q, Some(target), None));
    g.e_mut(source).mana = 100.0;
    add_buff(&mut g.world, source, "silence", BuffOptions::new(0.1));
    assert!(!g.cast(AbilitySlot::Q, Some(target), None));
    g.advance(0.1);
    assert!(!g.cast(AbilitySlot::Q, None, Some(v(f64::NAN, 0.0))));
    assert!(g.cast(AbilitySlot::Q, Some(target), None));
    assert_eq!(g.e(source).mana, 50.0);
    assert!(!g.cast(AbilitySlot::Q, Some(target), None));
}

#[test]
fn uses_swept_collision_and_the_closest_enemy_regardless_of_insertion_order() {
    let mut g = new_game("ilyra");
    let (source, target) = (g.source, g.target);
    g.e_mut(target).pos = v(1600.0, 1000.0);
    let close = g.dummy(v(1300.0, 1000.0), |_| {});
    let (seen, sink) = recorder();
    spawn_projectile(
        &mut g.world,
        ProjectileOptions::new(
            source,
            v(2000.0, 1000.0),
            100000.0,
            10.0,
            "test",
            move |_, e, _| sink.lock().unwrap().push(e),
        ),
    );
    g.step_dt(0.1);
    assert_eq!(seen_of(&seen), vec![close]);
    assert_eq!(count(&g, EntityKind::Projectile), 0);
}

#[test]
fn pierces_once_per_target_and_misses_enemies_outside_the_width() {
    let mut g = new_game("ilyra");
    let (source, target) = (g.source, g.target);
    let far = g.dummy(v(1600.0, 1000.0), |_| {});
    g.dummy(v(1200.0, 1200.0), |_| {});
    let (seen, sink) = recorder();
    let mut options = ProjectileOptions::new(
        source,
        v(2000.0, 1000.0),
        500.0,
        20.0,
        "test",
        move |_, e, _| sink.lock().unwrap().push(e),
    );
    options.pierce = true;
    spawn_projectile(&mut g.world, options);
    g.advance(2.1);
    assert_eq!(seen_of(&seen), vec![target, far]);
}

#[test]
fn queues_a_targeted_cast_until_in_range_and_cancels_on_a_new_move_order() {
    let mut g = new_game("grondak");
    let (source, target) = (g.source, g.target);
    g.e_mut(target).pos.x = 2200.0;
    assert!(!g.cast(AbilitySlot::E, Some(target), None));
    assert!(ability_state(g.e_mut(source)).pending.is_some());
    let max_mana = g.e(source).stats.max_mana;
    assert!((g.e(source).mana - max_mana).abs() < 0.005);
    g.advance(2.0);
    assert!(g.e(source).champ.as_ref().unwrap().cooldowns.e > 0.0);
    g.e_mut(source).champ.as_mut().unwrap().cooldowns.e = 0.0;
    g.e_mut(target).pos.x = 4000.0;
    g.cast(AbilitySlot::E, Some(target), None);
    g.command(
        "p1",
        ClientMsg::Move {
            pos: v(1000.0, 1000.0),
        },
    );
    g.step();
    assert!(ability_state(g.e_mut(source)).pending.is_none());
}

#[test]
fn interrupts_windups_applies_haste_once_and_prevents_rooted_dashes() {
    let mut g = new_game("ilyra");
    let (source, target) = (g.source, g.target);
    // Les sources de stats sont globales au processus et les ids d'entité se répètent d'un monde à
    // l'autre : on identifie aussi le monde par son adresse (`g` ne bouge plus après cette ligne).
    let world_addr = &g.world as *const World as usize;
    let handle = register_stat_source(move |w, e| {
        (w as *const World as usize == world_addr && e.id == source).then(|| StatModifiers {
            flat: PartialStats {
                ability_haste: Some(100.0),
                ..PartialStats::default()
            },
            ..StatModifiers::default()
        })
    });
    // Écart avec le TS : `combat/buffs` ne lit que les fiches de `lov-shared` (celle d'Ilyra est
    // enregistrée à l'exécution) et ne recalcule donc les stats d'un tel champion qu'en présence d'un
    // buff. Un buff sans effet force le recalcul que le `world.step()` du TS déclenchait seul.
    add_buff(&mut g.world, source, "test_noop", BuffOptions::new(60.0));
    g.step();
    assert!(g.cast(AbilitySlot::Q, Some(target), None));
    assert_eq!(g.e(source).champ.as_ref().unwrap().cooldowns.q, 3.0);
    add_buff(&mut g.world, source, "stun", BuffOptions::new(1.0));
    g.advance(0.4);
    assert_eq!(count(&g, EntityKind::Projectile), 0);
    assert!(!g.e(source).data.contains::<Casting>());
    handle.unsubscribe();
}

#[test]
fn ticks_zones_the_exact_number_of_times_and_breaks_tethers_on_distance() {
    let mut g = new_game("ilyra");
    let (source, target) = (g.source, g.target);
    let (ticks, complete) = (Arc::new(AtomicU32::new(0)), Arc::new(AtomicU32::new(0)));
    let (t, c) = (Arc::clone(&ticks), Arc::clone(&complete));
    let pos = g.e(target).pos;
    let mut zone = ZoneOptions::new(source, pos, 200.0, 3.0, "test", move |_, _, _| {
        t.fetch_add(1, Ordering::Relaxed);
    });
    zone.interval = Some(1.0);
    spawn_zone(&mut g.world, zone);
    tether(&mut g.world, source, target, 1.0, 500.0, move |_| {
        c.fetch_add(1, Ordering::Relaxed);
    });
    g.advance(0.5);
    g.e_mut(target).pos.x = 3000.0;
    g.advance(3.0);
    assert_eq!(ticks.load(Ordering::Relaxed), 3);
    assert_eq!(complete.load(Ordering::Relaxed), 0);
    assert_eq!(count(&g, EntityKind::Zone), 0);
}

// ---- Tests propres au portage Rust (primitives et crochets de kit, absents du TS) ----

#[test]
fn schedule_runs_due_tasks_in_time_order_latest_scheduled_first_on_ties_and_honours_cancel() {
    let mut g = new_game("ilyra");
    let source = g.source;
    let log = Arc::new(Mutex::new(Vec::new()));
    let push = |n: u32| {
        let log = Arc::clone(&log);
        move |_: &mut World| log.lock().unwrap().push(n)
    };
    schedule(&mut g.world, source, 0.1, push(1));
    schedule(&mut g.world, source, 0.1, push(2));
    let cancelled = schedule(&mut g.world, source, 0.1, push(3));
    schedule(&mut g.world, source, 0.05, push(4));
    cancelled.cancel();
    g.advance(0.2);
    assert_eq!(*log.lock().unwrap(), vec![4, 2, 1]);
}

#[test]
fn a_targeted_ally_spell_without_a_target_falls_back_to_the_caster() {
    let mut g = new_game("ilyra");
    let source = g.source;
    assert!(g.cast(AbilitySlot::E, None, Some(v(1000.0, 1000.0))));
    let c = g.e(source).champ.as_ref().unwrap();
    assert!(c.cooldowns.e > 0.0);
}

#[test]
fn kit_hooks_install_update_champion_update_entity_and_reset_dead() {
    let mut g = new_game("ilyra");
    let (source, target) = (g.source, g.target);
    let d = g.dummy(v(1400.0, 1000.0), |_| {});
    g.e_mut(d).data.insert(Halved);
    let mut opts = DamageOpts::new(Some(source), d, 100.0, DamageType::True);
    opts.ability = Some("test".into());
    assert_eq!(apply_damage(&mut g.world, opts), 50.0);
    g.step();
    g.step();
    // Seul le champion du kit reçoit `update_champion` ; toute entité vivante reçoit `update_entity`.
    assert_eq!(
        g.e(source).data.get::<ChampionTicks>().map(|t| t.0),
        Some(2)
    );
    assert!(g.e(target).data.get::<ChampionTicks>().is_none());
    assert_eq!(g.e(d).data.get::<EntityTicks>().map(|t| t.0), Some(2));
    g.e_mut(target).dead = true;
    g.step();
    assert_eq!(g.e(target).data.get::<DeadTicks>().map(|t| t.0), Some(1));
    assert!(g.e(target).data.get::<EntityTicks>().map(|t| t.0) == Some(2));
    assert_eq!(g.e(d).data.get::<DeadTicks>().map(|t| t.0), None);
}

#[test]
fn a_dash_stops_on_first_hit_and_reports_hit_and_end() {
    let mut g = new_game("ilyra");
    let (source, target) = (g.source, g.target);
    let (hits, sink) = recorder();
    let ended = Arc::new(AtomicU32::new(0));
    let e = Arc::clone(&ended);
    let mut dash = DashOptions::new("test", 1200.0);
    dash.stop_on_first = true;
    dash.width = Some(40.0);
    dash.on_hit = Some(Box::new(move |_, t| sink.lock().unwrap().push(t)));
    dash.on_end = Some(Box::new(move |_| {
        e.fetch_add(1, Ordering::Relaxed);
    }));
    start_dash(&mut g.world, source, v(1600.0, 1000.0), dash);
    g.advance(1.0);
    assert_eq!(seen_of(&hits), vec![target]);
    assert_eq!(ended.load(Ordering::Relaxed), 1);
    // Contact à 1250 - (40 / 2 + 65) = 1165 ; la séparation douce entre champions (movement) recule
    // ensuite un peu le lanceur, qui reste bien avant la cible.
    let x = g.e(source).pos.x;
    assert!(x > 1100.0 && x < 1170.0, "{x}");
    assert!(
        !g.e(source)
            .data
            .contains::<lov_sim::systems::movement::DashData>()
    );
}
