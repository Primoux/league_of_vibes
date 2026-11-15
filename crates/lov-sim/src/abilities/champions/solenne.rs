//! Solenne : port de `packages/server/src/abilities/champions/solenne.ts`.
//!
//! - Q : rayon qui blesse et immobilise le premier ennemi.
//! - W : soigne un allié et soi-même (un seul soin si elle se cible).
//! - E : bouclier et vitesse d'attaque sur un allié.
//! - R : après un délai, soigne et protège les alliés de la zone.
//! - Passif (grâce) : ses soins et boucliers sont amplifiés par les PV manquants de la cible.

use lov_shared::data::champions::get_champion;
use lov_shared::schema::CcType;
use lov_shared::types::{EntityId, PartialStats};

use crate::abilities::prelude::*;
use crate::combat::buffs::{BuffDefOverride, BuffOptions, add_buff};
use crate::combat::damage::HealOpts;

/// Amplification de la grâce pour `target` (1 sans PV manquants).
fn grace(world: &World, target: EntityId) -> f64 {
    let amp =
        get_champion("solenne").map_or(0.0, |d| def_param(&d.passive, "ampPerMissing", 1, 0.0));
    world
        .get(target)
        .map_or(1.0, |t| 1.0 + amp * (1.0 - t.hp / t.stats.max_hp))
}

fn ap(world: &World, ctx: &CastContext) -> f64 {
    ctx.source(world).map_or(0.0, |s| s.stats.ap)
}

fn q(world: &mut World, ctx: &CastContext) {
    let ctx = *ctx;
    line(world, &ctx, LineOptions::default(), move |world, target| {
        hit(world, &ctx, target);
        cc(world, &ctx, target, CcType::Root, param(&ctx, "root"), None);
    });
}

fn w(world: &mut World, ctx: &CastContext) {
    let Some(target) = ctx.target else { return };
    let amount = param(ctx, "heal") + ap(world, ctx) * param(ctx, "healAp");
    let main = amount * grace(world, target);
    apply_heal(world, HealOpts::new(Some(ctx.source), target, main));
    // Se cibler donne le soin principal seulement, jamais deux soins sur la même unité.
    if target != ctx.source {
        let own = amount * param(ctx, "selfPercent") * grace(world, ctx.source);
        apply_heal(world, HealOpts::new(Some(ctx.source), ctx.source, own));
    }
}

fn e(world: &mut World, ctx: &CastContext) {
    let Some(target) = ctx.target else { return };
    let duration = param(ctx, "duration");
    let amount =
        (param(ctx, "shield") + ap(world, ctx) * param(ctx, "shieldAp")) * grace(world, target);
    let mut shield = ShieldOptions::new(ctx.source, target, amount);
    shield.duration = Some(duration);
    shield.buff_id = Some("solenne_e".into());
    shield.fx_key = Some(ctx.def.fx_key.clone());
    apply_shield(world, shield);
    let mut speed = BuffOptions::new(duration);
    speed.source = Some(ctx.source);
    speed.params.stat_percent_mods = Some(PartialStats {
        attack_speed: Some(param(ctx, "attackSpeed")),
        ..PartialStats::default()
    });
    speed.definition = Some(BuffDefOverride {
        fx_key: Some(ctx.def.fx_key.clone()),
        ..BuffDefOverride::default()
    });
    add_buff(world, target, "solenne_e_speed", speed);
}

fn r(world: &mut World, ctx: &CastContext) {
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
            let heal = param(&ctx, "heal") + ap(world, &ctx) * param(&ctx, "healAp");
            for target in allies_in_radius(world, ctx.source, ctx.point, radius) {
                let amplification = grace(world, target);
                apply_heal(
                    world,
                    HealOpts::new(Some(ctx.source), target, heal * amplification),
                );
                let mut shield =
                    ShieldOptions::new(ctx.source, target, param(&ctx, "shield") * amplification);
                shield.duration = Some(param(&ctx, "duration"));
                shield.buff_id = Some("solenne_r".into());
                shield.fx_key = Some(ctx.def.fx_key.clone());
                apply_shield(world, shield);
            }
        },
    );
}

pub static KIT: Kit = Kit::new("solenne", [q, w, e, r]).with_targeting([
    TargetRule::Enemy,
    TargetRule::AllyChampion,
    TargetRule::AllyChampion,
    TargetRule::Enemy,
]);
