//! FX de Nyx, l'Ombre Sans Nom (événements : `lov-sim/src/abilities/champions/nyx.rs`).
//!
//! | sort | événements |
//! |------|------------|
//! | Q Lame du crépuscule | dégâts `nyx_q` sur la cible (+ marque de réinitialisation) |
//! | W Voile d'ombre | buff invisible (fxKey `nyx_w`) |
//! | E Pas de l'ombre | blink `nyx_e` derrière la cible |
//! | R Mort certaine | debuff `nyx_r` (3 s) puis explosion à son retrait |
//!
//! Portage de `packages/client/src/fx/champions/nyx.ts`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use bevy::prelude::*;
use lov_shared::types::{EntityId, GameEvent};

use crate::fx::core::palette::FxColors;
use crate::fx::core::runtime::{AuraId, Effect, Fx};
use crate::fx::effects::primitives::{LightningOptions, between, flash, lightning, shockwave};
use crate::fx::particles::{EmitOptions, EmitShape};
use crate::fx::registry::{FxHandlers, RegisterFx};

const PINK: Vec3 = Vec3::new(2.8, 0.4, 1.0);

fn shadow_puff(fx: &mut Fx, at: Vec3, n: f32) {
    let s = fx.styles();
    fx.ps().emit(
        s.dark_wisp,
        at,
        &EmitOptions {
            count: n,
            shape: EmitShape::Sphere,
            radius: 0.4,
            speed: between(0.5, 1.5),
            up: between(0.2, 0.8),
            life: [0.5, 0.9].into(),
            size: [0.6, 1.0].into(),
            ..default()
        },
    );
    fx.ps().emit(
        s.ember,
        at,
        &EmitOptions {
            count: (n / 2.0).round(),
            shape: EmitShape::Sphere,
            radius: 0.4,
            speed: between(0.5, 2.0),
            life: [0.3, 0.5].into(),
            size: [0.06, 0.1].into(),
            tint: PINK,
            ..default()
        },
    );
}

/// Entaille en X : deux traits roses croisés.
fn slash_x(fx: &mut Fx, at: Vec3) {
    for s in [-1.0, 1.0] {
        let a = Vec3::new(at.x - 0.55, at.y + 0.55 * s, at.z - 0.25 * s);
        let b = Vec3::new(at.x + 0.55, at.y - 0.55 * s, at.z + 0.25 * s);
        lightning(
            fx,
            a,
            b,
            PINK,
            LightningOptions {
                width: 0.16,
                life: 0.22,
                jag: 0.02,
                segments: 4,
            },
        );
    }
    flash(fx, at, 1.6, PINK, 0.15);
}

fn q_hit(fx: &mut Fx, e: &GameEvent) {
    if let GameEvent::Damage { target, .. } = e
        && let Some(v) = fx.entity_pos(*target, 1.1)
    {
        slash_x(fx, v);
    }
}

fn w_buff(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    if let GameEvent::BuffApplied { target, .. } = e
        && let Some(v) = fx.entity_pos(*target, 0.8)
    {
        shadow_puff(fx, v, 14.0);
    }
}

fn e_dash(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    if let GameEvent::Dash { from, to, .. } = e {
        let from = Fx::world_pos(*from, 0.8);
        let to = Fx::world_pos(*to, 0.8);
        shadow_puff(fx, from, 12.0);
        shadow_puff(fx, to, 10.0);
        flash(fx, to, 1.6, PINK, 0.2);
    }
}

/// Marque : sceau rose au-dessus de la tête, qui pulse de plus en plus vite.
struct Seal {
    key: String,
    aura: AuraId,
    target: EntityId,
    ended: Arc<AtomicBool>,
    duration: f32,
    t: f32,
    acc: f32,
}

impl Effect for Seal {
    fn update(&mut self, fx: &mut Fx, dt: f32) -> bool {
        self.t += dt;
        self.acc += dt;
        if self.ended.load(Ordering::Relaxed) || self.t > self.duration + 0.5 {
            return false;
        }
        let Some(pos) = fx.entity_pos(self.target, 0.0) else {
            return false;
        };
        let k = (self.t / self.duration).min(1.0);
        if self.acc > 0.05 {
            self.acc = 0.0;
            let s = fx.styles();
            let t = self.t;
            fx.ps().emit(
                s.rune,
                pos.with_y(2.4),
                &EmitOptions {
                    count: 1.0,
                    shape: EmitShape::Point,
                    life: 0.1.into(),
                    size: (0.5 + 0.15 * (t * (6.0 + k * 20.0)).sin()).into(),
                    spin: Some(3.0.into()),
                    tint: PINK,
                    ..default()
                },
            );
            fx.ps().emit(
                s.ring,
                pos.with_y(0.05),
                &EmitOptions {
                    count: 1.0,
                    shape: EmitShape::Point,
                    life: 0.1.into(),
                    size: (1.4 - k * 0.6).into(),
                    tint: PINK,
                    intensity: 0.4 + k,
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

fn r_buff(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let GameEvent::BuffApplied {
        target,
        buff_id,
        duration,
        ..
    } = e
    else {
        return;
    };
    if buff_id != "nyx_r" {
        return;
    }
    let key = format!("nyx_r:{target}");
    let ended = Arc::new(AtomicBool::new(false));
    let flag = ended.clone();
    let aura = fx.set_aura(&key, move |_| flag.store(true, Ordering::Relaxed));
    fx.add(Seal {
        key,
        aura,
        target: *target,
        ended,
        duration: (*duration as f32).max(0.5),
        t: 0.0,
        acc: 0.0,
    });
}

/// La marque explose.
fn r_buff_end(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let GameEvent::BuffRemoved {
        target, buff_id, ..
    } = e
    else {
        return;
    };
    if buff_id != "nyx_r" {
        return;
    }
    fx.end_aura(&format!("nyx_r:{target}"));
    let Some(pos) = fx.entity_pos(*target, 0.0) else {
        return;
    };
    let v = pos.with_y(1.1);
    flash(fx, v, 3.5, PINK, 0.3);
    shockwave(fx, pos, 2.2, 0.45, PINK, 0.16);
    shadow_puff(fx, v, 20.0);
    let s = fx.styles();
    fx.ps().emit(
        s.spark,
        v,
        &EmitOptions {
            count: 30.0,
            shape: EmitShape::Point,
            speed: between(4.0, 9.0),
            life: [0.25, 0.45].into(),
            size: [0.06, 0.1].into(),
            stretch: 0.06,
            tint: PINK,
            ..default()
        },
    );
}

pub fn register(app: &mut App) {
    app.register_fx(
        "nyx_w",
        FxHandlers {
            buff: Some(w_buff),
            ..default()
        },
    )
    .register_fx(
        "nyx_e",
        FxHandlers {
            dash: Some(e_dash),
            ..default()
        },
    )
    .register_fx(
        "nyx_r",
        FxHandlers {
            buff: Some(r_buff),
            buff_end: Some(r_buff_end),
            ..default()
        },
    )
    .on_ability_hit("nyx_q", q_hit);
}
