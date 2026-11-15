//! Le Touilleur : port de `packages/server/src/abilities/champions/touilleur.ts`.
//!
//! - Q : louche lancée qui blesse et ralentit le premier ennemi.
//! - W : zone télégraphiée qui blesse et ralentit.
//! - E : saut éclair et bouclier.
//! - R : apparaît dans le dos d'un champion ennemi, le frappe, l'étourdit et canalise (« touille »)
//!   tant que l'étourdissement dure. La canalisation s'arrête à la moindre rupture : contrôle ou
//!   silence du lanceur, ordre, déplacement forcé d'un des deux, stase, mort, purification.
//! - Passif : trois sorts espacés qui touchent un champion infligent des dégâts bonus et donnent de
//!   la vitesse.
//!
//! Différences avec le TypeScript : la canalisation (un `Map` par monde et une fermeture `end`) est la
//! donnée [`TouilleurChannel`] du lanceur, identifiée par un numéro de lancer ; l'étourdissement de R
//! porte ce numéro (`touilleurSeq`), ce qui remplace la comparaison d'identité d'objet du TS.

use std::sync::atomic::{AtomicU64, Ordering};

use lov_shared::constants::TICK_DT;
use lov_shared::data::champions::get_champion;
use lov_shared::math::{dist, dist2};
use lov_shared::protocol::ClientMsg;
use lov_shared::schema::{BuffKind, CcType};
use lov_shared::types::{DamageType, EntityId, GameEvent, PartialStats, Vec2};

use crate::abilities::engine::CastAnimUntil;
use crate::abilities::prelude::*;
use crate::combat::buffs::{
    BuffDefOverride, BuffOptions, Channeling, SourceMatch, Stasis, add_buff, can_act, get_buff,
    has_cc, remove_buff,
};
use crate::combat::damage::DamageOpts;
use crate::world::EventKind;

fn passive(key: &str) -> f64 {
    get_champion("touilleur").map_or(0.0, |d| def_param(&d.passive, key, 1, 0.0))
}

fn q(world: &mut World, ctx: &CastContext) {
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

fn w(world: &mut World, ctx: &CastContext) {
    let ctx = *ctx;
    let radius = ctx.def.radius.unwrap_or(0.0);
    delayed_area(
        world,
        ctx.source,
        ctx.point,
        radius,
        param(&ctx, "delay"),
        &ctx.def.fx_key,
        move |world| {
            for target in enemies_in_radius(world, ctx.source, ctx.point, radius) {
                hit(world, &ctx, target);
                cc(
                    world,
                    &ctx,
                    target,
                    CcType::Slow,
                    param(&ctx, "slowDuration"),
                    Some(param(&ctx, "slow")),
                );
            }
        },
    );
}

fn e(world: &mut World, ctx: &CastContext) {
    blink(world, ctx.source, ctx.point, ctx.def.range, &ctx.def.fx_key);
    let ap = ctx.source(world).map_or(0.0, |s| s.stats.ap);
    let mut shield = ShieldOptions::new(
        ctx.source,
        ctx.source,
        param(ctx, "shield") + ap * param(ctx, "shieldAp"),
    );
    shield.duration = Some(param(ctx, "duration"));
    shield.buff_id = Some(ctx.def.id.clone());
    apply_shield(world, shield);
}

// ------------------------------------------------------------------------------------------------
// R et sa canalisation
// ------------------------------------------------------------------------------------------------

/// Canalisation de R en cours, portée par le lanceur.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TouilleurChannel {
    pub seq: u64,
    pub target: EntityId,
    pub origin: Vec2,
    pub until: f64,
}

static SEQ: AtomicU64 = AtomicU64::new(1);
const SEQ_KEY: &str = "touilleurSeq";

/// L'étourdissement de `target` posé par ce lancer de R est-il toujours en place ?
fn stun_holds(world: &World, source: EntityId, target: EntityId, seq: u64) -> bool {
    world
        .get(target)
        .and_then(|t| get_buff(t, "stun", Some(source)))
        .is_some_and(|b| b.params.number(SEQ_KEY) == Some(seq as f64))
}

/// Termine la canalisation `seq` du lanceur (sans effet si elle est déjà finie ou remplacée).
fn end_channel(world: &mut World, source: EntityId, seq: u64) {
    let Some(ch) = world
        .get(source)
        .and_then(|s| s.data.get::<TouilleurChannel>())
        .copied()
        .filter(|c| c.seq == seq)
    else {
        return;
    };
    if let Some(s) = world.get_mut(source) {
        s.data.remove::<TouilleurChannel>();
        s.data.remove::<Channeling>();
    }
    if stun_holds(world, source, ch.target, seq) {
        remove_buff(world, ch.target, "stun", SourceMatch::Id(source));
    }
    remove_buff(
        world,
        source,
        "touilleur_r_channel",
        SourceMatch::Id(source),
    );
    if let Some(s) = world.get_mut(source)
        && !s.dead
    {
        s.anim = Some("idle".into());
    }
}

/// Un ordre du joueur qui rompt la canalisation.
fn cancels(msg: &ClientMsg) -> bool {
    matches!(
        msg,
        ClientMsg::Move { .. }
            | ClientMsg::AttackMove { .. }
            | ClientMsg::Attack { .. }
            | ClientMsg::Stop
            | ClientMsg::Recall
    )
}

/// Un tick de canalisation : vérifie chaque condition de rupture, sinon fige le lanceur face à sa
/// cible et se reprogramme au tick suivant.
fn channel_tick(world: &mut World, source: EntityId, seq: u64, break_range: f64) {
    let Some(ch) = world
        .get(source)
        .and_then(|s| s.data.get::<TouilleurChannel>())
        .copied()
        .filter(|c| c.seq == seq)
    else {
        return;
    };
    let time = world.time;
    let ordered = world
        .get(source)
        .and_then(|s| s.champ.as_ref())
        .is_some_and(|c| {
            world
                .commands
                .iter()
                .any(|q| q.player_id == c.player_id && cancels(&q.msg))
        });
    let broken = match (world.get(source), world.get(ch.target)) {
        (Some(s), Some(t)) => {
            time >= ch.until - 1e-8
                || s.dead
                || t.dead
                || !can_act(s)
                || has_cc(s, CcType::Silence)
                || s.data.contains::<Stasis>()
                || t.data.contains::<Stasis>()
                || ordered
                || !stun_holds(world, source, ch.target, seq)
                || dist2(s.pos, ch.origin) > 25.0 * 25.0
                || dist2(s.pos, t.pos) > break_range * break_range
        }
        _ => true,
    };
    if broken {
        end_channel(world, source, seq);
        return;
    }
    let Some(target_pos) = world.get(ch.target).map(|t| t.pos) else {
        return;
    };
    if let Some(s) = world.get_mut(source) {
        s.move_target = None;
        s.path = None;
        s.attack_target = None;
        s.anim = Some("cast_R".into());
        s.data.insert(CastAnimUntil(time + TICK_DT * 2.0));
        s.facing = (target_pos.y - s.pos.y).atan2(target_pos.x - s.pos.x);
    }
    schedule(world, source, TICK_DT.min(ch.until - time), move |world| {
        channel_tick(world, source, seq, break_range);
    });
}

fn start_channel(
    world: &mut World,
    ctx: &CastContext,
    target: EntityId,
    stun_duration: f64,
    seq: u64,
) {
    let source = ctx.source;
    let Some(origin) = world.get(source).map(|s| s.pos) else {
        return;
    };
    let until = world.time + stun_duration;
    if let Some(s) = world.get_mut(source) {
        s.data.insert(TouilleurChannel {
            seq,
            target,
            origin,
            until,
        });
        s.data.insert(Channeling);
        s.anim = Some("cast_R".into());
    }
    let mut buff = BuffOptions::new(stun_duration);
    buff.source = Some(source);
    buff.definition = Some(BuffDefOverride {
        fx_key: Some("touilleur_r_channel".into()),
        ..BuffDefOverride::default()
    });
    add_buff(world, source, "touilleur_r_channel", buff);
    let break_range = param(ctx, "breakRange");
    schedule(world, source, TICK_DT.min(stun_duration), move |world| {
        channel_tick(world, source, seq, break_range);
    });
}

fn r(world: &mut World, ctx: &CastContext) {
    let Some(target) = ctx.target else { return };
    let (Some(s), Some(t)) = (world.get(ctx.source), world.get(target)) else {
        return;
    };
    let reach = ctx.def.range + s.radius + t.radius;
    if t.champ.is_none()
        || !is_enemy(s, t)
        || !is_visible_target(world, s, t)
        || !world.is_targetable(target, ctx.source)
        || dist(s.pos, t.pos) > reach
    {
        return;
    }
    let offset = s.radius + t.radius + param(ctx, "offset");
    let (dx, dy) = (t.facing.cos(), t.facing.sin());
    let travel = ctx.def.range + s.radius + t.radius + offset;
    let behind = Vec2 {
        x: t.pos.x - dx * offset,
        y: t.pos.y - dy * offset,
    };
    let landing = valid_landing(s.pos, behind, travel, s.radius);
    let distance2 = dist2(landing, t.pos);
    let break_range = param(ctx, "breakRange");
    // Un mur ne doit pas transformer l'ultime en téléportation devant la cible ou dans son collider.
    let in_front = (landing.x - t.pos.x) * dx + (landing.y - t.pos.y) * dy >= -t.radius * 0.5;
    if in_front
        || distance2 > break_range * break_range
        || distance2 < (s.radius + t.radius).powi(2)
    {
        return;
    }
    let target_pos = t.pos;
    blink(world, ctx.source, landing, travel, &ctx.def.fx_key);
    if let Some(s) = world.get_mut(ctx.source) {
        s.facing = (target_pos.y - s.pos.y).atan2(target_pos.x - s.pos.x);
    }
    hit(world, ctx, target);
    let dead = |id| world.get(id).is_none_or(|e: &Entity| e.dead);
    if dead(target) || dead(ctx.source) {
        return;
    }
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let mut stun = BuffOptions::new(param(ctx, "duration"));
    stun.source = Some(ctx.source);
    stun.params.set_number(SEQ_KEY, seq as f64);
    stun.definition = Some(BuffDefOverride {
        kind: Some(BuffKind::Cc),
        cc: Some(CcType::Stun),
        fx_key: Some("touilleur_r_stun".into()),
        ..BuffDefOverride::default()
    });
    if let Some(applied) = add_buff(world, target, "stun", stun) {
        start_channel(world, ctx, target, applied.duration, seq);
    }
}

/// Lanceurs dont la canalisation vise `target` (ou est portée par lui).
fn channels_touching(world: &World, id: EntityId) -> Vec<(EntityId, TouilleurChannel)> {
    world
        .iter()
        .filter_map(|e| e.data.get::<TouilleurChannel>().map(|c| (e.id, *c)))
        .filter(|(source, c)| *source == id || c.target == id)
        .collect()
}

// ------------------------------------------------------------------------------------------------
// Passif
// ------------------------------------------------------------------------------------------------

/// Dernier sort compté par le passif (temps de jeu).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LastSpellHit(pub f64);

fn install(world: &mut World) {
    world.hooks.after_damage.push(Box::new(|world, ctx, dealt| {
        let Some(source) = ctx.credit.or(ctx.source) else {
            return;
        };
        let (Some(s), Some(t)) = (world.get(source), world.get(ctx.target)) else {
            return;
        };
        let is_touilleur = s
            .champ
            .as_ref()
            .is_some_and(|c| c.champion_id == "touilleur");
        let spell = ctx
            .ability
            .as_deref()
            .is_some_and(|a| a.starts_with("touilleur_") && a != "touilleur_p");
        if !is_touilleur
            || s.dead
            || t.champ.is_none()
            || t.team == s.team
            || dealt <= 0.0
            || !spell
        {
            return;
        }
        let time = world.time;
        let last = s
            .data
            .get::<LastSpellHit>()
            .map_or(f64::NEG_INFINITY, |l| l.0);
        if time - last < passive("lockout") - 1e-8 {
            return;
        }
        if let Some(s) = world.get_mut(source) {
            s.data.insert(LastSpellHit(time));
        }
        let max = passive("stacks");
        let mut count = BuffOptions::new(passive("duration"));
        count.source = Some(source);
        count.definition = Some(BuffDefOverride {
            max_stacks: Some(max as u32),
            fx_key: Some("touilleur_p".into()),
            ..BuffDefOverride::default()
        });
        let Some(buff) = add_buff(world, source, "touilleur_count", count) else {
            return;
        };
        if f64::from(buff.stacks) < max {
            return;
        }
        remove_buff(world, source, &buff.id, SourceMatch::Id(source));
        let mut speed = BuffOptions::new(passive("speedDuration"));
        speed.source = Some(source);
        speed.params.stat_percent_mods = Some(PartialStats {
            move_speed: Some(passive("speed")),
            ..PartialStats::default()
        });
        speed.definition = Some(BuffDefOverride {
            fx_key: Some("touilleur_p".into()),
            ..BuffDefOverride::default()
        });
        add_buff(world, source, "touilleur_p_speed", speed);
        let alive = world.get(ctx.target).is_some_and(|t| t.hp > 0.0 && !t.dead);
        let ap = world.get(source).map_or(0.0, |s| s.stats.ap);
        if alive {
            let mut bonus = DamageOpts::new(
                Some(source),
                ctx.target,
                passive("damage") + ap * passive("ap"),
                DamageType::Magic,
            );
            bonus.ability = Some("touilleur_p".into());
            apply_damage(world, bonus);
        }
    }));

    // Purification ou fin de l'étourdissement, ou du buff de canalisation : fin de la touille.
    world.events.on(EventKind::BuffRemoved, |world, ev| {
        let GameEvent::BuffRemoved {
            target, buff_id, ..
        } = ev
        else {
            return;
        };
        match buff_id.as_str() {
            "stun" => {
                for (source, ch) in channels_touching(world, *target) {
                    if ch.target == *target && !stun_holds(world, source, *target, ch.seq) {
                        end_channel(world, source, ch.seq);
                    }
                }
            }
            "touilleur_r_channel" => {
                if let Some(ch) = world
                    .get(*target)
                    .and_then(|s| s.data.get::<TouilleurChannel>())
                    .copied()
                {
                    end_channel(world, *target, ch.seq);
                }
            }
            _ => {}
        }
    });

    world.events.on(EventKind::Death, |world, ev| {
        let GameEvent::Death { target, .. } = ev else {
            return;
        };
        if let Some(e) = world.get_mut(*target) {
            e.data.remove::<LastSpellHit>();
        }
        for (source, ch) in channels_touching(world, *target) {
            end_channel(world, source, ch.seq);
        }
    });

    world.events.on(EventKind::Respawn, |world, ev| {
        if let GameEvent::Respawn { target, .. } = ev
            && let Some(e) = world.get_mut(*target)
        {
            e.data.remove::<LastSpellHit>();
        }
    });
}

pub static KIT: Kit = Kit::new("touilleur", [q, w, e, r])
    .with_targeting([
        TargetRule::Enemy,
        TargetRule::Enemy,
        TargetRule::Enemy,
        TargetRule::EnemyChampion,
    ])
    .with_install(install);
