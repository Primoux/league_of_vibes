//! FX de Corvin, l'Archer des Corbeaux (événements : `lov-sim/src/abilities/champions/corvin.rs`).
//!
//! | sort | événements |
//! |------|------------|
//! | P Marque du corbeau | buff `corvin_marks` (cumuls, fxKey `corvin_p`) ; la 3e marque est consommée |
//! | Q Flèche perçante | projectile `corvin_q` |
//! | W Piège à griffes | zone `corvin_w` (60 s) ; déclenché : root + revealed (fxKey `corvin_w`) sur la victime |
//! | E Envol | dash `corvin_e` (en arrière) + buff `corvin_e` (vitesse) |
//! | R Nuée | zone `corvin_r` (3 s) |
//!
//! Portage de `packages/client/src/fx/champions/corvin.ts`.

use std::collections::HashMap;
use std::f32::consts::TAU;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use lov_shared::types::{EntityId, GameEvent};

use crate::fx::core::palette::FxColors;
use crate::fx::core::runtime::{Effect, Fx, FxDecal};
use crate::fx::effects::mesh_bursts::{BurstKind, BurstMode, BurstSpawn, burst};
use crate::fx::effects::primitives::{
    ProjectileFx, ProjectileLook, ZoneOptions, between, edit_decal, flash, rand, rand_range,
    shockwave, timed, zone,
};
use crate::fx::particles::{EmitOptions, EmitShape};
use crate::fx::registry::{FxHandlers, ProjectileHandle, RegisterFx};
use crate::fx::shaders::DecalShape;

const GREEN: Vec3 = Vec3::new(0.8, 2.6, 0.7);
const INK: Vec3 = Vec3::new(0.12, 0.12, 0.18);

fn feathers(fx: &mut Fx, at: Vec3, n: f32, power: f32) {
    let s = fx.styles();
    fx.ps().emit(
        s.feather,
        at,
        &EmitOptions {
            count: n,
            shape: EmitShape::Sphere,
            radius: 0.4,
            speed: between(0.8 * power, 2.5 * power),
            up: between(0.5, 1.5),
            gravity: 1.2,
            drag: 1.5,
            life: [0.8, 1.4].into(),
            size: [0.22, 0.34].into(),
            spin: between(-3.0, 3.0),
            tint: INK,
            ..default()
        },
    );
}

// ---------------------------------------------------------------------------------- P : marques

#[derive(Debug)]
struct Mark {
    stacks: u32,
    until: f32,
}

/// Marques par cible.
#[derive(Resource, Default)]
struct Marks(HashMap<EntityId, Arc<Mutex<Mark>>>);

struct MarkFx {
    target: EntityId,
    state: Arc<Mutex<Mark>>,
    t: f32,
    acc: f32,
}

impl Effect for MarkFx {
    fn update(&mut self, fx: &mut Fx, dt: f32) -> bool {
        self.t += dt;
        self.acc += dt;
        let now = fx.time();
        let stacks = self
            .state
            .lock()
            .ok()
            .filter(|m| now <= m.until)
            .map_or(0, |m| m.stacks);
        let Some(pos) = fx.entity_pos(self.target, 0.0).filter(|_| stacks > 0) else {
            return false;
        };
        if self.acc < 0.05 {
            return true;
        }
        self.acc = 0.0;
        let s = fx.styles();
        for i in 0..stacks {
            let a = self.t * 2.0 + i as f32 / stacks.max(1) as f32 * TAU;
            let p = Vec3::new(pos.x + a.cos() * 0.5, 2.1, pos.z + a.sin() * 0.5);
            fx.ps().emit(
                s.rune,
                p,
                &EmitOptions {
                    count: 1.0,
                    shape: EmitShape::Point,
                    life: 0.12.into(),
                    size: 0.24.into(),
                    tint: GREEN,
                    ..default()
                },
            );
        }
        true
    }

    fn end(&mut self, fx: &mut Fx) {
        let mut marks = fx.world().get_resource_or_init::<Marks>();
        if marks
            .0
            .get(&self.target)
            .is_some_and(|s| Arc::ptr_eq(s, &self.state))
        {
            marks.0.remove(&self.target);
        }
    }
}

fn p_buff(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let GameEvent::BuffApplied {
        target,
        buff_id,
        duration,
        stacks,
        ..
    } = e
    else {
        return;
    };
    if buff_id != "corvin_marks" {
        return;
    }
    let until = fx.time() + (*duration as f32).max(0.3);
    {
        let marks = fx.world().get_resource_or_init::<Marks>();
        if let Some(prev) = marks.0.get(target)
            && let Ok(mut m) = prev.lock()
        {
            m.stacks = stacks.unwrap_or(m.stacks + 1);
            m.until = until;
            return;
        }
    }
    let state = Arc::new(Mutex::new(Mark {
        stacks: stacks.unwrap_or(1),
        until,
    }));
    fx.world()
        .get_resource_or_init::<Marks>()
        .0
        .insert(*target, state.clone());
    fx.add(MarkFx {
        target: *target,
        state,
        t: 0.0,
        acc: 0.0,
    });
}

fn p_buff_end(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let GameEvent::BuffRemoved {
        target, buff_id, ..
    } = e
    else {
        return;
    };
    if buff_id != "corvin_marks" {
        return;
    }
    let state = fx
        .world()
        .get_resource_or_init::<Marks>()
        .0
        .get(target)
        .cloned();
    let Some(state) = state else { return };
    let stacks = state.lock().map_or(0, |m| m.stacks);
    // marques consommées (3e coup) : explosion de plumes vertes
    if stacks >= 2
        && let Some(p) = fx.entity_pos(*target, 1.0)
    {
        flash(fx, p, 2.2, GREEN, 0.25);
        feathers(fx, p, 10.0, 1.4);
    }
    if let Ok(mut m) = state.lock() {
        m.stacks = 0;
    }
}

// ---------------------------------------------------------------------------------- Q

fn q_projectile(fx: &mut Fx, e: &GameEvent, _: &FxColors) -> Option<Box<dyn ProjectileHandle>> {
    let GameEvent::ProjectileSpawn {
        from, to, speed, ..
    } = e
    else {
        return None;
    };
    let feather = fx.styles().feather;
    let stop = ProjectileFx::launch(
        fx,
        Fx::world_pos(*from, 0.0),
        Fx::world_pos(*to, 0.0),
        Fx::scale(*speed as f32),
        ProjectileLook {
            color: GREEN,
            core: Some(Vec3::new(2.4, 3.0, 2.4)),
            size: 0.4,
            height: 1.2,
            ribbon_width: 0.12,
            ribbon_life: 0.25,
            trail_style: Some(feather),
            trail_density: 1.5,
            trail_size: [0.15, 0.22],
            trail_life: [0.5, 0.8],
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
    let s = fx.styles();
    fx.ps().emit(
        s.spark,
        at,
        &EmitOptions {
            count: 10.0,
            shape: EmitShape::Point,
            speed: between(2.0, 5.0),
            life: [0.15, 0.3].into(),
            size: [0.05, 0.08].into(),
            stretch: 0.05,
            tint: GREEN,
            ..default()
        },
    );
    feathers(fx, at, 3.0, 1.0);
}

// ---------------------------------------------------------------------------------- W : pièges

/// Pièges posés : centre et fin demandée.
#[derive(Resource, Default)]
struct Traps(Vec<(Vec3, Arc<AtomicBool>)>);

struct TrapFx {
    decal: FxDecal,
    done: Arc<AtomicBool>,
    duration: f32,
    t: f32,
}

impl Effect for TrapFx {
    fn update(&mut self, fx: &mut Fx, dt: f32) -> bool {
        self.t += dt;
        let o = (self.t * 4.0).min(1.0) * (0.6 + 0.4 * (self.t * 2.0).sin());
        edit_decal(fx, &self.decal, |m, _| {
            m.u.opacity = o;
            None
        });
        !self.done.load(Ordering::Relaxed) && self.t < self.duration
    }

    fn end(&mut self, fx: &mut Fx) {
        let done = self.done.clone();
        fx.world()
            .get_resource_or_init::<Traps>()
            .0
            .retain(|(_, d)| !Arc::ptr_eq(d, &done));
        fx.release_decal(self.decal.clone());
    }
}

/// Piège posé : griffes discrètes au sol (visible des alliés seulement : le serveur filtre la zone).
fn w_area(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
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
    let decal = fx.acquire_decal();
    let c = GREEN * 0.35;
    edit_decal(fx, &decal, |m, g| {
        m.set_color(c, c);
        m.u.fill = 0.1;
        m.u.edge = 0.04;
        m.u.pattern = 0.6;
        m.u.opacity = 0.0;
        Some(g.circle(m, center, r, DecalShape::Circle))
    });
    let done = Arc::new(AtomicBool::new(false));
    fx.world()
        .get_resource_or_init::<Traps>()
        .0
        .push((center, done.clone()));
    feathers(fx, center.with_y(0.3), 4.0, 0.6);
    fx.add(TrapFx {
        decal,
        done,
        duration: (*duration as f32).max(1.0),
        t: 0.0,
    });
}

/// Déclenché : la victime est immobilisée et révélée → le piège le plus proche se referme.
fn w_buff(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let GameEvent::BuffApplied {
        target, buff_id, ..
    } = e
    else {
        return;
    };
    if buff_id != "root" {
        return;
    }
    let Some(v) = fx.entity_pos(*target, 0.0) else {
        return;
    };
    {
        let traps = fx.world().get_resource_or_init::<Traps>();
        let best = traps
            .0
            .iter()
            .map(|(c, d)| (c.distance_squared(v), d))
            .filter(|(d2, _)| *d2 < 4.0)
            .min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, done)) = best {
            done.store(true, Ordering::Relaxed);
        }
    }
    shockwave(fx, v, 1.2, 0.3, GREEN, 0.1);
    for i in 0..6 {
        let a = i as f32 / 6.0 * TAU;
        burst(
            fx,
            BurstKind::RockSpikes,
            BurstSpawn {
                pos: Vec3::new(v.x + a.cos() * 0.45, 0.0, v.z + a.sin() * 0.45),
                mode: BurstMode::Spike,
                size: 0.35,
                life: 1.2,
                heading: Some(a.cos().atan2(a.sin())),
                tilt: Some(0.6),
                stretch: 1.4,
                color: Some(Vec3::new(0.25, 0.25, 0.3)),
                ..default()
            },
        );
    }
}

// ---------------------------------------------------------------------------------- E

fn e_dash(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let GameEvent::Dash {
        source,
        from,
        duration,
        ..
    } = e
    else {
        return;
    };
    let from = Fx::world_pos(*from, 0.8);
    flash(fx, from, 1.6, GREEN, 0.2);
    feathers(fx, from, 16.0, 1.5);
    let source = *source;
    timed(
        fx,
        (*duration as f32).max(0.05),
        move |fx, _, _| {
            if let Some(p) = fx.entity_pos(source, 0.8) {
                feathers(fx, p, 1.0, 0.5);
            }
        },
        |_| {},
    );
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
    if buff_id != "corvin_e" {
        return;
    }
    let target = *target;
    let mut acc = 0.0;
    timed(
        fx,
        (*duration as f32).max(0.3),
        move |fx, _, dt| {
            acc += dt;
            if acc < 0.12 {
                return;
            }
            let Some(p) = fx.entity_pos(target, 0.3) else {
                return;
            };
            acc = 0.0;
            feathers(fx, p, 1.0, 0.3);
        },
        |_| {},
    );
}

// ---------------------------------------------------------------------------------- R

/// Nuée : corbeaux (plumes) qui tournoient, ombres, éclats verts.
fn r_area(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
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
    let duration = (*duration as f32).max(0.5);
    zone(
        fx,
        center,
        r,
        duration,
        GREEN * 0.5,
        ZoneOptions {
            fill: Some(0.08),
            pattern: Some(0.4),
            ..default()
        },
    );
    let s = fx.styles();
    let mut acc = 0.0;
    timed(
        fx,
        duration,
        move |fx, t, dt| {
            let fade = ((1.0 - t) * duration * 2.0).min(1.0);
            acc += dt * 60.0 * fade;
            while acc >= 1.0 {
                acc -= 1.0;
                let a = rand() * TAU;
                let d = r * rand_range(0.3, 1.0);
                let v = Vec3::new(
                    center.x + a.cos() * d,
                    rand_range(0.8, 2.6),
                    center.z + a.sin() * d,
                );
                let vel = Vec3::new(-a.sin() * 5.0, rand_range(-0.5, 0.5), a.cos() * 5.0);
                fx.ps().emit(
                    s.feather,
                    v,
                    &EmitOptions {
                        count: 1.0,
                        shape: EmitShape::Point,
                        inherit: Some(vel),
                        life: [0.4, 0.7].into(),
                        size: [0.3, 0.45].into(),
                        spin: between(-6.0, 6.0),
                        tint: INK,
                        ..default()
                    },
                );
                if rand() < 0.15 {
                    fx.ps().emit(
                        s.ember,
                        v,
                        &EmitOptions {
                            count: 1.0,
                            shape: EmitShape::Point,
                            inherit: Some(vel),
                            life: 0.3.into(),
                            size: 0.08.into(),
                            tint: GREEN,
                            ..default()
                        },
                    );
                }
            }
            if rand() < dt * 6.0 {
                fx.ps().emit(
                    s.dark_wisp,
                    center,
                    &EmitOptions {
                        count: 1.0,
                        shape: EmitShape::Disc,
                        radius: r * 0.8,
                        up: between(0.2, 0.6),
                        life: [0.8, 1.2].into(),
                        size: [0.8, 1.3].into(),
                        ..default()
                    },
                );
            }
        },
        |_| {},
    );
}

pub fn register(app: &mut App) {
    app.init_resource::<Marks>()
        .init_resource::<Traps>()
        .register_fx(
            "corvin_p",
            FxHandlers {
                buff: Some(p_buff),
                buff_end: Some(p_buff_end),
                ..default()
            },
        )
        .register_fx(
            "corvin_q",
            FxHandlers {
                projectile: Some(q_projectile),
                hit: Some(q_hit),
                ..default()
            },
        )
        .register_fx(
            "corvin_w",
            FxHandlers {
                area: Some(w_area),
                buff: Some(w_buff),
                ..default()
            },
        )
        .register_fx(
            "corvin_e",
            FxHandlers {
                dash: Some(e_dash),
                buff: Some(e_buff),
                ..default()
            },
        )
        .register_fx(
            "corvin_r",
            FxHandlers {
                area: Some(r_area),
                ..default()
            },
        );
}

#[cfg(test)]
mod tests {
    use lov_shared::types::Vec2;

    use super::*;
    use crate::fx::core::runtime::FxRuntime;
    use crate::fx::effects::primitives::tests::fx_world;

    #[test]
    fn le_piege_pose_puis_retire() {
        let mut w = fx_world();
        let c = FxRuntime::default().colors("corvin_w");
        let e = GameEvent::AreaEffect {
            source: 1,
            fx_key: "corvin_w".into(),
            pos: Vec2 { x: 0.0, y: 0.0 },
            radius: 100.0,
            duration: 1.0,
        };
        w_area(&mut Fx::new(&mut w), &e, &c);
        assert_eq!(w.resource::<Traps>().0.len(), 1);
        for _ in 0..12 {
            FxRuntime::update(&mut w, 0.1);
        }
        assert!(w.resource::<Traps>().0.is_empty(), "piège oublié à la fin");
    }
}
