//! Nyx : port de `packages/server/src/abilities/champions/nyx.ts` et de ses branches de
//! `passives.ts`.
//!
//! - Q : frappe ciblée ; la recharge est remise à zéro si la cible meurt dans la fenêtre.
//! - W : invisibilité et vitesse (rompue par une attaque ou un sort).
//! - E : saut derrière une unité ou une balise ; blesse et ralentit un ennemi.
//! - R : marque un champion ; après `duration`, détone pour ses dégâts plus `storedPercent` des PV
//!   qu'il a perdus entre-temps.
//! - Passif : auto-attaques et sorts amplifiés contre une cible isolée.
//!
//! Différence avec le TypeScript : la détonation de R retrouve sa marque par un numéro de lancer
//! ([`NyxMarks`]) et non par identité d'objet. Une marque relancée avant la détonation de la
//! précédente ne détone donc qu'une fois (le TS pouvait détoner deux fois, la seconde à vide).

use std::sync::atomic::{AtomicU64, Ordering};

use lov_shared::data::champions::get_champion;
use lov_shared::math::dist2;
use lov_shared::schema::{BuffKind, CcType};
use lov_shared::types::{EntityId, GameEvent, PartialStats, Vec2};

use crate::abilities::prelude::*;
use crate::combat::buffs::{
    BuffDefOverride, BuffOptions, SourceMatch, add_buff, get_buff_mut, get_buffs, remove_buff,
};
use crate::combat::damage::DamageOpts;
use crate::world::EventKind;

fn passive(key: &str) -> f64 {
    get_champion("nyx").map_or(0.0, |d| def_param(&d.passive, key, 1, 0.0))
}

fn debuff(ctx: &CastContext, duration: f64) -> BuffOptions {
    let mut opts = BuffOptions::new(duration);
    opts.source = Some(ctx.source);
    opts.definition = Some(BuffDefOverride {
        kind: Some(BuffKind::Debuff),
        fx_key: Some(ctx.def.fx_key.clone()),
        ..BuffDefOverride::default()
    });
    opts
}

fn q(world: &mut World, ctx: &CastContext) {
    let Some(target) = ctx.target else { return };
    // Marque avant les dégâts : un Q mortel remet aussi sa recharge à zéro.
    add_buff(
        world,
        target,
        "nyx_q_reset",
        debuff(ctx, param(ctx, "resetWindow")),
    );
    hit(world, ctx, target);
}

fn w(world: &mut World, ctx: &CastContext) {
    let mut opts = BuffOptions::new(param(ctx, "duration"));
    opts.source = Some(ctx.source);
    opts.params.stat_percent_mods = Some(PartialStats {
        move_speed: Some(param(ctx, "moveSpeed")),
        ..PartialStats::default()
    });
    opts.definition = Some(BuffDefOverride {
        fx_key: Some(ctx.def.fx_key.clone()),
        ..BuffDefOverride::default()
    });
    add_buff(world, ctx.source, "invisible", opts);
}

fn e(world: &mut World, ctx: &CastContext) {
    let Some(target) = ctx.target else { return };
    let (Some(s), Some(t)) = (ctx.source(world), world.get(target)) else {
        return;
    };
    let offset = t.radius + s.radius + 20.0;
    let point = Vec2 {
        x: t.pos.x + t.facing.cos() * -offset,
        y: t.pos.y + t.facing.sin() * -offset,
    };
    let enemy = s.team != t.team;
    blink(
        world,
        ctx.source,
        point,
        ctx.def.range + offset,
        &ctx.def.fx_key,
    );
    if enemy {
        hit(world, ctx, target);
        cc(
            world,
            ctx,
            target,
            CcType::Slow,
            param(ctx, "slowDuration"),
            Some(param(ctx, "slow")),
        );
    }
}

/// Une marque de R en attente de détonation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NyxMark {
    pub source: EntityId,
    /// Numéro du lancer qui a posé la marque.
    pub seq: u64,
    /// PV perdus par la cible depuis la pose.
    pub stored: f64,
}

/// `nyx_r` portés par une entité, avec leurs dégâts accumulés.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NyxMarks(pub Vec<NyxMark>);

static SEQ: AtomicU64 = AtomicU64::new(1);

fn r(world: &mut World, ctx: &CastContext) {
    let ctx = *ctx;
    let Some(target) = ctx.target else { return };
    let duration = param(&ctx, "duration");
    let mut opts = debuff(&ctx, duration);
    opts.params.set_number("stored", 0.0);
    if add_buff(world, target, "nyx_r", opts).is_none() {
        return;
    }
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    if let Some(t) = world.get_mut(target) {
        let marks = &mut t.data.get_or_insert_with(NyxMarks::default).0;
        marks.retain(|m| m.source != ctx.source);
        marks.push(NyxMark {
            source: ctx.source,
            seq,
            stored: 0.0,
        });
    }
    schedule(world, ctx.source, duration, move |world| {
        let Some(t) = world.get_mut(target) else {
            return;
        };
        if t.dead {
            return;
        }
        let Some(marks) = t.data.get_mut::<NyxMarks>() else {
            return;
        };
        let Some(i) = marks.0.iter().position(|m| m.seq == seq) else {
            return;
        };
        let stored = marks.0.remove(i).stored;
        let Some(damage) = ctx.def.damage.as_ref() else {
            return;
        };
        let (Some(s), Some(t)) = (world.get(ctx.source), world.get(target)) else {
            return;
        };
        let amount =
            scaling(s, &damage.scaling, ctx.rank, Some(t)) + stored * param(&ctx, "storedPercent");
        remove_buff(world, target, "nyx_r", SourceMatch::Id(ctx.source));
        let mut opts = DamageOpts::new(
            Some(ctx.source),
            target,
            amount * ctx.amplification,
            damage.damage_type,
        );
        opts.ability = Some(ctx.def.id.clone());
        apply_damage(world, opts);
    });
}

fn is_nyx(world: &World, id: EntityId) -> bool {
    world
        .get(id)
        .and_then(|e| e.champ.as_ref())
        .is_some_and(|c| c.champion_id == "nyx")
}

fn install(world: &mut World) {
    // Passif : cible isolée.
    register_damage_modifier(world, |world, ctx| {
        let Some(source) = ctx.credit.or(ctx.source) else {
            return;
        };
        let own = ctx.is_auto_attack
            || ctx
                .ability
                .as_deref()
                .is_some_and(|a| a.starts_with("nyx_"));
        if !own || !is_nyx(world, source) {
            return;
        }
        let (Some(s), Some(t)) = (world.get(source), world.get(ctx.target)) else {
            return;
        };
        if s.team == t.team {
            return;
        }
        let r2 = passive("isolationRadius").powi(2);
        let isolated = !world.iter().any(|ally| {
            ally.id != t.id
                && !ally.dead
                && ally.team == t.team
                && is_unit(ally)
                && dist2(ally.pos, t.pos) <= r2
        });
        if isolated {
            ctx.amount *= 1.0 + passive("amp");
        }
    });

    // R : les PV réellement perdus par la cible (dégâts d'alliés compris, jamais le bouclier ni le
    // dépassement) s'accumulent dans chaque marque.
    world.hooks.after_damage.push(Box::new(|world, ctx, dealt| {
        if ctx.ability.as_deref() == Some("nyx_r") || ctx.credit.or(ctx.source).is_none() {
            return;
        }
        let Some(t) = world.get(ctx.target) else {
            return;
        };
        if !t.data.contains::<NyxMarks>() {
            return;
        }
        let lost = hp_lost(t, dealt);
        let sources: Vec<EntityId> = get_buffs(t)
            .iter()
            .filter(|b| b.id == "nyx_r")
            .filter_map(|b| b.source)
            .collect();
        let Some(t) = world.get_mut(ctx.target) else {
            return;
        };
        for source in sources {
            if let Some(m) = t
                .data
                .get_mut::<NyxMarks>()
                .and_then(|m| m.0.iter_mut().find(|m| m.source == source))
            {
                m.stored += lost;
            }
            if let Some(b) = get_buff_mut(t, "nyx_r", Some(source)) {
                let stored = b.params.number("stored").unwrap_or(0.0) + lost;
                b.params.set_number("stored", stored);
            }
        }
    }));

    // Q : la mort de la cible marquée remet la recharge à zéro.
    world.events.on(EventKind::Death, |world, ev| {
        let GameEvent::Death { target, .. } = ev else {
            return;
        };
        let Some(victim) = world.get(*target) else {
            return;
        };
        let owners: Vec<EntityId> = get_buffs(victim)
            .iter()
            .filter(|b| b.id == "nyx_q_reset")
            .filter_map(|b| b.source)
            .collect();
        for owner in owners {
            if let Some(c) = world.get_mut(owner).and_then(|o| o.champ.as_mut()) {
                c.cooldowns.q = 0.0;
            }
        }
    });
}

fn clear_marks(world: &mut World, id: EntityId) {
    if let Some(e) = world.get_mut(id) {
        e.data.remove::<NyxMarks>();
    }
}

pub static KIT: Kit = Kit::new("nyx", [q, w, e, r])
    .with_targeting([
        TargetRule::Enemy,
        TargetRule::Enemy,
        TargetRule::UnitOrWard,
        TargetRule::EnemyChampion,
    ])
    .with_install(install)
    .with_reset_dead(clear_marks);
