//! FX de Vrenna, la Lance des Steppes (événements : `lov-sim/src/abilities/champions/vrenna.rs`).
//!
//! | sort | événements |
//! |------|------------|
//! | P Saignée | buff `vrenna_bleed` (cumuls) sur la cible ; `vrenna_p_speed` sur Vrenna à 5 cumuls |
//! | Q Coup de lance | projectile `vrenna_q` ; la pointe attire : dash `vrenna_q_pull` sur la cible |
//! | W Cri de guerre | buff `vrenna_w` + soin |
//! | E Bond des steppes | dash `vrenna_e`, dégâts de zone à l'atterrissage |
//! | R Lance céleste | dash `vrenna_r` sur un champion, projection à l'arrivée |
//!
//! Portage de `packages/client/src/fx/champions/vrenna.ts`.

use std::collections::HashMap;
use std::f32::consts::PI;
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use lov_shared::types::{EntityId, GameEvent};

use crate::fx::core::palette::FxColors;
use crate::fx::core::runtime::{Effect, Fx};
use crate::fx::effects::primitives::{
    ProjectileFx, ProjectileLook, between, dash_trail, flash, rand, rand_range, shockwave, timed,
};
use crate::fx::particles::{EmitOptions, EmitShape};
use crate::fx::registry::{FxHandlers, ProjectileHandle, RegisterFx};

const GOLD: Vec3 = Vec3::new(2.8, 2.1, 0.5);
const WAR: Vec3 = Vec3::new(2.4, 0.5, 0.3);

#[derive(Debug)]
struct Bleed {
    stacks: u32,
    until: f32,
}

/// Saignement par cible : nombre de cumuls (rafraîchi à chaque réapplication).
#[derive(Resource, Default)]
struct Bleeds(HashMap<EntityId, Arc<Mutex<Bleed>>>);

struct BleedFx {
    target: EntityId,
    state: Arc<Mutex<Bleed>>,
    acc: f32,
}

impl Effect for BleedFx {
    fn update(&mut self, fx: &mut Fx, dt: f32) -> bool {
        let now = fx.time();
        let stacks = self
            .state
            .lock()
            .ok()
            .filter(|b| now <= b.until)
            .map_or(0, |b| b.stacks);
        self.acc += dt * (2.0 + stacks as f32 * 2.5);
        let Some(pos) = fx.entity_pos(self.target, 0.0).filter(|_| stacks > 0) else {
            return false;
        };
        let blood = fx.styles().blood;
        while self.acc >= 1.0 {
            self.acc -= 1.0;
            fx.ps().emit(
                blood,
                pos.with_y(rand_range(0.6, 1.4)),
                &EmitOptions {
                    count: 1.0,
                    shape: EmitShape::Sphere,
                    radius: 0.25,
                    speed: between(0.3, 0.8),
                    gravity: 8.0,
                    life: [0.35, 0.55].into(),
                    size: [0.06, 0.1].into(),
                    ..default()
                },
            );
        }
        true
    }

    fn end(&mut self, fx: &mut Fx) {
        let mut bleeds = fx.world().get_resource_or_init::<Bleeds>();
        if bleeds
            .0
            .get(&self.target)
            .is_some_and(|s| Arc::ptr_eq(s, &self.state))
        {
            bleeds.0.remove(&self.target);
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
    let target = *target;
    if buff_id == "vrenna_p_speed" {
        let ember = fx.styles().ember;
        timed(
            fx,
            (*duration as f32).max(0.3),
            move |fx, _, _| {
                if rand() < 0.4
                    && let Some(pos) = fx.entity_pos(target, 0.0)
                {
                    fx.ps().emit(
                        ember,
                        pos,
                        &EmitOptions {
                            count: 1.0,
                            shape: EmitShape::Ring,
                            radius: 0.4,
                            up: between(1.0, 2.0),
                            life: [0.3, 0.5].into(),
                            size: [0.06, 0.1].into(),
                            tint: WAR,
                            ..default()
                        },
                    );
                }
            },
            |_| {},
        );
        return;
    }
    if buff_id != "vrenna_bleed" {
        return;
    }
    let until = fx.time() + (*duration as f32).max(0.3);
    {
        let bleeds = fx.world().get_resource_or_init::<Bleeds>();
        if let Some(prev) = bleeds.0.get(&target)
            && let Ok(mut b) = prev.lock()
        {
            b.stacks = stacks.unwrap_or(b.stacks + 1);
            b.until = until;
            return;
        }
    }
    let state = Arc::new(Mutex::new(Bleed {
        stacks: stacks.unwrap_or(1),
        until,
    }));
    fx.world()
        .get_resource_or_init::<Bleeds>()
        .0
        .insert(target, state.clone());
    fx.add(BleedFx {
        target,
        state,
        acc: 0.0,
    });
}

fn p_buff_end(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    if let GameEvent::BuffRemoved {
        target, buff_id, ..
    } = e
        && buff_id == "vrenna_bleed"
    {
        let bleeds = fx.world().get_resource_or_init::<Bleeds>();
        if let Some(s) = bleeds.0.get(target)
            && let Ok(mut b) = s.lock()
        {
            b.stacks = 0;
        }
    }
}

/// Estoc : la lance file, pointe dorée, sillage rouge.
fn q_projectile(fx: &mut Fx, e: &GameEvent, _: &FxColors) -> Option<Box<dyn ProjectileHandle>> {
    let GameEvent::ProjectileSpawn {
        from, to, speed, ..
    } = e
    else {
        return None;
    };
    let spark = fx.styles().spark;
    let stop = ProjectileFx::launch(
        fx,
        Fx::world_pos(*from, 0.0),
        Fx::world_pos(*to, 0.0),
        Fx::scale(*speed as f32),
        ProjectileLook {
            color: WAR,
            core: Some(GOLD),
            size: 0.35,
            height: 1.1,
            ribbon_width: 0.2,
            ribbon_life: 0.18,
            trail_style: Some(spark),
            trail_density: 4.0,
            trail_size: [0.05, 0.08],
            trail_life: [0.15, 0.25],
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
    flash(fx, at, 1.4, GOLD, 0.15);
    let s = fx.styles();
    fx.ps().emit(
        s.blood,
        at,
        &EmitOptions {
            count: 8.0,
            shape: EmitShape::Point,
            speed: between(1.5, 3.0),
            gravity: 9.0,
            life: [0.3, 0.5].into(),
            size: [0.06, 0.09].into(),
            ..default()
        },
    );
}

/// La cible est arrachée vers Vrenna.
fn q_pull(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
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
            WAR * 0.5,
            true,
        );
    }
}

fn w_cast(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let GameEvent::AbilityCast { source, .. } = e else {
        return;
    };
    let Some(pos) = fx.entity_pos(*source, 0.0) else {
        return;
    };
    for i in 0..2 {
        fx.after(i as f32 * 0.12, move |fx| {
            shockwave(fx, pos, 2.2 - i as f32 * 0.5, 0.45, WAR, 0.12);
        });
    }
    flash(fx, pos.with_y(1.6), 2.4, WAR, 0.3);
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
    if buff_id != "vrenna_w" {
        return;
    }
    let target = *target;
    let flame = fx.styles().flame;
    let mut acc = 0.0;
    timed(
        fx,
        (*duration as f32).max(0.3),
        move |fx, _, dt| {
            acc += dt;
            if acc < 0.05 {
                return;
            }
            let Some(pos) = fx.entity_pos(target, 0.0) else {
                return;
            };
            acc = 0.0;
            fx.ps().emit(
                flame,
                pos,
                &EmitOptions {
                    count: 1.0,
                    shape: EmitShape::Ring,
                    radius: 0.45,
                    up: between(0.8, 1.5),
                    life: [0.3, 0.5].into(),
                    size: [0.2, 0.35].into(),
                    tint: WAR,
                    intensity: 0.6,
                    ..default()
                },
            );
        },
        |_| {},
    );
}

fn e_dash(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
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
    let source = *source;
    let from = Fx::world_pos(*from, 0.0);
    let to = Fx::world_pos(*to, 0.0);
    let duration = (*duration as f32).max(0.05);
    let s = fx.styles();
    fx.ps().emit(
        s.dust,
        from,
        &EmitOptions {
            count: 8.0,
            shape: EmitShape::Disc,
            radius: 0.4,
            up: between(0.5, 1.2),
            speed: between(0.5, 1.5),
            life: [0.6, 0.9].into(),
            size: [0.6, 1.1].into(),
            ..default()
        },
    );
    dash_trail(fx, source, from, to, duration, WAR * 0.6, false);
    // atterrissage : la lance frappe le sol
    fx.after(duration, move |fx| {
        let pos = fx.entity_pos(source, 0.0).unwrap_or(to);
        shockwave(fx, pos, 2.5, 0.4, WAR, 0.16);
        fx.ps().emit(
            s.dust,
            pos,
            &EmitOptions {
                count: 14.0,
                shape: EmitShape::Ring,
                radius: 0.6,
                speed: between(1.5, 3.0),
                up: between(0.3, 1.0),
                life: [0.6, 1.0].into(),
                size: [0.8, 1.3].into(),
                drag: 2.0,
                ..default()
            },
        );
        flash(fx, pos.with_y(0.6), 2.0, GOLD, 0.2);
    });
}

/// Bond vers le ciel puis la lance s'abat : traînée dorée, impact projetant.
fn r_dash(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
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
    let source = *source;
    let from = Fx::world_pos(*from, 0.0);
    let to = Fx::world_pos(*to, 0.0);
    let duration = (*duration as f32).max(0.05);
    dash_trail(fx, source, from, to, duration, GOLD, false);
    let glow = fx.styles().glow;
    timed(
        fx,
        duration,
        move |fx, t, _| {
            if let Some(pos) = fx.entity_pos(source, 0.0) {
                fx.ps().emit(
                    glow,
                    pos.with_y(1.0 + (t * PI).sin() * 1.5),
                    &EmitOptions {
                        count: 2.0,
                        shape: EmitShape::Point,
                        life: 0.25.into(),
                        size: 0.6.into(),
                        tint: GOLD,
                        ..default()
                    },
                );
            }
        },
        |_| {},
    );
    fx.after(duration, move |fx| impact_r(fx, source, to));
}

fn impact_r(fx: &mut Fx, source: EntityId, fallback: Vec3) {
    let pos = fx.entity_pos(source, 0.0).unwrap_or(fallback);
    flash(fx, pos.with_y(1.0), 3.5, GOLD, 0.3);
    shockwave(fx, pos, 2.6, 0.5, GOLD, 0.2);
    fx.shake(pos, 20.0, 0.25);
    let s = fx.styles();
    fx.ps().emit(
        s.spark,
        pos,
        &EmitOptions {
            count: 30.0,
            shape: EmitShape::Disc,
            radius: 0.5,
            up: between(3.0, 8.0),
            speed: between(1.0, 3.0),
            life: [0.3, 0.6].into(),
            size: [0.07, 0.12].into(),
            stretch: 0.06,
            gravity: 6.0,
            tint: GOLD,
            ..default()
        },
    );
    fx.ps().emit(
        s.dust,
        pos,
        &EmitOptions {
            count: 12.0,
            shape: EmitShape::Ring,
            radius: 0.6,
            speed: between(1.5, 3.0),
            up: between(0.3, 1.0),
            life: [0.7, 1.1].into(),
            size: [0.9, 1.4].into(),
            drag: 2.0,
            ..default()
        },
    );
}

pub fn register(app: &mut App) {
    app.init_resource::<Bleeds>()
        .register_fx(
            "vrenna_p",
            FxHandlers {
                buff: Some(p_buff),
                buff_end: Some(p_buff_end),
                ..default()
            },
        )
        .register_fx(
            "vrenna_q",
            FxHandlers {
                projectile: Some(q_projectile),
                hit: Some(q_hit),
                ..default()
            },
        )
        .register_fx(
            "vrenna_q_pull",
            FxHandlers {
                dash: Some(q_pull),
                ..default()
            },
        )
        .register_fx(
            "vrenna_w",
            FxHandlers {
                cast: Some(w_cast),
                buff: Some(w_buff),
                ..default()
            },
        )
        .register_fx(
            "vrenna_e",
            FxHandlers {
                dash: Some(e_dash),
                ..default()
            },
        )
        .register_fx(
            "vrenna_r",
            FxHandlers {
                dash: Some(r_dash),
                ..default()
            },
        );
}
