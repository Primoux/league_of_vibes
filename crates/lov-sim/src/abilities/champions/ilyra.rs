//! Ilyra : port de `packages/server/src/abilities/champions/ilyra.ts` et de ses branches de
//! `passives.ts`.
//!
//! - Q : éclat en ligne, blesse et ralentit le premier ennemi touché.
//! - W : explosion télégraphiée, blesse et ralentit fortement.
//! - E : bouclier sur un allié (ou soi) ; qui frappe le porteur est ralenti.
//! - R : blizzard, quatre tics de dégâts et un ralentissement croissant.
//! - Passif (givre) : chaque sort marque ; à `stacks` marques, étourdissement puis immunité temporaire
//!   de la cible à ce passif.

use lov_shared::data::champions::get_champion;
use lov_shared::schema::{BuffKind, CcType};
use lov_shared::types::EntityId;

use crate::abilities::prelude::*;
use crate::combat::buffs::{
    BuffDefOverride, BuffOptions, SourceMatch, add_buff, get_buff, remove_buff,
};

fn passive(key: &str, fallback: f64) -> f64 {
    get_champion("ilyra").map_or(fallback, |d| def_param(&d.passive, key, 1, fallback))
}

fn slow(world: &mut World, ctx: &CastContext, target: EntityId) {
    cc(
        world,
        ctx,
        target,
        CcType::Slow,
        param(ctx, "slowDuration"),
        Some(param(ctx, "slow")),
    );
}

fn q(world: &mut World, ctx: &CastContext) {
    let ctx = *ctx;
    line(world, &ctx, LineOptions::default(), move |world, target| {
        hit(world, &ctx, target);
        slow(world, &ctx, target);
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
                slow(world, &ctx, target);
            }
        },
    );
}

fn e(world: &mut World, ctx: &CastContext) {
    let Some(target) = ctx.target else { return };
    let ap = ctx.source(world).map_or(0.0, |s| s.stats.ap);
    let mut shield = ShieldOptions::new(
        ctx.source,
        target,
        param(ctx, "shield") + ap * param(ctx, "shieldAp"),
    );
    shield.duration = Some(param(ctx, "duration"));
    shield.buff_id = Some("ilyra_e".into());
    shield.fx_key = Some(ctx.def.fx_key.clone());
    apply_shield(world, shield);
}

fn r(world: &mut World, ctx: &CastContext) {
    let ctx = *ctx;
    let mut zone = ZoneOptions::new(
        ctx.source,
        ctx.point,
        ctx.def.radius.unwrap_or(0.0),
        param(&ctx, "duration"),
        ctx.def.fx_key.clone(),
        move |world, zone, elapsed| {
            let Some((pos, radius)) = world.get(zone).map(|z| (z.pos, z.radius)) else {
                return;
            };
            let slow = param(&ctx, "maxSlow").min(0.15 * elapsed);
            for target in enemies_in_radius(world, ctx.source, pos, radius) {
                hit(world, &ctx, target);
                cc(world, &ctx, target, CcType::Slow, 1.1, Some(slow));
            }
        },
    );
    zone.interval = Some(param_or(&ctx, "tickRate", 1.0));
    spawn_zone(world, zone);
}

/// Bouclier de givre (le porteur ralentit qui le frappe) et marques du passif.
fn install(world: &mut World) {
    register_damage_modifier(world, |world, ctx| {
        let Some(source) = ctx.credit.or(ctx.source) else {
            return;
        };
        let Some(t) = world.get(ctx.target) else {
            return;
        };
        let Some(frost) = get_buff(t, "ilyra_e", None) else {
            return;
        };
        let shielded = shield_remaining(t, frost) > 0.0;
        let (shield_source, target_team) = (frost.source, t.team);
        let enemy = world.get(source).is_some_and(|s| s.team != target_team);
        if shielded && enemy {
            let mut opts = BuffOptions::new(1.0);
            opts.source = shield_source;
            opts.params.amount = Some(0.2);
            add_buff(world, source, "slow", opts);
        }
    });

    world.hooks.after_damage.push(Box::new(|world, ctx, dealt| {
        let Some(source) = ctx.credit.or(ctx.source) else {
            return;
        };
        let ilyra_spell = ctx
            .ability
            .as_deref()
            .is_some_and(|a| a.starts_with("ilyra_"));
        let (Some(s), Some(t)) = (world.get(source), world.get(ctx.target)) else {
            return;
        };
        let is_ilyra = s.champ.as_ref().is_some_and(|c| c.champion_id == "ilyra");
        if !is_ilyra || t.dead || s.team == t.team || dealt <= 0.0 || t.hp <= 0.0 || !ilyra_spell {
            return;
        }
        if get_buff(t, "ilyra_frost_immunity", Some(source)).is_some() {
            return;
        }
        let target = ctx.target;
        let mut mark = BuffOptions::new(passive("markDuration", 4.0));
        mark.source = Some(source);
        mark.definition = Some(BuffDefOverride {
            kind: Some(BuffKind::Debuff),
            max_stacks: Some(passive("stacks", 3.0) as u32),
            fx_key: Some("ilyra_p".into()),
            ..BuffDefOverride::default()
        });
        let Some(buff) = add_buff(world, target, "ilyra_frost", mark) else {
            return;
        };
        if f64::from(buff.stacks) < passive("stacks", 3.0) {
            return;
        }
        remove_buff(world, target, &buff.id, SourceMatch::Id(source));
        let mut stun = BuffOptions::new(passive("stun", 0.0));
        stun.source = Some(source);
        stun.definition = Some(BuffDefOverride {
            fx_key: Some("ilyra_p".into()),
            ..BuffDefOverride::default()
        });
        add_buff(world, target, "stun", stun);
        let mut immunity = BuffOptions::new(passive("perTargetCooldown", 0.0));
        immunity.source = Some(source);
        add_buff(world, target, "ilyra_frost_immunity", immunity);
    }));
}

pub static KIT: Kit = Kit::new("ilyra", [q, w, e, r])
    .with_targeting([
        TargetRule::Enemy,
        TargetRule::Enemy,
        TargetRule::AllyChampion,
        TargetRule::Enemy,
    ])
    .with_install(install);
