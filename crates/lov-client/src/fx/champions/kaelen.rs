//! FX de Kaelen, la Lame de l'Aube (événements : `lov-sim/src/abilities/champions/kaelen.rs`).
//!
//! | sort | événements |
//! |------|------------|
//! | Q Taille ascendante | dash `kaelen_q` (1re, 2e) ou `kaelen_q_3` (3e, projette) ; dégâts en fin de dash |
//! | W Parade | buff `kaelen_w` (fenêtre) ; contre : buff stun (fxKey `kaelen_w`) sur l'attaquant |
//! | E Fente | buff de bouclier `kaelen_e` + dash `kaelen_e` |
//! | R Jugement solaire | abilityCast (0,4 s) puis dégâts `kaelen_r` sur la cible |
//!
//! Portage de `packages/client/src/fx/champions/kaelen.ts`.

use bevy::prelude::*;
use lov_shared::types::GameEvent;

use crate::fx::core::palette::FxColors;
use crate::fx::core::runtime::Fx;
use crate::fx::effects::auras::{ShellOptions, shield_shell};
use crate::fx::effects::primitives::{between, dash_trail, flash, shockwave, timed};
use crate::fx::particles::{EmitOptions, EmitShape};
use crate::fx::registry::{FxHandlers, RegisterFx};

const SUN: Vec3 = Vec3::new(2.8, 2.2, 1.0);
const LIGHT: Vec3 = Vec3::new(2.4, 2.4, 2.6);

/// Croissant d'étincelles devant `at` dans la direction `dir` (taille d'épée).
fn crescent(fx: &mut Fx, at: Vec3, dir: Vec3, radius: f32, color: Vec3, n: u32) {
    let base = dir.z.atan2(dir.x);
    let s = fx.styles();
    for i in 0..n {
        let a = base - 1.1 + 2.2 * i as f32 / (n - 1) as f32;
        let w = Vec3::new(
            at.x + a.cos() * radius,
            1.0 + (i as f32 * 0.7).sin() * 0.1,
            at.z + a.sin() * radius,
        );
        fx.ps().emit(
            s.spark,
            w,
            &EmitOptions {
                count: 1.0,
                shape: EmitShape::Point,
                speed: between(0.5, 1.0),
                inherit: Some(Vec3::new(-a.sin() * 4.0, 0.0, a.cos() * 4.0)),
                life: [0.15, 0.25].into(),
                size: [0.08, 0.12].into(),
                stretch: 0.05,
                tint: color,
                ..default()
            },
        );
    }
}

fn q_dash_fx(fx: &mut Fx, e: &GameEvent, third: bool) {
    let GameEvent::Dash {
        source,
        from,
        to,
        duration,
        ..
    } = e
    else {
        return;
    };
    let from = Fx::world_pos(*from, 0.0);
    let to = Fx::world_pos(*to, 0.0);
    let dir = (to - from).with_y(0.0).normalize_or_zero();
    let duration = (*duration as f32).max(0.05);
    let source = *source;
    dash_trail(fx, source, from, to, duration, SUN * 0.6, false);
    fx.after(duration, move |fx| {
        let pos = fx.entity_pos(source, 0.0).unwrap_or(to);
        crescent(fx, pos, dir, 1.1, SUN, if third { 30 } else { 20 });
        if third {
            // 3e frappe : tourbillon ascendant qui projette
            shockwave(fx, pos, 2.2, 0.4, SUN, 0.16);
            let glow = fx.styles().glow;
            timed(
                fx,
                0.4,
                move |fx, t, _| {
                    let a = t * 25.0;
                    let w = Vec3::new(pos.x + a.cos() * 0.9, 0.2 + t * 2.5, pos.z + a.sin() * 0.9);
                    fx.ps().emit(
                        glow,
                        w,
                        &EmitOptions {
                            count: 2.0,
                            shape: EmitShape::Point,
                            life: 0.3.into(),
                            size: 0.4.into(),
                            tint: SUN,
                            ..default()
                        },
                    );
                },
                |_| {},
            );
        }
    });
}

fn q_dash(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    q_dash_fx(fx, e, false);
}

fn q3_dash(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    q_dash_fx(fx, e, true);
}

fn w_buff(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let GameEvent::BuffApplied {
        target,
        buff_id,
        duration,
        source,
        ..
    } = e
    else {
        return;
    };
    if buff_id == "kaelen_w" {
        shield_shell(
            fx,
            *target,
            (*duration as f32).max(0.3),
            &format!("kaelen_w:{target}"),
            ShellOptions {
                color: SUN * 0.6,
                radius: 1.05,
                bands: 0.6,
                power: 3.0,
                ..default()
            },
        );
        return;
    }
    // contre réussi : l'attaquant est étourdi
    if buff_id == "stun"
        && let Some(v) = fx.entity_pos(*target, 1.0)
    {
        flash(fx, v, 2.6, SUN, 0.3);
        let s = fx.styles();
        fx.ps().emit(
            s.spark,
            v,
            &EmitOptions {
                count: 20.0,
                shape: EmitShape::Point,
                speed: between(3.0, 7.0),
                life: [0.2, 0.35].into(),
                size: [0.07, 0.12].into(),
                stretch: 0.05,
                tint: SUN,
                ..default()
            },
        );
        if let Some(src) = source {
            fx.end_aura(&format!("kaelen_w:{src}"));
        }
    }
}

fn end_shell(fx: &mut Fx, e: &GameEvent, id: &str) {
    if let GameEvent::BuffRemoved {
        target, buff_id, ..
    } = e
        && buff_id == id
    {
        fx.end_aura(&format!("{id}:{target}"));
    }
}

fn w_buff_end(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    end_shell(fx, e, "kaelen_w");
}

fn e_dash(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    if let GameEvent::Dash {
        source,
        from,
        to,
        duration,
        ..
    } = e
    {
        dash_trail(
            fx,
            *source,
            Fx::world_pos(*from, 0.0),
            Fx::world_pos(*to, 0.0),
            (*duration as f32).max(0.05),
            LIGHT * 0.6,
            false,
        );
    }
}

fn e_buff(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let GameEvent::BuffApplied {
        target,
        buff_id,
        duration,
        ..
    } = e
    else {
        return;
    };
    if buff_id != "kaelen_e" {
        return;
    }
    shield_shell(
        fx,
        *target,
        (*duration as f32).max(0.3),
        &format!("kaelen_e:{target}"),
        ShellOptions {
            color: Vec3::new(1.3, 1.2, 0.8),
            radius: 1.05,
            bands: 0.2,
            ..default()
        },
    );
}

fn e_buff_end(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    end_shell(fx, e, "kaelen_e");
}

/// Une lame de lumière se forme au-dessus de la cible pendant l'incantation.
fn r_cast(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let GameEvent::AbilityCast {
        target_id: Some(target),
        ..
    } = e
    else {
        return;
    };
    let target = *target;
    let s = fx.styles();
    timed(
        fx,
        0.4,
        move |fx, t, _| {
            let Some(pos) = fx.entity_pos(target, 0.0) else {
                return;
            };
            let v = pos.with_y(5.0 - t * 3.5);
            fx.ps().emit(
                s.spark,
                v,
                &EmitOptions {
                    count: 3.0,
                    shape: EmitShape::Sphere,
                    radius: 0.15,
                    life: 0.15.into(),
                    size: [0.12, 0.2].into(),
                    stretch: 0.1,
                    inherit: Some(Vec3::new(0.0, -14.0, 0.0)),
                    tint: SUN,
                    ..default()
                },
            );
            fx.ps().emit(
                s.glow,
                v,
                &EmitOptions {
                    count: 1.0,
                    shape: EmitShape::Point,
                    life: 0.12.into(),
                    size: (0.9 + t).into(),
                    tint: SUN,
                    ..default()
                },
            );
        },
        |_| {},
    );
}

/// Impact du jugement, à l'instant des dégâts serveur.
fn r_hit(fx: &mut Fx, e: &GameEvent) {
    let GameEvent::Damage { target, .. } = e else {
        return;
    };
    let Some(pos) = fx.entity_pos(*target, 0.0) else {
        return;
    };
    flash(fx, pos.with_y(1.0), 4.0, SUN, 0.35);
    shockwave(fx, pos, 2.4, 0.5, SUN, 0.2);
    fx.shake(pos, 25.0, 0.3);
    let s = fx.styles();
    fx.ps().emit(
        s.spark,
        pos,
        &EmitOptions {
            count: 40.0,
            shape: EmitShape::Disc,
            radius: 0.4,
            up: between(4.0, 10.0),
            speed: between(1.0, 3.0),
            life: [0.3, 0.6].into(),
            size: [0.08, 0.14].into(),
            stretch: 0.07,
            tint: SUN,
            ..default()
        },
    );
    fx.ps().emit(
        s.glow,
        pos,
        &EmitOptions {
            count: 8.0,
            shape: EmitShape::Disc,
            radius: 0.3,
            up: between(6.0, 10.0),
            life: [0.3, 0.5].into(),
            size: [0.6, 1.0].into(),
            tint: SUN,
            ..default()
        },
    );
}

pub fn register(app: &mut App) {
    app.register_fx(
        "kaelen_q",
        FxHandlers {
            dash: Some(q_dash),
            ..default()
        },
    )
    .register_fx(
        "kaelen_q_3",
        FxHandlers {
            dash: Some(q3_dash),
            ..default()
        },
    )
    .register_fx(
        "kaelen_w",
        FxHandlers {
            buff: Some(w_buff),
            buff_end: Some(w_buff_end),
            ..default()
        },
    )
    .register_fx(
        "kaelen_e",
        FxHandlers {
            dash: Some(e_dash),
            buff: Some(e_buff),
            buff_end: Some(e_buff_end),
            ..default()
        },
    )
    .register_fx(
        "kaelen_r",
        FxHandlers {
            cast: Some(r_cast),
            ..default()
        },
    )
    .on_ability_hit("kaelen_r", r_hit);
}
