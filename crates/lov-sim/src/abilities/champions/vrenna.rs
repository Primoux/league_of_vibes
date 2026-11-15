//! Vrenna : port de `packages/server/src/abilities/champions/vrenna.ts` et de ses branches de
//! `passives.ts`.
//!
//! - Q : chaîne en ligne qui traverse ; la pointe attire et projette.
//! - W : vitesse d'attaque et soin d'une part des PV manquants.
//! - E : bond (traverse les murs), blesse et ralentit à l'atterrissage.
//! - R : bond sur un champion, blesse et projette à l'impact.
//! - Passif (saignement) : auto-attaques et sorts cumulent un saignement (jusqu'à `maxStacks`) qui
//!   inflige des dégâts chaque seconde ; au maximum, Vrenna gagne de la vitesse d'attaque.

use lov_shared::data::champions::get_champion;
use lov_shared::math::dist;
use lov_shared::schema::{BuffKind, CcType};
use lov_shared::types::{DamageType, EntityId, PartialStats, Vec2};

use crate::abilities::prelude::*;
use crate::combat::buffs::{BuffDefOverride, BuffOptions, add_buff};
use crate::combat::damage::{DamageOpts, HealOpts};

fn passive(key: &str) -> f64 {
    get_champion("vrenna").map_or(0.0, |d| def_param(&d.passive, key, 1, 0.0))
}

fn q(world: &mut World, ctx: &CastContext) {
    let ctx = *ctx;
    let Some(origin) = ctx.source(world).map(|s| s.pos) else {
        return;
    };
    let options = LineOptions {
        pierce: true,
        ..LineOptions::default()
    };
    line(world, &ctx, options, move |world, target| {
        hit(world, &ctx, target);
        let (Some(t), Some(s)) = (world.get(target), world.get(ctx.source)) else {
            return;
        };
        if dist(t.pos, origin) < ctx.def.range - param(&ctx, "tipLength") {
            return;
        }
        let dir = direction(t.pos, s.pos, 0.0);
        let pos = t.pos;
        cc(world, &ctx, target, CcType::Knockup, 0.25, None);
        let pull = param(&ctx, "pull");
        let to = Vec2 {
            x: pos.x + dir.x * pull,
            y: pos.y + dir.y * pull,
        };
        let mut opts = DashOptions::new(format!("{}_pull", ctx.def.fx_key), 1500.0);
        opts.unstoppable = true;
        start_dash(world, target, to, opts);
    });
}

fn w(world: &mut World, ctx: &CastContext) {
    let mut buff = BuffOptions::new(param(ctx, "duration"));
    buff.source = Some(ctx.source);
    buff.params.stat_percent_mods = Some(PartialStats {
        attack_speed: Some(param(ctx, "attackSpeed")),
        ..PartialStats::default()
    });
    buff.definition = Some(BuffDefOverride {
        fx_key: Some(ctx.def.fx_key.clone()),
        ..BuffDefOverride::default()
    });
    add_buff(world, ctx.source, "vrenna_w", buff);
    let Some(s) = ctx.source(world) else { return };
    let amount = (s.stats.max_hp - s.hp) * param(ctx, "healMissing");
    apply_heal(world, HealOpts::new(Some(ctx.source), ctx.source, amount));
}

fn e(world: &mut World, ctx: &CastContext) {
    let ctx = *ctx;
    let mut opts = DashOptions::new(ctx.def.fx_key.clone(), ctx.def.speed.unwrap_or(0.0));
    opts.cross_walls = true;
    opts.on_end = Some(Box::new(move |world| {
        let Some(pos) = world.get(ctx.source).map(|s| s.pos) else {
            return;
        };
        for target in enemies_in_radius(world, ctx.source, pos, ctx.def.radius.unwrap_or(0.0)) {
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
    }));
    start_dash(world, ctx.source, ctx.point, opts);
}

fn r(world: &mut World, ctx: &CastContext) {
    let ctx = *ctx;
    let Some(target) = ctx.target else { return };
    let Some(to) = world.get(target).map(|t| t.pos) else {
        return;
    };
    let mut opts = DashOptions::new(ctx.def.fx_key.clone(), ctx.def.speed.unwrap_or(0.0));
    opts.cross_walls = true;
    opts.target = Some(target);
    opts.on_end = Some(Box::new(move |world| {
        let arrived = match (world.get(ctx.source), world.get(target)) {
            (Some(s), Some(t)) => dist_within(s, t, 200.0),
            _ => false,
        };
        if arrived {
            hit(world, &ctx, target);
            cc(
                world,
                &ctx,
                target,
                CcType::Knockup,
                param(&ctx, "knockup"),
                None,
            );
        }
    }));
    start_dash(world, ctx.source, to, opts);
}

/// Un saignement posé par une Vrenna.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bleed {
    pub source: EntityId,
    pub stacks: u32,
    pub expires: f64,
    pub next_tick: f64,
}

/// `entity.data.vrennaBleeds` : saignements portés, un par Vrenna.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Bleeds(pub Vec<Bleed>);

fn add_bleed(world: &mut World, source: EntityId, target: EntityId) {
    let time = world.time;
    let (max, duration) = (passive("maxStacks"), passive("duration"));
    let Some(t) = world.get_mut(target) else {
        return;
    };
    let bleeds = &mut t.data.get_or_insert_with(Bleeds::default).0;
    let i = match bleeds.iter().position(|b| b.source == source) {
        Some(i) => i,
        None => {
            bleeds.push(Bleed {
                source,
                stacks: 0,
                expires: 0.0,
                next_tick: time + 1.0,
            });
            bleeds.len() - 1
        }
    };
    let dot = &mut bleeds[i];
    dot.stacks = (max as u32).min(dot.stacks + 1);
    dot.expires = time + duration;
    let maxed = f64::from(dot.stacks) >= max;
    let mut mark = BuffOptions::new(duration);
    mark.source = Some(source);
    mark.definition = Some(BuffDefOverride {
        kind: Some(BuffKind::Debuff),
        max_stacks: Some(max as u32),
        fx_key: Some("vrenna_p".into()),
        ..BuffDefOverride::default()
    });
    add_buff(world, target, "vrenna_bleed", mark);
    if maxed {
        let mut speed = BuffOptions::new(duration);
        speed.source = Some(source);
        speed.params.stat_percent_mods = Some(PartialStats {
            attack_speed: Some(passive("attackSpeedAtMax")),
            ..PartialStats::default()
        });
        speed.definition = Some(BuffDefOverride {
            fx_key: Some("vrenna_p".into()),
            ..BuffDefOverride::default()
        });
        add_buff(world, source, "vrenna_p_speed", speed);
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
        let is_vrenna = s.champ.as_ref().is_some_and(|c| c.champion_id == "vrenna");
        if !is_vrenna || t.dead || s.team == t.team || dealt <= 0.0 || t.hp <= 0.0 {
            return;
        }
        let own_spell = ctx
            .ability
            .as_deref()
            .is_some_and(|a| a.starts_with("vrenna_") && a != "vrenna_p");
        if ctx.is_auto_attack || own_spell {
            add_bleed(world, source, ctx.target);
        }
    }));
}

/// Les saignements infligent `dps` par cumul à chaque seconde échue, jusqu'à leur expiration.
fn update_bleeds(world: &mut World, id: EntityId, _dt: f64) {
    let time = world.time;
    let dps = passive("dps");
    let Some(bleeds) = world.get(id).and_then(|e| e.data.get::<Bleeds>()).cloned() else {
        return;
    };
    let mut kept = Vec::with_capacity(bleeds.0.len());
    for mut dot in bleeds.0 {
        if !world.contains(dot.source) {
            continue;
        }
        while dot.next_tick <= time.min(dot.expires) + 1e-8
            && world.get(id).is_some_and(|e| !e.dead)
        {
            let mut opts = DamageOpts::new(
                Some(dot.source),
                id,
                f64::from(dot.stacks) * dps,
                DamageType::Physical,
            );
            opts.ability = Some("vrenna_p".into());
            apply_damage(world, opts);
            dot.next_tick += 1.0;
        }
        let dead = world.get(id).is_none_or(|e| e.dead);
        if time + 1e-8 < dot.expires && !dead {
            kept.push(dot);
        }
    }
    if let Some(e) = world.get_mut(id) {
        if kept.is_empty() {
            e.data.remove::<Bleeds>();
        } else {
            e.data.insert(Bleeds(kept));
        }
    }
}

fn clear_bleeds(world: &mut World, id: EntityId) {
    if let Some(e) = world.get_mut(id) {
        e.data.remove::<Bleeds>();
    }
}

pub static KIT: Kit = Kit::new("vrenna", [q, w, e, r])
    .with_targeting([
        TargetRule::Enemy,
        TargetRule::Enemy,
        TargetRule::Enemy,
        TargetRule::EnemyChampion,
    ])
    .with_install(install)
    .with_update_entity(update_bleeds)
    .with_reset_dead(clear_bleeds);
