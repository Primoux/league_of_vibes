//! Jungle : port de `packages/server/src/systems/jungle.ts`.
//!
//! Apparition et réapparition des camps, aggro de tout le camp, laisse et retour au camp (via
//! `world_ai::monsters`), buffs bleu et rouge (transférés au tueur d'un porteur), brûlure et
//! ralentissement du buff rouge sur auto-attaque, sanctuaire du Carapateur, soin de Châtiment.
//!
//! Différences avec le TypeScript :
//! - l'état des camps (un `WeakMap<World, Map>` en TS) est partagé entre le système et sa poignée
//!   [`JungleCamps`] ; `getCampTimers(world)` devient [`JungleCamps::timers`] ;
//! - les timers de camps sont exportés par un hook `objective_timers` propre à la jungle. En TS,
//!   `getObjectiveTimers` des objectifs les fusionnait avec les siens : le snapshot reçoit la même union ;
//! - `usesCoreCombat` est le paramètre `core_combat` (voir `runtime::attack`) ;
//! - `data.jungleBuffs`, `data.jungle` (sanctuaire), `data.nextScuttleSpeed` et `data.speedRadius`
//!   sont [`JungleBuffs`], [`ShrineData`] et [`NextScuttleSpeed`] ; `speedRadius` n'était lu nulle part
//!   en TS et devient la constante [`SHRINE_SPEED_RADIUS`].

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};

use lov_shared::constants::{TIMERS, scaled};
use lov_shared::map::CAMPS;
use lov_shared::types::{
    DamageType, EntityId, EntityKind, GameEvent, MonsterType, SummonerId, TeamOrNeutral, Vec2,
};

use crate::combat::damage::{HealOpts, apply_heal};
use crate::systems::vision::SightRadius;
use crate::world::{Entity, EventKind, System, World};
use crate::world_ai::burns::{BurnKey, refresh_burn, take_burn_tick};
use crate::world_ai::monsters::{
    MonsterExtra, MonsterState, aggro_monster, install_monster_hooks, spawn_monster, update_monster,
};
use crate::world_ai::runtime::{
    BuffOptions, DamageRequest, RETIRE_DELAY, add_buff, apply_damage, retire,
};

/// Durée d'un buff de jungle (s).
pub const JUNGLE_BUFF_DURATION: f64 = 120.0;
/// Durée de vie du sanctuaire du Carapateur (s).
pub const SHRINE_LIFETIME: f64 = 90.0;
/// Rayon de vision du sanctuaire.
pub const SHRINE_SIGHT: f64 = 900.0;
/// Rayon dans lequel le sanctuaire accélère les alliés.
pub const SHRINE_SPEED_RADIUS: f64 = 450.0;

/// `entity.data.jungleBuffs` : temps de jeu d'expiration des buffs bleu et rouge.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct JungleBuffs {
    pub blue: Option<f64>,
    pub red: Option<f64>,
}

/// `entity.data.jungle` : sanctuaire temporaire laissé par le Carapateur.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShrineData {
    pub expires_at: f64,
}

/// `entity.data.nextScuttleSpeed` : prochain rafraîchissement de l'accélération du sanctuaire.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NextScuttleSpeed(pub f64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Color {
    Blue,
    Red,
}

#[derive(Debug, Clone, Default)]
struct CampState {
    ids: Vec<EntityId>,
    next: f64,
}

/// État des camps de jungle, dans l'ordre de `CAMPS`.
#[derive(Debug, Clone, Default)]
struct Camps(Vec<CampState>);

/// Poignée sur l'état des camps d'une partie (le `getCampTimers` du TS).
#[derive(Debug, Clone, Default)]
pub struct JungleCamps(Arc<Mutex<Camps>>);

impl JungleCamps {
    fn lock(&self) -> MutexGuard<'_, Camps> {
        self.0.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Camps vides, par id, avec le temps de jeu de leur prochaine apparition.
    pub fn timers(&self) -> BTreeMap<String, f64> {
        let camps = self.lock();
        CAMPS
            .iter()
            .zip(camps.0.iter())
            .filter(|(_, st)| st.ids.is_empty())
            .map(|(camp, st)| (camp.id.clone(), st.next))
            .collect()
    }
}

fn first_spawn(monster: MonsterType) -> f64 {
    scaled(if monster == MonsterType::Scuttle {
        TIMERS.scuttle_first_spawn
    } else {
        TIMERS.jungle_first_spawn
    })
}

fn respawn_delay(monster: MonsterType) -> f64 {
    scaled(match monster {
        MonsterType::Scuttle => TIMERS.scuttle_respawn,
        MonsterType::BlueBuff | MonsterType::RedBuff => TIMERS.jungle_buff_respawn,
        _ => TIMERS.jungle_small_respawn,
    })
}

/// Accorde (ou prolonge) un buff de jungle jusqu'à `expires_at` ; une durée plus courte ne raccourcit
/// jamais un buff déjà porté.
fn grant(world: &mut World, champion: EntityId, color: Color, expires_at: f64) {
    let time = world.time;
    let Some(e) = world.get_mut(champion) else {
        return;
    };
    let buffs = e.data.get_or_insert_with(JungleBuffs::default);
    let slot = match color {
        Color::Blue => &mut buffs.blue,
        Color::Red => &mut buffs.red,
    };
    let expires_at = expires_at.max(slot.unwrap_or(0.0));
    *slot = Some(expires_at);
    let (id, params) = match color {
        Color::Blue => (
            "blue_buff",
            vec![("manaRegen".into(), 25.0), ("abilityHaste".into(), 10.0)],
        ),
        Color::Red => (
            "red_buff",
            vec![("onHitSlow".into(), 0.2), ("burn".into(), 12.0)],
        ),
    };
    add_buff(
        world,
        champion,
        id,
        BuffOptions {
            params,
            ..BuffOptions::new((expires_at - time).max(0.0))
        },
    );
}

fn is_unit(kind: EntityKind) -> bool {
    matches!(
        kind,
        EntityKind::Champion | EntityKind::Minion | EntityKind::Monster
    )
}

fn init_jungle(world: &mut World, camps: &JungleCamps) {
    install_monster_hooks(world);
    camps.lock().0 = CAMPS
        .iter()
        .map(|c| CampState {
            ids: Vec::new(),
            next: first_spawn(c.monster),
        })
        .collect();

    let timers = camps.clone();
    world
        .hooks
        .objective_timers
        .push(Box::new(move |_, _| Some(timers.timers())));

    let pack = camps.clone();
    world
        .hooks
        .after_damage
        .push(Box::new(move |world, ctx, dealt| {
            if dealt <= 0.0 {
                return;
            }
            let Some(source) = ctx.credit.or(ctx.source) else {
                return;
            };
            // Tout le camp de la victime réagit contre le crédité (propriétaire d'un projectile…).
            let camp = world
                .get(ctx.target)
                .and_then(|t| t.data.get::<MonsterState>())
                .and_then(|m| m.camp.clone());
            if let Some(camp) = camp
                && let Some(i) = CAMPS.iter().position(|c| c.id == camp)
            {
                let members = pack
                    .lock()
                    .0
                    .get(i)
                    .map(|s| s.ids.clone())
                    .unwrap_or_default();
                for m in members {
                    aggro_monster(world, m, source);
                }
            }
            // Buff rouge : ralentit et brûle la cible d'une vraie auto-attaque de champion.
            let direct_champ = world.get_opt(ctx.source).is_some_and(|s| s.champ.is_some());
            let Some(src) = world.get(source) else { return };
            let red_active = src
                .data
                .get::<JungleBuffs>()
                .and_then(|b| b.red)
                .is_some_and(|t| t > world.time);
            let Some(target) = world.get(ctx.target) else {
                return;
            };
            if ctx.is_auto_attack
                && direct_champ
                && src.champ.is_some()
                && !src.dead
                && red_active
                && target.team != src.team
                && target.hp > 0.0
                && is_unit(target.kind)
            {
                add_buff(
                    world,
                    ctx.target,
                    "red_slow",
                    BuffOptions {
                        source: Some(source),
                        params: vec![("slow".into(), 0.2)],
                        ..BuffOptions::new(2.0)
                    },
                );
                refresh_burn(world, ctx.target, BurnKey::Red, source);
            }
        }));

    world.events.on(EventKind::Death, |world, ev| {
        let GameEvent::Death { target, killer, .. } = ev else {
            return;
        };
        let time = world.time;
        let Some(t) = world.get(*target) else { return };
        let killer_champ = world
            .get_opt(*killer)
            .filter(|k| k.champ.is_some())
            .map(|k| k.id);
        if t.kind == EntityKind::Champion {
            let held = t.data.get::<JungleBuffs>().copied();
            if let (Some(k), Some(held)) = (killer_champ, held) {
                for (color, until) in [(Color::Blue, held.blue), (Color::Red, held.red)] {
                    if let Some(until) = until.filter(|&u| u > time) {
                        grant(world, k, color, until);
                    }
                }
            }
            if let Some(t) = world.get_mut(*target) {
                t.data.insert(JungleBuffs::default());
            }
        }
        let Some(t) = world.get(*target) else { return };
        let Some(home) = t
            .data
            .get::<MonsterState>()
            .filter(|m| m.camp.is_some())
            .map(|m| m.home)
        else {
            return;
        };
        let Some(k) = killer_champ else { return };
        let subtype = t.subtype.clone();
        let shrine_stats = t.stats.clone();
        match subtype.as_deref() {
            Some("blue_buff") => grant(world, k, Color::Blue, time + JUNGLE_BUFF_DURATION),
            Some("red_buff") => grant(world, k, Color::Red, time + JUNGLE_BUFF_DURATION),
            Some("scuttle") => spawn_shrine(world, k, home, shrine_stats),
            _ => {}
        }
        let Some(killer) = world.get(k) else { return };
        let smite = killer.champ.as_ref().is_some_and(|c| {
            c.summoners.d == SummonerId::Smite || c.summoners.f == SummonerId::Smite
        });
        if smite {
            let amount = killer.stats.max_hp * 0.08;
            apply_heal(world, HealOpts::new(Some(k), k, amount));
        }
    });
}

fn spawn_shrine(
    world: &mut World,
    owner: EntityId,
    pos: Vec2,
    mut stats: lov_shared::types::Stats,
) {
    let time = world.time;
    let Some(team) = world.get(owner).map(|o| o.team) else {
        return;
    };
    stats.max_hp = 1.0;
    let mut ward = Entity::new(EntityKind::Ward, team, pos, 30.0, 1.0, 0.0, stats);
    ward.subtype = Some("scuttle_shrine".into());
    ward.data.insert(ShrineData {
        expires_at: time + SHRINE_LIFETIME,
    });
    ward.data.insert(SightRadius(SHRINE_SIGHT));
    world.spawn(ward);
}

fn update_shrine(world: &mut World, team: TeamOrNeutral, pos: Vec2) {
    let time = world.time;
    let mut allies: Vec<EntityId> = Vec::new();
    for c in world.query(Some(EntityKind::Champion)) {
        let near = (c.pos.x - pos.x).hypot(c.pos.y - pos.y) <= SHRINE_SPEED_RADIUS;
        if !c.dead && c.team == team && near {
            allies.push(c.id);
        }
    }
    for champ in allies {
        let Some(c) = world.get_mut(champ) else {
            continue;
        };
        if time < c.data.get::<NextScuttleSpeed>().map_or(0.0, |n| n.0) {
            continue;
        }
        c.data.insert(NextScuttleSpeed(time + 0.75));
        add_buff(
            world,
            champ,
            "scuttle_speed",
            BuffOptions {
                params: vec![("moveSpeedPercent".into(), 0.2)],
                ..BuffOptions::new(1.0)
            },
        );
    }
}

fn update(
    world: &mut World,
    dt: f64,
    camps: &JungleCamps,
    core_combat: bool,
    ids: &mut Vec<EntityId>,
) {
    if world.winner.is_some() {
        return;
    }
    let time = world.time;
    for (i, camp) in CAMPS.iter().enumerate() {
        let mut spawn = false;
        {
            let mut st = camps.lock();
            let Some(state) = st.0.get_mut(i) else {
                continue;
            };
            let cleared = !state.ids.is_empty()
                && state
                    .ids
                    .iter()
                    .all(|&id| world.get(id).is_none_or(|e| e.dead));
            if cleared {
                state.ids.clear();
                state.next = time + respawn_delay(camp.monster);
            }
            if state.ids.is_empty() && time >= state.next {
                spawn = true;
            }
        }
        if !spawn {
            continue;
        }
        let mut spawned = vec![spawn_monster(
            world,
            camp.monster,
            camp.pos,
            camp.leash_radius,
            MonsterExtra {
                camp: Some(camp.id.clone()),
                ..MonsterExtra::default()
            },
        )];
        for small in camp.smalls.iter().flatten() {
            let pos = Vec2 {
                x: camp.pos.x + small.offset.x,
                y: camp.pos.y + small.offset.y,
            };
            spawned.push(spawn_monster(
                world,
                small.monster,
                pos,
                camp.leash_radius,
                MonsterExtra {
                    camp: Some(camp.id.clone()),
                    home: Some(camp.pos),
                    epic: false,
                },
            ));
        }
        if let Some(state) = camps.lock().0.get_mut(i) {
            state.ids = spawned;
        }
    }

    world.ids_into(None, ids);
    for &id in ids.iter() {
        let Some(e) = world.get(id) else { continue };
        let camp_monster = e
            .data
            .get::<MonsterState>()
            .is_some_and(|m| m.camp.is_some());
        let dead = e.dead;
        let shrine = (e.kind == EntityKind::Ward)
            .then(|| e.data.get::<ShrineData>().copied())
            .flatten();
        let (team, pos) = (e.team, e.pos);
        if camp_monster {
            if dead {
                retire(world, id, RETIRE_DELAY);
            } else {
                update_monster(world, id, dt, core_combat);
            }
        }
        if let Some(shrine) = shrine {
            if time >= shrine.expires_at {
                world.remove(id);
                continue;
            }
            update_shrine(world, team, pos);
        }
        while let Some(source) = take_burn_tick(world, id, BurnKey::Red) {
            apply_damage(
                world,
                DamageRequest {
                    source,
                    target: id,
                    amount: 12.0,
                    damage_type: DamageType::True,
                    ability: Some("red_burn".into()),
                },
            );
        }
        if let Some(e) = world.get_mut(id)
            && !e.dead
            && e.data
                .get::<JungleBuffs>()
                .and_then(|b| b.blue)
                .is_some_and(|t| t > time)
        {
            e.mana = e.stats.max_mana.min(e.mana + e.stats.max_mana * 0.01 * dt);
        }
    }
}

/// Le système de jungle et la poignée sur ses camps (timers d'apparition).
pub fn jungle_system_with_camps(core_combat: bool) -> (System, JungleCamps) {
    let camps = JungleCamps::default();
    let for_init = camps.clone();
    let for_update = camps.clone();
    let mut ids: Vec<EntityId> = Vec::new();
    let system = System::new("jungle", move |world, dt| {
        update(world, dt, &for_update, core_combat, &mut ids);
    })
    .with_init(move |world| init_jungle(world, &for_init));
    (system, camps)
}

/// Le système de jungle. `core_combat` : vrai quand le système `combat` fait partie de la partie.
pub fn jungle_system(core_combat: bool) -> System {
    jungle_system_with_camps(core_combat).0
}
