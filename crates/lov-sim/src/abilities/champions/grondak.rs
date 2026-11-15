//! Grondak : port de `packages/server/src/abilities/champions/grondak.ts`.
//!
//! - Q : rocher en ligne, traverse, blesse et ralentit.
//! - W : réduit les dégâts subis et provoque les ennemis proches.
//! - E : charge sur une cible, blesse et étourdit à l'arrivée.
//! - R : impact télégraphié, blesse et projette.
//! - Passif (granit) : bouclier de `shieldMaxHp` des PV max après `outOfCombat` secondes sans dégâts
//!   de champion ; il repousse une fois consommé.

use lov_shared::data::champions::get_champion;
use lov_shared::schema::CcType;

use crate::abilities::prelude::*;
use crate::combat::buffs::{
    BuffDefOverride, BuffOptions, LastChampionDamageAt, SourceMatch, add_buff, get_buff,
    remove_buff,
};
use lov_shared::types::EntityId;

/// Paramètre du passif de Grondak.
fn passive(key: &str) -> f64 {
    get_champion("grondak").map_or(0.0, |d| def_param(&d.passive, key, 1, 0.0))
}

fn q(world: &mut World, ctx: &CastContext) {
    let ctx = *ctx;
    let options = LineOptions {
        pierce: true,
        ..LineOptions::default()
    };
    line(world, &ctx, options, move |world, target| {
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
    let mut opts = BuffOptions::new(param(ctx, "duration"));
    opts.source = Some(ctx.source);
    opts.params.damage_reduction = Some(param(ctx, "damageReduction"));
    opts.definition = Some(BuffDefOverride {
        fx_key: Some(ctx.def.fx_key.clone()),
        ..BuffDefOverride::default()
    });
    add_buff(world, ctx.source, "grondak_w", opts);
    let Some(pos) = ctx.source(world).map(|s| s.pos) else {
        return;
    };
    for enemy in enemies_in_radius(world, ctx.source, pos, ctx.def.radius.unwrap_or(0.0)) {
        cc(world, ctx, enemy, CcType::Taunt, param(ctx, "taunt"), None);
    }
}

fn e(world: &mut World, ctx: &CastContext) {
    let ctx = *ctx;
    let Some(target) = ctx.target else { return };
    let Some(to) = world.get(target).map(|t| t.pos) else {
        return;
    };
    let mut opts = DashOptions::new(ctx.def.fx_key.clone(), ctx.def.speed.unwrap_or(0.0));
    opts.target = Some(target);
    opts.on_end = Some(Box::new(move |world| {
        let arrived = match (world.get(ctx.source), world.get(target)) {
            (Some(s), Some(t)) => dist_within(s, t, 200.0),
            _ => false,
        };
        if arrived {
            hit(world, &ctx, target);
            cc(world, &ctx, target, CcType::Stun, param(&ctx, "stun"), None);
        }
    }));
    start_dash(world, ctx.source, to, opts);
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
            for enemy in enemies_in_radius(world, ctx.source, ctx.point, radius) {
                hit(world, &ctx, enemy);
                cc(
                    world,
                    &ctx,
                    enemy,
                    CcType::Knockup,
                    param(&ctx, "knockup"),
                    None,
                );
            }
        },
    );
}

/// `source.data.grondak` : prochaine repousse du granit, bouclier actif.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Granite {
    pub next_at: f64,
    pub active: bool,
}

/// `updateGranite` : chaque tick, pour Grondak vivant.
fn update_granite(world: &mut World, id: EntityId, _dt: f64) {
    let wait = passive("outOfCombat");
    let time = world.time;
    let Some(e) = world.get(id) else { return };
    let remaining = get_buff(e, "grondak_p", Some(id)).map_or(0.0, |b| shield_remaining(e, b));
    let last_hit = e.data.get::<LastChampionDamageAt>().map_or(-wait, |l| l.0);
    let max_hp = e.stats.max_hp;
    let Some(e) = world.get_mut(id) else { return };
    let state = e.data.get_or_insert_with(|| Granite {
        next_at: time + wait,
        active: false,
    });
    if remaining > 0.0 {
        state.active = true;
        return;
    }
    if state.active {
        state.active = false;
        state.next_at = time + wait;
        remove_buff(world, id, "grondak_p", SourceMatch::Id(id));
    }
    let Some(state) = world.get_mut(id).and_then(|e| e.data.get_mut::<Granite>()) else {
        return;
    };
    state.next_at = state.next_at.max(last_hit + wait);
    if time >= state.next_at {
        state.active = true;
        let mut shield = ShieldOptions::new(id, id, max_hp * passive("shieldMaxHp"));
        shield.duration = Some(86_400.0);
        shield.buff_id = Some("grondak_p".into());
        shield.fx_key = Some("grondak_p".into());
        apply_shield(world, shield);
    }
}

pub static KIT: Kit = Kit::new("grondak", [q, w, e, r]).with_update_champion(update_granite);
