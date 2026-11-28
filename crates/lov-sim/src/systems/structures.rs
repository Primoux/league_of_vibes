//! Tours, inhibiteurs, nexus et plaques. Port de `packages/server/src/systems/structures.ts`.
//!
//! `init` fait naître les 22 tours, les 6 inhibiteurs et les 2 nexus, et branche les hooks (ordre
//! d'attaque des structures, fortification, plaques, rampe de dégâts, agresseur, mort). `update` gère
//! la réanimation des inhibiteurs, la résistance anti-backdoor et le tir des tours.
//!
//! Différences avec le TypeScript :
//! - les entités sont désignées par `EntityId` ; `entity.data.structure` est `StructureState`,
//!   `entity.data.towerAggressor` est `TowerAggressor` ;
//! - `usesCoreCombat(world)` (la présence du système `combat` dans `world.systems`) est le paramètre
//!   `core_combat` de `structures_system` ;
//! - `structureRevisions` et `structureLists` (des `WeakMap<World, …>`) n'existent pas : le monde n'a pas
//!   de table par monde. La révision qui invalidait le cache de `isStructureAttackable` devient une
//!   invalidation immédiate (`invalidate_structures` remet à zéro le cache de chaque structure) ; le
//!   cache est un atomique par structure, pour rester utilisable depuis les hooks `is_targetable` qui
//!   ne reçoivent que `&World`. Les listes de structures sont remplacées par un parcours des entités,
//!   dans l'ordre d'insertion (donc celui de naissance des structures), avec un tampon réutilisé ;
//! - une tour externe sans `StructureState` touchée pendant la fenêtre des plaques fait planter le
//!   TypeScript (`state.plates` sur `undefined`) ; ici le hook ne fait rien.

use std::sync::atomic::{AtomicU64, Ordering};

use lov_shared::constants::{TIMERS, scaled};
use lov_shared::data::structures::STRUCTURES;
use lov_shared::map::{INHIBITORS, NEXUS, StructureLane, TOWERS};
use lov_shared::math::dist2;
use lov_shared::types::{
    DamageType, EntityId, EntityKind, GameEvent, Lane, StructureKind, TEAMS, Team, TeamOrNeutral,
    TowerTier, Vec2, other_team,
};

use crate::systems::vision::is_visible_to;
use crate::world::{Entity, EventKind, System, World};
use crate::world_ai::runtime::{
    DamageRequest, PlateReward, PveCombat, Rewards, attack, in_range, is_structure, stats_with,
    team_of,
};

/// Cache de `isStructureAttackable` : 0 = vide, sinon `((tick + 1) << 1) | valeur`.
#[derive(Debug, Default)]
struct AttackableCache(AtomicU64);

impl AttackableCache {
    fn get(&self, tick: u64) -> Option<bool> {
        let c = self.0.load(Ordering::Relaxed);
        (c != 0 && c >> 1 == tick + 1).then_some(c & 1 == 1)
    }

    fn set(&self, tick: u64, value: bool) {
        self.0
            .store(((tick + 1) << 1) | u64::from(value), Ordering::Relaxed);
    }

    fn clear(&self) {
        self.0.store(0, Ordering::Relaxed);
    }
}

/// `StructureState` : `entity.data.structure`.
#[derive(Debug)]
pub struct StructureState {
    pub lane: StructureLane,
    pub tier: Option<TowerTier>,
    pub plates: u32,
    pub respawn_at: Option<f64>,
    pub destroyed: bool,
    pub ramp_target: Option<EntityId>,
    pub ramp: f64,
    pub bulwark_until: f64,
    pub bulwark: f64,
    attackable: AttackableCache,
}

/// `entity.data.towerAggressor` : le champion qui vient de frapper un allié sous la tour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TowerAggressor {
    pub id: EntityId,
    pub until: f64,
}

pub fn structure_state(e: &Entity) -> Option<&StructureState> {
    e.data.get::<StructureState>()
}

pub fn structure_state_mut(e: &mut Entity) -> Option<&mut StructureState> {
    e.data.get_mut::<StructureState>()
}

/// Remet à zéro le cache d'attaquabilité de toutes les structures (le `invalidateStructures` du TS).
fn invalidate_structures(world: &World) {
    for e in world.iter() {
        if let Some(s) = structure_state(e) {
            s.attackable.clear();
        }
    }
}

/// Une structure ne peut être attaquée que si celles qui la protègent sont tombées.
pub fn is_structure_attackable(world: &World, target: EntityId) -> bool {
    let Some(t) = world.get(target) else {
        return false;
    };
    if t.dead {
        return false;
    }
    let Some(state) = structure_state(t) else {
        return true;
    };
    if let Some(v) = state.attackable.get(world.tick) {
        return v;
    }
    let v = compute_attackable(world, t, state);
    state.attackable.set(world.tick, v);
    v
}

fn compute_attackable(world: &World, target: &Entity, state: &StructureState) -> bool {
    if target.kind == EntityKind::Tower && state.tier == Some(TowerTier::Outer) {
        return true;
    }
    for e in world.iter() {
        if e.team != target.team || e.dead || e.id == target.id {
            continue;
        }
        let Some(previous) = structure_state(e) else {
            continue;
        };
        if state.lane != StructureLane::Base
            && previous.lane == state.lane
            && e.kind == EntityKind::Tower
            && (target.kind == EntityKind::Inhibitor
                || (state.tier == Some(TowerTier::Inner)
                    && previous.tier == Some(TowerTier::Outer))
                || (state.tier == Some(TowerTier::Inhib)
                    && previous.tier != Some(TowerTier::Nexus)
                    && previous.tier != Some(TowerTier::Inhib)))
        {
            return false;
        }
        if state.lane == StructureLane::Base
            && target.kind == EntityKind::Nexus
            && e.kind == EntityKind::Tower
            && previous.tier == Some(TowerTier::Nexus)
        {
            return false;
        }
    }
    if state.lane == StructureLane::Base {
        return world
            .query(Some(EntityKind::Inhibitor))
            .any(|i| i.team == target.team && i.dead);
    }
    true
}

/// À appeler par l'API centrale avant réduction des résistances.
pub fn structure_damage_multiplier(world: &World, request: &DamageRequest) -> f64 {
    if !is_structure_attackable(world, request.target) {
        return 0.0;
    }
    let Some(target) = world.get(request.target) else {
        return 0.0;
    };
    if target.kind == EntityKind::Tower
        && target.subtype.as_deref() == Some("outer")
        && world.time < scaled(STRUCTURES.fortification_until)
    {
        0.5
    } else {
        1.0
    }
}

fn tier_name(tier: TowerTier) -> &'static str {
    match tier {
        TowerTier::Outer => "outer",
        TowerTier::Inner => "inner",
        TowerTier::Inhib => "inhib",
        TowerTier::Nexus => "nexus",
    }
}

fn lane_of(lane: Lane) -> StructureLane {
    match lane {
        Lane::Top => StructureLane::Top,
        Lane::Mid => StructureLane::Mid,
        Lane::Bot => StructureLane::Bot,
    }
}

fn team_or_neutral(team: Team) -> TeamOrNeutral {
    match team {
        Team::Blue => TeamOrNeutral::Blue,
        Team::Red => TeamOrNeutral::Red,
    }
}

fn spawn_structure(
    world: &mut World,
    kind: EntityKind,
    team: Team,
    lane: StructureLane,
    pos: Vec2,
    tier: Option<TowerTier>,
) {
    let def = tier.map(|t| STRUCTURES.towers.get(t));
    let hp = match def {
        Some(d) => d.hp,
        None if kind == EntityKind::Inhibitor => STRUCTURES.inhibitor_hp,
        None => STRUCTURES.nexus_hp,
    };
    let st = stats_with(
        hp,
        def.map_or(0.0, |d| d.ad),
        def.map_or(20.0, |d| d.armor),
        if kind == EntityKind::Tower {
            STRUCTURES.range
        } else {
            0.0
        },
        0.0,
        1.1,
    );
    let mut e = Entity::new(
        kind,
        team_or_neutral(team),
        pos,
        if kind == EntityKind::Nexus {
            220.0
        } else {
            100.0
        },
        hp,
        0.0,
        st,
    );
    e.subtype = Some(match tier {
        Some(t) => tier_name(t).to_owned(),
        None if kind == EntityKind::Inhibitor => "inhibitor".to_owned(),
        None => "nexus".to_owned(),
    });
    e.data.insert(PveCombat);
    e.data.insert(StructureState {
        lane,
        tier,
        plates: if tier == Some(TowerTier::Outer) { 5 } else { 0 },
        respawn_at: None,
        destroyed: false,
        ramp_target: None,
        ramp: 0.0,
        bulwark_until: 0.0,
        bulwark: 0.0,
        attackable: AttackableCache::default(),
    });
    e.data.insert(Rewards {
        local_gold: Some(def.map_or(0.0, |d| d.local_gold)),
        team_gold: Some(def.map_or(0.0, |d| d.team_gold)),
        ..Rewards::default()
    });
    world.spawn(e);
}

fn spawn_structures(world: &mut World) {
    for t in TOWERS.iter() {
        spawn_structure(
            world,
            EntityKind::Tower,
            t.team,
            t.lane,
            t.pos,
            Some(t.tier),
        );
    }
    for i in INHIBITORS.iter() {
        spawn_structure(
            world,
            EntityKind::Inhibitor,
            i.team,
            lane_of(i.lane),
            i.pos,
            None,
        );
    }
    for team in TEAMS {
        spawn_structure(
            world,
            EntityKind::Nexus,
            team,
            StructureLane::Base,
            *NEXUS.get(team),
            None,
        );
    }
}

fn tower_damage(target: &Entity, state: &StructureState) -> f64 {
    if target.kind == EntityKind::Minion {
        target.stats.max_hp
            * match target.subtype.as_deref() {
                Some("cannon" | "super") => 0.14,
                Some("caster") => 0.7,
                _ => 0.45,
            }
    } else {
        // `STRUCTURES.towers[state.tier!]` : une tour a toujours un rang.
        let tier = state.tier.unwrap_or(TowerTier::Outer);
        STRUCTURES.towers.get(tier).ad * (1.0 + state.ramp * 0.4)
    }
}

fn acquire_tower_target(world: &World, tower: &Entity) -> Option<EntityId> {
    let mut best: Option<EntityId> = None;
    let mut minion = false;
    let mut distance = STRUCTURES.range.powi(2);
    for candidate in world.iter() {
        if candidate.dead
            || candidate.team == tower.team
            || candidate.team == TeamOrNeutral::Neutral
            || !matches!(candidate.kind, EntityKind::Minion | EntityKind::Champion)
            // Une cible invisible pour la tour serait refusée par le combat : la tour ne tirerait sur
            // personne (#313).
            || !is_visible_to(candidate, tower.team, true)
        {
            continue;
        }
        let d = dist2(tower.pos, candidate.pos);
        if d > STRUCTURES.range.powi(2) {
            continue;
        }
        let preferred = candidate.kind == EntityKind::Minion;
        if (!minion && preferred)
            || (preferred == minion
                && (d < distance || (d == distance && best.is_none_or(|b| candidate.id < b))))
        {
            best = Some(candidate.id);
            minion = preferred;
            distance = d;
        }
    }
    best
}

/// Le système des structures (après les sbires dans l'ordre canonique).
///
/// `core_combat` : vrai quand le système `combat` fait partie de la partie ; il gère alors armement,
/// projectiles et hooks, et les tours ne font que désigner leur cible (voir `world_ai::runtime::attack`).
pub fn structures_system(core_combat: bool) -> System {
    // Tampon d'ids réutilisé d'un tick à l'autre.
    let mut ids: Vec<EntityId> = Vec::new();
    System::new("structures", move |world, _dt| {
        update(world, &mut ids, core_combat)
    })
    .with_init(init)
}

fn init(world: &mut World) {
    spawn_structures(world);
    world.hooks.before_damage.push(Box::new(|world, ctx| {
        let Some(target) = world.get(ctx.target) else {
            return;
        };
        if !is_structure(target) {
            return;
        }
        if let Some(source) = ctx.source.and_then(|s| world.get(s))
            && source.kind == EntityKind::Minion
        {
            ctx.multiplier *= if source.subtype.as_deref() == Some("super") {
                0.6
            } else {
                0.35
            };
        }
        if !is_structure_attackable(world, ctx.target) {
            ctx.multiplier = 0.0;
        } else if target.kind == EntityKind::Tower
            && target.subtype.as_deref() == Some("outer")
            && world.time < scaled(STRUCTURES.fortification_until)
        {
            ctx.multiplier *= 0.5;
        }
    }));
    world.hooks.is_targetable.push(Box::new(|world, target, _| {
        world.get(target).is_none_or(|t| !is_structure(t)) || is_structure_attackable(world, target)
    }));
    world.hooks.before_attack_hit.push(Box::new(|world, ctx| {
        let (attacker, target) = (ctx.attacker, ctx.target);
        let Some(a) = world.get_mut(attacker) else {
            return;
        };
        if a.kind != EntityKind::Tower {
            return;
        }
        let Some(state) = structure_state_mut(a) else {
            return;
        };
        if state.ramp_target != Some(target) {
            state.ramp_target = Some(target);
            state.ramp = 0.0;
        }
        let (Some(a), Some(t)) = (world.get(attacker), world.get(target)) else {
            return;
        };
        let Some(state) = structure_state(a) else {
            return;
        };
        ctx.amount = tower_damage(t, state);
        if t.kind == EntityKind::Minion {
            ctx.damage_type = DamageType::True;
        }
    }));
    world.hooks.after_attack_hit.push(Box::new(|world, ctx, _| {
        let is_champion = world
            .get(ctx.target)
            .is_some_and(|t| t.kind == EntityKind::Champion);
        if !is_champion {
            return;
        }
        if let Some(a) = world.get_mut(ctx.attacker)
            && a.kind == EntityKind::Tower
            && let Some(state) = structure_state_mut(a)
        {
            state.ramp = (state.ramp + 1.0).min(3.0);
        }
    }));
    // Tampon des tours à marquer, réutilisé d'un coup à l'autre.
    let mut towers: Vec<EntityId> = Vec::new();
    world
        .hooks
        .after_damage
        .push(Box::new(move |world, ctx, dealt| {
            let Some(source_id) = ctx.credit.or(ctx.source) else {
                return;
            };
            if dealt <= 0.0 {
                return;
            }
            let target_id = ctx.target;
            let time = world.time;
            let (Some(source), Some(target)) = (world.get(source_id), world.get(target_id)) else {
                return;
            };
            if source.kind == EntityKind::Champion
                && target.kind == EntityKind::Champion
                && source.team != target.team
            {
                towers.clear();
                towers.extend(
                    world
                        .query(Some(EntityKind::Tower))
                        .filter(|t| {
                            !t.dead
                                && t.team == target.team
                                && in_range(t, source, STRUCTURES.range)
                        })
                        .map(|t| t.id),
                );
                for &id in &towers {
                    if let Some(t) = world.get_mut(id) {
                        t.data.insert(TowerAggressor {
                            id: source_id,
                            until: time + 3.0,
                        });
                    }
                }
            }
            plates_after_damage(world, source_id, target_id);
        }));
    world.events.on(EventKind::Death, |world, event| {
        let GameEvent::Death { target, .. } = event else {
            return;
        };
        let Some(t) = world.get(*target) else { return };
        if !is_structure(t) {
            return;
        }
        let Some(state) = structure_state(t) else {
            return;
        };
        if state.destroyed {
            return;
        }
        let (kind, team) = (t.kind, t.team);
        invalidate_structures(world);
        let time = world.time;
        if let Some(state) = world.get_mut(*target).and_then(structure_state_mut) {
            state.destroyed = true;
            if kind == EntityKind::Inhibitor {
                state.respawn_at = Some(time + scaled(TIMERS.inhibitor_respawn));
            }
        }
        let Some(team) = team_of(team) else { return };
        let structure_kind = match kind {
            EntityKind::Tower => StructureKind::Tower,
            EntityKind::Inhibitor => StructureKind::Inhibitor,
            _ => StructureKind::Nexus,
        };
        world.emit(GameEvent::StructureDestroyed {
            target: *target,
            kind: structure_kind,
            team,
            plate: None,
        });
        if kind == EntityKind::Nexus && world.winner.is_none() {
            let winner = other_team(team);
            world.winner = Some(winner);
            world.emit(GameEvent::Announcement {
                key: "victory".to_owned(),
                team: Some(winner),
                params: None,
            });
        }
    });
}

/// Plaques d'une tour externe : une par cinquième de PV franchi, y compris plusieurs sur un même coup.
fn plates_after_damage(world: &mut World, source_id: EntityId, target_id: EntityId) {
    let time = world.time;
    let Some(target) = world.get(target_id) else {
        return;
    };
    if target.kind != EntityKind::Tower
        || target.subtype.as_deref() != Some("outer")
        || time >= scaled(TIMERS.tower_plates_until)
    {
        return;
    }
    if structure_state(target).is_none() {
        return;
    }
    let Some(team) = team_of(target.team) else {
        return;
    };
    let target_team = target.team;
    let pos = target.pos;
    let remaining = (target.hp / (target.stats.max_hp / 5.0)).ceil().max(0.0) as u32;
    loop {
        let Some(state) = world.get_mut(target_id).and_then(structure_state_mut) else {
            return;
        };
        if state.plates <= remaining {
            return;
        }
        state.plates -= 1;
        let reward = PlateReward {
            gold: STRUCTURES.plate_gold,
            radius: STRUCTURES.plate_radius,
            source: source_id,
        };
        if let Some(t) = world.get_mut(target_id) {
            t.data.insert(reward);
        }
        world.emit(GameEvent::StructureDestroyed {
            target: target_id,
            kind: StructureKind::Tower,
            team,
            plate: Some(true),
        });
        let nearby = world
            .query(Some(EntityKind::Champion))
            .filter(|c| {
                !c.dead
                    && c.team != target_team
                    && dist2(c.pos, pos) <= STRUCTURES.plate_radius.powi(2)
            })
            .count();
        let time = world.time;
        if let Some(t) = world.get_mut(target_id) {
            if let Some(state) = structure_state_mut(t) {
                state.bulwark = nearby.saturating_sub(1) as f64 * 35.0;
                state.bulwark_until = time + 30.0;
            }
            t.data.insert(reward);
        }
    }
}

fn update(world: &mut World, ids: &mut Vec<EntityId>, core_combat: bool) {
    if world.winner.is_some() {
        return;
    }
    ids.clear();
    ids.extend(world.iter().filter(|e| is_structure(e)).map(|e| e.id));
    for &id in ids.iter() {
        update_structure(world, id, core_combat);
    }
}

fn update_structure(world: &mut World, id: EntityId, core_combat: bool) {
    let time = world.time;
    let Some(e) = world.get_mut(id) else { return };
    let Some(state) = e.data.get_mut::<StructureState>() else {
        return;
    };
    if e.dead {
        if e.kind == EntityKind::Inhibitor
            && let Some(at) = state.respawn_at
            && time >= at
        {
            e.dead = false;
            e.hp = e.stats.max_hp;
            e.anim = Some("idle".to_owned());
            state.destroyed = false;
            state.respawn_at = None;
            let team = e.team;
            invalidate_structures(world);
            if let Some(team) = team_of(team) {
                world.emit(GameEvent::Announcement {
                    key: "inhibitor_respawn".to_owned(),
                    team: Some(team),
                    params: None,
                });
            }
        }
        return;
    }
    if e.kind != EntityKind::Tower {
        return;
    }
    let Some(tier) = state.tier else { return };
    let def = STRUCTURES.towers.get(tier);
    let (team, pos) = (e.team, e.pos);
    let backdoor = !world.query(Some(EntityKind::Minion)).any(|m| {
        !m.dead && m.team != team && dist2(pos, m.pos) <= STRUCTURES.backdoor_radius.powi(2)
    });
    let Some(e) = world.get_mut(id) else { return };
    let Some(state) = e.data.get_mut::<StructureState>() else {
        return;
    };
    let armor = def.armor
        + if backdoor {
            STRUCTURES.backdoor_resistance
        } else {
            0.0
        }
        + if time < state.bulwark_until {
            state.bulwark
        } else {
            0.0
        };
    if time >= scaled(TIMERS.tower_plates_until) {
        state.plates = 0;
    }
    e.stats.armor = armor;
    e.stats.mr = armor;
    let aggressor = e.data.get::<TowerAggressor>().copied();
    let current = e.attack_target;
    let Some(tower) = world.get(id) else { return };
    let mut target = aggressor
        .filter(|a| a.until > time)
        .and_then(|a| world.get(a.id))
        .filter(|t| {
            !t.dead && in_range(tower, t, STRUCTURES.range) && is_visible_to(t, tower.team, true)
        })
        .map(|t| t.id);
    if target.is_none() {
        target = current
            .and_then(|c| world.get(c))
            .filter(|c| {
                !c.dead
                    && in_range(tower, c, STRUCTURES.range)
                    && is_visible_to(c, tower.team, true)
            })
            .map(|c| c.id);
    }
    if target.is_none() {
        target = acquire_tower_target(world, tower);
    }
    let Some(target) = target else {
        if let Some(e) = world.get_mut(id) {
            e.attack_target = None;
            if let Some(state) = e.data.get_mut::<StructureState>() {
                state.ramp = 0.0;
                state.ramp_target = None;
            }
        }
        return;
    };
    let Some(target_entity) = world.get(target) else {
        return;
    };
    let target_is_champion = target_entity.kind == EntityKind::Champion;
    if let Some(state) = world.get_mut(id).and_then(structure_state_mut)
        && state.ramp_target != Some(target)
    {
        state.ramp_target = Some(target);
        state.ramp = 0.0;
    }
    let (Some(tower), Some(target_entity)) = (world.get(id), world.get(target)) else {
        return;
    };
    let Some(state) = structure_state(tower) else {
        return;
    };
    let damage = tower_damage(target_entity, state);
    if attack(world, id, target, Some(damage), core_combat)
        && !core_combat
        && target_is_champion
        && let Some(state) = world.get_mut(id).and_then(structure_state_mut)
    {
        state.ramp = (state.ramp + 1.0).min(3.0);
    }
}
