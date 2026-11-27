//! FX de Sable, la Tireuse des Dunes (événements : `lov-sim/src/abilities/champions/sable.rs`).
//!
//! | sort | événements |
//! |------|------------|
//! | P Œil du désert | buff `sable_p_speed` (vitesse après un coup sur un champion) |
//! | Q Tir explosif | buff `sable_q` (arme chargée) ; zone `sable_q_explosion` à l'impact de l'attaque |
//! | W Volée | 7 projectiles `sable_w` en cône |
//! | E Roulade | dash `sable_e` + buff `sable_e` (critique garanti) |
//! | R Balle du jugement | abilityCast (1 s de visée) puis projectile `sable_r` global |
//!
//! Portage de `packages/client/src/fx/champions/sable.ts`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use bevy::prelude::*;
use lov_shared::types::{EntityId, GameEvent};

use crate::fx::core::palette::FxColors;
use crate::fx::core::runtime::{AuraId, Effect, Fx};
use crate::fx::effects::primitives::{
    ProjectileFx, ProjectileLook, ProjectileStop, between, edit_decal, flash, shockwave, timed,
};
use crate::fx::particles::{EmitOptions, EmitShape};
use crate::fx::registry::{FxHandlers, ProjectileHandle, RegisterFx};

const FIRE: Vec3 = Vec3::new(2.8, 1.2, 0.3);
const MUZZLE: Vec3 = Vec3::new(3.0, 2.2, 1.0);
const LASER: Vec3 = Vec3::new(2.6, 0.3, 0.2);

/// Lueur persistante sur l'arme (tant que le buff tient).
struct Charged {
    key: String,
    aura: AuraId,
    target: EntityId,
    ended: Arc<AtomicBool>,
    until: f32,
    color: Vec3,
    acc: f32,
}

impl Effect for Charged {
    fn update(&mut self, fx: &mut Fx, dt: f32) -> bool {
        self.acc += dt;
        if self.ended.load(Ordering::Relaxed) || fx.time() > self.until {
            return false;
        }
        let Some(pos) = fx.entity_pos(self.target, 1.2) else {
            return false;
        };
        if self.acc > 0.05 {
            self.acc = 0.0;
            let s = fx.styles();
            fx.ps().emit(
                s.ember,
                pos,
                &EmitOptions {
                    count: 1.0,
                    shape: EmitShape::Sphere,
                    radius: 0.35,
                    up: between(0.3, 0.8),
                    life: [0.3, 0.5].into(),
                    size: [0.06, 0.1].into(),
                    tint: self.color,
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

fn charged(fx: &mut Fx, target: EntityId, buff_id: &str, duration: f32, color: Vec3) {
    let key = format!("{buff_id}:{target}");
    let ended = Arc::new(AtomicBool::new(false));
    let flag = ended.clone();
    let aura = fx.set_aura(&key, move |_| flag.store(true, Ordering::Relaxed));
    let until = fx.time() + duration.max(0.3);
    fx.add(Charged {
        key,
        aura,
        target,
        ended,
        until,
        color,
        acc: 0.0,
    });
}

fn muzzle_flash(fx: &mut Fx, source: EntityId, toward: Option<Vec3>, scale: f32) {
    let Some(mut v) = fx.entity_pos(source, 1.25) else {
        return;
    };
    if let Some(t) = toward {
        v += (t.with_y(1.25) - v).with_y(0.0).normalize_or_zero() * 0.7;
    }
    flash(fx, v, 1.4 * scale, MUZZLE, 0.12);
    let s = fx.styles();
    fx.ps().emit(
        s.smoke,
        v,
        &EmitOptions {
            count: (3.0 * scale).round(),
            shape: EmitShape::Sphere,
            radius: 0.1,
            up: between(0.3, 0.6),
            speed: between(0.2, 0.6),
            life: [0.5, 0.8].into(),
            size: [0.3, 0.5].into(),
            ..default()
        },
    );
}

fn tracer(fx: &mut Fx, e: &GameEvent, size: f32, ribbon: f32, life: f32) -> Option<ProjectileStop> {
    let GameEvent::ProjectileSpawn {
        from, to, speed, ..
    } = e
    else {
        return None;
    };
    Some(ProjectileFx::launch(
        fx,
        Fx::world_pos(*from, 0.0),
        Fx::world_pos(*to, 0.0),
        Fx::scale(*speed as f32),
        ProjectileLook {
            color: FIRE,
            core: Some(MUZZLE),
            size,
            height: 1.2,
            ribbon_width: ribbon,
            ribbon_life: life,
            ..default()
        },
    ))
}

fn p_buff(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let GameEvent::BuffApplied {
        target, duration, ..
    } = e
    else {
        return;
    };
    let target = *target;
    let dust = fx.styles().dust;
    let mut acc = 0.0;
    timed(
        fx,
        (*duration as f32).max(0.3),
        move |fx, _, dt| {
            acc += dt;
            if acc < 0.06 {
                return;
            }
            let Some(pos) = fx.entity_pos(target, 0.0) else {
                return;
            };
            acc = 0.0;
            fx.ps().emit(
                dust,
                pos,
                &EmitOptions {
                    count: 1.0,
                    shape: EmitShape::Disc,
                    radius: 0.25,
                    up: between(0.2, 0.5),
                    life: [0.4, 0.6].into(),
                    size: [0.4, 0.6].into(),
                    ..default()
                },
            );
        },
        |_| {},
    );
}

fn buff_charged(fx: &mut Fx, e: &GameEvent, id: &str, color: Vec3) {
    if let GameEvent::BuffApplied {
        target,
        buff_id,
        duration,
        ..
    } = e
        && buff_id == id
    {
        charged(fx, *target, buff_id, *duration as f32, color);
    }
}

fn end_charged(fx: &mut Fx, e: &GameEvent, id: &str) {
    if let GameEvent::BuffRemoved {
        target, buff_id, ..
    } = e
        && buff_id == id
    {
        fx.end_aura(&format!("{id}:{target}"));
    }
}

fn q_buff(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    buff_charged(fx, e, "sable_q", FIRE);
}

fn q_buff_end(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    end_charged(fx, e, "sable_q");
}

fn q_explosion(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let GameEvent::AreaEffect { pos, radius, .. } = e else {
        return;
    };
    let center = Fx::world_pos(*pos, 0.0);
    let r = Fx::scale(*radius as f32);
    flash(fx, center.with_y(0.8), r * 1.2, FIRE, 0.22);
    shockwave(fx, center, r * 1.1, 0.35, FIRE, 0.14);
    let s = fx.styles();
    fx.ps().emit(
        s.flame,
        center,
        &EmitOptions {
            count: 16.0,
            shape: EmitShape::Disc,
            radius: r * 0.6,
            up: between(1.0, 2.5),
            speed: between(0.5, 1.5),
            life: [0.35, 0.6].into(),
            size: [0.4, 0.7].into(),
            tint: FIRE,
            ..default()
        },
    );
    fx.ps().emit(
        s.smoke,
        center,
        &EmitOptions {
            count: 8.0,
            shape: EmitShape::Disc,
            radius: r * 0.5,
            up: between(0.5, 1.2),
            life: [0.8, 1.2].into(),
            size: [0.6, 1.0].into(),
            ..default()
        },
    );
}

fn w_cast(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    if let GameEvent::AbilityCast {
        source, target_pos, ..
    } = e
    {
        muzzle_flash(fx, *source, target_pos.map(|p| Fx::world_pos(p, 0.0)), 1.6);
    }
}

fn w_projectile(fx: &mut Fx, e: &GameEvent, _: &FxColors) -> Option<Box<dyn ProjectileHandle>> {
    tracer(fx, e, 0.22, 0.07, 0.12).map(|p| Box::new(p) as Box<dyn ProjectileHandle>)
}

fn w_hit(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    if let GameEvent::ProjectileHit { pos, .. } = e {
        let s = fx.styles();
        fx.ps().emit(
            s.spark,
            Fx::world_pos(*pos, 1.0),
            &EmitOptions {
                count: 6.0,
                shape: EmitShape::Point,
                speed: between(2.0, 4.0),
                life: [0.12, 0.2].into(),
                size: [0.04, 0.07].into(),
                stretch: 0.04,
                tint: FIRE,
                ..default()
            },
        );
    }
}

fn e_dash(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let GameEvent::Dash {
        source, duration, ..
    } = e
    else {
        return;
    };
    let source = *source;
    let dust = fx.styles().dust;
    timed(
        fx,
        (*duration as f32).max(0.05),
        move |fx, _, _| {
            if let Some(pos) = fx.entity_pos(source, 0.0) {
                fx.ps().emit(
                    dust,
                    pos,
                    &EmitOptions {
                        count: 2.0,
                        shape: EmitShape::Disc,
                        radius: 0.3,
                        up: between(0.3, 0.8),
                        speed: between(0.3, 1.0),
                        life: [0.5, 0.8].into(),
                        size: [0.6, 1.0].into(),
                        ..default()
                    },
                );
            }
        },
        |_| {},
    );
}

fn e_buff(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    buff_charged(fx, e, "sable_e", MUZZLE);
}

fn e_buff_end(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    end_charged(fx, e, "sable_e");
}

/// Visée : un laser rouge se stabilise vers la cible pendant 1 s.
fn r_cast(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let GameEvent::AbilityCast {
        source,
        target_pos: Some(target_pos),
        ..
    } = e
    else {
        return;
    };
    let source = *source;
    let to = Fx::world_pos(*target_pos, 0.0);
    let d = fx.acquire_decal();
    edit_decal(fx, &d, |m, _| {
        m.set_color(LASER, LASER);
        m.u.fill = 0.8;
        m.u.edge = 0.02;
        m.u.opacity = 0.0;
        None
    });
    let d2 = d.clone();
    timed(
        fx,
        1.0,
        move |fx, t, _| {
            let Some(from) = fx.entity_pos(source, 0.0) else {
                return;
            };
            let dir = (to - from).with_y(0.0).normalize_or_zero();
            edit_decal(fx, &d, |m, g| {
                m.u.opacity = 0.3 + 0.7 * t * (0.7 + 0.3 * (t * 40.0).sin());
                Some(g.rect(m, from, from + dir * 30.0, 0.05 + (1.0 - t) * 0.1))
            });
        },
        move |fx| fx.release_decal(d2),
    );
}

fn r_projectile(fx: &mut Fx, e: &GameEvent, _: &FxColors) -> Option<Box<dyn ProjectileHandle>> {
    if let GameEvent::ProjectileSpawn { source, to, .. } = e {
        muzzle_flash(fx, *source, Some(Fx::world_pos(*to, 0.0)), 1.8);
    }
    tracer(fx, e, 0.9, 0.35, 0.6).map(|p| Box::new(p) as Box<dyn ProjectileHandle>)
}

fn r_hit(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let GameEvent::ProjectileHit { pos, .. } = e else {
        return;
    };
    let at = Fx::world_pos(*pos, 1.0);
    flash(fx, at, 3.0, FIRE, 0.3);
    shockwave(fx, at.with_y(0.0), 2.0, 0.4, FIRE, 0.14);
    fx.shake(at, 20.0, 0.25);
    let s = fx.styles();
    fx.ps().emit(
        s.spark,
        at,
        &EmitOptions {
            count: 30.0,
            shape: EmitShape::Point,
            speed: between(4.0, 9.0),
            life: [0.25, 0.45].into(),
            size: [0.07, 0.12].into(),
            stretch: 0.06,
            gravity: 6.0,
            tint: FIRE,
            ..default()
        },
    );
    for i in 0..3 {
        fx.after(i as f32 * 0.05, move |fx| {
            fx.ps().emit(
                s.smoke,
                at,
                &EmitOptions {
                    count: 3.0,
                    shape: EmitShape::Sphere,
                    radius: 0.3,
                    up: between(0.4, 1.0),
                    life: [0.8, 1.2].into(),
                    size: [0.5, 0.9].into(),
                    ..default()
                },
            );
        });
    }
}

pub fn register(app: &mut App) {
    app.register_fx(
        "sable_p",
        FxHandlers {
            buff: Some(p_buff),
            ..default()
        },
    )
    .register_fx(
        "sable_q",
        FxHandlers {
            buff: Some(q_buff),
            buff_end: Some(q_buff_end),
            ..default()
        },
    )
    .register_fx(
        "sable_q_explosion",
        FxHandlers {
            area: Some(q_explosion),
            ..default()
        },
    )
    .register_fx(
        "sable_w",
        FxHandlers {
            cast: Some(w_cast),
            projectile: Some(w_projectile),
            hit: Some(w_hit),
            ..default()
        },
    )
    .register_fx(
        "sable_e",
        FxHandlers {
            dash: Some(e_dash),
            buff: Some(e_buff),
            buff_end: Some(e_buff_end),
            ..default()
        },
    )
    .register_fx(
        "sable_r",
        FxHandlers {
            cast: Some(r_cast),
            projectile: Some(r_projectile),
            hit: Some(r_hit),
            ..default()
        },
    );
}
