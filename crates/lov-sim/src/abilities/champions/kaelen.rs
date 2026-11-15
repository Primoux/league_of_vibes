//! Kaelen : port de `packages/server/src/abilities/champions/kaelen.ts` et de ses branches de
//! `passives.ts`.
//!
//! - Q : trois ruées (relances) ; la troisième projette.
//! - W : parade : bloque le prochain coup de champion et étourdit un attaquant de mêlée (géré par
//!   `combat::buffs`, qui lit le buff `kaelen_w`).
//! - E : bouclier proportionnel à l'AD bonus, puis ruée.
//! - R : coup ciblé sur un champion, amplifié par ses PV manquants.
//! - Passif : une auto-attaque sur `everyN` inflige `bonus` de plus et soigne de `heal` des dégâts.

use lov_shared::data::champions::get_champion;
use lov_shared::schema::CcType;

use crate::abilities::prelude::*;
use crate::combat::buffs::{BaseAd, BuffDefOverride, BuffOptions, add_buff};

fn passive(key: &str) -> f64 {
    get_champion("kaelen").map_or(0.0, |d| def_param(&d.passive, key, 1, 0.0))
}

fn q(world: &mut World, ctx: &CastContext) {
    let ctx = *ctx;
    let third = ctx.cast_index == 2;
    let fx = if third {
        "kaelen_q_3".to_owned()
    } else {
        ctx.def.fx_key.clone()
    };
    let mut opts = DashOptions::new(fx, ctx.def.speed.unwrap_or(0.0));
    opts.on_end = Some(Box::new(move |world| {
        let Some(pos) = world.get(ctx.source).map(|s| s.pos) else {
            return;
        };
        for target in enemies_in_radius(world, ctx.source, pos, ctx.def.radius.unwrap_or(0.0)) {
            hit(world, &ctx, target);
            if third {
                cc(
                    world,
                    &ctx,
                    target,
                    CcType::Knockup,
                    param(&ctx, "knockup"),
                    None,
                );
            }
        }
    }));
    start_dash(world, ctx.source, ctx.point, opts);
}

fn w(world: &mut World, ctx: &CastContext) {
    let mut opts = BuffOptions::new(param(ctx, "window"));
    opts.source = Some(ctx.source);
    opts.params.set_number("stun", param(ctx, "stun"));
    opts.definition = Some(BuffDefOverride {
        fx_key: Some(ctx.def.fx_key.clone()),
        ..BuffDefOverride::default()
    });
    add_buff(world, ctx.source, "kaelen_w", opts);
}

fn e(world: &mut World, ctx: &CastContext) {
    let Some(s) = ctx.source(world) else { return };
    let base_ad = s.data.get::<BaseAd>().map_or(s.stats.ad, |b| b.0);
    let bonus_ad = (s.stats.ad - base_ad).max(0.0);
    let mut shield = ShieldOptions::new(
        ctx.source,
        ctx.source,
        param(ctx, "shield") + bonus_ad * param(ctx, "shieldBonusAd"),
    );
    shield.duration = Some(param(ctx, "shieldDuration"));
    shield.buff_id = Some("kaelen_e".into());
    shield.fx_key = Some(ctx.def.fx_key.clone());
    apply_shield(world, shield);
    let opts = DashOptions::new(ctx.def.fx_key.clone(), ctx.def.speed.unwrap_or(0.0));
    start_dash(world, ctx.source, ctx.point, opts);
}

fn r(world: &mut World, ctx: &CastContext) {
    let Some(target) = ctx.target else { return };
    let Some(t) = world.get(target) else { return };
    let missing = 1.0 - t.hp / t.stats.max_hp;
    hit_with(
        world,
        ctx,
        target,
        1.0 + param(ctx, "missingHpAmp") * missing,
        None,
    );
}

fn is_kaelen(world: &World, id: lov_shared::types::EntityId) -> bool {
    world
        .get(id)
        .and_then(|e| e.champ.as_ref())
        .is_some_and(|c| c.champion_id == "kaelen")
}

fn install(world: &mut World) {
    // La prochaine auto-attaque est la `everyN`-ième : bonus de dégâts.
    register_damage_modifier(world, |world, ctx| {
        let Some(source) = ctx.credit.or(ctx.source) else {
            return;
        };
        if !ctx.is_auto_attack || !is_kaelen(world, source) {
            return;
        }
        let (Some(s), Some(t)) = (world.get(source), world.get(ctx.target)) else {
            return;
        };
        if s.team == t.team {
            return;
        }
        let attacks = s.data.get::<PassiveState>().map_or(0, |p| p.attacks);
        let every = passive("everyN") as u32;
        if every > 0 && (attacks + 1).is_multiple_of(every) {
            ctx.amount *= 1.0 + passive("bonus");
        }
    });
    // Compte les coups réussis ; le `everyN`-ième soigne.
    world.hooks.after_damage.push(Box::new(|world, ctx, dealt| {
        let Some(source) = ctx.credit.or(ctx.source) else {
            return;
        };
        if !ctx.is_auto_attack || !is_kaelen(world, source) {
            return;
        }
        let Some(target_team) = world.get(ctx.target).filter(|t| !t.dead).map(|t| t.team) else {
            return;
        };
        let Some(s) = world.get_mut(source) else {
            return;
        };
        if s.team == target_team {
            return;
        }
        let state = passive_state(s);
        state.attacks += 1;
        let every = passive("everyN") as u32;
        if every > 0 && state.attacks.is_multiple_of(every) {
            apply_heal(
                world,
                crate::combat::damage::HealOpts::new(Some(source), source, dealt * passive("heal")),
            );
        }
    }));
}

pub static KIT: Kit = Kit::new("kaelen", [q, w, e, r])
    .with_targeting([
        TargetRule::Enemy,
        TargetRule::Enemy,
        TargetRule::Enemy,
        TargetRule::EnemyChampion,
    ])
    .with_install(install);
