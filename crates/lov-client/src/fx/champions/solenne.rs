//! FX de Solenne, la Prêtresse de l'Aube (événements : `lov-sim/src/abilities/champions/solenne.rs`).
//!
//! | sort | événements |
//! |------|------------|
//! | Q Lien lumineux | projectile `solenne_q` ; root (statut générique, doré) |
//! | W Bénédiction | abilityCast (targetId) puis soins (événements heal, FX génériques) |
//! | E Bouclier sacré | buff de bouclier `solenne_e` + buff `solenne_e_speed` |
//! | R Aube | `solenne_r_telegraph` (0,5 s) puis `solenne_r` : soins + boucliers `solenne_r` |
//!
//! Portage de `packages/client/src/fx/champions/solenne.ts`.

use std::f32::consts::TAU;

use bevy::prelude::*;
use lov_shared::types::GameEvent;

use crate::fx::core::palette::FxColors;
use crate::fx::core::runtime::Fx;
use crate::fx::effects::auras::{ShellOptions, shield_shell};
use crate::fx::effects::primitives::{
    ProjectileFx, ProjectileLook, TelegraphOptions, between, flash, rand, shockwave, telegraph,
    timed,
};
use crate::fx::particles::{EmitOptions, EmitShape};
use crate::fx::registry::{FxHandlers, ProjectileHandle, RegisterFx};

const DAWN: Vec3 = Vec3::new(2.8, 2.3, 1.2);
const SKY: Vec3 = Vec3::new(1.0, 2.3, 2.6);

fn q_projectile(fx: &mut Fx, e: &GameEvent, _: &FxColors) -> Option<Box<dyn ProjectileHandle>> {
    let GameEvent::ProjectileSpawn {
        from, to, speed, ..
    } = e
    else {
        return None;
    };
    let sparkles = fx.styles().sparkles;
    let stop = ProjectileFx::launch(
        fx,
        Fx::world_pos(*from, 0.0),
        Fx::world_pos(*to, 0.0),
        Fx::scale(*speed as f32),
        ProjectileLook {
            color: DAWN,
            core: Some(Vec3::splat(3.0)),
            size: 0.6,
            height: 1.1,
            ribbon_width: 0.35,
            ribbon_life: 0.35,
            trail_style: Some(sparkles),
            trail_density: 3.0,
            trail_size: [0.25, 0.4],
            ..default()
        },
    );
    Some(Box::new(stop))
}

fn q_hit(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let GameEvent::ProjectileHit { pos, .. } = e else {
        return;
    };
    let at = Fx::world_pos(*pos, 1.0);
    flash(fx, at, 2.0, DAWN, 0.2);
    let s = fx.styles();
    fx.ps().emit(
        s.sparkles,
        at,
        &EmitOptions {
            count: 10.0,
            shape: EmitShape::Sphere,
            radius: 0.4,
            speed: between(1.0, 2.0),
            life: [0.4, 0.7].into(),
            size: [0.3, 0.45].into(),
            tint: DAWN,
            ..default()
        },
    );
}

/// Rayon de bénédiction de Solenne vers l'allié.
fn w_cast(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let GameEvent::AbilityCast {
        source, target_id, ..
    } = e
    else {
        return;
    };
    let Some(from) = fx.entity_pos(*source, 1.2) else {
        return;
    };
    let s = fx.styles();
    fx.ps().emit(
        s.sparkles,
        from,
        &EmitOptions {
            count: 8.0,
            shape: EmitShape::Sphere,
            radius: 0.5,
            life: [0.4, 0.6].into(),
            size: [0.3, 0.4].into(),
            tint: DAWN,
            ..default()
        },
    );
    let Some(target) = target_id.filter(|t| t != source) else {
        return;
    };
    let to = fx.entity_pos(target, 1.1).unwrap_or(from);
    let _ = ProjectileFx::launch(
        fx,
        from,
        to,
        22.0,
        ProjectileLook {
            color: DAWN,
            core: Some(SKY),
            size: 0.4,
            height: 1.1,
            ribbon_width: 0.25,
            ribbon_life: 0.4,
            target: Some(target),
            ..default()
        },
    );
}

fn shell(fx: &mut Fx, e: &GameEvent, id: &str, color: Vec3, radius: f32, bands: f32) -> bool {
    let GameEvent::BuffApplied {
        target,
        buff_id,
        duration,
        ..
    } = e
    else {
        return false;
    };
    if buff_id != id {
        return false;
    }
    shield_shell(
        fx,
        *target,
        (*duration as f32).max(0.3),
        &format!("{id}:{target}"),
        ShellOptions {
            color,
            radius,
            bands,
            ..default()
        },
    );
    true
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
    if shell(fx, e, "solenne_e", Vec3::new(1.6, 1.3, 0.6), 1.1, 0.3) {
        if let Some(v) = fx.entity_pos(*target, 1.0) {
            flash(fx, v, 2.2, DAWN, 0.25);
        }
    } else if buff_id == "solenne_e_speed" {
        let target = *target;
        let sparkles = fx.styles().sparkles;
        timed(
            fx,
            (*duration as f32).max(0.3),
            move |fx, _, _| {
                if rand() < 0.3
                    && let Some(pos) = fx.entity_pos(target, 1.0)
                {
                    fx.ps().emit(
                        sparkles,
                        pos,
                        &EmitOptions {
                            count: 1.0,
                            shape: EmitShape::Sphere,
                            radius: 0.5,
                            life: 0.4.into(),
                            size: 0.3.into(),
                            tint: SKY,
                            ..default()
                        },
                    );
                }
            },
            |_| {},
        );
    }
}

fn e_buff_end(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    end_shell(fx, e, "solenne_e");
}

/// Colonne de lumière : grand faisceau vertical, anneau solaire, pluie d'étincelles.
fn r_area(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let GameEvent::AreaEffect { pos, radius, .. } = e else {
        return;
    };
    let center = Fx::world_pos(*pos, 0.0);
    let r = Fx::scale(*radius as f32);
    flash(fx, center.with_y(2.0), r * 0.8, DAWN, 0.45);
    shockwave(fx, center, r * 1.1, 0.6, DAWN, 0.25);
    let s = fx.styles();
    timed(
        fx,
        0.7,
        move |fx, t, _| {
            let k = 1.0 - t;
            fx.ps().emit(
                s.spark,
                center,
                &EmitOptions {
                    count: 6.0,
                    shape: EmitShape::Disc,
                    radius: r * 0.5 * k + 0.3,
                    up: between(10.0, 16.0),
                    life: [0.3, 0.5].into(),
                    size: [0.1, 0.18].into(),
                    stretch: 0.08,
                    tint: DAWN,
                    ..default()
                },
            );
            fx.ps().emit(
                s.glow,
                center,
                &EmitOptions {
                    count: 1.0,
                    shape: EmitShape::Disc,
                    radius: r * 0.3,
                    up: between(6.0, 10.0),
                    life: [0.3, 0.5].into(),
                    size: [0.8, 1.2].into(),
                    tint: DAWN,
                    intensity: 0.4,
                    ..default()
                },
            );
        },
        |_| {},
    );
    fx.ps().emit(
        s.sparkles,
        center,
        &EmitOptions {
            count: 30.0,
            shape: EmitShape::Disc,
            radius: r * 0.9,
            up: between(0.5, 2.0),
            life: [0.8, 1.3].into(),
            size: [0.3, 0.5].into(),
            tint: SKY,
            ..default()
        },
    );
}

/// Bouclier de l'aube sur chaque allié touché.
fn r_buff(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    shell(fx, e, "solenne_r", Vec3::new(1.5, 1.2, 0.5), 1.05, 0.2);
}

fn r_buff_end(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    end_shell(fx, e, "solenne_r");
}

/// Avant l'aube : cercle doré, motes de lumière qui descendent.
fn r_telegraph(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let GameEvent::AreaEffect {
        pos,
        radius,
        duration,
        ..
    } = e
    else {
        return;
    };
    let center = Fx::world_pos(*pos, 0.0);
    let r = Fx::scale(*radius as f32);
    let delay = (*duration as f32).max(0.15);
    telegraph(
        fx,
        center,
        r,
        delay,
        DAWN * 0.5,
        TelegraphOptions {
            color2: Some(DAWN),
            fill: Some(0.12),
            ..default()
        },
    );
    let glow = fx.styles().glow;
    timed(
        fx,
        delay,
        move |fx, _, _| {
            let a = rand() * TAU;
            let d = rand().sqrt() * r;
            let p = Vec3::new(center.x + a.cos() * d, 4.0, center.z + a.sin() * d);
            fx.ps().emit(
                glow,
                p,
                &EmitOptions {
                    count: 1.0,
                    shape: EmitShape::Point,
                    inherit: Some(Vec3::new(0.0, -7.0, 0.0)),
                    life: 0.5.into(),
                    size: 0.35.into(),
                    tint: DAWN,
                    ..default()
                },
            );
        },
        |_| {},
    );
}

pub fn register(app: &mut App) {
    app.register_fx(
        "solenne_q",
        FxHandlers {
            projectile: Some(q_projectile),
            hit: Some(q_hit),
            ..default()
        },
    )
    .register_fx(
        "solenne_w",
        FxHandlers {
            cast: Some(w_cast),
            ..default()
        },
    )
    .register_fx(
        "solenne_e",
        FxHandlers {
            buff: Some(e_buff),
            buff_end: Some(e_buff_end),
            ..default()
        },
    )
    .register_fx(
        "solenne_r",
        FxHandlers {
            area: Some(r_area),
            buff: Some(r_buff),
            buff_end: Some(r_buff_end),
            ..default()
        },
    )
    .register_fx(
        "solenne_r_telegraph",
        FxHandlers {
            area: Some(r_telegraph),
            ..default()
        },
    );
}
