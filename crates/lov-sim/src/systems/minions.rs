//! Sbires : vagues, voies, ciblage et riposte aux agressions de champions.
//! Port de `packages/server/src/systems/minions.ts`.
//!
//! Différences avec le TypeScript :
//! - les entités sont désignées par `EntityId` ; `entity.data.minion` est `MinionState` (le chemin de la
//!   voie, partagé par les sbires d'une même vague, est un `Arc<[Vec2]>`) ;
//! - l'état des vagues (`waves`, un `WeakMap<World, …>` en TS) appartient au système ;
//! - `usesCoreCombat(world)` est le paramètre `core_combat` de `minions_system` (voir
//!   `world_ai::runtime`) ;
//! - les sbires sont parcourus par id (copie des ids dans un tampon réutilisé) : un sbire né pendant le
//!   tick par un autre système n'est visité qu'au tick suivant.

use std::sync::Arc;

use lov_shared::constants::{TIMERS, scaled};
use lov_shared::data::minions::minion as minion_def;
use lov_shared::map::{StructureLane, lane_path_for};
use lov_shared::math::dist2;
use lov_shared::types::{
    EntityId, EntityKind, Lane, MinionType, TEAMS, Team, TeamOrNeutral, Vec2, other_team,
};

use crate::systems::structures::{is_structure_attackable, structure_state};
use crate::world::{Entity, System, World};
use crate::world_ai::runtime::{
    PveCombat, RETIRE_DELAY, Rewards, XpReward, attack, in_range, is_structure, retire, stats_with,
};

/// `MinionState` : `entity.data.minion`.
#[derive(Debug, Clone)]
pub struct MinionState {
    pub lane: Lane,
    pub path: Arc<[Vec2]>,
    pub waypoint: usize,
    pub aggro: Option<EntityId>,
    pub aggro_until: f64,
}

const LANES: [Lane; 3] = [Lane::Top, Lane::Mid, Lane::Bot];

fn type_name(kind: MinionType) -> &'static str {
    match kind {
        MinionType::Melee => "melee",
        MinionType::Caster => "caster",
        MinionType::Cannon => "cannon",
        MinionType::Super => "super",
    }
}

fn structure_lane(lane: Lane) -> StructureLane {
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

/// Fait naître une vague (3 mêlée, 3 lanceurs, un canon une vague sur N, un super-sbire dans la voie
/// d'un inhibiteur ennemi détruit) dans chaque voie de chaque équipe. `count` est le numéro de vague.
pub fn spawn_wave(world: &mut World, count: u32) {
    for team in TEAMS {
        for lane in LANES {
            let mut composition = [MinionType::Melee; 8];
            let mut len = 6;
            for slot in &mut composition[3..6] {
                *slot = MinionType::Caster;
            }
            let every = if world.time >= scaled(TIMERS.cannon_late_at) {
                TIMERS.cannon_every_n_waves_late
            } else {
                TIMERS.cannon_every_n_waves
            };
            if count.is_multiple_of(every) {
                composition[len] = MinionType::Cannon;
                len += 1;
            }
            let enemy = team_or_neutral(other_team(team));
            if world.query(Some(EntityKind::Inhibitor)).any(|e| {
                e.team == enemy
                    && e.dead
                    && structure_state(e).map(|s| s.lane) == Some(structure_lane(lane))
            }) {
                composition[len] = MinionType::Super;
                len += 1;
            }
            let path: Arc<[Vec2]> = lane_path_for(lane, team).into();
            let (start, next) = (path[0], path[1]);
            let length = (next.x - start.x).hypot(next.y - start.y);
            let growth = (world.time / scaled(90.0)).floor();
            for (index, &kind) in composition[..len].iter().enumerate() {
                let def = minion_def(kind);
                let hp = def.hp
                    + growth
                        * if kind == MinionType::Cannon {
                            45.0
                        } else {
                            20.0
                        };
                let index = index as f64;
                let mut e = Entity::new(
                    EntityKind::Minion,
                    team_or_neutral(team),
                    Vec2 {
                        x: start.x - (next.x - start.x) / length * index * 75.0,
                        y: start.y - (next.y - start.y) / length * index * 75.0,
                    },
                    35.0,
                    hp,
                    0.0,
                    stats_with(
                        hp,
                        def.ad + growth * 1.5,
                        def.armor + growth,
                        def.range,
                        def.speed,
                        def.attack_speed,
                    ),
                );
                e.subtype = Some(type_name(kind).to_owned());
                e.data.insert(PveCombat);
                e.data.insert(XpReward(def.xp));
                e.data.insert(MinionState {
                    lane,
                    path: Arc::clone(&path),
                    waypoint: 1,
                    aggro: None,
                    aggro_until: 0.0,
                });
                e.data.insert(Rewards {
                    gold: Some(
                        def.gold
                            + if kind == MinionType::Cannon {
                                (growth * 3.0).min(30.0)
                            } else {
                                growth * 0.2
                            },
                    ),
                    xp: Some(def.xp),
                    last_hit_only: Some(true),
                    xp_radius: Some(1400.0),
                    ..Rewards::default()
                });
                world.spawn(e);
            }
        }
    }
}

fn valid_target(world: &World, source: &Entity, target: &Entity) -> bool {
    !target.dead
        && target.team != source.team
        && target.team != TeamOrNeutral::Neutral
        && (matches!(target.kind, EntityKind::Champion | EntityKind::Minion)
            || (is_structure(target) && is_structure_attackable(world, target.id)))
}

/// Une seule passe, distance avant les règles de structure (qui sont plus coûteuses).
fn acquire_target(world: &World, source: &Entity) -> Option<EntityId> {
    let mut best: Option<EntityId> = None;
    let mut priority = 3;
    let mut distance = 650.0_f64.powi(2);
    for candidate in world.iter() {
        if candidate.dead
            || candidate.team == source.team
            || candidate.team == TeamOrNeutral::Neutral
        {
            continue;
        }
        let d = dist2(source.pos, candidate.pos);
        if d > 650.0_f64.powi(2) || !valid_target(world, source, candidate) {
            continue;
        }
        let attacking_ally = world
            .get_opt(candidate.attack_target)
            .is_some_and(|t| t.team == source.team);
        let rank = if attacking_ally && candidate.kind == EntityKind::Minion {
            0
        } else if attacking_ally && candidate.kind == EntityKind::Champion {
            1
        } else {
            2
        };
        if rank < priority
            || (rank == priority
                && (d < distance || (d == distance && best.is_none_or(|b| candidate.id < b))))
        {
            best = Some(candidate.id);
            priority = rank;
            distance = d;
        }
    }
    best
}

/// État des vagues (`waves` en TS) et tampon d'ids du système.
struct Waves {
    next: f64,
    count: u32,
    ids: Vec<EntityId>,
}

/// Le système des sbires (avant les structures dans l'ordre canonique).
///
/// `core_combat` : vrai quand le système `combat` fait partie de la partie ; il gère alors armement,
/// projectiles et hooks, et les sbires ne font que désigner leur cible (voir `world_ai::runtime::attack`).
pub fn minions_system(core_combat: bool) -> System {
    let mut waves = Waves {
        next: scaled(TIMERS.minion_first_wave),
        count: 0,
        ids: Vec::new(),
    };
    System::new("minions", move |world, _dt| {
        update(world, &mut waves, core_combat)
    })
    .with_init(init)
}

fn init(world: &mut World) {
    // Tampon des sbires à alerter, réutilisé d'un coup à l'autre.
    let mut alerted: Vec<EntityId> = Vec::new();
    world
        .hooks
        .after_damage
        .push(Box::new(move |world, ctx, dealt| {
            if dealt <= 0.0 {
                return;
            }
            let Some(source_id) = ctx.credit.or(ctx.source) else {
                return;
            };
            let (Some(source), Some(target)) = (world.get(source_id), world.get(ctx.target)) else {
                return;
            };
            if source.team == target.team
                || source.kind != EntityKind::Champion
                || target.kind != EntityKind::Champion
            {
                return;
            }
            alerted.clear();
            alerted.extend(
                world
                    .query(Some(EntityKind::Minion))
                    .filter(|e| {
                        !e.dead
                            && e.data.contains::<MinionState>()
                            && e.team == target.team
                            && in_range(e, source, 600.0)
                    })
                    .map(|e| e.id),
            );
            let until = world.time + 3.0;
            for &id in &alerted {
                if let Some(state) = world
                    .get_mut(id)
                    .and_then(|e| e.data.get_mut::<MinionState>())
                {
                    state.aggro = Some(source_id);
                    state.aggro_until = until;
                }
            }
        }));
}

fn update(world: &mut World, waves: &mut Waves, core_combat: bool) {
    if world.winner.is_some() {
        return;
    }
    if world.time >= waves.next {
        waves.count += 1;
        spawn_wave(world, waves.count);
        waves.next += scaled(TIMERS.minion_wave_interval);
    }
    world.ids_into(Some(EntityKind::Minion), &mut waves.ids);
    for &id in &waves.ids {
        update_minion(world, id, core_combat);
    }
}

fn update_minion(world: &mut World, id: EntityId, core_combat: bool) {
    let time = world.time;
    let Some(e) = world.get(id) else { return };
    let Some(minion) = e.data.get::<MinionState>() else {
        return;
    };
    if e.dead {
        retire(world, id, RETIRE_DELAY);
        return;
    }
    let mut target = if minion.aggro_until > time {
        minion
            .aggro
            .and_then(|a| world.get(a))
            .filter(|t| valid_target(world, e, t) && in_range(e, t, 800.0))
            .map(|t| t.id)
    } else {
        None
    };
    if target.is_none() {
        target = acquire_target(world, e);
    }
    if let Some(t) = target {
        attack(world, id, t, None, core_combat);
        return;
    }
    let Some(e) = world.get_mut(id) else { return };
    e.attack_target = None;
    let pos = e.pos;
    let Some(minion) = e.data.get_mut::<MinionState>() else {
        return;
    };
    let mut waypoint = minion.path.get(minion.waypoint).copied();
    if let Some(w) = waypoint
        && (pos.x - w.x).hypot(pos.y - w.y) < 50.0
    {
        minion.waypoint += 1;
        waypoint = minion.path.get(minion.waypoint).copied();
    }
    e.move_target = waypoint;
}
