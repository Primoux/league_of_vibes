//! Moteur de sorts : port de `packages/server/src/abilities/engine.ts`.
//!
//! `cast_ability` valide et dépense une fois les ressources ; un sort ciblé hors de portée devient un
//! ordre de déplacement annulable (`AbilityState::pending`). `update_abilities` fait avancer les temps
//! de recharge, les incantations, les commandes de lancer, puis les primitives et les passifs.
//!
//! Différences avec le TypeScript, imposées par Rust ou par la découpe en kits :
//! - `entity.data.abilities` est `AbilityState`, `data.casting` est le marqueur `Casting` de
//!   `combat::buffs` et `data.castAnimUntil` est `CastAnimUntil` ;
//! - les cas par champion du TS (cible alliée d'Ilyra et de Solenne, cible de Nyx, ultimes réservés aux
//!   champions, sort renforcé de Zephyr) sont des données du kit (`TargetRule`, `Kit::on_cast`) ;
//! - `cooldownWithHaste` est défini ici (`systems/leveling.ts`, d'où il vient, n'est pas porté) ;
//! - `initPassives` n'est plus appelé à chaque lancer ni à chaque tick : c'est l'`init` du système
//!   `abilities`, une seule fois par monde.

pub use lov_shared::types::AbilitySlot;

use lov_shared::math::dist;
use lov_shared::protocol::ClientMsg;
use lov_shared::schema::{CcType, Resource, Targeting};
use lov_shared::types::{EntityId, EntityKind, GameEvent, Vec2};

use crate::abilities::context::{CastContext, at_rank, def_param};
use crate::abilities::passives::{PassivesScratch, update_passives};
use crate::abilities::primitives::{
    PrimitivesScratch, endpoint, is_unit, is_visible_target, update_primitives, valid_point,
};
use crate::abilities::registry::{Kit, TargetRule, champion_def, kit_for};
use crate::combat::buffs::{Casting, Stasis, break_invisibility, can_cast, has_cc};
use crate::systems::combat::cancel_attack;
use crate::systems::movement::DashData;
use crate::world::{ChampionState, Entity, QueuedCommand, World};

/// Les quatre emplacements, dans l'ordre des tableaux (Q, W, E, R).
pub const SLOTS: [AbilitySlot; 4] = [
    AbilitySlot::Q,
    AbilitySlot::W,
    AbilitySlot::E,
    AbilitySlot::R,
];

/// Index d'un emplacement dans les tableaux par sort.
pub const fn slot_index(slot: AbilitySlot) -> usize {
    match slot {
        AbilitySlot::Q => 0,
        AbilitySlot::W => 1,
        AbilitySlot::E => 2,
        AbilitySlot::R => 3,
    }
}

/// `CastCommand` : le message `cast` du protocole.
#[derive(Debug, Clone, PartialEq)]
pub struct CastCommand {
    pub slot: AbilitySlot,
    pub target_id: Option<EntityId>,
    pub target_pos: Option<Vec2>,
}

impl CastCommand {
    /// Le `cast` d'un message client (`None` pour tout autre message).
    pub fn from_msg(msg: &ClientMsg) -> Option<Self> {
        match msg {
            ClientMsg::Cast {
                slot,
                target_pos,
                target_id,
            } => Some(Self {
                slot: *slot,
                target_id: *target_id,
                target_pos: *target_pos,
            }),
            _ => None,
        }
    }
}

/// Sort à relancer (Q recastable…) : lancers restants et fenêtre.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Recast {
    pub remaining: f64,
    pub expires: f64,
    pub index: u32,
    pub ready_at: f64,
}

/// Incantation en cours.
#[derive(Debug, Clone)]
pub struct Windup {
    pub until: f64,
    pub context: CastContext,
    kit: &'static Kit,
}

/// `entity.data.abilities`.
#[derive(Debug, Clone, Default)]
pub struct AbilityState {
    pub pending: Option<CastCommand>,
    pub windup: Option<Windup>,
    /// Q, W, E, R.
    pub recasts: [Option<Recast>; 4],
    pub casts: u32,
}

/// `entity.data.castAnimUntil` : l'animation de lancer finit à ce temps de jeu.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CastAnimUntil(pub f64);

/// `abilityState(e)` : créé à la demande.
pub fn ability_state(e: &mut Entity) -> &mut AbilityState {
    e.data.get_or_insert_with(AbilityState::default)
}

/// `cooldownWithHaste(seconds, haste)`.
pub fn cooldown_with_haste(seconds: f64, haste: f64) -> f64 {
    seconds * 100.0 / (100.0 + haste.max(0.0))
}

/// `c.cooldowns[slot]`.
pub fn cooldown(c: &ChampionState, slot: AbilitySlot) -> f64 {
    match slot {
        AbilitySlot::Q => c.cooldowns.q,
        AbilitySlot::W => c.cooldowns.w,
        AbilitySlot::E => c.cooldowns.e,
        AbilitySlot::R => c.cooldowns.r,
    }
}

/// `c.cooldowns[slot] = seconds`.
pub fn set_cooldown(c: &mut ChampionState, slot: AbilitySlot, seconds: f64) {
    match slot {
        AbilitySlot::Q => c.cooldowns.q = seconds,
        AbilitySlot::W => c.cooldowns.w = seconds,
        AbilitySlot::E => c.cooldowns.e = seconds,
        AbilitySlot::R => c.cooldowns.r = seconds,
    }
}

/// `isMobility(targeting)`.
pub fn is_mobility(targeting: Targeting) -> bool {
    matches!(targeting, Targeting::Dash | Targeting::TargetedDash)
}

fn resolve_target(
    world: &World,
    source_id: EntityId,
    target_id: Option<EntityId>,
    rule: TargetRule,
) -> Option<EntityId> {
    let source = world.get(source_id)?;
    let ally = rule == TargetRule::AllyChampion;
    let target = if target_id.is_none() && ally {
        source
    } else {
        world.get(target_id?)?
    };
    if target.dead
        || target.data.contains::<Stasis>()
        || !is_visible_target(world, source, target)
        || !world.is_targetable(target.id, source_id)
    {
        return None;
    }
    if ally {
        return (target.team == source.team && target.kind == EntityKind::Champion)
            .then_some(target.id);
    }
    if rule == TargetRule::UnitOrWard {
        return ((is_unit(target) || target.kind == EntityKind::Ward) && target.id != source_id)
            .then_some(target.id);
    }
    if target.team == source.team || !is_unit(target) {
        return None;
    }
    if rule == TargetRule::EnemyChampion && target.champ.is_none() {
        return None;
    }
    Some(target.id)
}

/// Annule l'incantation en cours (CC, mort, cible disparue).
pub fn interrupt_cast(e: &mut Entity) {
    ability_state(e).windup = None;
    e.data.remove::<Casting>();
    if !e.dead && e.anim.as_deref().is_some_and(|a| a.starts_with("cast_")) {
        e.anim = Some("idle".to_owned());
    }
}

fn execute(world: &mut World, ctx: &CastContext, kit: &Kit) {
    (kit.abilities[slot_index(ctx.slot)])(world, ctx);
    let until = world.time + 0.2;
    if let Some(e) = world.get_mut(ctx.source) {
        e.data.insert(CastAnimUntil(until));
    }
}

/// Valide et dépense une fois les ressources. Un sort ciblé hors de portée devient un ordre de
/// déplacement annulable (si `allow_queue`) et renvoie faux. Vrai si le sort est lancé (ou en
/// incantation).
pub fn cast_ability(
    world: &mut World,
    source_id: EntityId,
    command: &CastCommand,
    allow_queue: bool,
) -> bool {
    let Some(source) = world.get(source_id) else {
        return false;
    };
    let Some(c) = source.champ.as_ref() else {
        return false;
    };
    if !can_cast(source) || source.data.contains::<DashData>() {
        return false;
    }
    let slot = command.slot;
    let idx = slot_index(slot);
    let Some(champ_def) = champion_def(&c.champion_id) else {
        return false;
    };
    let def = champ_def.abilities.get(slot);
    let Some(kit) = kit_for(&c.champion_id) else {
        return false;
    };
    let rank = *c.ability_levels.get(slot);
    if rank < 1 {
        return false;
    }
    if is_mobility(def.targeting)
        && (has_cc(source, CcType::Root) || has_cc(source, CcType::Grounded))
    {
        return false;
    }
    let (pos, radius, mana, cd) = (source.pos, source.radius, source.mana, cooldown(c, slot));
    let time = world.time;
    let recast = {
        let Some(e) = world.get_mut(source_id) else {
            return false;
        };
        let state = ability_state(e);
        if let Some(r) = state.recasts[idx]
            && time > r.expires + 1e-8
        {
            state.recasts[idx] = None;
        }
        state.recasts[idx]
    };
    if (recast.is_none() && cd > 1e-8) || recast.is_some_and(|r| time < r.ready_at - 1e-8) {
        return false;
    }
    let cost = if recast.is_some() {
        0.0
    } else {
        at_rank(&def.cost, rank, 0.0)
    };
    let uses_resource = champ_def.resource != Resource::None;
    if uses_resource && mana + 1e-8 < cost {
        return false;
    }
    let targeted = matches!(def.targeting, Targeting::Targeted | Targeting::TargetedDash);
    let target = if targeted {
        resolve_target(world, source_id, command.target_id, kit.targeting[idx])
    } else {
        None
    };
    if targeted && target.is_none() {
        return false;
    }
    let target_pos = target.and_then(|t| world.get(t)).map(|t| (t.pos, t.radius));
    if let Some((tpos, tradius)) = target_pos
        && dist(pos, tpos) > def.range + radius + tradius
    {
        if allow_queue && let Some(e) = world.get_mut(source_id) {
            ability_state(e).pending = Some(command.clone());
            e.move_target = Some(tpos);
            e.path = None;
        }
        return false;
    }
    if !targeted && def.targeting != Targeting::SelfCast && !valid_point(command.target_pos) {
        return false;
    }
    let desired = target_pos
        .map(|t| t.0)
        .or(command.target_pos)
        .unwrap_or(pos);
    let point = if matches!(def.targeting, Targeting::Skillshot | Targeting::Cone) {
        desired
    } else {
        endpoint(pos, desired, def.range)
    };
    if let Some(e) = world.get_mut(source_id) {
        ability_state(e).pending = None;
        e.move_target = None;
        e.path = None;
        e.attack_target = None;
        cancel_attack(e);
    }
    break_invisibility(world, source_id);
    let Some(e) = world.get_mut(source_id) else {
        return false;
    };
    if uses_resource {
        e.mana -= cost;
    }
    if recast.is_none()
        && let Some(c) = e.champ.as_mut()
    {
        set_cooldown(
            c,
            slot,
            cooldown_with_haste(at_rank(&def.cooldown, rank, 0.0), e.stats.ability_haste),
        );
    }
    let cast_index = recast.map_or(0, |r| r.index);
    let state = ability_state(e);
    match recast {
        Some(mut r) => {
            r.remaining -= 1.0;
            r.index += 1;
            r.ready_at = time + def_param(def, "recastDelay", rank, 0.3);
            state.recasts[idx] = (r.remaining > 0.0).then_some(r);
        }
        None => {
            let charges = def_param(def, "charges", rank, 1.0);
            let remaining = def_param(def, "recasts", rank, (charges - 1.0).max(0.0));
            if remaining > 0.0 {
                state.recasts[idx] = Some(Recast {
                    remaining,
                    expires: time + def_param(def, "window", rank, 4.0),
                    index: 1,
                    ready_at: time + def_param(def, "recastDelay", rank, 0.3),
                });
            }
        }
    }
    let casts = state.casts;
    state.casts += 1;
    let amplification = kit.on_cast.map_or(1.0, |f| f(world, source_id, casts));
    let ctx = CastContext {
        source: source_id,
        def,
        slot,
        rank,
        point,
        target,
        cast_index,
        amplification,
    };
    if let Some(e) = world.get_mut(source_id) {
        e.anim = Some(format!("cast_{slot:?}"));
        e.facing = (point.y - e.pos.y).atan2(point.x - e.pos.x);
    }
    world.emit(GameEvent::AbilityCast {
        source: source_id,
        champion_id: champ_def.id.clone(),
        slot,
        ability_id: def.id.clone(),
        target_pos: Some(point),
        target_id: target,
        fx_key: Some(def.fx_key.clone()),
    });
    let cast_time = def.cast_time.unwrap_or(0.0);
    if cast_time > 0.0 {
        if let Some(e) = world.get_mut(source_id) {
            e.data.insert(Casting);
            ability_state(e).windup = Some(Windup {
                until: time + cast_time,
                context: ctx,
                kit,
            });
        }
    } else {
        execute(world, &ctx, kit);
    }
    true
}

/// Tampons réutilisés d'un tick à l'autre.
#[derive(Default)]
pub struct AbilityScratch {
    champions: Vec<EntityId>,
    primitives: PrimitivesScratch,
    passives: PassivesScratch,
}

/// `updateAbilities(world, dt)` ; le système `abilities` réutilise ses tampons (voir `update_abilities_with`).
pub fn update_abilities(world: &mut World, dt: f64) {
    update_abilities_with(world, dt, &mut AbilityScratch::default());
}

/// Un tick du moteur : recharges, incantations, commandes `cast`, sorts en attente, primitives, passifs.
pub fn update_abilities_with(world: &mut World, dt: f64, sc: &mut AbilityScratch) {
    world.ids_into(Some(EntityKind::Champion), &mut sc.champions);
    for &id in &sc.champions {
        update_champion(world, id, dt);
    }
    let commands = std::mem::take(&mut world.commands);
    for QueuedCommand { player_id, msg } in &commands {
        let Some(id) = world.champion_id_of(player_id) else {
            continue;
        };
        let clears_pending = matches!(
            msg,
            ClientMsg::Move { .. }
                | ClientMsg::AttackMove { .. }
                | ClientMsg::Attack { .. }
                | ClientMsg::Stop
                | ClientMsg::Recall
        );
        let cast = CastCommand::from_msg(msg);
        if !clears_pending && cast.is_none() {
            continue;
        }
        if let Some(e) = world.get_mut(id) {
            ability_state(e).pending = None;
        }
        if let Some(command) = cast {
            cast_ability(world, id, &command, true);
        }
    }
    // Commandes ajoutées pendant le parcours : après les existantes (comme le système `input`).
    let added = std::mem::replace(&mut world.commands, commands);
    world.commands.extend(added);
    for &id in &sc.champions {
        retry_pending(world, id);
    }
    update_primitives(world, dt, &mut sc.primitives);
    update_passives(world, dt, &mut sc.passives);
}

enum Windowed {
    None,
    Interrupt,
    Execute(CastContext, &'static Kit),
}

fn update_champion(world: &mut World, id: EntityId, dt: f64) {
    let time = world.time;
    let Some(e) = world.get_mut(id) else { return };
    for slot in SLOTS {
        if let Some(c) = e.champ.as_mut() {
            let left = (cooldown(c, slot) - dt).max(0.0);
            set_cooldown(c, slot, left);
        }
        let state = ability_state(e);
        if state.recasts[slot_index(slot)].is_some_and(|r| time > r.expires) {
            state.recasts[slot_index(slot)] = None;
        }
    }
    if e.dead {
        interrupt_cast(e);
        let state = ability_state(e);
        state.pending = None;
        state.recasts = [None; 4];
        return;
    }
    let windup = ability_state(e).windup.clone();
    let Some(windup) = windup else {
        let until = e.data.get::<CastAnimUntil>().map_or(0.0, |c| c.0);
        if e.anim.as_deref().is_some_and(|a| a.starts_with("cast_")) && until < time {
            e.anim = Some("idle".to_owned());
        }
        return;
    };
    e.move_target = None;
    e.path = None;
    let ctx = windup.context;
    let mobility_blocked =
        is_mobility(ctx.def.targeting) && (has_cc(e, CcType::Root) || has_cc(e, CcType::Grounded));
    let controlled = [
        CcType::Stun,
        CcType::Silence,
        CcType::Knockup,
        CcType::Suppress,
        CcType::Taunt,
        CcType::Fear,
    ]
    .into_iter()
    .any(|cc| has_cc(e, cc));
    let target_gone = ctx
        .target
        .is_some_and(|t| world.get(t).is_none_or(|t| t.dead));
    let action = if controlled || mobility_blocked || target_gone {
        Windowed::Interrupt
    } else if time + 1e-8 >= windup.until {
        Windowed::Execute(ctx, windup.kit)
    } else {
        Windowed::None
    };
    match action {
        Windowed::None => {}
        Windowed::Interrupt => {
            if let Some(e) = world.get_mut(id) {
                interrupt_cast(e);
            }
        }
        Windowed::Execute(ctx, kit) => {
            if let Some(e) = world.get_mut(id) {
                ability_state(e).windup = None;
                e.data.remove::<Casting>();
            }
            execute(world, &ctx, kit);
        }
    }
}

/// Sort ciblé en attente : on le relance chaque tick tant que la cible reste visible, sinon on abandonne.
fn retry_pending(world: &mut World, id: EntityId) {
    let Some(pending) = world
        .get_mut(id)
        .and_then(|e| ability_state(e).pending.clone())
    else {
        return;
    };
    let target = pending.target_id.and_then(|t| world.get(t)).filter(|t| {
        !t.dead
            && world
                .get(id)
                .is_some_and(|e| is_visible_target(world, e, t))
    });
    let Some(tpos) = target.map(|t| t.pos) else {
        if let Some(e) = world.get_mut(id) {
            ability_state(e).pending = None;
            e.move_target = None;
        }
        return;
    };
    if !cast_ability(world, id, &pending, false)
        && world.get(id).is_some_and(can_cast)
        && let Some(e) = world.get_mut(id)
    {
        e.move_target = Some(tpos);
    }
}
