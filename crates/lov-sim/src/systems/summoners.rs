//! Sorts d'invocateur : port de `packages/server/src/systems/summoners.ts`.
//!
//! Saut éclair, Embrasement, Soins, Barrière, Fatigue, Téléportation, Châtiment, Fantôme, Purge.
//! Les valeurs viennent de `lov_shared::data::summoners` ; les recharges ne sont pas réduites par
//! l'accélération de compétence.
//!
//! Différences avec le TypeScript :
//! - la canalisation de Téléportation désigne sa cible par `EntityId` (le TS gardait l'objet) ;
//! - `init` ne s'exécute qu'une fois par monde (`World::init`) : le `WeakSet` `initialized` n'a pas
//!   d'équivalent ;
//! - les tics d'Embrasement sont des tâches `schedule` portées par le lanceur (en TS, des tâches du
//!   monde) : elles s'exécutent même s'il meurt, comme en TS.

use lov_shared::data::summoners::summoner;
use lov_shared::math::dist2;
use lov_shared::protocol::ClientMsg;
use lov_shared::schema::{CcType, Targeting};
use lov_shared::types::{
    DamageType, EntityId, EntityKind, GameEvent, PartialStats, SummonerId, SummonerSlot, Vec2,
};

use crate::abilities::effects::{ShieldOptions, apply_shield};
use crate::abilities::primitives::{
    blink, is_visible_target, schedule, valid_landing, valid_point,
};
use crate::combat::buffs::{
    BuffDefOverride, BuffOptions, BuffParams, Casting, Channeling, Stasis, add_buff, can_act,
    cleanse, has_cc,
};
use crate::combat::damage::{DamageOpts, HealOpts, apply_damage, apply_heal};
use crate::systems::movement::DashData;
use crate::world::{Entity, EventKind, System, World};

/// Canalisation de Téléportation en cours.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TeleportChannel {
    pub until: f64,
    pub target: EntityId,
    pub from: Vec2,
    pub interrupted: bool,
    /// PV du lanceur à la dernière vérification : un dégât qui les fait baisser interrompt la
    /// canalisation, un dégât absorbé par un bouclier non (#318).
    pub hp: f64,
}

/// `entity.data.summoners`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SummonerState {
    pub channel: Option<TeleportChannel>,
}

/// Paramètre numérique d'un sort (0 s'il manque : les données sont vérifiées par les tests).
fn param(id: SummonerId, key: &str) -> f64 {
    summoner(id)
        .params
        .as_ref()
        .and_then(|p| p.get(key))
        .copied()
        .unwrap_or(0.0)
}

fn slot_value<T: Copy>(map: &lov_shared::types::SummonerMap<T>, slot: SummonerSlot) -> T {
    match slot {
        SummonerSlot::D => map.d,
        SummonerSlot::F => map.f,
    }
}

fn can_use(e: &Entity, id: SummonerId) -> bool {
    if e.dead || e.data.contains::<Stasis>() {
        return false;
    }
    if id == SummonerId::Cleanse {
        return !has_cc(e, CcType::Suppress);
    }
    if !can_act(e) || e.data.contains::<Channeling>() {
        return false;
    }
    let movement_spell = matches!(id, SummonerId::Flash | SummonerId::Teleport);
    !(movement_spell
        && (has_cc(e, CcType::Root)
            || has_cc(e, CcType::Grounded)
            || has_cc(e, CcType::Silence)
            || e.data.contains::<Casting>()
            || e.data.contains::<DashData>()))
}

/// La cible convient-elle au sort ciblé `id` ?
fn valid_target(world: &World, e: &Entity, id: SummonerId, range: f64, target: &Entity) -> bool {
    let reach = range + e.radius + target.radius;
    if target.dead || target.data.contains::<Stasis>() || dist2(e.pos, target.pos) > reach * reach {
        return false;
    }
    if id == SummonerId::Teleport {
        return target.team == e.team
            && matches!(
                target.kind,
                EntityKind::Tower
                    | EntityKind::Minion
                    | EntityKind::Ward
                    | EntityKind::Inhibitor
                    | EntityKind::Nexus
            );
    }
    if target.team == e.team || !is_visible_target(world, e, target) {
        return false;
    }
    if id == SummonerId::Smite {
        matches!(target.kind, EntityKind::Monster | EntityKind::Minion)
    } else {
        target.kind == EntityKind::Champion
    }
}

/// Lance le sort d'invocateur de l'emplacement `slot`. Faux si le lancer est refusé.
pub fn cast_summoner(
    world: &mut World,
    id: EntityId,
    slot: SummonerSlot,
    target_id: Option<EntityId>,
    target_pos: Option<Vec2>,
) -> bool {
    let Some(e) = world.get(id) else { return false };
    let Some(c) = &e.champ else { return false };
    let spell = slot_value(&c.summoners, slot);
    let def = summoner(spell);
    let level = f64::from(c.level);
    let cooldown = match slot {
        SummonerSlot::D => c.cooldowns.d,
        SummonerSlot::F => c.cooldowns.f,
    };
    if cooldown > 1e-8 || !can_use(e, spell) {
        return false;
    }
    let target = world.get_opt(target_id);
    if def.targeting == Targeting::Targeted
        && !target.is_some_and(|t| valid_target(world, e, spell, def.range, t))
    {
        return false;
    }
    if spell == SummonerId::Flash && !valid_point(target_pos) {
        return false;
    }
    let from = e.pos;
    let target = target.map(|t| t.id);
    let p = |key| param(spell, key);

    if let Some(c) = world.get_mut(id).and_then(|e| e.champ.as_mut()) {
        match slot {
            SummonerSlot::D => c.cooldowns.d = def.cooldown,
            SummonerSlot::F => c.cooldowns.f = def.cooldown,
        }
    }
    match spell {
        SummonerId::Flash => {
            if let Some(point) = target_pos {
                blink(world, id, point, def.range, &def.fx_key);
            }
        }
        SummonerId::Ignite => {
            let Some(victim) = target else { return false };
            let duration = p("duration");
            add_buff(world, victim, "ignite", {
                let mut o = BuffOptions::new(duration);
                o.source = Some(id);
                o.params.grievous = Some(p("grievous"));
                o
            });
            let per_tick = (p("baseDamage") + p("damagePerLevel") * level) / duration;
            let ticks = duration as u32;
            for i in 1..=ticks {
                schedule(world, id, f64::from(i), move |world| {
                    if world.get(victim).is_some_and(|v| !v.dead) {
                        apply_damage(
                            world,
                            DamageOpts {
                                ability: Some("sum_ignite".into()),
                                ..DamageOpts::new(Some(id), victim, per_tick, DamageType::True)
                            },
                        );
                    }
                });
            }
        }
        SummonerId::Heal => {
            let range2 = def.range * def.range;
            let mut allies: Vec<(f64, EntityId)> = world
                .query(Some(EntityKind::Champion))
                .filter(|a| {
                    a.id != id && a.team == world.get(id).map_or(a.team, |e| e.team) && !a.dead
                })
                .filter(|a| dist2(a.pos, from) <= range2)
                .map(|a| (a.hp / a.stats.max_hp, a.id))
                .collect();
            allies.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            let amount = p("baseHeal") + level * p("healPerLevel");
            let recipients = std::iter::once(id).chain(allies.first().map(|a| a.1));
            for ally in recipients.collect::<Vec<_>>() {
                apply_heal(world, HealOpts::new(Some(id), ally, amount));
                add_buff(world, ally, "sum_heal_speed", {
                    let mut o = BuffOptions::new(p("speedDuration"));
                    o.source = Some(id);
                    o.params.stat_percent_mods = Some(PartialStats {
                        move_speed: Some(p("speed")),
                        ..PartialStats::default()
                    });
                    o.definition = Some(BuffDefOverride {
                        fx_key: Some(def.fx_key.clone()),
                        ..BuffDefOverride::default()
                    });
                    o
                });
            }
        }
        SummonerId::Barrier => {
            apply_shield(
                world,
                ShieldOptions {
                    duration: Some(p("duration")),
                    buff_id: Some("sum_barrier".into()),
                    fx_key: Some(def.fx_key.clone()),
                    ..ShieldOptions::new(id, id, p("baseShield") + p("shieldPerLevel") * level)
                },
            );
        }
        SummonerId::Exhaust => {
            let Some(victim) = target else { return false };
            add_buff(world, victim, "exhaust", {
                let mut o = BuffOptions::new(p("duration"));
                o.source = Some(id);
                o.params = BuffParams {
                    amount: Some(p("slow")),
                    damage_dealt_reduction: Some(p("damageReduction")),
                    ..BuffParams::default()
                };
                o
            });
        }
        SummonerId::Teleport => {
            let Some(dest) = target else { return false };
            let Some(dest_pos) = world.get(dest).map(|t| t.pos) else {
                return false;
            };
            let channel = p("channelDuration");
            let until = world.time + channel;
            if let Some(e) = world.get_mut(id) {
                let hp = e.hp;
                e.data.get_or_insert_with(SummonerState::default).channel = Some(TeleportChannel {
                    until,
                    target: dest,
                    from,
                    interrupted: false,
                    hp,
                });
                e.data.insert(Channeling);
                e.move_target = None;
                e.path = None;
                e.attack_target = None;
                e.anim = Some("channel".into());
            }
            world.emit(GameEvent::AreaEffect {
                source: id,
                fx_key: "sum_teleport_channel".into(),
                pos: dest_pos,
                radius: 150.0,
                duration: channel,
            });
        }
        SummonerId::Smite => {
            let Some(victim) = target else { return false };
            apply_damage(
                world,
                DamageOpts {
                    ability: Some("sum_smite".into()),
                    ..DamageOpts::new(Some(id), victim, p("damage"), DamageType::True)
                },
            );
            let is_monster = world
                .get(victim)
                .is_some_and(|v| v.kind == EntityKind::Monster);
            if is_monster && let Some(max_hp) = world.get(id).map(|e| e.stats.max_hp) {
                apply_heal(world, HealOpts::new(Some(id), id, max_hp * p("healMaxHp")));
            }
        }
        SummonerId::Ghost => {
            let speed = p("speedMin") + (p("speedMax") - p("speedMin")) * (level - 1.0) / 17.0;
            add_buff(world, id, "sum_ghost", {
                let mut o = BuffOptions::new(p("duration"));
                o.source = Some(id);
                o.params.ghosted = Some(true);
                o.params.stat_percent_mods = Some(PartialStats {
                    move_speed: Some(speed),
                    ..PartialStats::default()
                });
                o.definition = Some(BuffDefOverride {
                    fx_key: Some(def.fx_key.clone()),
                    ..BuffDefOverride::default()
                });
                o
            });
        }
        SummonerId::Cleanse => {
            cleanse(world, id);
            add_buff(world, id, "cleanse", {
                let mut o = BuffOptions::new(p("duration"));
                o.source = Some(id);
                o.params.tenacity = Some(p("tenacity"));
                o
            });
        }
    }
    let to = if spell == SummonerId::Flash {
        world.get(id).map(|e| e.pos)
    } else {
        world.get_opt(target).map(|t| t.pos)
    };
    world.emit(GameEvent::SummonerCast {
        source: id,
        spell,
        from,
        to,
        target_id: target,
        fx_key: Some(def.fx_key.clone()),
    });
    true
}

fn init_summoners(world: &mut World) {
    world.events.on(EventKind::Damage, |world, ev| {
        let GameEvent::Damage { target, .. } = ev else {
            return;
        };
        // Seuls les PV perdus comptent : un coup absorbé par un bouclier ne coupe rien (#318).
        let Some(e) = world.get_mut(*target) else {
            return;
        };
        let hp = e.hp;
        if let Some(ch) = e
            .data
            .get_mut::<SummonerState>()
            .and_then(|s| s.channel.as_mut())
            && hp < ch.hp - 1e-9
        {
            ch.interrupted = true;
        }
    });
}

/// Une commande de ce type interrompt la canalisation de Téléportation.
fn interrupts_channel(msg: &ClientMsg) -> bool {
    matches!(
        msg,
        ClientMsg::Move { .. }
            | ClientMsg::AttackMove { .. }
            | ClientMsg::Attack { .. }
            | ClientMsg::Stop
            | ClientMsg::Cast { .. }
            | ClientMsg::Recall
            | ClientMsg::Summoner { .. }
    )
}

fn update_channel(world: &mut World, id: EntityId) {
    let time = world.time;
    let Some(e) = world.get(id) else { return };
    let Some(ch) = e.data.get::<SummonerState>().and_then(|s| s.channel) else {
        return;
    };
    let player = e.champ.as_ref().map(|c| c.player_id.as_str());
    let moved_or_acted = world
        .commands
        .iter()
        .any(|q| Some(q.player_id.as_str()) == player && interrupts_channel(&q.msg));
    let dest = world.get(ch.target).filter(|t| !t.dead && t.team == e.team);
    let interrupted = ch.interrupted
        || e.dead
        || !can_act(e)
        || has_cc(e, CcType::Silence)
        || has_cc(e, CcType::Root)
        || dest.is_none()
        || moved_or_acted
        || dist2(e.pos, ch.from) > 1.0;
    // Dégâts silencieux (brûlures…), qui n'émettent pas d'événement : PV en baisse depuis le tick
    // précédent.
    let interrupted = interrupted || e.hp < ch.hp - 1e-9;
    if !interrupted && time + 1e-8 < ch.until {
        if let Some(e) = world.get_mut(id) {
            e.move_target = None;
            e.path = None;
            let hp = e.hp;
            if let Some(ch) = e
                .data
                .get_mut::<SummonerState>()
                .and_then(|s| s.channel.as_mut())
            {
                ch.hp = hp;
            }
        }
        return;
    }
    if !interrupted && let Some(d) = dest {
        let (radius, from) = (e.radius, e.pos);
        let to = valid_landing(
            d.pos,
            Vec2 {
                x: d.pos.x + d.radius + radius + 10.0,
                y: d.pos.y,
            },
            350.0,
            radius,
        );
        if let Some(e) = world.get_mut(id) {
            e.pos = to;
        }
        world.emit(GameEvent::Dash {
            source: id,
            from,
            to,
            duration: 0.0,
            fx_key: Some("sum_teleport".into()),
        });
    }
    let Some(e) = world.get_mut(id) else { return };
    if let Some(s) = e.data.get_mut::<SummonerState>() {
        s.channel = None;
    }
    e.data.remove::<Channeling>();
    if !e.dead {
        e.anim = Some("idle".into());
    }
    let pos = e.pos;
    world.emit(GameEvent::AreaEffect {
        source: id,
        fx_key: if interrupted {
            "sum_teleport_cancel"
        } else {
            "sum_teleport_arrive"
        }
        .into(),
        pos,
        radius: 150.0,
        duration: 0.3,
    });
}

fn update(world: &mut World, dt: f64, ids: &mut Vec<EntityId>) {
    world.ids_into(Some(EntityKind::Champion), ids);
    for &id in ids.iter() {
        if let Some(c) = world.get_mut(id).and_then(|e| e.champ.as_mut()) {
            c.cooldowns.d = (c.cooldowns.d - dt).max(0.0);
            c.cooldowns.f = (c.cooldowns.f - dt).max(0.0);
        }
        update_channel(world, id);
    }
    for i in 0..world.commands.len() {
        let ClientMsg::Summoner {
            slot,
            target_pos,
            target_id,
        } = world.commands[i].msg
        else {
            continue;
        };
        if let Some(id) = world.champion_id_of(&world.commands[i].player_id) {
            cast_summoner(world, id, slot, target_id, target_pos);
        }
    }
}

/// Le système des sorts d'invocateur : recharges, canalisation de Téléportation, puis commandes.
pub fn summoners_system() -> System {
    let mut ids: Vec<EntityId> = Vec::new();
    System::new("summoners", move |world, dt| update(world, dt, &mut ids)).with_init(init_summoners)
}
