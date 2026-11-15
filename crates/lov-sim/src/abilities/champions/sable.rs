//! Sable : port de `packages/server/src/abilities/champions/sable.ts` et de ses branches de
//! `passives.ts`.
//!
//! - Q : renforce la prochaine auto-attaque, qui explose autour de la cible.
//! - W : éventail de balles ; chaque ennemi n'est touché qu'une fois, et ralenti.
//! - E : roulade ; la prochaine auto-attaque est un coup critique renforcé.
//! - R : tir qui traverse les sbires et s'arrête au premier champion.
//! - Passif : les coups critiques frappent plus fort ; toucher un champion donne de la vitesse.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use lov_shared::data::champions::get_champion;
use lov_shared::schema::CcType;
use lov_shared::types::{EntityId, GameEvent, PartialStats, Vec2};

use crate::abilities::prelude::*;
use crate::combat::buffs::{
    BuffDefOverride, BuffOptions, SourceMatch, add_buff, get_buff, remove_buff,
};
use crate::combat::damage::DamageOpts;

fn passive(key: &str) -> f64 {
    get_champion("sable").map_or(0.0, |d| def_param(&d.passive, key, 1, 0.0))
}

fn fx(ctx: &CastContext) -> Option<BuffDefOverride> {
    Some(BuffDefOverride {
        fx_key: Some(ctx.def.fx_key.clone()),
        ..BuffDefOverride::default()
    })
}

fn q(world: &mut World, ctx: &CastContext) {
    let mut opts = BuffOptions::new(param_or(ctx, "empowerWindow", 6.0));
    opts.source = Some(ctx.source);
    opts.params.set_number("rank", f64::from(ctx.rank));
    opts.definition = fx(ctx);
    add_buff(world, ctx.source, "sable_q", opts);
}

fn w(world: &mut World, ctx: &CastContext) {
    let ctx = *ctx;
    let Some(s) = ctx.source(world) else { return };
    let (pos, facing) = (s.pos, s.facing);
    let dir = direction(pos, ctx.point, facing);
    let angle = dir.y.atan2(dir.x);
    let count = param(&ctx, "bullets") as u32;
    let spread = param(&ctx, "coneAngle").to_radians();
    let hit_targets: Arc<Mutex<HashSet<EntityId>>> = Arc::default();
    for i in 0..count {
        let a = angle - spread / 2.0 + f64::from(i) * spread / f64::from(count.max(2) - 1);
        let to = Vec2 {
            x: pos.x + a.cos() * ctx.def.range,
            y: pos.y + a.sin() * ctx.def.range,
        };
        let seen = Arc::clone(&hit_targets);
        let bullet = ProjectileOptions::new(
            ctx.source,
            to,
            ctx.def.speed.unwrap_or(2000.0),
            30.0,
            ctx.def.fx_key.clone(),
            move |world, target, _| {
                let first = seen.lock().map(|mut s| s.insert(target)).unwrap_or(false);
                if first {
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
        spawn_projectile(world, bullet);
    }
}

fn e(world: &mut World, ctx: &CastContext) {
    let mut opts = BuffOptions::new(param(ctx, "empowerWindow"));
    opts.source = Some(ctx.source);
    opts.definition = fx(ctx);
    add_buff(world, ctx.source, "sable_e", opts);
    let dash = DashOptions::new(ctx.def.fx_key.clone(), ctx.def.speed.unwrap_or(0.0));
    start_dash(world, ctx.source, ctx.point, dash);
}

fn r(world: &mut World, ctx: &CastContext) {
    let ctx = *ctx;
    let options = LineOptions {
        pierce: true,
        stop_on_champion: true,
        ..LineOptions::default()
    };
    line(world, &ctx, options, move |world, target| {
        hit(world, &ctx, target);
    });
}

fn is_sable(world: &World, id: EntityId) -> bool {
    world
        .get(id)
        .and_then(|e| e.champ.as_ref())
        .is_some_and(|c| c.champion_id == "sable")
}

/// Explosion de l'auto-attaque renforcée par Q, autour de la cible.
fn explode(world: &mut World, source: EntityId, target: EntityId, rank: u32) {
    let Some(def) = get_champion("sable").map(|d| &d.abilities.q) else {
        return;
    };
    let Some(pos) = world.get(target).map(|t| t.pos) else {
        return;
    };
    let radius = def.radius.unwrap_or(0.0);
    world.emit(GameEvent::AreaEffect {
        source,
        fx_key: "sable_q_explosion".into(),
        pos,
        radius,
        duration: 0.3,
    });
    let Some(damage) = def.damage.as_ref() else {
        return;
    };
    for enemy in enemies_in_radius(world, source, pos, radius) {
        let (Some(s), Some(e)) = (world.get(source), world.get(enemy)) else {
            continue;
        };
        let amount = scaling(s, &damage.scaling, rank, Some(e));
        let mut opts = DamageOpts::new(
            Some(source),
            enemy,
            amount,
            lov_shared::types::DamageType::Physical,
        );
        opts.ability = Some(def.id.clone());
        apply_damage(world, opts);
    }
}

fn install(world: &mut World) {
    // E : la prochaine auto-attaque est un critique ; tout critique frappe plus fort.
    register_damage_modifier(world, |world, ctx| {
        let Some(source) = ctx.credit.or(ctx.source) else {
            return;
        };
        if !ctx.is_auto_attack || !is_sable(world, source) {
            return;
        }
        let (Some(s), Some(t)) = (world.get(source), world.get(ctx.target)) else {
            return;
        };
        if s.team == t.team {
            return;
        }
        if get_buff(s, "sable_e", Some(source)).is_some() {
            if !ctx.crit {
                ctx.amount *= 1.75;
            }
            ctx.crit = true;
            remove_buff(world, source, "sable_e", SourceMatch::Id(source));
        }
        if ctx.crit {
            ctx.amount *= 1.0 + passive("critBonus");
        }
    });

    world.hooks.after_damage.push(Box::new(|world, ctx, dealt| {
        let Some(source) = ctx.credit.or(ctx.source) else {
            return;
        };
        let (Some(s), Some(t)) = (world.get(source), world.get(ctx.target)) else {
            return;
        };
        if !is_sable(world, source) || t.dead || s.team == t.team {
            return;
        }
        let target_is_champ = t.champ.is_some();
        if ctx.is_auto_attack {
            let rank = get_buff(s, "sable_q", Some(source))
                .map(|b| b.params.number("rank").unwrap_or(1.0) as u32);
            if let Some(rank) = rank {
                remove_buff(world, source, "sable_q", SourceMatch::Id(source));
                explode(world, source, ctx.target, rank);
            }
        }
        let alive = world.get(ctx.target).is_some_and(|t| t.hp > 0.0);
        if dealt > 0.0 && alive && target_is_champ {
            let mut speed = BuffOptions::new(passive("duration"));
            speed.source = Some(source);
            speed.params.stat_percent_mods = Some(PartialStats {
                move_speed: Some(passive("moveSpeed")),
                ..PartialStats::default()
            });
            speed.definition = Some(BuffDefOverride {
                fx_key: Some("sable_p".into()),
                ..BuffDefOverride::default()
            });
            add_buff(world, source, "sable_p_speed", speed);
        }
    }));
}

pub static KIT: Kit = Kit::new("sable", [q, w, e, r]).with_install(install);
