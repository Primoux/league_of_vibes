//! Mélenchon, le Tribun indigné : port de `packages/server/src/abilities/champions/melenchon.ts`.
//!
//! - Q : question en ligne qui blesse et ralentit le premier ennemi.
//! - W : motion télégraphiée qui blesse et réduit au silence.
//! - E : saut éclair et bouclier ; un hologramme explose au point de départ.
//! - R : déclaration : bouclier proportionnel aux PV max, dégâts et provocation autour de lui.
//! - Passif (indignation) : trois sorts espacés qui touchent un champion donnent un bouclier et de
//!   la vitesse.
//!
//! Différence avec le TypeScript : le dernier cumul (un `Map<id, temps>` par monde en TS) est la
//! donnée [`LastIndignation`] du lanceur, effacée à sa mort et à sa réapparition.

use lov_shared::data::champions::get_champion;
use lov_shared::schema::CcType;
use lov_shared::types::{EntityId, GameEvent, PartialStats};

use crate::abilities::prelude::*;
use crate::combat::buffs::{BuffDefOverride, BuffOptions, SourceMatch, add_buff, remove_buff};
use crate::world::EventKind;

fn passive(key: &str) -> f64 {
    get_champion("melenchon").map_or(0.0, |d| def_param(&d.passive, key, 1, 0.0))
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
                let mut silence = BuffOptions::new(param(&ctx, "silence"));
                silence.source = Some(ctx.source);
                silence.definition = Some(BuffDefOverride {
                    fx_key: Some(ctx.def.fx_key.clone()),
                    ..BuffDefOverride::default()
                });
                add_buff(world, target, "silence", silence);
            }
        },
    );
}

fn e(world: &mut World, ctx: &CastContext) {
    let ctx = *ctx;
    let Some(origin) = ctx.source(world).map(|s| s.pos) else {
        return;
    };
    blink(world, ctx.source, ctx.point, ctx.def.range, &ctx.def.fx_key);
    let ap = ctx.source(world).map_or(0.0, |s| s.stats.ap);
    let mut shield = ShieldOptions::new(
        ctx.source,
        ctx.source,
        param(&ctx, "shield") + ap * param(&ctx, "shieldAp"),
    );
    shield.duration = Some(param(&ctx, "duration"));
    shield.buff_id = Some("melenchon_e".into());
    shield.fx_key = Some(ctx.def.fx_key.clone());
    apply_shield(world, shield);
    let radius = ctx.def.radius.unwrap_or(0.0);
    delayed_area(
        world,
        ctx.source,
        origin,
        radius,
        param(&ctx, "delay"),
        &ctx.def.fx_key,
        move |world| {
            for target in enemies_in_radius(world, ctx.source, origin, radius) {
                hit(world, &ctx, target);
            }
        },
    );
}

fn r(world: &mut World, ctx: &CastContext) {
    let Some(s) = ctx.source(world) else { return };
    let (pos, max_hp, ap) = (s.pos, s.stats.max_hp, s.stats.ap);
    let mut shield = ShieldOptions::new(
        ctx.source,
        ctx.source,
        max_hp * param(ctx, "shieldMaxHp") + ap * param(ctx, "shieldAp"),
    );
    shield.duration = Some(param(ctx, "duration"));
    shield.buff_id = Some("melenchon_r".into());
    shield.fx_key = Some(ctx.def.fx_key.clone());
    apply_shield(world, shield);
    let radius = ctx.def.radius.unwrap_or(0.0);
    world.emit(GameEvent::AreaEffect {
        source: ctx.source,
        fx_key: ctx.def.fx_key.clone(),
        pos,
        radius,
        duration: param(ctx, "duration"),
    });
    for target in enemies_in_radius(world, ctx.source, pos, radius) {
        hit(world, ctx, target);
        cc(world, ctx, target, CcType::Taunt, param(ctx, "taunt"), None);
    }
}

/// Dernier cumul d'indignation (temps de jeu).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LastIndignation(pub f64);

fn forget(world: &mut World, id: EntityId) {
    if let Some(e) = world.get_mut(id) {
        e.data.remove::<LastIndignation>();
    }
}

fn install(world: &mut World) {
    world.hooks.after_damage.push(Box::new(|world, ctx, dealt| {
        let Some(source) = ctx.credit.or(ctx.source) else {
            return;
        };
        let (Some(s), Some(t)) = (world.get(source), world.get(ctx.target)) else {
            return;
        };
        let is_tribune = s
            .champ
            .as_ref()
            .is_some_and(|c| c.champion_id == "melenchon");
        let spell = ctx
            .ability
            .as_deref()
            .is_some_and(|a| a.starts_with("melenchon_"));
        if !is_tribune || s.dead || t.champ.is_none() || s.team == t.team || !spell || dealt <= 0.0
        {
            return;
        }
        let time = world.time;
        let last = s
            .data
            .get::<LastIndignation>()
            .map_or(f64::NEG_INFINITY, |l| l.0);
        if time - last < passive("hitLockout") {
            return;
        }
        if let Some(s) = world.get_mut(source) {
            s.data.insert(LastIndignation(time));
        }
        let max = passive("maxStacks");
        let mut stack = BuffOptions::new(passive("stackDuration"));
        stack.source = Some(source);
        stack.definition = Some(BuffDefOverride {
            max_stacks: Some(max as u32),
            fx_key: Some("melenchon_p".into()),
            ..BuffDefOverride::default()
        });
        let Some(buff) = add_buff(world, source, "melenchon_indignation", stack) else {
            return;
        };
        if f64::from(buff.stacks) < max {
            return;
        }
        remove_buff(world, source, &buff.id, SourceMatch::Id(source));
        let Some(s) = world.get(source) else { return };
        let level = s.champ.as_ref().map_or(1.0, |c| f64::from(c.level));
        let amount = passive("shield")
            + level * passive("shieldPerLevel")
            + s.stats.ap * passive("shieldAp");
        let duration = passive("duration");
        let mut shield = ShieldOptions::new(source, source, amount);
        shield.duration = Some(duration);
        shield.buff_id = Some("melenchon_p".into());
        shield.fx_key = Some("melenchon_p".into());
        apply_shield(world, shield);
        let mut speed = BuffOptions::new(duration);
        speed.source = Some(source);
        speed.params.stat_percent_mods = Some(PartialStats {
            move_speed: Some(passive("moveSpeed")),
            ..PartialStats::default()
        });
        speed.definition = Some(BuffDefOverride {
            fx_key: Some("melenchon_p".into()),
            ..BuffDefOverride::default()
        });
        add_buff(world, source, "melenchon_p_speed", speed);
    }));
    world.events.on(EventKind::Death, |world, ev| {
        if let GameEvent::Death { target, .. } = ev {
            forget(world, *target);
        }
    });
    world.events.on(EventKind::Respawn, |world, ev| {
        if let GameEvent::Respawn { target, .. } = ev {
            forget(world, *target);
        }
    });
}

pub static KIT: Kit = Kit::new("melenchon", [q, w, e, r]).with_install(install);
