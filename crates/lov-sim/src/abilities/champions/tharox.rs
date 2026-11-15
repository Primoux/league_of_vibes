//! Tharox : port de `packages/server/src/abilities/champions/tharox.ts` et de ses branches de
//! `passives.ts`.
//!
//! - Q : charge qui s'arrête sur la première unité, la blesse, la projette et la repousse.
//! - W : résistances et aura de dégâts qui le suit.
//! - E : onde autour de lui, blesse et ralentit.
//! - R : charge qui traverse, blesse et étourdit les champions.
//! - Passif (élan) : se déplacer charge l'élan ; à 100, la prochaine auto-attaque inflige un
//!   pourcentage des PV max de la cible et la ralentit.

use lov_shared::data::champions::get_champion;
use lov_shared::math::dist;
use lov_shared::schema::CcType;
use lov_shared::types::{DamageType, EntityId, Vec2};

use crate::abilities::prelude::*;
use crate::combat::buffs::{BuffDefOverride, BuffOptions, add_buff};
use crate::combat::damage::DamageOpts;

fn passive(key: &str, fallback: f64) -> f64 {
    get_champion("tharox").map_or(fallback, |d| def_param(&d.passive, key, 1, fallback))
}

fn q(world: &mut World, ctx: &CastContext) {
    let ctx = *ctx;
    let Some(s) = ctx.source(world) else { return };
    let dir = direction(s.pos, ctx.point, s.facing);
    let mut opts = DashOptions::new(ctx.def.fx_key.clone(), ctx.def.speed.unwrap_or(0.0));
    opts.width = ctx.def.width;
    opts.stop_on_first = true;
    opts.on_hit = Some(Box::new(move |world, target| {
        hit(world, &ctx, target);
        cc(world, &ctx, target, CcType::Knockup, 0.25, None);
        let Some(pos) = world.get(target).map(|t| t.pos) else {
            return;
        };
        let back = param(&ctx, "knockback");
        let to = Vec2 {
            x: pos.x + dir.x * back,
            y: pos.y + dir.y * back,
        };
        let mut push = DashOptions::new(format!("{}_knockback", ctx.def.fx_key), 1200.0);
        push.unstoppable = true;
        start_dash(world, target, to, push);
    }));
    start_dash(world, ctx.source, ctx.point, opts);
}

fn w(world: &mut World, ctx: &CastContext) {
    let ctx = *ctx;
    let duration = param(&ctx, "duration");
    let resists = param(&ctx, "resists");
    let mut buff = BuffOptions::new(duration);
    buff.source = Some(ctx.source);
    buff.params.stat_mods = Some(lov_shared::types::PartialStats {
        armor: Some(resists),
        mr: Some(resists),
        ..Default::default()
    });
    buff.definition = Some(BuffDefOverride {
        fx_key: Some(ctx.def.fx_key.clone()),
        ..BuffDefOverride::default()
    });
    add_buff(world, ctx.source, "tharox_w", buff);
    let Some(pos) = ctx.source(world).map(|s| s.pos) else {
        return;
    };
    let mut zone = ZoneOptions::new(
        ctx.source,
        pos,
        ctx.def.radius.unwrap_or(0.0),
        duration,
        ctx.def.fx_key.clone(),
        move |world, zone, _| {
            let Some((pos, radius)) = world.get(zone).map(|z| (z.pos, z.radius)) else {
                return;
            };
            for target in enemies_in_radius(world, ctx.source, pos, radius) {
                hit(world, &ctx, target);
            }
        },
    );
    zone.follow = Some(ctx.source);
    zone.interval = Some(1.0);
    spawn_zone(world, zone);
}

fn e(world: &mut World, ctx: &CastContext) {
    let Some(pos) = ctx.source(world).map(|s| s.pos) else {
        return;
    };
    for target in enemies_in_radius(world, ctx.source, pos, ctx.def.radius.unwrap_or(0.0)) {
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

fn r(world: &mut World, ctx: &CastContext) {
    let ctx = *ctx;
    let mut opts = DashOptions::new(ctx.def.fx_key.clone(), ctx.def.speed.unwrap_or(0.0));
    opts.width = ctx.def.width;
    opts.on_hit = Some(Box::new(move |world, target| {
        hit(world, &ctx, target);
        if world.get(target).is_some_and(|t| t.champ.is_some()) {
            cc(world, &ctx, target, CcType::Stun, param(&ctx, "stun"), None);
        }
    }));
    start_dash(world, ctx.source, ctx.point, opts);
}

/// L'élan monte avec la distance parcourue.
fn update_momentum(world: &mut World, id: EntityId, _dt: f64) {
    let per_unit = passive("perUnit", 0.0);
    let Some(e) = world.get_mut(id) else { return };
    let pos = e.pos;
    let s = passive_state(e);
    s.momentum = 100f64.min(s.momentum + dist(s.last_pos, pos) * per_unit);
    s.last_pos = pos;
}

fn install(world: &mut World) {
    world.hooks.after_damage.push(Box::new(|world, ctx, _| {
        let Some(source) = ctx.credit.or(ctx.source) else {
            return;
        };
        if !ctx.is_auto_attack {
            return;
        }
        let (Some(s), Some(t)) = (world.get(source), world.get(ctx.target)) else {
            return;
        };
        let is_tharox = s.champ.as_ref().is_some_and(|c| c.champion_id == "tharox");
        let charged = s
            .data
            .get::<PassiveState>()
            .is_some_and(|p| p.momentum >= 100.0);
        if !is_tharox || t.dead || s.team == t.team || t.hp <= 0.0 || !charged {
            return;
        }
        let (target, max_hp) = (ctx.target, t.stats.max_hp);
        if let Some(s) = world.get_mut(source) {
            passive_state(s).momentum = 0.0;
        }
        let mut opts = DamageOpts::new(
            Some(source),
            target,
            max_hp * passive("bonusTargetMaxHp", 0.0),
            DamageType::Physical,
        );
        opts.ability = Some("tharox_p".into());
        apply_damage(world, opts);
        let mut slow = BuffOptions::new(passive("slowDuration", 1.5));
        slow.source = Some(source);
        slow.params.amount = Some(passive("slow", 0.0));
        slow.definition = Some(BuffDefOverride {
            fx_key: Some("tharox_p".into()),
            ..BuffDefOverride::default()
        });
        add_buff(world, target, "slow", slow);
    }));
}

pub static KIT: Kit = Kit::new("tharox", [q, w, e, r])
    .with_install(install)
    .with_update_champion(update_momentum);
