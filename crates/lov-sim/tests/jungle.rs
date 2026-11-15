//! Tests de la jungle : port de `packages/server/test/jungle.test.ts` et `jungle.bugs.test.ts`.
//!
//! `getCampTimers(world)` devient `JungleCamps::timers()`, la poignée renvoyée par
//! `jungle_system_with_camps`.

mod common;

use common::{champ_as, v};
use lov_shared::constants::{TIMERS, scaled};
use lov_shared::map::CAMPS;
use lov_shared::schema::CcType;
use lov_shared::types::{
    DamageType, EntityId, EntityKind, GameEvent, MonsterType, SummonerId, Team, TeamOrNeutral, Vec2,
};
use lov_sim::combat::buffs::{get_buff, has_cc};
use lov_sim::combat::damage::{DamageOpts, Owner, apply_damage};
use lov_sim::systems::buffs::buffs_system;
use lov_sim::systems::jungle::{JungleBuffs, JungleCamps, jungle_system_with_camps};
use lov_sim::systems::movement::movement_system;
use lov_sim::world_ai::burns::{BurnKey, burn};
use lov_sim::world_ai::monsters::MonsterState;
use lov_sim::world_ai::runtime::{DamageRequest, apply_damage as pve_damage, stats_with};
use lov_sim::{Entity, System, World};

struct Jungle {
    world: World,
    systems: Vec<System>,
    camps: JungleCamps,
}

impl Jungle {
    /// `world.step(dt)`.
    fn step(&mut self, dt: f64) {
        self.world.step_dt(&mut self.systems, dt);
    }

    fn e(&self, id: EntityId) -> &Entity {
        self.world.get(id).unwrap()
    }

    fn e_mut(&mut self, id: EntityId) -> &mut Entity {
        self.world.get_mut(id).unwrap()
    }

    fn monsters(&self) -> Vec<EntityId> {
        self.world
            .query(Some(EntityKind::Monster))
            .map(|e| e.id)
            .collect()
    }

    fn monster(&self, subtype: &str) -> EntityId {
        self.world
            .query(Some(EntityKind::Monster))
            .find(|e| e.subtype.as_deref() == Some(subtype))
            .map(|e| e.id)
            .expect("monstre absent")
    }

    fn camp(&self, camp: &str) -> Vec<EntityId> {
        self.world
            .query(Some(EntityKind::Monster))
            .filter(|e| {
                e.data
                    .get::<MonsterState>()
                    .is_some_and(|m| m.camp.as_deref() == Some(camp))
            })
            .map(|e| e.id)
            .collect()
    }

    fn wards(&self) -> Vec<EntityId> {
        self.world
            .query(Some(EntityKind::Ward))
            .map(|e| e.id)
            .collect()
    }
}

fn with_smite(world: &mut World, team: Team, pos: Vec2) -> EntityId {
    let id = champ_as(world, team, pos, "grondak");
    world
        .get_mut(id)
        .unwrap()
        .champ
        .as_mut()
        .unwrap()
        .summoners
        .f = SummonerId::Smite;
    id
}

fn make(first: System) -> Jungle {
    let (jungle, camps) = jungle_system_with_camps(false);
    let mut systems = vec![first, jungle];
    let mut world = World::new(1);
    world.init(&mut systems);
    Jungle {
        world,
        systems,
        camps,
    }
}

/// `setup()` de `jungle.test.ts` : mouvement et jungle, un champion bleu avec Châtiment.
fn setup() -> (Jungle, EntityId) {
    let mut j = make(movement_system());
    let c = with_smite(&mut j.world, Team::Blue, v(3700.0, 6100.0));
    (j, c)
}

fn true_hit(world: &mut World, source: EntityId, target: EntityId, amount: f64) {
    pve_damage(
        world,
        DamageRequest {
            source,
            target,
            amount,
            damage_type: DamageType::True,
            ability: None,
        },
    );
}

// ------------------------------------------------------------------------------------------------
// jungle.test.ts
// ------------------------------------------------------------------------------------------------

#[test]
fn fait_apparaitre_tous_les_camps_au_bon_moment() {
    let (mut j, _) = setup();
    j.step(scaled(TIMERS.jungle_first_spawn) - 0.1);
    assert!(j.monsters().is_empty());
    j.step(0.1);
    let expected: usize = CAMPS
        .iter()
        .filter(|c| c.monster != MonsterType::Scuttle)
        .map(|c| 1 + c.smalls.as_ref().map_or(0, Vec::len))
        .sum();
    assert_eq!(j.monsters().len(), expected);
    let until = scaled(TIMERS.scuttle_first_spawn) - j.world.time;
    j.step(until);
    let crabs = j
        .monsters()
        .into_iter()
        .filter(|&id| j.e(id).subtype.as_deref() == Some("scuttle"))
        .count();
    assert_eq!(crabs, 2);
}

#[test]
fn aggro_tout_le_camp_et_reset_hors_laisse() {
    let (mut j, c) = setup();
    j.step(scaled(TIMERS.jungle_first_spawn));
    let wolf = j.monster("wolves");
    let wolf_pos = j.e(wolf).pos;
    j.e_mut(c).pos = wolf_pos;
    true_hit(&mut j.world, c, wolf, 100.0);
    let pack = j.camp("blue_wolves");
    assert!(pack.iter().all(|&id| j.e(id).attack_target == Some(c)));
    let home = j.e(wolf).pos;
    j.e_mut(c).pos = v(1000.0, 1000.0);
    j.e_mut(wolf).pos.x += 900.0;
    j.step(lov_shared::constants::TICK_DT);
    assert!(j.e(wolf).data.get::<MonsterState>().unwrap().resetting);
    for _ in 0..180 {
        j.step(lov_shared::constants::TICK_DT);
    }
    let w = j.e(wolf);
    assert_eq!(w.hp, w.stats.max_hp);
    assert_eq!(w.pos, home);
    assert_eq!(w.attack_target, None);
}

#[test]
fn ne_reapparait_qu_apres_la_mort_de_tout_le_camp() {
    let (mut j, c) = setup();
    j.step(scaled(TIMERS.jungle_first_spawn));
    let pack = j.camp("blue_wolves");
    true_hit(&mut j.world, c, pack[0], 100_000.0);
    j.step(lov_shared::constants::TICK_DT);
    assert!(!j.camps.timers().contains_key("blue_wolves"));
    for &e in &pack[1..] {
        true_hit(&mut j.world, c, e, 100_000.0);
    }
    j.step(lov_shared::constants::TICK_DT);
    let at = j.camps.timers()["blue_wolves"];
    assert!((at - (j.world.time + scaled(TIMERS.jungle_small_respawn))).abs() < 1e-6);
    let until = at - j.world.time;
    j.step(until);
    let alive = j
        .camp("blue_wolves")
        .into_iter()
        .filter(|&id| !j.e(id).dead)
        .count();
    assert_eq!(alive, 3);
}

#[test]
fn accorde_le_buff_bleu_et_transfere_sa_duree_au_tueur() {
    let (mut j, c) = setup();
    j.step(scaled(TIMERS.jungle_first_spawn));
    let blue = j.monster("blue_buff");
    true_hit(&mut j.world, c, blue, 100_000.0);
    let expires = j.e(c).data.get::<JungleBuffs>().unwrap().blue;
    let pos = j.e(c).pos;
    let enemy = with_smite(&mut j.world, Team::Red, pos);
    j.step(10.0);
    true_hit(&mut j.world, enemy, c, 100_000.0);
    assert_eq!(j.e(enemy).data.get::<JungleBuffs>().unwrap().blue, expires);
    assert_eq!(
        j.e(c).data.get::<JungleBuffs>(),
        Some(&JungleBuffs::default())
    );
}

#[test]
fn cree_un_sanctuaire_allie_temporaire_apres_le_carapateur() {
    let (mut j, c) = setup();
    j.step(scaled(TIMERS.scuttle_first_spawn));
    let crab = j.monster("scuttle");
    true_hit(&mut j.world, c, crab, 100_000.0);
    let wards = j.wards();
    assert_eq!(wards.len(), 1);
    assert_eq!(j.e(wards[0]).team, TeamOrNeutral::Blue);
    j.step(90.0);
    assert!(j.wards().is_empty());
}

// Tests ajoutés : effets de jungle sans équivalent direct dans les tests TS.
#[test]
fn le_chatiment_soigne_le_tueur_d_un_monstre_de_camp() {
    let (mut j, c) = setup();
    j.step(scaled(TIMERS.jungle_first_spawn));
    j.e_mut(c).hp = 100.0;
    let gromp = j.monster("gromp");
    true_hit(&mut j.world, c, gromp, 100_000.0);
    let e = j.e(c);
    assert!((e.hp - (100.0 + e.stats.max_hp * 0.08)).abs() < 1e-6);
}

#[test]
fn le_sanctuaire_accelere_les_allies_proches() {
    let (mut j, c) = setup();
    j.step(scaled(TIMERS.scuttle_first_spawn));
    let crab = j.monster("scuttle");
    true_hit(&mut j.world, c, crab, 100_000.0);
    let shrine = j.wards()[0];
    let pos = j.e(shrine).pos;
    j.e_mut(c).pos = pos;
    j.step(lov_shared::constants::TICK_DT);
    assert!(get_buff(j.e(c), "scuttle_speed", None).is_some());
}

// ------------------------------------------------------------------------------------------------
// jungle.bugs.test.ts
// ------------------------------------------------------------------------------------------------

/// `redBuffWorld()` : buffs et jungle, le champion source tue le buff rouge, une cible rouge à côté.
fn red_buff_world() -> (Jungle, EntityId, EntityId, f64) {
    let mut j = make(buffs_system());
    let source = with_smite(&mut j.world, Team::Blue, v(6200.0, 3600.0));
    let speed = j.e(source).stats.move_speed;
    j.step(scaled(TIMERS.jungle_first_spawn));
    let red = j.monster("red_buff");
    apply_damage(
        &mut j.world,
        DamageOpts::new(Some(source), red, 100_000.0, DamageType::True),
    );
    let target = with_smite(&mut j.world, Team::Red, v(6250.0, 3600.0));
    j.world.events.drain();
    (j, source, target, speed)
}

fn auto_hit(world: &mut World, source: EntityId, target: EntityId) {
    let mut opts = DamageOpts::new(Some(source), target, 1.0, DamageType::True);
    opts.is_auto_attack = true;
    apply_damage(world, opts);
}

fn burns(world: &World) -> usize {
    world
        .events
        .pending
        .iter()
        .filter(|e| matches!(e, GameEvent::Damage { ability: Some(a), .. } if a == "red_burn"))
        .count()
}

#[test]
fn le_buff_rouge_ne_ralentit_pas_son_porteur() {
    let (j, source, _, speed) = red_buff_world();
    let e = j.e(source);
    assert_eq!(e.stats.move_speed, speed);
    assert!(!has_cc(e, CcType::Slow));
    assert_eq!(get_buff(e, "red_buff", None).unwrap().def.cc, None);
}

#[test]
fn le_buff_rouge_ne_se_declenche_que_sur_une_vraie_auto_attaque() {
    let (mut j, source, target, _) = red_buff_world();
    apply_damage(
        &mut j.world,
        DamageOpts::new(Some(source), target, 1.0, DamageType::True),
    );
    assert!(get_buff(j.e(target), "red_slow", None).is_none());
    assert!(burn(j.e(target), BurnKey::Red).is_none());
}

#[test]
fn le_buff_rouge_brule_et_ralentit_un_sbire_ou_un_monstre() {
    for kind in [EntityKind::Minion, EntityKind::Monster] {
        let (mut j, source, _, _) = red_buff_world();
        let team = if kind == EntityKind::Monster {
            TeamOrNeutral::Neutral
        } else {
            TeamOrNeutral::Red
        };
        let target = j.world.spawn(Entity::new(
            kind,
            team,
            v(6250.0, 3600.0),
            30.0,
            2000.0,
            0.0,
            stats_with(2000.0, 0.0, 0.0, 125.0, 300.0, 1.0),
        ));
        auto_hit(&mut j.world, source, target);
        assert!(
            get_buff(j.e(target), "red_slow", None).is_some(),
            "{kind:?}"
        );
        j.step(1.0);
        assert_eq!(burns(&j.world), 1, "{kind:?}");
    }
}

#[test]
fn le_buff_rouge_continue_de_bruler_pendant_des_attaques_rapides() {
    let (mut j, source, target, _) = red_buff_world();
    for _ in 0..8 {
        auto_hit(&mut j.world, source, target);
        j.step(0.25);
    }
    assert_eq!(burns(&j.world), 2);
}

#[test]
fn les_trois_tics_sont_resolus_meme_apres_un_long_pas() {
    let (mut j, source, target, _) = red_buff_world();
    auto_hit(&mut j.world, source, target);
    j.step(3.0 + 1e-6);
    assert_eq!(burns(&j.world), 3);
}

#[test]
fn ne_raccourcit_pas_le_buff_du_tueur() {
    let (mut j, source, target, _) = red_buff_world();
    let expiry = j.e(source).data.get::<JungleBuffs>().unwrap().red;
    let short = j.world.time + 10.0;
    j.e_mut(target).data.insert(JungleBuffs {
        red: Some(short),
        blue: None,
    });
    apply_damage(
        &mut j.world,
        DamageOpts::new(Some(source), target, 100_000.0, DamageType::True),
    );
    assert_eq!(j.e(source).data.get::<JungleBuffs>().unwrap().red, expiry);
}

#[test]
fn un_camp_reagit_a_des_degats_inferieurs_a_une_unite() {
    let (mut j, source, _, _) = red_buff_world();
    let monster = j.monster("gromp");
    apply_damage(
        &mut j.world,
        DamageOpts::new(Some(source), monster, 0.1, DamageType::True),
    );
    assert_eq!(j.e(monster).attack_target, Some(source));
}

#[test]
fn un_projectile_fait_reagir_le_camp_contre_son_proprietaire() {
    let (mut j, source, _, _) = red_buff_world();
    let monster = j.monster("gromp");
    let pos = j.e(monster).pos;
    let mut p = Entity::new(
        EntityKind::Projectile,
        TeamOrNeutral::Blue,
        pos,
        1.0,
        1.0,
        0.0,
        stats_with(1.0, 0.0, 0.0, 125.0, 0.0, 1.0),
    );
    p.data.insert(Owner(source));
    let projectile = j.world.spawn(p);
    apply_damage(
        &mut j.world,
        DamageOpts::new(Some(projectile), monster, 0.1, DamageType::True),
    );
    assert_eq!(j.e(monster).attack_target, Some(source));
}
