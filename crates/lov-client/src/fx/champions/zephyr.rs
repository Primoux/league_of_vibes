//! FX de Zephyr, l'Arcaniste Instable (événements : `lov-sim/src/abilities/champions/zephyr.rs`).
//!
//! | sort | événements |
//! |------|------------|
//! | Q Orbe boomerang | projectile `zephyr_q` (aller), puis `zephyr_q_return` (retour, tête chercheuse) |
//! | W Brèche | blink `zephyr_w` ; à l'origine : `zephyr_w_explosion_telegraph` puis `zephyr_w_explosion` |
//! | E Chaîne arcanique | debuff `zephyr_e` (lien) sur la cible ; à la fin : root (statut générique) |
//! | R Barrage arcanique | rayons `zephyr_r` (0,5 s d'incantation, plusieurs charges) |
//!
//! Portage de `packages/client/src/fx/champions/zephyr.ts`.

use std::f32::consts::{PI, TAU};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use bevy::prelude::*;
use lov_shared::types::{EntityId, GameEvent};

use crate::fx::core::palette::FxColors;
use crate::fx::core::runtime::{AuraId, Effect, Fx};
use crate::fx::effects::primitives::{
    LightningOptions, ProjectileFx, ProjectileLook, TelegraphOptions, between, flash, lightning,
    rand, shockwave, telegraph, timed,
};
use crate::fx::particles::{EmitOptions, EmitShape};
use crate::fx::registry::{FxHandlers, ProjectileHandle, RegisterFx};

const VIOLET: Vec3 = Vec3::new(1.8, 0.8, 2.8);
const CYAN: Vec3 = Vec3::new(0.6, 2.4, 2.8);

fn arcane_burst(fx: &mut Fx, at: Vec3, scale: f32) {
    flash(fx, at, 2.0 * scale, VIOLET, 0.25);
    let s = fx.styles();
    fx.ps().emit(
        s.sparkles,
        at,
        &EmitOptions {
            count: (10.0 * scale).round(),
            shape: EmitShape::Sphere,
            radius: 0.3 * scale,
            speed: between(1.0, 3.0),
            life: [0.3, 0.6].into(),
            size: [0.25, 0.4].into(),
            tint: CYAN,
            ..default()
        },
    );
    fx.ps().emit(
        s.spark,
        at,
        &EmitOptions {
            count: (14.0 * scale).round(),
            shape: EmitShape::Point,
            speed: between(3.0, 6.0),
            life: [0.2, 0.35].into(),
            size: [0.05, 0.09].into(),
            stretch: 0.05,
            tint: VIOLET,
            ..default()
        },
    );
}

fn orb(
    fx: &mut Fx,
    e: &GameEvent,
    color: Vec3,
    core: Vec3,
    homing: bool,
) -> Option<Box<dyn ProjectileHandle>> {
    let GameEvent::ProjectileSpawn {
        source,
        from,
        to,
        speed,
        ..
    } = e
    else {
        return None;
    };
    let glow = fx.styles().glow;
    let mut spin = 0.0;
    let stop = ProjectileFx::launch(
        fx,
        Fx::world_pos(*from, 0.0),
        Fx::world_pos(*to, 0.0),
        Fx::scale(*speed as f32),
        ProjectileLook {
            color,
            core: Some(core),
            size: 0.7,
            height: 1.1,
            ribbon_width: 0.3,
            ribbon_life: 0.25,
            target: homing.then_some(*source),
            on_step: Some(Box::new(move |fx, head, _, dt| {
                spin += dt * 12.0;
                // deux étincelles en orbite autour de l'orbe
                for i in 0..2 {
                    let a = spin + i as f32 * PI;
                    let w = Vec3::new(head.x + a.cos() * 0.3, head.y + a.sin() * 0.3, head.z);
                    fx.ps().emit(
                        glow,
                        w,
                        &EmitOptions {
                            count: 1.0,
                            shape: EmitShape::Point,
                            life: 0.2.into(),
                            size: 0.18.into(),
                            tint: core,
                            ..default()
                        },
                    );
                }
            })),
            ..default()
        },
    );
    Some(Box::new(stop))
}

fn q_projectile(fx: &mut Fx, e: &GameEvent, _: &FxColors) -> Option<Box<dyn ProjectileHandle>> {
    orb(fx, e, VIOLET, CYAN, false)
}

fn q_return(fx: &mut Fx, e: &GameEvent, _: &FxColors) -> Option<Box<dyn ProjectileHandle>> {
    orb(fx, e, CYAN, VIOLET, true)
}

fn q_hit(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    if let GameEvent::ProjectileHit { pos, .. } = e {
        arcane_burst(fx, Fx::world_pos(*pos, 1.0), 0.8);
    }
}

/// Blink : déchirure au départ, apparition à l'arrivée.
fn w_dash(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    if let GameEvent::Dash { from, to, .. } = e {
        let from = Fx::world_pos(*from, 1.0);
        let to = Fx::world_pos(*to, 1.0);
        arcane_burst(fx, from, 1.0);
        arcane_burst(fx, to, 0.8);
        lightning(
            fx,
            from,
            to,
            VIOLET,
            LightningOptions {
                width: 0.12,
                life: 0.25,
                jag: 0.25,
                ..default()
            },
        );
    }
}

/// La brèche laissée derrière crépite pendant le délai.
fn w_explosion_telegraph(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
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
    let delay = (*duration as f32).max(0.2);
    telegraph(
        fx,
        center,
        r,
        delay,
        VIOLET * 0.5,
        TelegraphOptions {
            color2: Some(VIOLET),
            fill: Some(0.12),
            ..default()
        },
    );
    let glow = fx.styles().glow;
    let mut acc = 0.0;
    timed(
        fx,
        delay,
        move |fx, t, dt| {
            acc += dt;
            if acc < 0.07 {
                return;
            }
            acc = 0.0;
            let a = rand() * TAU;
            let v = center.with_y(1.0);
            let w = Vec3::new(
                center.x + a.cos() * r * 0.6,
                0.4 + rand(),
                center.z + a.sin() * r * 0.6,
            );
            let color = if rand() < 0.5 { VIOLET } else { CYAN };
            lightning(
                fx,
                v,
                w,
                color,
                LightningOptions {
                    width: 0.06,
                    life: 0.12,
                    jag: 0.15,
                    segments: 5,
                },
            );
            fx.ps().emit(
                glow,
                v,
                &EmitOptions {
                    count: 1.0,
                    shape: EmitShape::Point,
                    life: 0.15.into(),
                    size: (0.8 + t).into(),
                    tint: VIOLET,
                    ..default()
                },
            );
        },
        |_| {},
    );
}

fn w_explosion(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    if let GameEvent::AreaEffect { pos, radius, .. } = e {
        let center = Fx::world_pos(*pos, 0.0);
        shockwave(
            fx,
            center,
            Fx::scale(*radius as f32) * 1.1,
            0.45,
            VIOLET,
            0.16,
        );
        arcane_burst(fx, center.with_y(0.8), 1.6);
    }
}

/// Chaîne arcanique : un éclair relie Zephyr et la cible tant que le lien tient.
struct Chain {
    key: String,
    aura: AuraId,
    source: EntityId,
    target: EntityId,
    ended: Arc<AtomicBool>,
    until: f32,
    acc: f32,
}

impl Effect for Chain {
    fn update(&mut self, fx: &mut Fx, dt: f32) -> bool {
        self.acc += dt;
        if self.ended.load(Ordering::Relaxed) || fx.time() > self.until {
            return false;
        }
        let (Some(a), Some(b)) = (
            fx.entity_pos(self.source, 1.1),
            fx.entity_pos(self.target, 1.1),
        ) else {
            return false;
        };
        if self.acc >= 0.06 {
            self.acc = 0.0;
            lightning(
                fx,
                a,
                b,
                VIOLET,
                LightningOptions {
                    width: 0.1,
                    life: 0.1,
                    jag: 0.18,
                    segments: 8,
                },
            );
            let s = fx.styles();
            fx.ps().emit(
                s.sparkles,
                b,
                &EmitOptions {
                    count: 1.0,
                    shape: EmitShape::Sphere,
                    radius: 0.4,
                    life: 0.3.into(),
                    size: 0.3.into(),
                    tint: CYAN,
                    ..default()
                },
            );
        }
        true
    }

    fn end(&mut self, fx: &mut Fx) {
        fx.forget_aura(&self.key, self.aura);
    }
}

fn e_buff(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let GameEvent::BuffApplied {
        target,
        buff_id,
        duration,
        source: Some(source),
        ..
    } = e
    else {
        return;
    };
    if buff_id != "zephyr_e" {
        return;
    }
    let key = format!("zephyr_e:{target}");
    let ended = Arc::new(AtomicBool::new(false));
    let flag = ended.clone();
    let aura = fx.set_aura(&key, move |_| flag.store(true, Ordering::Relaxed));
    let until = fx.time() + (*duration as f32).max(0.3);
    fx.add(Chain {
        key,
        aura,
        source: *source,
        target: *target,
        ended,
        until,
        acc: 0.0,
    });
}

fn e_buff_end(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let GameEvent::BuffRemoved {
        target, buff_id, ..
    } = e
    else {
        return;
    };
    if buff_id != "zephyr_e" {
        return;
    }
    fx.end_aura(&format!("zephyr_e:{target}"));
    if let Some(v) = fx.entity_pos(*target, 1.0) {
        arcane_burst(fx, v, 1.2);
    }
}

/// Charge entre les mains pendant l'incantation.
fn r_cast(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let GameEvent::AbilityCast { source, .. } = e else {
        return;
    };
    let source = *source;
    let glow = fx.styles().glow;
    timed(
        fx,
        0.5,
        move |fx, t, _| {
            if let Some(pos) = fx.entity_pos(source, 1.3) {
                fx.ps().emit(
                    glow,
                    pos,
                    &EmitOptions {
                        count: 2.0,
                        shape: EmitShape::Sphere,
                        radius: 0.8 * (1.0 - t),
                        speed: between(0.0, 0.2),
                        life: 0.15.into(),
                        size: (0.3 + t * 0.6).into(),
                        tint: CYAN,
                        ..default()
                    },
                );
            }
        },
        |_| {},
    );
}

/// Rayon : faisceau épais et très rapide.
fn r_projectile(fx: &mut Fx, e: &GameEvent, _: &FxColors) -> Option<Box<dyn ProjectileHandle>> {
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
            color: VIOLET,
            core: Some(CYAN),
            size: 1.2,
            height: 1.2,
            ribbon_width: 0.7,
            ribbon_life: 0.4,
            trail_style: Some(sparkles),
            trail_density: 3.0,
            trail_size: [0.3, 0.5],
            trail_life: [0.3, 0.5],
            ..default()
        },
    );
    Some(Box::new(stop))
}

fn r_hit(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    if let GameEvent::ProjectileHit { pos, .. } = e {
        let at = Fx::world_pos(*pos, 1.0);
        arcane_burst(fx, at, 2.0);
        shockwave(fx, at.with_y(0.0), 2.0, 0.4, CYAN, 0.14);
    }
}

pub fn register(app: &mut App) {
    app.register_fx(
        "zephyr_q",
        FxHandlers {
            projectile: Some(q_projectile),
            hit: Some(q_hit),
            ..default()
        },
    )
    .register_fx(
        "zephyr_q_return",
        FxHandlers {
            projectile: Some(q_return),
            hit: Some(q_hit),
            ..default()
        },
    )
    .register_fx(
        "zephyr_w",
        FxHandlers {
            dash: Some(w_dash),
            ..default()
        },
    )
    .register_fx(
        "zephyr_w_explosion_telegraph",
        FxHandlers {
            area: Some(w_explosion_telegraph),
            ..default()
        },
    )
    .register_fx(
        "zephyr_w_explosion",
        FxHandlers {
            area: Some(w_explosion),
            ..default()
        },
    )
    .register_fx(
        "zephyr_e",
        FxHandlers {
            buff: Some(e_buff),
            buff_end: Some(e_buff_end),
            ..default()
        },
    )
    .register_fx(
        "zephyr_r",
        FxHandlers {
            cast: Some(r_cast),
            projectile: Some(r_projectile),
            hit: Some(r_hit),
            ..default()
        },
    );
}
