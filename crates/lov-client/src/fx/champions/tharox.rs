//! FX de Tharox, la Charge Sauvage (événements : `lov-sim/src/abilities/champions/tharox.rs`).
//!
//! | sort | événements |
//! |------|------------|
//! | P Élan | buff slow (fxKey `tharox_p`) sur la cible de l'attaque renforcée → impact de défenses |
//! | Q Défenses | dash `tharox_q`, puis dash `tharox_q_knockback` sur la cible repoussée |
//! | W Écorce de fer | buff `tharox_w` (résistances) + zone `tharox_w` qui suit Tharox (dégâts par seconde) |
//! | E Piétinement | abilityCast + dégâts `tharox_e` (aucune zone émise) |
//! | R Ruée sauvage | abilityCast (1 s de préparation) puis dash `tharox_r` |
//!
//! Portage de `packages/client/src/fx/champions/tharox.ts`.

use std::f32::consts::TAU;

use bevy::prelude::*;
use lov_shared::data::champions::get_champion;
use lov_shared::types::{EntityId, GameEvent};

use crate::fx::core::palette::{FxColors, linear};
use crate::fx::core::runtime::Fx;
use crate::fx::effects::auras::{ShellOptions, shield_shell};
use crate::fx::effects::mesh_bursts::{BurstKind, BurstMode, BurstSpawn, burst};
use crate::fx::effects::primitives::{
    between, dash_trail, edit_decal, flash, rand, rand_range, shockwave, timed,
};
use crate::fx::particles::{EmitOptions, EmitShape};
use crate::fx::registry::{FxHandlers, RegisterFx};
use crate::fx::shaders::DecalShape;

const EMBER: Vec3 = Vec3::new(2.6, 0.8, 0.3);
const RUST: Vec3 = Vec3::new(1.2, 0.55, 0.25);

fn dust(fx: &mut Fx, at: Vec3, n: f32, r: f32) {
    let s = fx.styles();
    fx.ps().emit(
        s.dust,
        at,
        &EmitOptions {
            count: n,
            shape: EmitShape::Disc,
            radius: r,
            up: between(0.3, 1.0),
            speed: between(0.5, 1.5),
            life: [0.6, 1.0].into(),
            size: [0.7, 1.2].into(),
            drag: 2.0,
            ..default()
        },
    );
}

fn embers(fx: &mut Fx, at: Vec3, n: f32) {
    let s = fx.styles();
    fx.ps().emit(
        s.ember,
        at,
        &EmitOptions {
            count: n,
            shape: EmitShape::Sphere,
            radius: 0.3,
            up: between(1.0, 2.5),
            speed: between(0.5, 1.5),
            life: [0.4, 0.8].into(),
            size: [0.06, 0.12].into(),
            gravity: 2.0,
            tint: EMBER,
            ..default()
        },
    );
}

/// Charge : traînée de poussière et braises tout le long.
fn charge(
    fx: &mut Fx,
    source: EntityId,
    from: Vec3,
    to: Vec3,
    duration: f32,
    c: &FxColors,
    big: bool,
) {
    let color = if big { EMBER } else { c.accent * 0.6 };
    dash_trail(fx, source, from, to, duration, color, true);
    let mut acc = 0.0;
    timed(
        fx,
        duration.max(0.05),
        move |fx, _, dt| {
            acc += dt;
            if acc < 0.03 {
                return;
            }
            let Some(pos) = fx.entity_pos(source, 0.0) else {
                return;
            };
            acc = 0.0;
            dust(fx, pos, if big { 3.0 } else { 2.0 }, 0.5);
            embers(fx, pos.with_y(0.8), if big { 3.0 } else { 1.0 });
        },
        |_| {},
    );
}

fn dash_args(e: &GameEvent) -> Option<(EntityId, Vec3, Vec3, f32)> {
    match e {
        GameEvent::Dash {
            source,
            from,
            to,
            duration,
            ..
        } => Some((
            *source,
            Fx::world_pos(*from, 0.0),
            Fx::world_pos(*to, 0.0),
            *duration as f32,
        )),
        _ => None,
    }
}

fn p_buff(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let GameEvent::BuffApplied { target, .. } = e else {
        return;
    };
    let Some(v) = fx.entity_pos(*target, 1.0) else {
        return;
    };
    flash(fx, v, 2.0, EMBER, 0.2);
    let s = fx.styles();
    fx.ps().emit(
        s.spark,
        v,
        &EmitOptions {
            count: 14.0,
            shape: EmitShape::Point,
            speed: between(3.0, 6.0),
            life: [0.2, 0.35].into(),
            size: [0.06, 0.1].into(),
            stretch: 0.05,
            gravity: 6.0,
            tint: EMBER,
            ..default()
        },
    );
    shockwave(fx, v.with_y(0.0), 1.2, 0.3, EMBER, 0.1);
}

fn q_dash(fx: &mut Fx, e: &GameEvent, c: &FxColors) {
    if let Some((source, from, to, duration)) = dash_args(e) {
        charge(fx, source, from, to, duration, c, false);
    }
}

/// La cible repoussée glisse dans la poussière.
fn q_knockback(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let Some((source, from, _, duration)) = dash_args(e) else {
        return;
    };
    flash(fx, from.with_y(1.0), 2.0, EMBER, 0.2);
    dust(fx, from, 8.0, 0.4);
    timed(
        fx,
        duration.max(0.05),
        move |fx, _, _| {
            if let Some(pos) = fx.entity_pos(source, 0.0) {
                dust(fx, pos, 2.0, 0.3);
            }
        },
        |_| {},
    );
}

fn w_buff(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let GameEvent::BuffApplied {
        target,
        buff_id,
        duration,
        ..
    } = e
    else {
        return;
    };
    if buff_id != "tharox_w" {
        return;
    }
    shield_shell(
        fx,
        *target,
        (*duration as f32).max(0.5),
        &format!("tharox_w:{target}"),
        ShellOptions {
            color: RUST,
            radius: 1.2,
            height: 1.05,
            bands: 0.5,
            power: 1.4,
            ..default()
        },
    );
}

fn w_buff_end(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    if let GameEvent::BuffRemoved {
        target, buff_id, ..
    } = e
        && buff_id == "tharox_w"
    {
        fx.end_aura(&format!("tharox_w:{target}"));
    }
}

/// Zone qui suit Tharox : anneau de braises qui brûle autour de lui.
fn w_area(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let GameEvent::AreaEffect {
        source,
        radius,
        duration,
        ..
    } = e
    else {
        return;
    };
    let source = *source;
    let r = Fx::scale(*radius as f32);
    let d = fx.acquire_decal();
    let c = EMBER * 0.6;
    edit_decal(fx, &d, |m, _| {
        m.set_color(c, c);
        m.u.fill = 0.06;
        m.u.pattern = 0.8;
        m.u.edge = 0.1;
        m.u.opacity = 0.0;
        None
    });
    let ember = fx.styles().ember;
    let d2 = d.clone();
    timed(
        fx,
        (*duration as f32).max(0.3),
        move |fx, t, _| {
            let Some(pos) = fx.entity_pos(source, 0.0) else {
                return;
            };
            edit_decal(fx, &d, |m, g| {
                m.u.opacity = (t * 8.0).min((1.0 - t) * 6.0).min(1.0);
                Some(g.circle(m, pos, r, DecalShape::Circle))
            });
            if rand() < 0.6 {
                fx.ps().emit(
                    ember,
                    pos,
                    &EmitOptions {
                        count: 1.0,
                        shape: EmitShape::Ring,
                        radius: r * 0.9,
                        up: between(0.6, 1.4),
                        life: [0.4, 0.7].into(),
                        size: [0.06, 0.12].into(),
                        tint: EMBER,
                        ..default()
                    },
                );
            }
        },
        move |fx| fx.release_decal(d2),
    );
}

fn w_hit(fx: &mut Fx, e: &GameEvent) {
    if let GameEvent::Damage { target, .. } = e
        && let Some(v) = fx.entity_pos(*target, 0.8)
    {
        embers(fx, v, 4.0);
    }
}

/// Le sabot s'abat (~0,25 s dans l'animation) : onde, poussière, cailloux.
fn e_cast(fx: &mut Fx, e: &GameEvent, c: &FxColors) {
    let GameEvent::AbilityCast { source, .. } = e else {
        return;
    };
    let source = *source;
    let Some(start) = fx.entity_pos(source, 0.0) else {
        return;
    };
    let radius = get_champion("tharox")
        .and_then(|t| t.abilities.e.radius)
        .unwrap_or(375.0);
    let r = Fx::scale(radius as f32);
    let accent = c.accent_hdr;
    fx.after(0.25, move |fx| {
        let pos = fx.entity_pos(source, 0.0).unwrap_or(start);
        shockwave(fx, pos, r, 0.45, accent, 0.2);
        let s = fx.styles();
        fx.ps().emit(
            s.dust,
            pos,
            &EmitOptions {
                count: 18.0,
                shape: EmitShape::Ring,
                radius: r * 0.5,
                speed: between(1.5, 3.0),
                up: between(0.2, 0.8),
                life: [0.6, 1.0].into(),
                size: [0.8, 1.4].into(),
                drag: 2.0,
                ..default()
            },
        );
        for _ in 0..8 {
            let a = rand() * TAU;
            burst(
                fx,
                BurstKind::RockDebris,
                BurstSpawn {
                    pos: Vec3::new(pos.x + a.cos() * 0.5, 0.2, pos.z + a.sin() * 0.5),
                    mode: BurstMode::Debris,
                    size: rand_range(0.08, 0.15),
                    life: 0.9,
                    vel: Vec3::new(a.cos() * 2.0, rand_range(3.0, 5.0), a.sin() * 2.0),
                    color: Some(linear("#6b5d4d")),
                    ..default()
                },
            );
        }
    });
}

fn e_hit(fx: &mut Fx, e: &GameEvent) {
    if let GameEvent::Damage { target, .. } = e
        && let Some(v) = fx.entity_pos(*target, 0.0)
    {
        dust(fx, v, 4.0, 0.3);
    }
}

/// Préparation : il gratte le sol, vapeur et braises.
fn r_cast(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let GameEvent::AbilityCast { source, .. } = e else {
        return;
    };
    let source = *source;
    let smoke = fx.styles().smoke;
    timed(
        fx,
        1.0,
        move |fx, _, _| {
            let Some(pos) = fx.entity_pos(source, 0.0) else {
                return;
            };
            dust(fx, pos, 1.0, 0.5);
            if rand() < 0.5 {
                fx.ps().emit(
                    smoke,
                    pos.with_y(1.6),
                    &EmitOptions {
                        count: 1.0,
                        shape: EmitShape::Point,
                        up: between(0.5, 1.0),
                        speed: between(0.2, 0.5),
                        life: [0.5, 0.8].into(),
                        size: [0.3, 0.5].into(),
                        ..default()
                    },
                );
            }
        },
        |_| {},
    );
}

fn r_dash(fx: &mut Fx, e: &GameEvent, c: &FxColors) {
    if let Some((source, from, to, duration)) = dash_args(e) {
        charge(fx, source, from, to, duration, c, true);
    }
}

fn r_hit(fx: &mut Fx, e: &GameEvent) {
    let GameEvent::Damage { target, .. } = e else {
        return;
    };
    let Some(v) = fx.entity_pos(*target, 1.0) else {
        return;
    };
    flash(fx, v, 2.8, EMBER, 0.25);
    shockwave(fx, v.with_y(0.0), 1.8, 0.4, EMBER, 0.14);
    fx.shake(v, 25.0, 0.3);
    embers(fx, v, 12.0);
}

pub fn register(app: &mut App) {
    app.register_fx(
        "tharox_p",
        FxHandlers {
            buff: Some(p_buff),
            ..default()
        },
    )
    .register_fx(
        "tharox_q",
        FxHandlers {
            dash: Some(q_dash),
            ..default()
        },
    )
    .register_fx(
        "tharox_q_knockback",
        FxHandlers {
            dash: Some(q_knockback),
            ..default()
        },
    )
    .register_fx(
        "tharox_w",
        FxHandlers {
            buff: Some(w_buff),
            buff_end: Some(w_buff_end),
            area: Some(w_area),
            ..default()
        },
    )
    .register_fx(
        "tharox_e",
        FxHandlers {
            cast: Some(e_cast),
            ..default()
        },
    )
    .register_fx(
        "tharox_r",
        FxHandlers {
            cast: Some(r_cast),
            dash: Some(r_dash),
            ..default()
        },
    )
    .on_ability_hit("tharox_w", w_hit)
    .on_ability_hit("tharox_e", e_hit)
    .on_ability_hit("tharox_r", r_hit);
}
