//! FX des 9 sorts d'invocateur, alignés sur les événements du serveur (`systems/summoners`) :
//!
//! | sort      | événements                                                       |
//! |-----------|------------------------------------------------------------------|
//! | flash     | dash (blink) + summonerCast                                      |
//! | ignite    | summonerCast + buff `ignite` (fxKey `sum_ignite`)                |
//! | heal      | summonerCast + buff `sum_heal_speed`                             |
//! | barrier   | buff `sum_barrier` (bouclier)                                    |
//! | exhaust   | summonerCast + buff `exhaust` (fxKey `sum_exhaust`)              |
//! | teleport  | zone `sum_teleport_channel` → `_arrive` / `_cancel`, dash        |
//! | smite     | summonerCast (targetId)                                          |
//! | ghost     | buff `sum_ghost`                                                 |
//! | cleanse   | summonerCast + buff                                              |
//!
//! Portage de `packages/client/src/fx/summoners.ts`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use bevy::prelude::*;
use lov_shared::types::{EntityId, GameEvent};

use crate::fx::core::palette::FxColors;
use crate::fx::core::runtime::{AuraId, Effect, Fx, FxDecal};
use crate::fx::effects::auras::{ShellOptions, shield_shell};
use crate::fx::effects::primitives::{
    LightningOptions, between, edit_decal, flash, lightning, rand, rand_range, shockwave, timed,
};
use crate::fx::particles::{EmitOptions, EmitShape};
use crate::fx::registry::{FxHandlers, RegisterFx};
use crate::fx::shaders::DecalShape;
use crate::fx::world::events::burn;

const FLASH: Vec3 = Vec3::new(2.8, 2.3, 0.9);
const IGNITE: Vec3 = Vec3::new(2.6, 0.9, 0.25);
const HEAL: Vec3 = Vec3::new(0.5, 2.4, 0.9);
const BARRIER: Vec3 = Vec3::new(1.5, 1.2, 0.5);
const EXHAUST: Vec3 = Vec3::new(1.3, 0.6, 2.2);
const TP: Vec3 = Vec3::new(1.8, 0.7, 2.6);
const SMITE: Vec3 = Vec3::new(2.8, 2.2, 0.8);
const GHOST: Vec3 = Vec3::new(0.6, 1.8, 2.2);
const CLEANSE: Vec3 = Vec3::new(1.4, 2.4, 2.8);

/// Source et cible d'un sort d'invocateur ciblé, à hauteur `h`.
fn source_and_target(fx: &mut Fx, e: &GameEvent, h: f32) -> Option<(Vec3, Vec3)> {
    let GameEvent::SummonerCast {
        source,
        target_id: Some(t),
        ..
    } = e
    else {
        return None;
    };
    Some((fx.entity_pos(*source, h)?, fx.entity_pos(*t, h)?))
}

fn buff_target(e: &GameEvent) -> Option<(EntityId, f32)> {
    match e {
        GameEvent::BuffApplied {
            target, duration, ..
        } => Some((*target, *duration as f32)),
        _ => None,
    }
}

// ---------------------------------------------------------------------------------- flash

fn flash_dash(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let GameEvent::Dash { from, to, .. } = e else {
        return;
    };
    let from = Fx::world_pos(*from, 1.0);
    let to = Fx::world_pos(*to, 1.0);
    flash(fx, from, 2.4, FLASH, 0.3);
    flash(fx, to, 2.8, FLASH, 0.35);
    lightning(
        fx,
        from,
        to,
        FLASH,
        LightningOptions {
            width: 0.3,
            life: 0.3,
            jag: 0.15,
            segments: 8,
        },
    );
    let s = fx.styles();
    fx.ps().emit(
        s.spark,
        from,
        &EmitOptions {
            count: 16.0,
            shape: EmitShape::Line,
            to: Some(to),
            radius: 0.3,
            speed: between(0.5, 2.0),
            life: [0.2, 0.4].into(),
            size: [0.06, 0.1].into(),
            stretch: 0.05,
            tint: FLASH,
            ..default()
        },
    );
    fx.ps().emit(
        s.sparkles,
        to,
        &EmitOptions {
            count: 10.0,
            shape: EmitShape::Sphere,
            radius: 0.5,
            speed: between(0.5, 1.5),
            life: [0.3, 0.6].into(),
            size: [0.3, 0.45].into(),
            tint: FLASH,
            ..default()
        },
    );
    shockwave(fx, to.with_y(0.0), 1.4, 0.3, FLASH, 0.1);
}

/// Le blink porte déjà tout le FX.
fn nothing(_: &mut Fx, _: &GameEvent, _: &FxColors) {}

// ---------------------------------------------------------------------------------- embrasement

fn ignite_cast(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let Some((p, q)) = source_and_target(fx, e, 1.2) else {
        return;
    };
    lightning(
        fx,
        p,
        q,
        IGNITE,
        LightningOptions {
            width: 0.14,
            life: 0.25,
            jag: 0.12,
            ..default()
        },
    );
    flash(fx, q, 1.8, IGNITE, 0.3);
}

fn ignite_buff(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    if let Some((target, duration)) = buff_target(e) {
        burn(fx, target, duration, IGNITE);
    }
}

// ---------------------------------------------------------------------------------- soin

fn heal_cast(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let GameEvent::SummonerCast { source, .. } = e else {
        return;
    };
    if let Some(p) = fx.entity_pos(*source, 0.0) {
        shockwave(fx, p, 2.2, 0.5, HEAL, 0.14);
    }
}

fn heal_buff(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let Some((target, duration)) = buff_target(e) else {
        return;
    };
    let Some(pos) = fx.entity_pos(target, 0.0) else {
        return;
    };
    flash(fx, pos.with_y(1.0), 2.2, HEAL, 0.3);
    let s = fx.styles();
    fx.ps().emit(
        s.cross,
        pos,
        &EmitOptions {
            count: 10.0,
            shape: EmitShape::Disc,
            radius: 0.6,
            up: between(1.5, 3.0),
            life: [0.6, 1.0].into(),
            size: [0.22, 0.32].into(),
            tint: HEAL,
            ..default()
        },
    );
    timed(
        fx,
        duration.max(0.3),
        move |fx, _, _| {
            if rand() < 0.5
                && let Some(pos) = fx.entity_pos(target, 0.0)
            {
                fx.ps().emit(
                    s.glow,
                    pos,
                    &EmitOptions {
                        count: 1.0,
                        shape: EmitShape::Ring,
                        radius: 0.4,
                        up: between(0.6, 1.2),
                        life: [0.3, 0.5].into(),
                        size: [0.2, 0.3].into(),
                        tint: HEAL,
                        ..default()
                    },
                );
            }
        },
        |_| {},
    );
}

// ---------------------------------------------------------------------------------- barrière

fn barrier_buff(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let GameEvent::BuffApplied {
        target,
        buff_id,
        duration,
        ..
    } = e
    else {
        return;
    };
    if buff_id != "sum_barrier" {
        return;
    }
    shield_shell(
        fx,
        *target,
        (*duration as f32).max(0.3),
        &format!("sum_barrier:{target}"),
        ShellOptions {
            color: BARRIER,
            radius: 1.15,
            bands: 0.35,
            power: 2.0,
            ..default()
        },
    );
    if let Some(p) = fx.entity_pos(*target, 1.0) {
        flash(fx, p, 2.4, BARRIER, 0.3);
    }
}

fn barrier_end(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    if let GameEvent::BuffRemoved {
        target, buff_id, ..
    } = e
        && buff_id == "sum_barrier"
    {
        fx.end_aura(&format!("sum_barrier:{target}"));
    }
}

// ---------------------------------------------------------------------------------- fatigue

fn exhaust_cast(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let Some((p, q)) = source_and_target(fx, e, 1.2) else {
        return;
    };
    lightning(
        fx,
        p,
        q,
        EXHAUST,
        LightningOptions {
            width: 0.12,
            life: 0.3,
            jag: 0.2,
            ..default()
        },
    );
}

fn exhaust_buff(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let Some((target, duration)) = buff_target(e) else {
        return;
    };
    let mut acc = 0.0;
    let s = fx.styles();
    timed(
        fx,
        duration.max(0.3),
        move |fx, _, dt| {
            acc += dt;
            if acc < 0.08 {
                return;
            }
            let Some(pos) = fx.entity_pos(target, 0.0) else {
                return;
            };
            acc = 0.0;
            fx.ps().emit(
                s.dark_wisp,
                pos.with_y(1.2),
                &EmitOptions {
                    count: 1.0,
                    shape: EmitShape::Sphere,
                    radius: 0.4,
                    up: between(-0.4, 0.2),
                    life: [0.5, 0.8].into(),
                    size: [0.4, 0.6].into(),
                    ..default()
                },
            );
            fx.ps().emit(
                s.ember,
                pos.with_y(1.6),
                &EmitOptions {
                    count: 1.0,
                    shape: EmitShape::Sphere,
                    radius: 0.4,
                    gravity: 2.0,
                    life: [0.4, 0.6].into(),
                    size: [0.08, 0.12].into(),
                    tint: EXHAUST,
                    ..default()
                },
            );
        },
        |_| {},
    );
}

// ---------------------------------------------------------------------------------- téléportation

struct TpColumn {
    key: String,
    aura: AuraId,
    center: Vec3,
    duration: f32,
    decal: FxDecal,
    ended: Arc<AtomicBool>,
    opacity: f32,
    t: f32,
    acc: f32,
}

impl Effect for TpColumn {
    fn update(&mut self, fx: &mut Fx, dt: f32) -> bool {
        self.t += dt;
        self.acc += dt;
        let k = (self.t / self.duration).min(1.0);
        let ended = self.ended.load(Ordering::Relaxed);
        if ended {
            self.opacity -= dt * 5.0;
        }
        let opacity = self.opacity;
        edit_decal(fx, &self.decal, |m, _| {
            m.u.progress = k;
            m.u.opacity = opacity.max(0.0);
            None
        });
        if ended {
            return self.opacity > 0.0;
        }
        if self.acc > 0.05 {
            self.acc = 0.0;
            let s = fx.styles();
            let c = self.center;
            fx.ps().emit(
                s.spark,
                c,
                &EmitOptions {
                    count: 1.0 + (k * 3.0).round(),
                    shape: EmitShape::Ring,
                    radius: 0.9,
                    up: between(3.0, 6.0 + k * 6.0),
                    life: [0.3, 0.5].into(),
                    size: [0.05, 0.09].into(),
                    stretch: 0.06,
                    tint: TP,
                    ..default()
                },
            );
            fx.ps().emit(
                s.rune,
                c.with_y(0.1 + k * 2.0),
                &EmitOptions {
                    count: 1.0,
                    shape: EmitShape::Point,
                    life: 0.2.into(),
                    size: (1.3 - k * 0.5).into(),
                    spin: Some(2.0.into()),
                    tint: TP,
                    ..default()
                },
            );
        }
        self.t < self.duration + 1.0
    }

    fn end(&mut self, fx: &mut Fx) {
        fx.forget_aura(&self.key, self.aura);
        fx.release_decal(self.decal.clone());
    }
}

/// Colonne de téléportation qui se charge pendant `duration`, annulable par la clé `key`.
fn tp_column(fx: &mut Fx, center: Vec3, duration: f32, key: String) {
    let ended = Arc::new(AtomicBool::new(false));
    let flag = ended.clone();
    let aura = fx.set_aura(&key, move |_| flag.store(true, Ordering::Relaxed));
    let decal = fx.acquire_decal();
    edit_decal(fx, &decal, |m, g| {
        m.set_color(TP, TP);
        m.u.fill = 0.15;
        m.u.pattern = 1.0;
        m.u.progress = 0.0;
        Some(g.circle(m, center, 1.1, DecalShape::Circle))
    });
    fx.add(TpColumn {
        key,
        aura,
        center,
        duration,
        decal,
        ended,
        opacity: 1.0,
        t: 0.0,
        acc: 0.0,
    });
}

/// Canal au départ, sous le lanceur.
fn teleport_cast(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let GameEvent::SummonerCast { source, .. } = e else {
        return;
    };
    if let Some(p) = fx.entity_pos(*source, 0.0) {
        tp_column(fx, p, 4.0, format!("tp_from:{source}"));
    }
}

/// Arrivée : le lanceur apparaît (blink serveur).
fn teleport_dash(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let GameEvent::Dash { to, .. } = e else {
        return;
    };
    let to = Fx::world_pos(*to, 1.0);
    flash(fx, to, 3.5, TP, 0.4);
    let s = fx.styles();
    fx.ps().emit(
        s.spark,
        to.with_y(0.0),
        &EmitOptions {
            count: 30.0,
            shape: EmitShape::Disc,
            radius: 0.6,
            up: between(6.0, 12.0),
            life: [0.3, 0.5].into(),
            size: [0.08, 0.12].into(),
            stretch: 0.07,
            tint: TP,
            ..default()
        },
    );
}

/// Destination : la colonne se charge pendant la canalisation.
fn teleport_channel(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    if let GameEvent::AreaEffect {
        source,
        pos,
        duration,
        ..
    } = e
    {
        tp_column(
            fx,
            Fx::world_pos(*pos, 0.0),
            (*duration as f32).max(0.5),
            format!("tp_to:{source}"),
        );
    }
}

fn teleport_arrive(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    if let GameEvent::AreaEffect { source, pos, .. } = e {
        fx.end_aura(&format!("tp_from:{source}"));
        fx.end_aura(&format!("tp_to:{source}"));
        shockwave(fx, Fx::world_pos(*pos, 0.0), 2.0, 0.4, TP, 0.14);
    }
}

fn teleport_cancel(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    if let GameEvent::AreaEffect { source, pos, .. } = e {
        fx.end_aura(&format!("tp_from:{source}"));
        fx.end_aura(&format!("tp_to:{source}"));
        let s = fx.styles();
        fx.ps().emit(
            s.sparkles,
            Fx::world_pos(*pos, 0.6),
            &EmitOptions {
                count: 8.0,
                shape: EmitShape::Sphere,
                radius: 0.6,
                life: [0.3, 0.5].into(),
                size: [0.3, 0.4].into(),
                tint: TP,
                intensity: 0.6,
                ..default()
            },
        );
    }
}

// ---------------------------------------------------------------------------------- châtiment, fantôme, purge

fn smite_cast(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let GameEvent::SummonerCast {
        target_id: Some(t), ..
    } = e
    else {
        return;
    };
    let Some(q) = fx.entity_pos(*t, 0.0) else {
        return;
    };
    // éclair divin qui tombe du ciel sur la cible
    let top = Vec3::new(
        q.x + rand_range(-0.4, 0.4),
        9.0,
        q.z + rand_range(-0.4, 0.4),
    );
    lightning(
        fx,
        top,
        q.with_y(0.6),
        SMITE,
        LightningOptions {
            width: 0.45,
            life: 0.35,
            jag: 0.4,
            segments: 12,
        },
    );
    flash(fx, q.with_y(1.0), 3.2, SMITE, 0.35);
    shockwave(fx, q, 2.0, 0.4, SMITE, 0.14);
    let s = fx.styles();
    fx.ps().emit(
        s.spark,
        q,
        &EmitOptions {
            count: 24.0,
            shape: EmitShape::Point,
            speed: between(3.0, 7.0),
            up: between(1.0, 3.0),
            life: [0.3, 0.5].into(),
            size: [0.07, 0.12].into(),
            stretch: 0.05,
            gravity: 8.0,
            tint: SMITE,
            ..default()
        },
    );
}

fn ghost_buff(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let Some((target, duration)) = buff_target(e) else {
        return;
    };
    let mut acc = 0.0;
    let s = fx.styles();
    timed(
        fx,
        duration.max(0.3),
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
                s.glow,
                pos.with_y(0.9),
                &EmitOptions {
                    count: 1.0,
                    shape: EmitShape::Sphere,
                    radius: 0.35,
                    life: [0.4, 0.6].into(),
                    size: [0.6, 0.9].into(),
                    tint: GHOST,
                    intensity: 0.35,
                    ..default()
                },
            );
            fx.ps().emit(
                s.swirl,
                pos.with_y(0.05),
                &EmitOptions {
                    count: 1.0,
                    shape: EmitShape::Point,
                    life: 0.35.into(),
                    size: 0.9.into(),
                    spin: Some(4.0.into()),
                    tint: GHOST,
                    intensity: 0.5,
                    ..default()
                },
            );
        },
        |_| {},
    );
}

fn cleanse_cast(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let GameEvent::SummonerCast { source, .. } = e else {
        return;
    };
    let source = *source;
    let Some(pos) = fx.entity_pos(source, 0.0) else {
        return;
    };
    flash(fx, pos.with_y(1.0), 2.6, CLEANSE, 0.3);
    shockwave(fx, pos, 1.8, 0.4, CLEANSE, 0.1);
    let s = fx.styles();
    timed(
        fx,
        0.5,
        move |fx, t, _| {
            let Some(pos) = fx.entity_pos(source, 0.0) else {
                return;
            };
            let a = t * 18.0;
            let p = Vec3::new(pos.x + a.cos() * 0.6, 0.2 + t * 2.2, pos.z + a.sin() * 0.6);
            fx.ps().emit(
                s.sparkles,
                p,
                &EmitOptions {
                    count: 1.0,
                    shape: EmitShape::Point,
                    life: 0.4.into(),
                    size: 0.35.into(),
                    tint: CLEANSE,
                    ..default()
                },
            );
        },
        |_| {},
    );
}

/// Enregistre les FX des sorts d'invocateur.
pub fn register(app: &mut App) {
    app.register_fx(
        "sum_flash",
        FxHandlers {
            dash: Some(flash_dash),
            summoner: Some(nothing),
            ..default()
        },
    )
    .register_fx(
        "sum_ignite",
        FxHandlers {
            summoner: Some(ignite_cast),
            buff: Some(ignite_buff),
            ..default()
        },
    )
    .register_fx(
        "sum_heal",
        FxHandlers {
            summoner: Some(heal_cast),
            buff: Some(heal_buff),
            ..default()
        },
    )
    .register_fx(
        "sum_barrier",
        FxHandlers {
            buff: Some(barrier_buff),
            buff_end: Some(barrier_end),
            ..default()
        },
    )
    .register_fx(
        "sum_exhaust",
        FxHandlers {
            summoner: Some(exhaust_cast),
            buff: Some(exhaust_buff),
            ..default()
        },
    )
    .register_fx(
        "sum_teleport",
        FxHandlers {
            summoner: Some(teleport_cast),
            dash: Some(teleport_dash),
            ..default()
        },
    )
    .register_fx(
        "sum_teleport_channel",
        FxHandlers {
            area: Some(teleport_channel),
            ..default()
        },
    )
    .register_fx(
        "sum_teleport_arrive",
        FxHandlers {
            area: Some(teleport_arrive),
            ..default()
        },
    )
    .register_fx(
        "sum_teleport_cancel",
        FxHandlers {
            area: Some(teleport_cancel),
            ..default()
        },
    )
    .register_fx(
        "sum_smite",
        FxHandlers {
            summoner: Some(smite_cast),
            ..default()
        },
    )
    .register_fx(
        "sum_ghost",
        FxHandlers {
            buff: Some(ghost_buff),
            ..default()
        },
    )
    .register_fx(
        "sum_cleanse",
        FxHandlers {
            summoner: Some(cleanse_cast),
            ..default()
        },
    );
}

#[cfg(test)]
mod tests {
    use lov_shared::data::summoners::SUMMONERS;

    use super::*;
    use crate::fx::core::runtime::FxRuntime;
    use crate::fx::effects::primitives::tests::fx_world;
    use crate::fx::registry::FxRegistry;

    #[test]
    fn chaque_sort_d_invocateur_a_un_fx_dedie() {
        let mut app = App::new();
        register(&mut app);
        let reg = app.world().resource::<FxRegistry>();
        for def in SUMMONERS.iter() {
            assert!(reg.get(&def.fx_key).is_some(), "{} sans FX", def.fx_key);
        }
    }

    #[test]
    fn la_colonne_de_teleportation_s_arrete_a_l_arrivee() {
        let mut w = fx_world();
        let pos = lov_shared::types::Vec2 { x: 0.0, y: 0.0 };
        let c = FxRuntime::default().colors("sum_teleport");
        let area = |key: &str| GameEvent::AreaEffect {
            source: 3,
            fx_key: key.into(),
            pos,
            radius: 100.0,
            duration: 4.0,
        };
        teleport_channel(&mut Fx::new(&mut w), &area("sum_teleport_channel"), &c);
        for _ in 0..10 {
            FxRuntime::update(&mut w, 0.1);
        }
        assert_eq!(w.resource::<FxRuntime>().active_effects(), 1);
        teleport_arrive(&mut Fx::new(&mut w), &area("sum_teleport_arrive"), &c);
        for _ in 0..10 {
            FxRuntime::update(&mut w, 0.1);
        }
        assert_eq!(w.resource::<FxRuntime>().active_effects(), 0);
    }
}
