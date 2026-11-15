//! Zephyr : port de `packages/server/src/abilities/champions/zephyr.ts` et de sa branche dans
//! `engine.ts` (sort renforcé).
//!
//! - Q : rafale aller (dégâts magiques, traverse) puis retour vers Zephyr (dégâts bruts).
//! - W : saut éclair ; le point de départ explose après un délai.
//! - E : lien ; s'il tient, blesse et immobilise.
//! - R : trois rayons (relances) qui ne touchent que les champions.
//! - Passif : tous les `every` lancers, le suivant est amplifié de `amp` et rend `mana`.

use lov_shared::data::champions::get_champion;
use lov_shared::schema::{BuffKind, CcType};
use lov_shared::types::{DamageType, EntityId};

use crate::abilities::prelude::*;
use crate::combat::buffs::{BuffDefOverride, BuffOptions, SourceMatch, add_buff, remove_buff};

fn passive(key: &str, fallback: f64) -> f64 {
    get_champion("zephyr").map_or(fallback, |d| def_param(&d.passive, key, 1, fallback))
}

fn q(world: &mut World, ctx: &CastContext) {
    let ctx = *ctx;
    let options = LineOptions {
        pierce: true,
        on_end: Some(Box::new(move |world, pos| {
            let Some(home) = world.get(ctx.source).map(|s| s.pos) else {
                return;
            };
            let mut back = ProjectileOptions::new(
                ctx.source,
                home,
                ctx.def.speed.unwrap_or(0.0),
                ctx.def.width.unwrap_or(0.0),
                "zephyr_q_return",
                move |world, target, _| {
                    hit_with(world, &ctx, target, 1.0, Some(DamageType::True));
                },
            );
            back.from = Some(pos);
            back.homing_to = Some(ctx.source);
            back.pierce = true;
            spawn_projectile(world, back);
        })),
        ..LineOptions::default()
    };
    line(world, &ctx, options, move |world, target| {
        hit(world, &ctx, target);
    });
}

fn w(world: &mut World, ctx: &CastContext) {
    let ctx = *ctx;
    let Some(origin) = ctx.source(world).map(|s| s.pos) else {
        return;
    };
    blink(world, ctx.source, ctx.point, ctx.def.range, &ctx.def.fx_key);
    let radius = ctx.def.radius.unwrap_or(0.0);
    delayed_area(
        world,
        ctx.source,
        origin,
        radius,
        param(&ctx, "delay"),
        &format!("{}_explosion", ctx.def.fx_key),
        move |world| {
            for target in enemies_in_radius(world, ctx.source, origin, radius) {
                hit(world, &ctx, target);
            }
        },
    );
}

fn e(world: &mut World, ctx: &CastContext) {
    let ctx = *ctx;
    let Some(target) = ctx.target else { return };
    let tether_time = param(&ctx, "tetherTime");
    let mut mark = BuffOptions::new(tether_time);
    mark.source = Some(ctx.source);
    mark.definition = Some(BuffDefOverride {
        kind: Some(BuffKind::Debuff),
        fx_key: Some(ctx.def.fx_key.clone()),
        ..BuffDefOverride::default()
    });
    add_buff(world, target, "zephyr_e", mark);
    tether(
        world,
        ctx.source,
        target,
        tether_time,
        param(&ctx, "breakRange"),
        move |world| {
            remove_buff(world, target, "zephyr_e", SourceMatch::Id(ctx.source));
            hit(world, &ctx, target);
            cc(world, &ctx, target, CcType::Root, param(&ctx, "root"), None);
        },
    );
}

fn r(world: &mut World, ctx: &CastContext) {
    let ctx = *ctx;
    let options = LineOptions {
        only_champions: true,
        ..LineOptions::default()
    };
    line(world, &ctx, options, move |world, target| {
        hit(world, &ctx, target);
    });
}

/// Sort renforcé : après `every` lancers, le suivant est amplifié et rend du mana.
fn on_cast(world: &mut World, source: EntityId, casts: u32) -> f64 {
    let every = passive("every", 0.0) as u32;
    let empowered = casts > 0 && casts % (every + 1) == every;
    if !empowered {
        return 1.0;
    }
    if let Some(s) = world.get_mut(source) {
        s.mana = s.stats.max_mana.min(s.mana + passive("mana", 30.0));
    }
    1.0 + passive("amp", 0.25)
}

pub static KIT: Kit = Kit::new("zephyr", [q, w, e, r]).with_on_cast(on_cast);
