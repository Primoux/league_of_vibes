//! FX de 67 Master : chiffres de lumière (6 cyan, 7 violet) en géométrie, sans texture ni asset tiers.
//!
//! | sort | événements |
//! |------|------------|
//! | P | dégâts `master67_p` : un 7 s'élève de la cible |
//! | Q | projectile `master67_q` : un 6 file vers la cible |
//! | W | zone `master67_w_telegraph` (un 7 qui tombe) puis `master67_w` |
//! | E | dash `master67_e` : un 6 au départ, un 7 à l'arrivée |
//! | R | abilityCast `master67_r` (télégraphie), zones `master67_r_six` / `master67_r_seven` |
//!
//! Portage de `packages/client/src/fx/champions/master67.ts`. Les chiffres sont des tubes le long du tracé
//! du web (Catmull-Rom centripète pour le 6, segments pour le 7), tournés vers la caméra.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use lov_shared::types::GameEvent;

use crate::fx::core::palette::{FxColors, linear};
use crate::fx::core::runtime::{Billboard, Fx};
use crate::fx::effects::primitives::{
    ProjectileFx, ProjectileLook, TelegraphOptions, between, dash_trail, flash, shockwave,
    telegraph, timed,
};
use crate::fx::particles::{EmitOptions, EmitShape};
use crate::fx::registry::{FxHandlers, ProjectileHandle, RegisterFx};

fn cyan() -> Vec3 {
    linear("#35e6e0")
}

fn violet() -> Vec3 {
    linear("#ad77ff")
}

const SIX: &[[f32; 2]] = &[
    [0.19, 0.38],
    [-0.04, 0.42],
    [-0.26, 0.16],
    [-0.25, -0.2],
    [-0.08, -0.34],
    [0.17, -0.27],
    [0.22, -0.08],
    [0.06, 0.06],
    [-0.23, -0.02],
];
const SEVEN: &[[f32; 2]] = &[[-0.25, 0.35], [0.25, 0.35], [-0.12, -0.35]];
const TUBE_RADIUS: f32 = 0.042;
const RADIAL: usize = 5;

/// Point d'une Catmull-Rom centripète (`CatmullRomCurve3` de three.js, `centripetal`) à `t ∈ 0..1`.
fn catmull_rom(points: &[Vec2], t: f32) -> Vec2 {
    let l = points.len();
    let p = (l - 1) as f32 * t;
    let mut i = p.floor() as usize;
    let mut w = p - i as f32;
    if i >= l - 1 {
        i = l - 2;
        w = 1.0;
    }
    let p1 = points[i];
    let p2 = points[i + 1];
    // extrémités prolongées par symétrie, comme three.js pour une courbe ouverte
    let p0 = if i > 0 { points[i - 1] } else { p1 * 2.0 - p2 };
    let p3 = if i + 2 < l {
        points[i + 2]
    } else {
        p2 * 2.0 - p1
    };
    let dt = |a: Vec2, b: Vec2| a.distance_squared(b).powf(0.25).max(1e-4);
    let (mut dt0, mut dt1, mut dt2) = (dt(p0, p1), dt(p1, p2), dt(p2, p3));
    if dt1 < 1e-4 {
        dt1 = 1.0;
    }
    if dt0 < 1e-4 {
        dt0 = dt1;
    }
    if dt2 < 1e-4 {
        dt2 = dt1;
    }
    let comp = |x0: f32, x1: f32, x2: f32, x3: f32| {
        // polynôme cubique non uniforme (`initNonuniformCatmullRom`)
        let t1 = (x1 - x0) / dt0 - (x2 - x0) / (dt0 + dt1) + (x2 - x1) / dt1;
        let t2 = (x2 - x1) / dt1 - (x3 - x1) / (dt1 + dt2) + (x3 - x2) / dt2;
        let (t1, t2) = (t1 * dt1, t2 * dt1);
        let c0 = x1;
        let c1 = t1;
        let c2 = -3.0 * x1 + 3.0 * x2 - 2.0 * t1 - t2;
        let c3 = 2.0 * x1 - 2.0 * x2 + t1 + t2;
        c0 + c1 * w + c2 * w * w + c3 * w * w * w
    };
    Vec2::new(comp(p0.x, p1.x, p2.x, p3.x), comp(p0.y, p1.y, p2.y, p3.y))
}

/// Échantillons du tracé d'un chiffre.
fn digit_path(digit: u8) -> Vec<Vec2> {
    if digit == 6 {
        let pts: Vec<Vec2> = SIX.iter().map(|&[x, y]| Vec2::new(x, y)).collect();
        (0..=26)
            .map(|k| catmull_rom(&pts, k as f32 / 26.0))
            .collect()
    } else {
        // trois segments droits, échantillonnés comme les 10 segments de tube du web
        let pts: Vec<Vec2> = SEVEN.iter().map(|&[x, y]| Vec2::new(x, y)).collect();
        let mut out = Vec::new();
        for w in pts.windows(2) {
            for k in 0..5 {
                out.push(w[0].lerp(w[1], k as f32 / 5.0));
            }
        }
        out.push(pts[pts.len() - 1]);
        out
    }
}

/// Tube le long d'un tracé plan (plan XY, face +Z).
fn tube(path: &[Vec2]) -> Mesh {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices = Vec::new();
    for (k, p) in path.iter().enumerate() {
        let prev = path[k.saturating_sub(1)];
        let next = path[(k + 1).min(path.len() - 1)];
        let tan = (next - prev).normalize_or(Vec2::X);
        let side = Vec3::new(-tan.y, tan.x, 0.0);
        for r in 0..RADIAL {
            let a = r as f32 / RADIAL as f32 * std::f32::consts::TAU;
            let n = side * a.cos() + Vec3::Z * a.sin();
            positions.push((p.extend(0.0) + n * TUBE_RADIUS).to_array());
            normals.push(n.to_array());
        }
    }
    for k in 0..path.len() - 1 {
        for r in 0..RADIAL {
            let a = (k * RADIAL + r) as u32;
            let b = (k * RADIAL + (r + 1) % RADIAL) as u32;
            let c = a + RADIAL as u32;
            let d = b + RADIAL as u32;
            indices.extend([a, c, b, b, c, d]);
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_indices(Indices::U32(indices))
}

/// Maillages partagés des deux chiffres.
#[derive(Resource)]
struct Digits {
    six: Handle<Mesh>,
    seven: Handle<Mesh>,
}

impl FromWorld for Digits {
    fn from_world(world: &mut World) -> Self {
        let mut meshes = world.resource_mut::<Assets<Mesh>>();
        Self {
            six: meshes.add(tube(&digit_path(6))),
            seven: meshes.add(tube(&digit_path(7))),
        }
    }
}

/// Un chiffre affiché : entité face caméra et son matériau (opacité propre).
#[derive(Clone)]
struct Glyph {
    entity: Entity,
    material: Handle<StandardMaterial>,
    color: Vec3,
}

impl Glyph {
    fn spawn(fx: &mut Fx, digit: u8, at: Vec3, color: Vec3, scale: f32) -> Glyph {
        let mesh = {
            let d = fx.world().get_resource_or_init::<Digits>();
            if digit == 6 {
                d.six.clone()
            } else {
                d.seven.clone()
            }
        };
        let c = color * 1.8;
        let material =
            fx.world()
                .resource_mut::<Assets<StandardMaterial>>()
                .add(StandardMaterial {
                    base_color: Color::LinearRgba(LinearRgba::new(c.x, c.y, c.z, 1.0)),
                    unlit: true,
                    alpha_mode: AlphaMode::Blend,
                    ..default()
                });
        let entity = fx
            .world()
            .spawn((
                Name::new("fx-master67-digit"),
                Mesh3d(mesh),
                MeshMaterial3d(material.clone()),
                Transform::from_translation(at).with_scale(Vec3::splat(scale)),
                Billboard::default(),
            ))
            .id();
        Glyph {
            entity,
            material,
            color: c,
        }
    }

    fn place(&self, fx: &mut Fx, at: Vec3) {
        if let Some(mut tf) = fx.world().get_mut::<Transform>(self.entity) {
            tf.translation = at;
        }
    }

    fn opacity(&self, fx: &mut Fx, o: f32) {
        if let Some(mut m) = fx
            .world()
            .resource_mut::<Assets<StandardMaterial>>()
            .get_mut(&self.material)
        {
            let c = self.color;
            m.base_color = Color::LinearRgba(LinearRgba::new(c.x, c.y, c.z, o.clamp(0.0, 1.0)));
        }
    }

    /// Destruction idempotente, y compris quand un projectile est arrêté à l'impact.
    fn dispose(&self, fx: &mut Fx) {
        let world = fx.world();
        if let Ok(e) = world.get_entity_mut(self.entity) {
            e.despawn();
        }
        world
            .resource_mut::<Assets<StandardMaterial>>()
            .remove(&self.material);
    }
}

fn rising_digit(fx: &mut Fx, digit: u8, at: Vec3, color: Vec3, size: f32) {
    let g = Glyph::spawn(fx, digit, at, color, size);
    let g2 = g.clone();
    timed(
        fx,
        0.8,
        move |fx, t, _| {
            g.place(fx, at + Vec3::Y * t);
            g.opacity(fx, 1.0 - t * t);
        },
        move |fx| g2.dispose(fx),
    );
}

fn q_projectile(fx: &mut Fx, e: &GameEvent, _: &FxColors) -> Option<Box<dyn ProjectileHandle>> {
    let GameEvent::ProjectileSpawn {
        from, to, speed, ..
    } = e
    else {
        return None;
    };
    let from = Fx::world_pos(*from, 1.05);
    let digit = Glyph::spawn(fx, 6, from, cyan(), 1.1);
    let d2 = digit.clone();
    let stop = ProjectileFx::launch(
        fx,
        from,
        Fx::world_pos(*to, 0.0),
        Fx::scale(*speed as f32),
        ProjectileLook {
            color: cyan(),
            size: 0.22,
            height: 1.05,
            ribbon_width: 0.12,
            on_step: Some(Box::new(move |fx, pos, _, _| digit.place(fx, pos))),
            on_end: Some(Box::new(move |fx, _| d2.dispose(fx))),
            ..default()
        },
    );
    Some(Box::new(stop))
}

fn q_hit(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    if let GameEvent::ProjectileHit { pos, .. } = e {
        flash(fx, Fx::world_pos(*pos, 1.0), 1.0, cyan(), 0.25);
        shockwave(fx, Fx::world_pos(*pos, 0.0), 0.9, 0.35, cyan(), 0.12);
    }
}

fn w_telegraph(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    let GameEvent::AreaEffect {
        pos,
        radius,
        duration,
        ..
    } = e
    else {
        return;
    };
    let at = Fx::world_pos(*pos, 0.0);
    let duration = *duration as f32;
    telegraph(
        fx,
        at,
        Fx::scale(*radius as f32),
        duration,
        violet(),
        TelegraphOptions::default(),
    );
    let g = Glyph::spawn(fx, 7, at, violet(), 2.3);
    let g2 = g.clone();
    timed(
        fx,
        duration,
        move |fx, t, _| g.place(fx, at.with_y(at.y + 3.5 * (1.0 - t) + 0.3)),
        move |fx| g2.dispose(fx),
    );
}

fn w_area(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    if let GameEvent::AreaEffect { pos, radius, .. } = e {
        let at = Fx::world_pos(*pos, 0.0);
        shockwave(fx, at, Fx::scale(*radius as f32), 0.55, violet(), 0.12);
        flash(fx, at, 2.7, violet(), 0.25);
    }
}

fn e_dash(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    if let GameEvent::Dash {
        source, from, to, ..
    } = e
    {
        dash_trail(
            fx,
            *source,
            Fx::world_pos(*from, 0.0),
            Fx::world_pos(*to, 0.0),
            0.1,
            cyan(),
            false,
        );
        rising_digit(fx, 6, Fx::world_pos(*from, 0.8), cyan(), 1.7);
        rising_digit(fx, 7, Fx::world_pos(*to, 0.8), violet(), 1.7);
    }
}

fn r_zone(fx: &mut Fx, e: &GameEvent, digit: u8, color: Vec3) {
    let GameEvent::AreaEffect { pos, radius, .. } = e else {
        return;
    };
    let at = Fx::world_pos(*pos, 0.0);
    let r = Fx::scale(*radius as f32);
    shockwave(fx, at, r, 0.67, color, 0.08);
    rising_digit(fx, digit, Fx::world_pos(*pos, 2.1), color, 3.5);
    let s = fx.styles();
    fx.ps().emit(
        s.spark,
        at,
        &EmitOptions {
            count: 36.0,
            shape: EmitShape::Disc,
            radius: r,
            size: [0.12, 0.28].into(),
            life: [0.4, 0.8].into(),
            up: between(1.0, 2.5),
            tint: color,
            ..default()
        },
    );
}

fn r_six(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    r_zone(fx, e, 6, cyan());
}

fn r_seven(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    r_zone(fx, e, 7, violet());
}

fn r_cast(fx: &mut Fx, e: &GameEvent, _: &FxColors) {
    if let GameEvent::AbilityCast { source, .. } = e
        && let Some(at) = fx.entity_pos(*source, 0.0)
    {
        telegraph(
            fx,
            at,
            Fx::scale(467.0),
            0.67,
            cyan(),
            TelegraphOptions {
                color2: Some(violet()),
                fill: Some(0.06),
                ..default()
            },
        );
    }
}

fn p_hit(fx: &mut Fx, e: &GameEvent) {
    if let GameEvent::Damage { target, .. } = e
        && let Some(at) = fx.entity_pos(*target, 1.5)
    {
        rising_digit(fx, 7, at, violet(), 1.7);
    }
}

pub fn register(app: &mut App) {
    app.register_fx(
        "master67_q",
        FxHandlers {
            projectile: Some(q_projectile),
            hit: Some(q_hit),
            ..default()
        },
    )
    .register_fx(
        "master67_w_telegraph",
        FxHandlers {
            area: Some(w_telegraph),
            ..default()
        },
    )
    .register_fx(
        "master67_w",
        FxHandlers {
            area: Some(w_area),
            ..default()
        },
    )
    .register_fx(
        "master67_e",
        FxHandlers {
            dash: Some(e_dash),
            ..default()
        },
    )
    .register_fx(
        "master67_r_six",
        FxHandlers {
            area: Some(r_six),
            ..default()
        },
    )
    .register_fx(
        "master67_r_seven",
        FxHandlers {
            area: Some(r_seven),
            ..default()
        },
    )
    .register_fx(
        "master67_r",
        FxHandlers {
            cast: Some(r_cast),
            ..default()
        },
    )
    .on_ability_hit("master67_p", p_hit);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::core::runtime::FxRuntime;
    use crate::fx::effects::primitives::tests::fx_world;

    #[test]
    fn la_courbe_passe_par_les_points_du_trace() {
        let pts: Vec<Vec2> = SIX.iter().map(|&[x, y]| Vec2::new(x, y)).collect();
        assert!(catmull_rom(&pts, 0.0).distance(pts[0]) < 1e-5);
        assert!(catmull_rom(&pts, 1.0).distance(pts[8]) < 1e-5);
        assert!(catmull_rom(&pts, 0.5).distance(pts[4]) < 1e-5);
        assert_eq!(digit_path(7).len(), 11);
    }

    #[test]
    fn un_chiffre_s_eleve_puis_disparait() {
        let mut w = fx_world();
        rising_digit(&mut Fx::new(&mut w), 7, Vec3::ZERO, violet(), 1.7);
        let n = |w: &mut World| w.query::<&Billboard>().iter(w).count();
        assert_eq!(n(&mut w), 1);
        for _ in 0..10 {
            FxRuntime::update(&mut w, 0.1);
        }
        assert_eq!(n(&mut w), 0);
        assert_eq!(w.resource::<Assets<StandardMaterial>>().len(), 0);
    }
}
