//! FX de Grondak, le Rempart de Granit : pierre, poussière, fissures de magma ambré.
//!
//! | sort | FX |
//! |------|----|
//! | P Peau de granit | coque ambrée et éclats de roche en orbite tant que le bouclier tient |
//! | Q Poing sismique | fissure qui court au sol, pics de roche jaillissants, impact poussiéreux |
//! | W Rempart | rugissement (ondes de provocation) et armure de pierre pendant 3 s |
//! | E Charge de roche | ruée dans la poussière, impact étourdissant |
//! | R Effondrement | sol qui se fissure (télégraphie), éruption de pics, débris projetés |
//!
//! Portage de `packages/client/src/fx/champions/grondak.ts`.

use std::f32::consts::{PI, TAU};

use bevy::prelude::*;
use lov_shared::data::champions::get_champion;
use lov_shared::types::GameEvent;

use crate::fx::core::palette::{FxColors, linear};
use crate::fx::core::runtime::Fx;
use crate::fx::effects::auras::{ShellOptions, shield_shell};
use crate::fx::effects::mesh_bursts::{BurstKind, BurstMode, BurstSpawn, burst};
use crate::fx::effects::primitives::{
    ProjectileFx, ProjectileLook, TelegraphOptions, between, dash_trail, decal_opacity, edit_decal,
    flash, rand, rand_range, shockwave, telegraph, timed,
};
use crate::fx::particles::{EmitOptions, EmitShape};
use crate::fx::registry::{FxHandlers, ProjectileHandle, RegisterFx};

fn stone() -> Vec3 {
    const STONE: [&str; 4] = ["#7a6a58", "#6b5d4d", "#8c7b66", "#5a5047"];
    linear(STONE[((rand() * 4.0) as usize).min(3)])
}

/// Gerbe de débris de roche autour de `at`.
fn rock_burst(fx: &mut Fx, at: Vec3, n: u32, power: f32, size: [f32; 2]) {
    for _ in 0..n {
        let a = rand() * TAU;
        let h = rand_range(0.3, 1.0) * power;
        burst(
            fx,
            BurstKind::RockDebris,
            BurstSpawn {
                pos: Vec3::new(at.x + a.cos() * 0.2, at.y + 0.2, at.z + a.sin() * 0.2),
                mode: BurstMode::Debris,
                size: rand_range(size[0], size[1]),
                life: rand_range(0.9, 1.4),
                vel: Vec3::new(
                    a.cos() * rand_range(1.0, 3.0) * power * 0.5,
                    h * 5.0,
                    a.sin() * rand_range(1.0, 3.0) * power * 0.5,
                ),
                color: Some(stone()),
                ..default()
            },
        );
    }
}

/// Pic de roche qui jaillit du sol.
fn spike(
    fx: &mut Fx,
    at: Vec3,
    size: f32,
    life: f32,
    heading: Option<f32>,
    tilt: Option<f32>,
    delay: f32,
) {
    burst(
        fx,
        BurstKind::RockSpikes,
        BurstSpawn {
            pos: at,
            mode: BurstMode::Spike,
            size,
            life,
            heading: Some(heading.unwrap_or_else(|| rand() * TAU)),
            tilt: Some(tilt.unwrap_or_else(|| rand_range(-0.2, 0.2))),
            stretch: rand_range(1.6, 2.4),
            color: Some(stone()),
            delay,
            ..default()
        },
    );
}

fn dust_ring(fx: &mut Fx, at: Vec3, radius: f32, n: f32) {
    let s = fx.styles();
    fx.ps().emit(
        s.dust,
        at,
        &EmitOptions {
            count: n,
            shape: EmitShape::Ring,
            radius,
            speed: between(1.0, 2.5),
            up: between(0.2, 0.8),
            life: [0.7, 1.2].into(),
            size: [0.7, 1.4].into(),
            drag: 2.0,
            ..default()
        },
    );
}

fn magma_sparks(fx: &mut Fx, at: Vec3, c: &FxColors, n: f32, up: [f32; 2]) {
    let s = fx.styles();
    fx.ps().emit(
        s.spark,
        at,
        &EmitOptions {
            count: n,
            shape: EmitShape::Disc,
            radius: 0.4,
            speed: between(1.0, 3.0),
            up: between(up[0], up[1]),
            life: [0.4, 0.8].into(),
            size: [0.05, 0.1].into(),
            stretch: 0.06,
            gravity: 9.0,
            tint: c.accent_hdr,
            ..default()
        },
    );
}

// ---------------------------------------------------------------------------------- P

fn p_buff(fx: &mut Fx, e: &GameEvent, c: &FxColors) {
    let GameEvent::BuffApplied {
        target, duration, ..
    } = e
    else {
        return;
    };
    let accent = c.accent_hdr;
    let mut acc = 0.0;
    let s = fx.styles();
    // état fréquent (présent dès 8 s hors combat) : coque discrète, liseré fin, peu de bandes
    shield_shell(
        fx,
        *target,
        if *duration > 0.0 {
            *duration as f32
        } else {
            30.0
        },
        &format!("grondak_p:{target}"),
        ShellOptions {
            color: Vec3::new(0.75, 0.45, 0.16),
            radius: 1.15,
            height: 1.05,
            bands: 0.08,
            power: 3.2,
            on_step: Some(Box::new(move |fx, center, t, dt| {
                acc += dt;
                if acc < 0.12 {
                    return;
                }
                acc = 0.0;
                // éclats de roche lumineux en orbite (particules courtes = traînée)
                for i in 0..3 {
                    let a = t * 1.6 + i as f32 / 3.0 * TAU;
                    let p = Vec3::new(
                        center.x + a.cos() * 1.05,
                        center.y - 0.2 + (t * 2.0 + i as f32).sin() * 0.25,
                        center.z + a.sin() * 1.05,
                    );
                    fx.ps().emit(
                        s.ember,
                        p,
                        &EmitOptions {
                            count: 1.0,
                            shape: EmitShape::Point,
                            life: 0.35.into(),
                            size: 0.16.into(),
                            tint: accent,
                            ..default()
                        },
                    );
                }
            })),
            on_end: Some(Box::new(move |fx, center| {
                rock_burst(fx, center, 8, 0.6, [0.12, 0.22]);
                fx.ps().emit(
                    s.dust,
                    center,
                    &EmitOptions {
                        count: 6.0,
                        shape: EmitShape::Sphere,
                        radius: 0.6,
                        speed: between(0.5, 1.5),
                        life: [0.6, 0.9].into(),
                        size: [0.6, 1.0].into(),
                        ..default()
                    },
                );
            })),
        },
    );
    if let Some(p) = fx.entity_pos(*target, 1.0) {
        flash(fx, p, 2.4, accent, 0.3);
    }
}

fn p_buff_end(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    if let GameEvent::BuffRemoved { target, .. } = e {
        fx.end_aura(&format!("grondak_p:{target}"));
    }
}

// ---------------------------------------------------------------------------------- Q

fn q_cast(fx: &mut Fx, e: &GameEvent, c: &FxColors) {
    let GameEvent::AbilityCast {
        source, target_pos, ..
    } = e
    else {
        return;
    };
    let Some(v) = fx.entity_pos(*source, 0.0) else {
        return;
    };
    // le poing frappe le sol devant lui (≈ 0,25 s dans l'animation)
    let target = target_pos.map_or(v, |p| Fx::world_pos(p, 0.0));
    let dir = (target - v).with_y(0.0).normalize_or_zero();
    let at = v + dir * 0.7;
    let c = *c;
    fx.after(0.22, move |fx| {
        dust_ring(fx, at, 0.4, 8.0);
        shockwave(fx, at, 1.2, 0.35, c.accent_hdr, 0.12);
        rock_burst(fx, at, 5, 0.6, [0.1, 0.18]);
    });
}

fn q_projectile(fx: &mut Fx, e: &GameEvent, c: &FxColors) -> Option<Box<dyn ProjectileHandle>> {
    let GameEvent::ProjectileSpawn {
        from,
        to,
        speed,
        width,
        ..
    } = e
    else {
        return None;
    };
    // fissure qui court au sol : pas de projectile visible, des pics jaillissent sur son passage
    let from = Fx::world_pos(*from, 0.0);
    let to = Fx::world_pos(*to, 0.0);
    let crack = fx.acquire_decal();
    let crack_color = c.accent * 0.9;
    edit_decal(fx, &crack, |m, _| {
        m.set_color(crack_color, crack_color);
        m.u.fill = 0.3;
        m.u.edge = 0.05;
        m.u.pattern = 0.8;
        None
    });
    let width = Fx::scale(*width as f32) * 0.28;
    let speed = Fx::scale(*speed as f32);
    let c = *c;
    let crack2 = crack.clone();
    let mut since = 0.0;
    let mut side = 1.0;
    let stop = ProjectileFx::launch(
        fx,
        from,
        to,
        speed,
        ProjectileLook {
            color: c.accent_hdr,
            size: 0.0,
            height: 0.0,
            ribbon_width: 0.0,
            on_step: Some(Box::new(move |fx, head, dir, dt| {
                edit_decal(fx, &crack, |m, g| Some(g.rect(m, from, head, width)));
                since += speed * dt;
                while since > 0.42 {
                    since -= 0.42;
                    side = -side;
                    let off = Vec3::new(-dir.z, 0.0, dir.x) * (side * rand_range(0.05, 0.3));
                    let v = head + off;
                    spike(
                        fx,
                        v,
                        rand_range(0.32, 0.5),
                        0.9,
                        Some(dir.x.atan2(dir.z)),
                        Some(side * 0.35),
                        0.0,
                    );
                    let s = fx.styles();
                    fx.ps().emit(
                        s.dust,
                        v,
                        &EmitOptions {
                            count: 2.0,
                            shape: EmitShape::Disc,
                            radius: 0.2,
                            up: between(0.4, 1.0),
                            speed: between(0.3, 0.8),
                            life: [0.6, 1.0].into(),
                            size: [0.6, 1.0].into(),
                            ..default()
                        },
                    );
                    magma_sparks(fx, v, &c, 2.0, [1.0, 3.0]);
                }
            })),
            on_end: Some(Box::new(move |fx, _| {
                let crack3 = crack2.clone();
                timed(
                    fx,
                    0.8,
                    move |fx, t, _| decal_opacity(fx, &crack2, 1.0 - t),
                    move |fx| fx.release_decal(crack3),
                );
            })),
            ..default()
        },
    );
    Some(Box::new(stop))
}

fn q_hit(fx: &mut Fx, e: &GameEvent, c: &FxColors) {
    let GameEvent::ProjectileHit { pos, .. } = e else {
        return;
    };
    let at = Fx::world_pos(*pos, 0.0);
    rock_burst(fx, at, 7, 0.8, [0.1, 0.2]);
    let s = fx.styles();
    fx.ps().emit(
        s.dust,
        at,
        &EmitOptions {
            count: 8.0,
            shape: EmitShape::Sphere,
            radius: 0.4,
            speed: between(1.0, 2.0),
            up: between(0.5, 1.2),
            life: [0.6, 1.0].into(),
            size: [0.7, 1.2].into(),
            drag: 2.0,
            ..default()
        },
    );
    flash(fx, at.with_y(0.9), 1.6, c.accent_hdr, 0.2);
}

// ---------------------------------------------------------------------------------- W

/// Rugissement : ondes de provocation autour de Grondak (rayon du sort).
fn w_cast(fx: &mut Fx, e: &GameEvent, c: &FxColors) {
    let GameEvent::AbilityCast { source, .. } = e else {
        return;
    };
    let Some(center) = fx.entity_pos(*source, 0.0) else {
        return;
    };
    let radius = get_champion("grondak")
        .and_then(|g| g.abilities.w.radius)
        .unwrap_or(325.0);
    let r = Fx::scale(radius as f32);
    flash(fx, center.with_y(1.6), 2.6, c.accent_hdr, 0.35);
    let accent = c.accent_hdr;
    for i in 0..3 {
        fx.after(i as f32 * 0.12, move |fx| {
            shockwave(fx, center, r * (1.0 - i as f32 * 0.15), 0.5, accent, 0.14);
        });
    }
    dust_ring(fx, center, r * 0.6, 14.0);
    rock_burst(fx, center, 6, 0.5, [0.1, 0.16]);
}

/// Buff `grondak_w` (réduction de dégâts) : armure de pierre. Les `taunt` posés par le sort passent par
/// les statuts.
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
    if buff_id != "grondak_w" {
        return;
    }
    let s = fx.styles();
    let mut dust_acc = 0.0;
    shield_shell(
        fx,
        *target,
        (*duration as f32).max(0.5),
        &format!("grondak_w:{target}"),
        ShellOptions {
            color: Vec3::new(0.9, 0.6, 0.3),
            radius: 1.2,
            height: 1.05,
            bands: 0.45,
            power: 1.6,
            on_step: Some(Box::new(move |fx, cen, _, dt| {
                dust_acc += dt * 12.0; // ~12 bouffées de poussière par seconde
                if dust_acc < 1.0 {
                    return;
                }
                dust_acc -= 1.0;
                fx.ps().emit(
                    s.dust,
                    cen.with_y(0.1),
                    &EmitOptions {
                        count: 1.0,
                        shape: EmitShape::Ring,
                        radius: 0.8,
                        up: between(0.3, 0.8),
                        life: [0.5, 0.8].into(),
                        size: [0.4, 0.7].into(),
                        ..default()
                    },
                );
            })),
            ..default()
        },
    );
}

fn w_buff_end(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    if let GameEvent::BuffRemoved {
        target, buff_id, ..
    } = e
        && buff_id == "grondak_w"
    {
        fx.end_aura(&format!("grondak_w:{target}"));
    }
}

// ---------------------------------------------------------------------------------- E

/// Charge : poussière tout le long, impact à l'arrivée (l'étourdissement est un statut générique).
fn e_dash(fx: &mut Fx, e: &GameEvent, c: &FxColors) {
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
    let duration = (*duration as f32).max(0.05);
    let source = *source;
    dash_trail(fx, source, from, to, duration, c.accent * 0.7, true);
    rock_burst(fx, from, 5, 0.5, [0.1, 0.18]);
    let c = *c;
    let s = fx.styles();
    timed(
        fx,
        duration,
        move |fx, _, _| {
            let Some(pos) = fx.entity_pos(source, 0.0) else {
                return;
            };
            fx.ps().emit(
                s.dust,
                pos,
                &EmitOptions {
                    count: 2.0,
                    shape: EmitShape::Disc,
                    radius: 0.4,
                    up: between(0.3, 1.0),
                    speed: between(0.5, 1.5),
                    life: [0.6, 1.0].into(),
                    size: [0.8, 1.3].into(),
                    drag: 2.0,
                    ..default()
                },
            );
            magma_sparks(fx, pos.with_y(0.5), &c, 1.0, [0.5, 2.0]);
        },
        |_| {},
    );
    fx.after(duration, move |fx| {
        let pos = fx.entity_pos(source, 0.0).unwrap_or(to);
        shockwave(fx, pos, 1.6, 0.4, c.accent_hdr, 0.14);
        fx.shake(pos, 15.0, 0.2);
        rock_burst(fx, pos, 10, 1.0, [0.12, 0.24]);
        dust_ring(fx, pos, 0.6, 12.0);
        flash(fx, pos.with_y(1.1), 2.4, c.accent_hdr, 0.25);
    });
}

// ---------------------------------------------------------------------------------- R

fn r_cast(fx: &mut Fx, e: &GameEvent, c: &FxColors) {
    let GameEvent::AbilityCast { source, .. } = e else {
        return;
    };
    let Some(v) = fx.entity_pos(*source, 2.2) else {
        return;
    };
    flash(fx, v, 3.0, c.accent_hdr, 0.5);
    magma_sparks(fx, v.with_y(1.5), c, 12.0, [1.0, 3.0]);
}

/// Détonation (0,25 s) après la télégraphie.
fn r_area(fx: &mut Fx, e: &GameEvent, c: &FxColors) {
    if let GameEvent::AreaEffect { pos, radius, .. } = e {
        erupt(fx, Fx::world_pos(*pos, 0.0), Fx::scale(*radius as f32), c);
    }
}

/// Télégraphie : le sol tremble, petits cailloux qui sautent, fissures qui s'illuminent.
fn r_telegraph(fx: &mut Fx, e: &GameEvent, c: &FxColors) {
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
        c.accent * 0.8,
        TelegraphOptions {
            color2: Some(c.accent_hdr),
            fill: Some(0.1),
            ..default()
        },
    );
    let c = *c;
    timed(
        fx,
        delay,
        move |fx, t, _| {
            if rand() < 0.5 {
                let a = rand() * TAU;
                let d = rand().sqrt() * r;
                let v = Vec3::new(center.x + a.cos() * d, 0.05, center.z + a.sin() * d);
                burst(
                    fx,
                    BurstKind::RockDebris,
                    BurstSpawn {
                        pos: v,
                        mode: BurstMode::Debris,
                        size: rand_range(0.06, 0.12),
                        life: 0.5,
                        vel: Vec3::new(0.0, rand_range(1.5, 3.0) * (0.5 + t), 0.0),
                        color: Some(stone()),
                        ..default()
                    },
                );
                if rand() < 0.4 {
                    magma_sparks(fx, v, &c, 1.0, [0.5, 1.5]);
                }
            }
        },
        |_| {},
    );
}

/// Éruption de l'ultime : anneau de grands pics, débris projetés en l'air, nuage de poussière.
fn erupt(fx: &mut Fx, center: Vec3, r: f32, c: &FxColors) {
    flash(fx, center.with_y(0.5), r * 1.4, c.accent_hdr, 0.35);
    fx.shake(center, 40.0, 0.45);
    shockwave(fx, center, r * 1.15, 0.6, c.accent_hdr, 0.25);
    let n = 14;
    for i in 0..n {
        let a = i as f32 / n as f32 * TAU + rand_range(-0.1, 0.1);
        let d = r * rand_range(0.55, 0.9);
        let v = Vec3::new(center.x + a.cos() * d, 0.0, center.z + a.sin() * d);
        // inclinés vers l'extérieur
        spike(
            fx,
            v,
            rand_range(0.55, 0.85),
            rand_range(1.2, 1.6),
            Some(a.cos().atan2(a.sin()) + PI),
            Some(-0.35),
            rand_range(0.0, 0.08),
        );
    }
    for _ in 0..5 {
        let a = rand() * TAU;
        let v = Vec3::new(
            center.x + a.cos() * r * 0.25,
            0.0,
            center.z + a.sin() * r * 0.25,
        );
        spike(fx, v, rand_range(0.45, 0.7), 1.3, None, None, 0.05);
    }
    rock_burst(fx, center, 24, 1.6, [0.12, 0.3]);
    let s = fx.styles();
    fx.ps().emit(
        s.dust,
        center,
        &EmitOptions {
            count: 26.0,
            shape: EmitShape::Disc,
            radius: r * 0.9,
            speed: between(0.5, 2.0),
            up: between(0.5, 2.0),
            life: [1.0, 1.8].into(),
            size: [1.0, 2.0].into(),
            drag: 1.5,
            ..default()
        },
    );
    magma_sparks(fx, center, c, 30.0, [3.0, 8.0]);
}

pub fn register(app: &mut App) {
    app.register_fx(
        "grondak_p",
        FxHandlers {
            buff: Some(p_buff),
            buff_end: Some(p_buff_end),
            ..default()
        },
    )
    .register_fx(
        "grondak_q",
        FxHandlers {
            cast: Some(q_cast),
            projectile: Some(q_projectile),
            hit: Some(q_hit),
            ..default()
        },
    )
    .register_fx(
        "grondak_w",
        FxHandlers {
            cast: Some(w_cast),
            buff: Some(w_buff),
            buff_end: Some(w_buff_end),
            ..default()
        },
    )
    .register_fx(
        "grondak_e",
        FxHandlers {
            dash: Some(e_dash),
            ..default()
        },
    )
    .register_fx(
        "grondak_r",
        FxHandlers {
            cast: Some(r_cast),
            area: Some(r_area),
            ..default()
        },
    )
    .register_fx(
        "grondak_r_telegraph",
        FxHandlers {
            area: Some(r_telegraph),
            ..default()
        },
    );
}

#[cfg(test)]
mod tests {
    use lov_shared::types::Vec2;

    use super::*;
    use crate::fx::core::runtime::FxRuntime;
    use crate::fx::effects::MeshBursts;
    use crate::fx::effects::primitives::tests::fx_world;

    #[test]
    fn l_eruption_fait_jaillir_pics_et_debris() {
        let mut w = fx_world();
        w.init_resource::<MeshBursts>();
        let c = FxRuntime::default().colors("grondak_r");
        let e = GameEvent::AreaEffect {
            source: 1,
            fx_key: "grondak_r".into(),
            pos: Vec2 { x: 0.0, y: 0.0 },
            radius: 400.0,
            duration: 0.25,
        };
        r_area(&mut Fx::new(&mut w), &e, &c);
        let mb = w.resource::<MeshBursts>();
        assert_eq!(mb.alive(BurstKind::RockSpikes), 19);
        assert_eq!(mb.alive(BurstKind::RockDebris), 24);
    }
}
