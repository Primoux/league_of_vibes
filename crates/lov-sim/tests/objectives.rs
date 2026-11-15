//! Tests des objectifs épiques : port de `packages/server/test/objectives.test.ts` et
//! `objectives.bugs.test.ts`.
//!
//! `getObjectiveState(world)` devient la poignée `Objectives` renvoyée par
//! `objectives_system_with_state`. Le test de snapshot fusionne directement les hooks
//! `objective_timers`, comme le fait `buildSnapshot` (pas encore porté).

mod common;

use std::collections::BTreeMap;

use common::{champ_as, v};
use lov_shared::constants::{TIMERS, scaled};
use lov_shared::map::StructureLane;
use lov_shared::types::{
    AbilitySlot, DamageType, DragonElement, EntityId, EntityKind, GameEvent, Lane, SummonerId,
    Team, TeamOrNeutral, Vec2,
};
use lov_sim::combat::buffs::get_buff;
use lov_sim::combat::damage::{DamageOpts, apply_damage};
use lov_sim::items::{ColossusCall, EconomyState, HeraldEye, LeviathanBuff};
use lov_sim::systems::buffs::buffs_system;
use lov_sim::systems::minions::{MinionState, minions_system, spawn_wave};
use lov_sim::systems::objectives::{
    DragonElementTag, ElderBuff, LeviathanMinion, Objectives, objectives_system_with_state,
    use_herald_eye,
};
use lov_sim::systems::structures::{structure_state, structures_system};
use lov_sim::world_ai::runtime::{DamageRequest, apply_damage as pve_damage};
use lov_sim::{Entity, System, World};

struct Game {
    world: World,
    systems: Vec<System>,
    obj: Objectives,
}

impl Game {
    fn step(&mut self, dt: f64) {
        self.world.step_dt(&mut self.systems, dt);
    }

    fn tick(&mut self) {
        self.world.step(&mut self.systems);
    }

    fn e(&self, id: EntityId) -> &Entity {
        self.world.get(id).unwrap()
    }

    fn e_mut(&mut self, id: EntityId) -> &mut Entity {
        self.world.get_mut(id).unwrap()
    }

    fn damage_events(&self, ability: &str) -> usize {
        self.world
            .events
            .pending
            .iter()
            .filter(|e| matches!(e, GameEvent::Damage { ability: Some(a), .. } if a == ability))
            .count()
    }
}

fn smite_champion(world: &mut World, player_team: Team, pos: Vec2) -> EntityId {
    let id = champ_as(world, player_team, pos, "grondak");
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

/// `setup()` : sbires, structures et objectifs (sans combat central ni système de buffs).
fn setup() -> (Game, EntityId) {
    let (objectives, obj) = objectives_system_with_state(false, false);
    let mut systems = vec![minions_system(false), structures_system(false), objectives];
    let mut world = World::new(1);
    world.init(&mut systems);
    let c = smite_champion(&mut world, Team::Blue, v(7900.0, 4100.0));
    (
        Game {
            world,
            systems,
            obj,
        },
        c,
    )
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

/// Attend le prochain drake puis le tue.
fn dragon(g: &mut Game, champion: EntityId) -> EntityId {
    let next = g.obj.lock().dragon.next;
    let dt = (next - g.world.time).max(0.01);
    g.step(dt);
    let target = g.obj.lock().dragon.ids[0];
    true_hit(&mut g.world, champion, target, 100_000.0);
    target
}

fn element(g: &Game, id: EntityId) -> Option<DragonElement> {
    g.e(id).data.get::<DragonElementTag>().map(|t| t.0)
}

// ------------------------------------------------------------------------------------------------
// objectives.test.ts
// ------------------------------------------------------------------------------------------------

#[test]
fn publie_les_timers_et_garde_un_etat_isole_par_partie() {
    let (g, _) = setup();
    assert_eq!(
        g.obj.lock().timers()["dragon"],
        scaled(TIMERS.dragon_first_spawn)
    );
    let (other, _) = setup();
    other.obj.lock().dragon_count = 7;
    assert_eq!(g.obj.lock().dragon_count, 0);
}

#[test]
fn type_de_la_faille_au_troisieme_drake_et_ame_au_quatrieme() {
    let (mut g, c) = setup();
    dragon(&mut g, c);
    dragon(&mut g, c);
    let third = dragon(&mut g, c);
    let fourth = dragon(&mut g, c);
    assert_eq!(element(&g, fourth), element(&g, third));
    let st = g.obj.lock().clone();
    assert_eq!(st.blue.dragons.len(), 4);
    assert_eq!(st.blue.soul, element(&g, third));
    assert!(st.elder);
    assert_eq!(
        st.timers()["elder_dragon"],
        g.world.time + scaled(TIMERS.elder_respawn)
    );
    let souls = g
        .world
        .events
        .pending
        .iter()
        .filter(|e| matches!(e, GameEvent::Announcement { key, .. } if key == "dragon_soul"))
        .count();
    assert_eq!(souls, 1);
}

#[test]
fn l_ancien_execute_sous_20_et_n_accorde_pas_son_buff_aux_morts() {
    let (mut g, c) = setup();
    for _ in 0..4 {
        dragon(&mut g, c);
    }
    let pos = g.e(c).pos;
    let dead_ally = smite_champion(&mut g.world, Team::Blue, pos);
    g.e_mut(dead_ally).dead = true;
    let elder = dragon(&mut g, c);
    assert_eq!(g.e(elder).subtype.as_deref(), Some("elder_dragon"));
    assert!(!g.e(dead_ally).data.contains::<ElderBuff>());
    let enemy = smite_champion(&mut g.world, Team::Red, pos);
    let max = g.e(enemy).stats.max_hp;
    g.e_mut(enemy).hp = max * 0.19;
    true_hit(&mut g.world, c, enemy, 1.0);
    assert!(g.e(enemy).dead);
    assert!(g.damage_events("elder_execute") > 0);
}

#[test]
fn le_leviathan_renforce_les_vivants_et_les_sbires_proches_puis_expire() {
    let (mut g, c) = setup();
    g.e_mut(c).pos = v(2000.0, 2000.0);
    let dead_ally = smite_champion(&mut g.world, Team::Blue, v(2000.0, 2000.0));
    g.e_mut(dead_ally).dead = true;
    g.step(scaled(TIMERS.leviathan_spawn));
    let target = g.obj.lock().leviathan.ids[0];
    true_hit(&mut g.world, c, target, 100_000.0);
    assert_eq!(
        g.e(c)
            .data
            .get::<LeviathanBuff>()
            .and_then(|b| b.recall_seconds),
        Some(4.0)
    );
    assert!(!g.e(dead_ally).data.contains::<LeviathanBuff>());
    spawn_wave(&mut g.world, 1);
    g.tick();
    let minion = g
        .world
        .query(Some(EntityKind::Minion))
        .find(|e| {
            e.team == TeamOrNeutral::Blue
                && e.data
                    .get::<MinionState>()
                    .is_some_and(|m| m.lane == Lane::Mid)
        })
        .map(|e| e.id)
        .unwrap();
    assert!(g.e(minion).data.contains::<LeviathanMinion>());
    g.step(scaled(TIMERS.leviathan_buff_duration));
    assert!(!g.e(c).data.contains::<LeviathanBuff>());
    assert!(!g.e(minion).data.contains::<LeviathanMinion>());
}

#[test]
fn trois_larves_donnent_trois_cumuls_puis_le_colosse_les_remplace() {
    let (mut g, c) = setup();
    g.step(scaled(TIMERS.grubs_spawn));
    let grubs = g.obj.lock().grubs.ids.clone();
    for id in grubs {
        true_hit(&mut g.world, c, id, 100_000.0);
    }
    assert_eq!(g.obj.lock().blue.grubs, 3);
    let until = scaled(TIMERS.herald_spawn) - g.world.time;
    g.step(until);
    assert_eq!(g.obj.lock().herald.ids.len(), 1);
    let alive = g
        .world
        .query(Some(EntityKind::Monster))
        .filter(|e| !e.dead && e.subtype.as_deref() == Some("grub"))
        .count();
    assert_eq!(alive, 0);
}

#[test]
fn l_oeil_est_a_usage_unique_et_son_invocation_charge_une_tour() {
    let (mut g, c) = setup();
    g.step(scaled(TIMERS.herald_spawn));
    let herald = g.obj.lock().herald.ids[0];
    true_hit(&mut g.world, c, herald, 100_000.0);
    let tower = g
        .world
        .query(Some(EntityKind::Tower))
        .find(|e| {
            e.team == TeamOrNeutral::Red
                && e.subtype.as_deref() == Some("outer")
                && structure_state(e).is_some_and(|s| s.lane == StructureLane::Mid)
        })
        .map(|e| e.id)
        .unwrap();
    let tpos = g.e(tower).pos;
    g.e_mut(c).pos = v(tpos.x - 300.0, tpos.y);
    assert!(use_herald_eye(&mut g.world, c));
    assert!(!use_herald_eye(&mut g.world, c));
    g.tick();
    let t = g.e(tower);
    assert!(t.hp < t.stats.max_hp - 1000.0);
}

#[test]
fn retire_le_colosse_non_tue_et_laisse_la_fosse_au_leviathan() {
    let (mut g, _) = setup();
    g.step(scaled(TIMERS.herald_spawn));
    let id = g.obj.lock().herald.ids[0];
    let until = scaled(TIMERS.herald_despawn) - g.world.time;
    g.step(until);
    assert!(g.world.get(id).is_none());
    let until = scaled(TIMERS.leviathan_spawn) - g.world.time;
    g.step(until);
    assert_eq!(g.obj.lock().leviathan.ids.len(), 1);
}

// ------------------------------------------------------------------------------------------------
// objectives.test.ts — effets des six âmes
// ------------------------------------------------------------------------------------------------

fn combatants(soul: DragonElement) -> (Game, EntityId, EntityId) {
    let (mut g, c) = setup();
    let pos = g.e(c).pos;
    let enemy = smite_champion(&mut g.world, Team::Red, pos);
    g.obj.lock().blue.soul = Some(soul);
    (g, c, enemy)
}

#[test]
fn infernal_explose_avec_recharge_sans_boucle() {
    let (mut g, c, enemy) = combatants(DragonElement::Infernal);
    true_hit(&mut g.world, c, enemy, 1.0);
    let after = g.e(enemy).hp;
    assert!(after < g.e(enemy).stats.max_hp - 10.0);
    true_hit(&mut g.world, c, enemy, 1.0);
    assert_eq!(g.e(enemy).hp, after - 1.0);
}

#[test]
fn ocean_soigne_apres_avoir_touche_un_champion() {
    let (mut g, c, enemy) = combatants(DragonElement::Ocean);
    let max = g.e(c).stats.max_hp;
    g.e_mut(c).hp = max * 0.4;
    let initial = g.e(c).hp;
    true_hit(&mut g.world, c, enemy, 1.0);
    g.step(1.0);
    assert!(g.e(c).hp > initial);
}

#[test]
fn montagne_bouclier_renouvelable_hors_combat() {
    let (mut g, c, _) = combatants(DragonElement::Mountain);
    g.tick();
    assert!(g.e(c).shield > 180.0);
}

#[test]
fn nuage_vitesse_apres_l_ultime_puis_expiration() {
    let (mut g, c, _) = combatants(DragonElement::Cloud);
    let speed = g.e(c).stats.move_speed;
    g.world.emit(GameEvent::AbilityCast {
        source: c,
        champion_id: "grondak".into(),
        slot: AbilitySlot::R,
        ability_id: "grondak_r".into(),
        target_pos: None,
        target_id: None,
        fx_key: None,
    });
    g.tick();
    assert_eq!(g.e(c).stats.move_speed, speed * 1.5);
    g.step(6.0);
    assert!((g.e(c).stats.move_speed - speed).abs() < 1e-6);
}

#[test]
fn hextech_degats_et_ralentissement_en_chaine() {
    let (mut g, c, enemy) = combatants(DragonElement::Hextech);
    let speed = g.e(enemy).stats.move_speed;
    true_hit(&mut g.world, c, enemy, 1.0);
    g.tick();
    let e = g.e(enemy);
    assert_eq!(e.hp, e.stats.max_hp - 81.0);
    assert!((e.stats.move_speed - speed * 0.6).abs() < 1e-6);
}

#[test]
fn chimtech_amplifie_sous_50_pourcent_de_pv() {
    let (mut g, c, enemy) = combatants(DragonElement::Chemtech);
    let max = g.e(c).stats.max_hp;
    g.e_mut(c).hp = max * 0.4;
    true_hit(&mut g.world, c, enemy, 100.0);
    let e = g.e(enemy);
    assert!((e.hp - (e.stats.max_hp - 111.0)).abs() < 1e-6);
}

// ------------------------------------------------------------------------------------------------
// objectives.test.ts — raccordements
// ------------------------------------------------------------------------------------------------

#[test]
fn consomme_une_demande_d_invocation_une_seule_fois_et_expire_l_oeil() {
    let (mut g, c) = setup();
    g.e_mut(c).data.insert(EconomyState {
        colossus_call: Some(ColossusCall {
            team: Team::Blue,
            x: 7900.0,
            y: 4100.0,
        }),
        ..EconomyState::default()
    });
    g.tick();
    g.tick();
    let summoned = g
        .world
        .query(Some(EntityKind::Monster))
        .filter(|e| e.subtype.as_deref() == Some("summoned_herald"))
        .count();
    assert_eq!(summoned, 1);
    assert!(
        g.e(c)
            .data
            .get::<EconomyState>()
            .unwrap()
            .colossus_call
            .is_none()
    );
    let expires_at = g.world.time + 1.0;
    g.e_mut(c).data.insert(HeraldEye { expires_at });
    g.step(1.0);
    assert!(!g.e(c).data.contains::<HeraldEye>());
    assert!(!use_herald_eye(&mut g.world, c));
}

#[test]
fn expose_les_timers_et_rend_les_buffs_aux_champions_reanimes() {
    let (mut g, c) = setup();
    let mut timers = BTreeMap::new();
    for hook in g.world.hooks.objective_timers.iter() {
        timers.extend(hook(&g.world, Team::Blue).unwrap_or_default());
    }
    assert_eq!(timers["dragon"], scaled(TIMERS.dragon_first_spawn));
    g.e_mut(c).dead = true;
    g.obj.lock().blue.dragons = vec![DragonElement::Infernal, DragonElement::Infernal];
    g.e_mut(c).dead = false;
    let pos = g.e(c).pos;
    g.world.emit(GameEvent::Respawn { target: c, pos });
    assert_eq!(
        get_buff(g.e(c), "dragon_infernal", None).map(|b| b.stacks),
        Some(2)
    );
    let ad = g.e(c).stats.ad;
    g.world.emit(GameEvent::Respawn { target: c, pos });
    assert_eq!(g.e(c).stats.ad, ad, "pas de double cumul à la réanimation");
}

// ------------------------------------------------------------------------------------------------
// objectives.bugs.test.ts
// ------------------------------------------------------------------------------------------------

/// `combatants()` des régressions : buffs, structures et objectifs.
fn bug_world() -> (Game, EntityId, EntityId) {
    let (objectives, obj) = objectives_system_with_state(false, true);
    let mut systems = vec![buffs_system(), structures_system(false), objectives];
    let mut world = World::new(1);
    world.init(&mut systems);
    let source = smite_champion(&mut world, Team::Blue, v(6000.0, 6000.0));
    let target = smite_champion(&mut world, Team::Red, v(6100.0, 6000.0));
    (
        Game {
            world,
            systems,
            obj,
        },
        source,
        target,
    )
}

fn with_elder(g: &mut Game, source: EntityId) {
    g.obj.lock().blue.elder_until = 10.0;
    g.e_mut(source).data.insert(ElderBuff { expires_at: 10.0 });
}

fn hit(g: &mut Game, source: EntityId, target: EntityId, amount: f64, ability: Option<&str>) {
    let mut opts = DamageOpts::new(Some(source), target, amount, DamageType::True);
    opts.ability = ability.map(Into::into);
    apply_damage(&mut g.world, opts);
}

#[test]
fn l_ancien_brule_pendant_des_attaques_repetees() {
    let (mut g, source, target) = bug_world();
    with_elder(&mut g, source);
    for _ in 0..8 {
        hit(&mut g, source, target, 1.0, Some("test_hit"));
        g.step(0.25);
    }
    assert_eq!(g.damage_events("elder_burn"), 2);
}

#[test]
fn l_ancien_resout_les_tics_echus_apres_un_long_pas() {
    let (mut g, source, target) = bug_world();
    with_elder(&mut g, source);
    hit(&mut g, source, target, 1.0, Some("test_hit"));
    g.step(3.0 + 1e-6);
    assert_eq!(g.damage_events("elder_burn"), 3);
}

#[test]
fn les_larves_ne_frappent_pas_une_tour_deja_detruite() {
    let (mut g, source, _) = bug_world();
    g.world.time = 200.0;
    g.obj.lock().blue.grubs = 1;
    let tower = g
        .world
        .query(Some(EntityKind::Tower))
        .find(|e| e.team == TeamOrNeutral::Red && e.subtype.as_deref() == Some("outer"))
        .map(|e| e.id)
        .unwrap();
    hit(&mut g, source, tower, 100_000.0, None);
    let damage: Vec<f64> = g
        .world
        .events
        .pending
        .iter()
        .filter_map(|e| match e {
            GameEvent::Damage { target, amount, .. } if *target == tower => Some(*amount),
            _ => None,
        })
        .collect();
    assert_eq!(damage.len(), 1);
    assert!(damage.iter().all(|&a| a >= 0.0));
}

#[test]
fn l_ame_infernale_explose_sur_un_coup_letal_sans_refrapper_la_victime() {
    let (mut g, source, target) = bug_world();
    g.obj.lock().blue.soul = Some(DragonElement::Infernal);
    g.e_mut(target).hp = 1.0;
    let neighbor = smite_champion(&mut g.world, Team::Red, v(6150.0, 6000.0));
    hit(&mut g, source, target, 10.0, None);
    assert!(g.e(target).dead);
    let n = g.e(neighbor);
    assert!(n.hp < n.stats.max_hp);
    let on_target: Vec<f64> = g
        .world
        .events
        .pending
        .iter()
        .filter_map(|e| match e {
            GameEvent::Damage {
                target: t, amount, ..
            } if *t == target => Some(*amount),
            _ => None,
        })
        .collect();
    assert_eq!(on_target.len(), 1);
    assert!(on_target.iter().all(|&a| a >= 0.0));
}

#[test]
fn l_ancien_execute_aussi_sur_un_impact_inferieur_a_une_unite() {
    let (mut g, source, target) = bug_world();
    with_elder(&mut g, source);
    let max = g.e(target).stats.max_hp;
    g.e_mut(target).hp = max * 0.19;
    hit(&mut g, source, target, 0.1, None);
    assert!(g.e(target).dead);
}

#[test]
fn l_ame_infernale_reagit_a_un_impact_inferieur_a_une_unite() {
    let (mut g, source, target) = bug_world();
    g.obj.lock().blue.soul = Some(DragonElement::Infernal);
    let hp = g.e(target).hp;
    hit(&mut g, source, target, 0.1, None);
    assert!(hp - g.e(target).hp > 1.0);
}
