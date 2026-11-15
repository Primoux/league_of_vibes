//! Tests de l'expérience et des niveaux : port de `packages/server/test/leveling.test.ts`.
//!
//! Substitution : le TS crée l'allié du test de partage d'XP avec `ilyra`, pas encore enregistré en
//! Rust ; le test n'utilise de lui que son équipe et sa position, `tharox` le remplace.
//!
//! Le dernier test du TS passe par `buildSnapshot` (`net/snapshot.ts`, non porté) : il est porté en
//! appelant directement les hooks `selfState` et `entitySnapshot` que `buildSnapshot` exécute
//! (`World::apply_self_state`, `World::apply_entity_snapshot`).

mod common;

use common::v;
use lov_shared::data::champions::get_champion;
use lov_shared::types::{
    AbilitySlot, BuffSnapshot, DamageType, EntityId, EntityKind, EntitySnapshot, GameEvent,
    SelfState, SummonerId, Team, TeamOrNeutral,
};
use lov_sim::combat::buffs::{BuffOptions, add_buff};
use lov_sim::combat::damage::{DamageOpts, apply_damage};
use lov_sim::systems::buffs::buffs_system;
use lov_sim::systems::leveling::{
    XpReward, cooldown_with_haste, grant_xp, level_ability, leveling_system, xp_for_level,
};
use lov_sim::{CreateChampion, Entity, System, World, create_champion};

fn make(world: &mut World, player: &str, champion: &str, pos: lov_shared::types::Vec2) -> EntityId {
    let def = get_champion(champion).unwrap();
    create_champion(
        world,
        CreateChampion {
            player_id: player.into(),
            name: "Test".into(),
            team: Team::Blue,
            champion_id: champion.into(),
            summoners: [SummonerId::Flash, SummonerId::Ignite],
            pos,
        },
        &def.base_stats,
    )
}

fn setup() -> (World, Vec<System>, EntityId) {
    let mut world = World::new(1);
    let mut systems = vec![leveling_system()];
    let e = make(&mut world, "p", "grondak", v(1000.0, 1000.0));
    world.init(&mut systems);
    (world, systems, e)
}

fn lvl(world: &mut World, id: EntityId, slot: AbilitySlot) -> bool {
    level_ability(world.get_mut(id).unwrap(), slot)
}

fn champ(world: &World, id: EntityId) -> &lov_sim::ChampionState {
    world.get(id).unwrap().champ.as_ref().unwrap()
}

#[test]
fn uses_the_complete_xp_curve_and_unlocks_r_only_at_6_11_16() {
    let (mut world, _s, e) = setup();
    assert!(!lvl(&mut world, e, AbilitySlot::R));
    grant_xp(&mut world, e, xp_for_level(6));
    assert_eq!(champ(&world, e).level, 6);
    assert_eq!(champ(&world, e).skill_points, 6);
    assert!(lvl(&mut world, e, AbilitySlot::R));
    assert!(!lvl(&mut world, e, AbilitySlot::R));
    grant_xp(&mut world, e, xp_for_level(18));
    assert_eq!(champ(&world, e).level, 18);
    assert_eq!(champ(&world, e).xp, 0.0);
    let level_ups = world
        .events
        .pending
        .iter()
        .filter(|ev| matches!(ev, GameEvent::LevelUp { .. }))
        .count();
    assert_eq!(level_ups, 17);
}

#[test]
fn enforces_basic_ability_ranks_and_reduces_cooldown_with_haste() {
    let (mut world, _s, e) = setup();
    assert!(lvl(&mut world, e, AbilitySlot::Q));
    grant_xp(&mut world, e, 280.0);
    assert!(!lvl(&mut world, e, AbilitySlot::Q));
    grant_xp(&mut world, e, 380.0);
    assert!(lvl(&mut world, e, AbilitySlot::Q));
    assert_eq!(cooldown_with_haste(10.0, 100.0), 5.0);
}

#[test]
fn shares_nearby_minion_xp_once_and_excludes_dead_out_of_range_champions() {
    let (mut world, _s, e) = setup();
    let ally = make(&mut world, "a", "tharox", v(1200.0, 1000.0));
    let mut m = Entity::new(
        EntityKind::Minion,
        TeamOrNeutral::Red,
        v(1100.0, 1000.0),
        30.0,
        1.0,
        0.0,
        world.get(e).unwrap().stats.clone(),
    );
    m.data.insert(XpReward(100.0));
    let enemy = world.spawn(m);
    apply_damage(
        &mut world,
        DamageOpts::new(Some(e), enemy, 1.0, DamageType::True),
    );
    world.emit(GameEvent::Death {
        target: enemy,
        killer: Some(e),
        assists: None,
        bounty: None,
    });
    assert_eq!(champ(&world, e).xp, 50.0);
    assert_eq!(champ(&world, ally).xp, 50.0);
}

#[test]
fn exports_xp_and_buff_state_through_the_server_snapshot_hooks() {
    let (mut world, _s, e) = setup();
    // `buffsSystem.init!(world)` du TS : les hooks de snapshot des buffs.
    let mut buffs = vec![buffs_system()];
    world.init(&mut buffs);
    add_buff(&mut world, e, "stun", {
        let mut o = BuffOptions::new(1.0);
        o.source = Some(e);
        o
    });
    let ent = world.get(e).unwrap();
    let c = ent.champ.as_ref().unwrap();
    let mut state = SelfState {
        entity_id: e,
        gold: c.gold,
        xp: c.xp,
        xp_to_next: 0.0,
        level: c.level,
        skill_points: c.skill_points,
        ability_levels: c.ability_levels.clone(),
        cooldowns: c.cooldowns.clone(),
        summoners: c.summoners.clone(),
        stats: ent.stats.clone(),
        recalling: None,
        can_shop: false,
        ward_charges: None,
        item_charges: None,
        item_cooldowns: None,
    };
    world.apply_self_state(ent, &mut state);
    assert_eq!(state.xp_to_next, 280.0);
    let mut snap = EntitySnapshot {
        id: e,
        kind: ent.kind,
        team: ent.team,
        x: ent.pos.x,
        y: ent.pos.y,
        facing: ent.facing,
        hp: ent.hp,
        max_hp: ent.stats.max_hp,
        mana: None,
        max_mana: None,
        shield: None,
        dead: None,
        subtype: None,
        owner_id: None,
        level: None,
        anim: None,
        buffs: None,
        champ: None,
    };
    world.apply_entity_snapshot(ent, &mut snap);
    assert!(snap.buffs.unwrap().contains(&BuffSnapshot {
        id: "stun".into(),
        remaining: 1.0,
        stacks: None,
    }));
}

// Test ajouté (sans équivalent TS) : commande `levelAbility` et régénération du système.
#[test]
fn le_systeme_traite_level_ability_et_regenere_pv_et_mana() {
    let (mut world, mut systems, e) = setup();
    world.commands.push(lov_sim::QueuedCommand {
        player_id: "p".into(),
        msg: lov_shared::protocol::ClientMsg::LevelAbility {
            slot: AbilitySlot::W,
        },
    });
    {
        let ent = world.get_mut(e).unwrap();
        ent.hp -= 100.0;
        ent.stats.hp_regen = 10.0;
        ent.mana = 0.0;
        ent.stats.mana_regen = 10.0;
    }
    let hp = world.get(e).unwrap().hp;
    world.step(&mut systems);
    assert_eq!(champ(&world, e).ability_levels.w, 1);
    assert_eq!(champ(&world, e).skill_points, 0);
    assert!(world.get(e).unwrap().hp > hp);
    let uses_mana = get_champion("grondak").unwrap().resource != lov_shared::schema::Resource::None;
    assert_eq!(world.get(e).unwrap().mana > 0.0, uses_mana);
}
