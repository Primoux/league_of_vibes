//! Corvin : port de `packages/server/src/abilities/champions/corvin.ts` et de ses branches de
//! `passives.ts`.
//!
//! - Q : carreau en ligne qui traverse ; marque chaque cible touchée.
//! - W : piège invisible (trois au plus) : immobilise et révèle le premier ennemi, puis disparaît.
//! - E : bond en arrière de la visée, avec un bonus de vitesse.
//! - R : zone qui blesse, révèle et réduit les soins.
//! - Passif : auto-attaques et Q posent des marques ; à `marks` marques, dégâts magiques d'un
//!   pourcentage des PV max de la cible.

use lov_shared::data::champions::get_champion;
use lov_shared::math::dist;
use lov_shared::schema::{BuffKind, CcType};
use lov_shared::types::{DamageType, EntityId, EntityKind, PartialStats, Vec2};

use crate::abilities::prelude::*;
use crate::combat::buffs::{BuffDefOverride, BuffOptions, SourceMatch, add_buff, remove_buff};
use crate::combat::damage::DamageOpts;

fn passive(key: &str) -> f64 {
    get_champion("corvin").map_or(0.0, |d| def_param(&d.passive, key, 1, 0.0))
}

fn q(world: &mut World, ctx: &CastContext) {
    let ctx = *ctx;
    let options = LineOptions {
        pierce: true,
        ..LineOptions::default()
    };
    line(world, &ctx, options, move |world, target| {
        hit(world, &ctx, target);
    });
}

/// Révèle `target` pendant `duration` secondes.
fn reveal(world: &mut World, ctx: &CastContext, target: EntityId, duration: f64) {
    let mut opts = BuffOptions::new(duration);
    opts.source = Some(ctx.source);
    opts.params.revealed = Some(true);
    opts.definition = Some(BuffDefOverride {
        kind: Some(BuffKind::Debuff),
        fx_key: Some(ctx.def.fx_key.clone()),
        ..BuffDefOverride::default()
    });
    add_buff(world, target, "revealed", opts);
}

fn w(world: &mut World, ctx: &CastContext) {
    let ctx = *ctx;
    let mut traps: Vec<EntityId> = world
        .query(Some(EntityKind::Zone))
        .filter(|z| {
            z.subtype.as_deref() == Some("corvin_w")
                && z.data.get::<OwnerId>().is_some_and(|o| o.0 == ctx.source)
        })
        .map(|z| z.id)
        .collect();
    let max = param(&ctx, "maxTraps") as usize;
    while !traps.is_empty() && traps.len() >= max {
        remove_zone(world, traps.remove(0));
    }
    let mut zone = ZoneOptions::new(
        ctx.source,
        ctx.point,
        ctx.def.radius.unwrap_or(0.0),
        param(&ctx, "lifetime"),
        ctx.def.fx_key.clone(),
        move |world, zone, _| {
            let Some((pos, radius)) = world.get(zone).map(|z| (z.pos, z.radius)) else {
                return;
            };
            let enemies = enemies_in_radius(world, ctx.source, pos, radius);
            let nearest = enemies.into_iter().min_by(|a, b| {
                let d = |id: &EntityId| world.get(*id).map_or(f64::MAX, |e| dist(e.pos, pos));
                d(a).total_cmp(&d(b))
            });
            let Some(target) = nearest else { return };
            hit(world, &ctx, target);
            cc(world, &ctx, target, CcType::Root, param(&ctx, "root"), None);
            reveal(world, &ctx, target, 3.0);
            remove_zone(world, zone);
        },
    );
    zone.interval = Some(1.0 / 30.0);
    zone.invisible = true;
    spawn_zone(world, zone);
}

fn e(world: &mut World, ctx: &CastContext) {
    let Some(s) = ctx.source(world) else { return };
    let dir = direction(s.pos, ctx.point, s.facing);
    let to = Vec2 {
        x: s.pos.x - dir.x * ctx.def.range,
        y: s.pos.y - dir.y * ctx.def.range,
    };
    let mut opts = DashOptions::new(ctx.def.fx_key.clone(), ctx.def.speed.unwrap_or(0.0));
    opts.cross_walls = true;
    start_dash(world, ctx.source, to, opts);
    let mut speed = BuffOptions::new(param(ctx, "duration"));
    speed.source = Some(ctx.source);
    speed.params.stat_percent_mods = Some(PartialStats {
        move_speed: Some(param(ctx, "moveSpeed")),
        ..PartialStats::default()
    });
    speed.definition = Some(BuffDefOverride {
        fx_key: Some(ctx.def.fx_key.clone()),
        ..BuffDefOverride::default()
    });
    add_buff(world, ctx.source, "corvin_e", speed);
}

fn r(world: &mut World, ctx: &CastContext) {
    let ctx = *ctx;
    let mut zone = ZoneOptions::new(
        ctx.source,
        ctx.point,
        ctx.def.radius.unwrap_or(0.0),
        param(&ctx, "duration"),
        ctx.def.fx_key.clone(),
        move |world, zone, _| {
            let Some((pos, radius)) = world.get(zone).map(|z| (z.pos, z.radius)) else {
                return;
            };
            for target in enemies_in_radius(world, ctx.source, pos, radius) {
                hit(world, &ctx, target);
                let mut grievous = BuffOptions::new(1.2);
                grievous.source = Some(ctx.source);
                grievous.params.grievous = Some(param(&ctx, "grievous"));
                add_buff(world, target, "grievous", grievous);
                reveal(world, &ctx, target, 1.2);
            }
        },
    );
    zone.interval = Some(1.0);
    spawn_zone(world, zone);
}

/// Pose une marque ; la dernière déclenche les dégâts en pourcentage des PV max.
fn mark(world: &mut World, source: EntityId, target: EntityId) {
    let marks = passive("marks");
    let mut opts = BuffOptions::new(passive("markDuration"));
    opts.source = Some(source);
    opts.definition = Some(BuffDefOverride {
        kind: Some(BuffKind::Debuff),
        max_stacks: Some(marks as u32),
        fx_key: Some("corvin_p".into()),
        ..BuffDefOverride::default()
    });
    let Some(buff) = add_buff(world, target, "corvin_marks", opts) else {
        return;
    };
    if f64::from(buff.stacks) < marks {
        return;
    }
    remove_buff(world, target, &buff.id, SourceMatch::Id(source));
    let Some(max_hp) = world.get(target).map(|t| t.stats.max_hp) else {
        return;
    };
    let mut dmg = DamageOpts::new(
        Some(source),
        target,
        max_hp * passive("targetMaxHp"),
        DamageType::Magic,
    );
    dmg.ability = Some("corvin_p".into());
    apply_damage(world, dmg);
}

fn install(world: &mut World) {
    world.hooks.after_damage.push(Box::new(|world, ctx, dealt| {
        let Some(source) = ctx.credit.or(ctx.source) else {
            return;
        };
        let (Some(s), Some(t)) = (world.get(source), world.get(ctx.target)) else {
            return;
        };
        let is_corvin = s.champ.as_ref().is_some_and(|c| c.champion_id == "corvin");
        if !is_corvin || t.dead || s.team == t.team || t.hp <= 0.0 {
            return;
        }
        if ctx.is_auto_attack {
            mark(world, source, ctx.target);
        }
        let alive = world.get(ctx.target).is_some_and(|t| t.hp > 0.0);
        if dealt > 0.0 && alive && ctx.ability.as_deref() == Some("corvin_q") {
            mark(world, source, ctx.target);
        }
    }));
}

pub static KIT: Kit = Kit::new("corvin", [q, w, e, r]).with_install(install);
