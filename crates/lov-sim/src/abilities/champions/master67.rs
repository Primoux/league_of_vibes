//! 67 Master : port de `packages/server/src/abilities/champions/master67.ts`.
//!
//! - Q : projectile qui blesse et ralentit le premier ennemi.
//! - W : zone télégraphiée qui blesse et immobilise.
//! - E : saut éclair et bouclier.
//! - R : « SIX… SEVEN ! » : une onde qui ralentit, puis une seconde qui projette.
//! - Passif : le septième sort qui touche un champion (au plus un par `hitLockout`) inflige des dégâts
//!   magiques bonus.
//!
//! Différence avec le TypeScript : le dernier impact compté (un `Map<id, temps>` par monde en TS) est
//! la donnée [`LastSpellHit`] du lanceur, effacée à sa mort et à sa réapparition comme l'entrée du TS.

use lov_shared::data::champions::get_champion;
use lov_shared::schema::CcType;
use lov_shared::types::{DamageType, EntityId, GameEvent};

use crate::abilities::prelude::*;
use crate::combat::buffs::{BuffDefOverride, BuffOptions, SourceMatch, add_buff, remove_buff};
use crate::combat::damage::DamageOpts;
use crate::world::EventKind;

fn passive(key: &str) -> f64 {
    get_champion("master67").map_or(0.0, |d| def_param(&d.passive, key, 1, 0.0))
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
                cc(world, &ctx, target, CcType::Root, param(&ctx, "root"), None);
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

/// Une onde de R autour du lanceur ; la seconde (« seven ») projette au lieu de ralentir.
fn pulse(world: &mut World, ctx: &CastContext, seven: bool) {
    let Some(pos) = world.get(ctx.source).filter(|s| !s.dead).map(|s| s.pos) else {
        return;
    };
    let radius = ctx.def.radius.unwrap_or(0.0);
    world.emit(GameEvent::AreaEffect {
        source: ctx.source,
        fx_key: format!("master67_r_{}", if seven { "seven" } else { "six" }),
        pos,
        radius,
        duration: 0.67,
    });
    for target in enemies_in_radius(world, ctx.source, pos, radius) {
        hit(world, ctx, target);
        if seven {
            cc(
                world,
                ctx,
                target,
                CcType::Knockup,
                param(ctx, "knockup"),
                None,
            );
        } else {
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
}

fn r(world: &mut World, ctx: &CastContext) {
    let ctx = *ctx;
    pulse(world, &ctx, false);
    schedule(world, ctx.source, param(&ctx, "delay"), move |world| {
        pulse(world, &ctx, true);
    });
}

/// Dernier impact de sort compté par le passif (temps de jeu).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LastSpellHit(pub f64);

fn forget(world: &mut World, id: EntityId) {
    if let Some(e) = world.get_mut(id) {
        e.data.remove::<LastSpellHit>();
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
        let is_master = s
            .champ
            .as_ref()
            .is_some_and(|c| c.champion_id == "master67");
        let spell = ctx
            .ability
            .as_deref()
            .is_some_and(|a| a.starts_with("master67_") && a != "master67_p");
        if !is_master || s.dead || t.champ.is_none() || s.team == t.team || dealt <= 0.0 || !spell {
            return;
        }
        let time = world.time;
        let last = s
            .data
            .get::<LastSpellHit>()
            .map_or(f64::NEG_INFINITY, |l| l.0);
        if time - last < passive("hitLockout") - 1e-8 {
            return;
        }
        if let Some(s) = world.get_mut(source) {
            s.data.insert(LastSpellHit(time));
        }
        let max = passive("maxStacks");
        let mut count = BuffOptions::new(passive("duration"));
        count.source = Some(source);
        count.definition = Some(BuffDefOverride {
            max_stacks: Some(max as u32),
            fx_key: Some("master67_p".into()),
            ..BuffDefOverride::default()
        });
        let Some(buff) = add_buff(world, source, "master67_count", count) else {
            return;
        };
        if f64::from(buff.stacks) < max {
            return;
        }
        remove_buff(world, source, &buff.id, SourceMatch::Id(source));
        // `after_damage` passe avant la résolution de la mort : ne pas refrapper des PV déjà à zéro.
        let target_alive = world.get(ctx.target).is_some_and(|t| t.hp > 0.0 && !t.dead);
        let ap = world.get(source).map_or(0.0, |s| s.stats.ap);
        if target_alive {
            let mut bonus = DamageOpts::new(
                Some(source),
                ctx.target,
                passive("damage") + ap * passive("damageAp"),
                DamageType::Magic,
            );
            bonus.ability = Some("master67_p".into());
            apply_damage(world, bonus);
        }
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

pub static KIT: Kit = Kit::new("master67", [q, w, e, r]).with_install(install);
