//! Ambiance de la carte : lucioles dans la jungle, pollen au-dessus des voies, brume et scintillements sur
//! la rivière, étincelles qui montent des fontaines.
//!
//! Portage de `packages/client/src/fx/world/ambient.ts`. Émission seulement dans l'emprise de la caméra
//! (coins de l'écran projetés au sol), proportionnelle à la surface de chaque zone visible ; aucune
//! allocation par image.
//!
//! Écart : le web donne à l'ambiance un petit système de particules dédié, pour que le lot principal
//! puisse se vider entre deux combats ; ici les styles d'ambiance vivent dans le système principal (un
//! second système demanderait un second jeu de lots GPU au socle FX). L'ambiance n'émet que pendant une
//! partie.

use bevy::prelude::*;
use lov_shared::constants::MAP_SIZE;
use lov_shared::map::{FOUNTAINS, LANE_HALF_WIDTH, RIVER_HALF_WIDTH};

use crate::fx::core::curves::presets::{FADE_IN_OUT, ONE, POP_SHRINK};
use crate::fx::core::rng::Rng;
use crate::fx::particles::atlas::Tex;
use crate::fx::particles::system::{Blend, ParticleStyle, ParticleStyleDef};
use crate::fx::particles::{EmitOptions, EmitShape, ParticleSystem};
use crate::net::GameState;
use crate::render::RENDER_SCALE as S;
use crate::render::camera::MainCamera;
use crate::render::map::geometry::{dist_to_lane, dist_to_river, terrain_height};

/// Particules par seconde et par million d'unités² de jeu visibles.
const RATE_FIREFLY: f32 = 0.9;
const RATE_POLLEN: f32 = 0.7;
const RATE_MIST: f32 = 0.12;
const RATE_SPARKLE: f32 = 1.4;
const TICK: f32 = 0.1;

const C_FIREFLY: Vec3 = Vec3::new(0.75, 1.0, 0.35);
const C_POLLEN: Vec3 = Vec3::new(1.0, 0.95, 0.75);
const C_MIST: Vec3 = Vec3::new(0.85, 0.95, 1.0);
const C_SPARK: Vec3 = Vec3::new(0.85, 0.97, 1.0);
const C_BLUE: Vec3 = Vec3::new(0.35, 0.65, 1.0);
const C_RED: Vec3 = Vec3::new(1.0, 0.4, 0.3);

/// La rivière s'écoule vers le bas-droite : jeu (1, -1) → monde (1, 0, 1).
const FLOW: Vec3 = Vec3::new(0.127_279_22, 0.0, 0.127_279_22);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Zone {
    Jungle,
    Lane,
    River,
    None,
}

/// Zone d'ambiance d'un point de jeu.
pub fn zone_at(x: f64, y: f64) -> Zone {
    if x < 0.0 || y < 0.0 || x > MAP_SIZE || y > MAP_SIZE {
        return Zone::None;
    }
    let near = |f: lov_shared::types::Vec2| (x - f.x).hypot(y - f.y) < 2300.0;
    if near(FOUNTAINS.blue) || near(FOUNTAINS.red) {
        return Zone::None;
    }
    if dist_to_river(x, y) < RIVER_HALF_WIDTH - 80.0
        && (x - y).abs() * std::f64::consts::FRAC_1_SQRT_2 < 5800.0
    {
        return Zone::River;
    }
    if dist_to_lane(x, y) < LANE_HALF_WIDTH + 150.0 {
        return Zone::Lane;
    }
    Zone::Jungle
}

/// Emprise de la caméra au sol, en coordonnées de jeu.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl View {
    fn w(&self) -> f64 {
        self.max_x - self.min_x
    }
    fn h(&self) -> f64 {
        self.max_y - self.min_y
    }
}

/// Styles et compteurs de l'ambiance.
#[derive(Resource)]
pub struct Ambient {
    firefly: ParticleStyle,
    mote: ParticleStyle,
    mist: ParticleStyle,
    sparkle: ParticleStyle,
    acc: f32,
    carry: [f32; 4],
    rng: Rng,
    pub enabled: bool,
}

const FIREFLY_ALPHA: &[[f32; 2]] = &[
    [0.0, 0.0],
    [0.15, 1.0],
    [0.3, 0.35],
    [0.45, 1.0],
    [0.6, 0.4],
    [0.75, 0.9],
    [1.0, 0.0],
];
const MIST_ALPHA: &[[f32; 2]] = &[[0.0, 0.0], [0.35, 0.07], [0.7, 0.05], [1.0, 0.0]];
const MIST_SIZE: &[[f32; 2]] = &[[0.0, 0.7], [1.0, 1.3]];
const SPARKLE_ALPHA: &[[f32; 2]] = &[[0.0, 0.0], [0.4, 1.0], [1.0, 0.0]];

impl Ambient {
    /// Définit les styles d'ambiance dans le système de particules.
    pub fn new(ps: &mut ParticleSystem) -> Self {
        // luciole : clignote (alpha en dents de scie douces), taille constante
        let firefly = ps.define_style(ParticleStyleDef {
            alpha: FIREFLY_ALPHA,
            size: ONE,
            ..ParticleStyleDef::new(Tex::Glow)
        });
        let mote = ps.define_style(ParticleStyleDef {
            alpha: FADE_IN_OUT,
            size: ONE,
            ..ParticleStyleDef::new(Tex::Dot)
        });
        let mist = ps.define_style(ParticleStyleDef {
            blend: Blend::Normal,
            alpha: MIST_ALPHA,
            size: MIST_SIZE,
            ..ParticleStyleDef::new(Tex::Glow)
        });
        let sparkle = ps.define_style(ParticleStyleDef {
            alpha: SPARKLE_ALPHA,
            size: POP_SHRINK,
            ..ParticleStyleDef::new(Tex::Sparkles)
        });
        Self {
            firefly,
            mote,
            mist,
            sparkle,
            acc: 0.0,
            carry: [0.0; 4],
            rng: Rng::default(),
            enabled: true,
        }
    }

    fn spawn(&mut self, ps: &mut ParticleSystem, kind: usize, expected: f32, v: &View, zone: Zone) {
        let n = expected + self.carry[kind];
        let whole = n.floor();
        self.carry[kind] = n - whole;
        for _ in 0..whole as u32 {
            let x = v.min_x + self.rng.next() * v.w();
            let y = v.min_y + self.rng.next() * v.h();
            if zone_at(x, y) != zone {
                continue;
            }
            let ground = (terrain_height(x, y) * S as f64) as f32;
            let (wx, wz) = (x as f32 * S, -(y as f32) * S);
            let r = self.rng.next_f32();
            match kind {
                0 => ps.emit(
                    self.firefly,
                    Vec3::new(wx, ground + 0.25 + r * 1.1, wz),
                    &EmitOptions {
                        life: [3.0, 5.5].into(),
                        size: [0.07, 0.12].into(),
                        speed: Some([0.08, 0.22].into()),
                        drag: 0.15,
                        up: Some([-0.03, 0.06].into()),
                        tint: C_FIREFLY,
                        intensity: 2.6,
                        ..default()
                    },
                ),
                1 => ps.emit(
                    self.mote,
                    Vec3::new(wx, ground + 0.15 + r * 1.2, wz),
                    &EmitOptions {
                        life: [4.0, 7.0].into(),
                        size: [0.025, 0.045].into(),
                        speed: Some([0.04, 0.12].into()),
                        up: Some([0.02, 0.08].into()),
                        tint: C_POLLEN,
                        intensity: 0.9,
                        ..default()
                    },
                ),
                2 => ps.emit(
                    self.mist,
                    Vec3::new(wx, -0.12, wz),
                    &EmitOptions {
                        life: [6.0, 9.0].into(),
                        size: [3.0, 4.5].into(),
                        speed: Some(0.0.into()),
                        up: Some([0.0, 0.02].into()),
                        inherit: Some(FLOW),
                        tint: C_MIST,
                        ..default()
                    },
                ),
                _ => ps.emit(
                    self.sparkle,
                    Vec3::new(wx, -0.12, wz),
                    &EmitOptions {
                        life: [0.5, 0.9].into(),
                        size: [0.08, 0.16].into(),
                        speed: Some(0.0.into()),
                        inherit: Some(FLOW),
                        tint: C_SPARK,
                        intensity: 2.2,
                        ..default()
                    },
                ),
            }
        }
    }

    /// Étincelles aux couleurs d'équipe qui montent des fontaines (si visibles).
    fn fountains(&self, ps: &mut ParticleSystem, step: f32, v: &View) {
        for (f, color) in [(FOUNTAINS.blue, C_BLUE), (FOUNTAINS.red, C_RED)] {
            if f.x < v.min_x - 600.0
                || f.x > v.max_x + 600.0
                || f.y < v.min_y - 600.0
                || f.y > v.max_y + 600.0
            {
                continue;
            }
            let p = Vec3::new(
                f.x as f32 * S,
                (terrain_height(f.x, f.y) * S as f64) as f32 + 0.3,
                -(f.y as f32) * S,
            );
            ps.emit(
                self.mote,
                p,
                &EmitOptions {
                    count: (step * 30.0).round().max(1.0),
                    shape: EmitShape::Ring,
                    radius: 3.4,
                    life: [1.6, 2.6].into(),
                    size: [0.05, 0.09].into(),
                    speed: Some([-0.05, 0.05].into()),
                    up: Some([0.5, 1.1].into()),
                    drag: 0.3,
                    tint: color,
                    intensity: 2.4,
                    ..default()
                },
            );
            ps.emit(
                self.firefly,
                p + Vec3::Y * 2.2,
                &EmitOptions {
                    count: (step * 6.0).round().max(1.0),
                    shape: EmitShape::Sphere,
                    radius: 0.9,
                    life: [1.0, 1.8].into(),
                    size: [0.1, 0.18].into(),
                    speed: Some([0.1, 0.3].into()),
                    up: Some([0.2, 0.4].into()),
                    tint: color,
                    intensity: 2.0,
                    ..default()
                },
            );
        }
    }
}

/// Emprise au sol (y = 0) de la caméra, en coordonnées de jeu.
pub fn camera_view(camera: &Camera, tf: &GlobalTransform) -> Option<View> {
    let size = camera.logical_viewport_size()?;
    let mut v = View {
        min_x: f64::INFINITY,
        min_y: f64::INFINITY,
        max_x: f64::NEG_INFINITY,
        max_y: f64::NEG_INFINITY,
    };
    for c in [
        Vec2::ZERO,
        Vec2::new(size.x, 0.0),
        Vec2::new(0.0, size.y),
        size,
    ] {
        let ray = camera.viewport_to_world(tf, c).ok()?;
        // un coin au-dessus de l'horizon : on borne la distance
        let t = if ray.direction.y < -1e-3 {
            (-ray.origin.y / ray.direction.y).min(200.0)
        } else {
            200.0
        };
        let p = ray.origin + *ray.direction * t;
        let (gx, gy) = ((p.x / S) as f64, (-p.z / S) as f64);
        v.min_x = v.min_x.min(gx);
        v.min_y = v.min_y.min(gy);
        v.max_x = v.max_x.max(gx);
        v.max_y = v.max_y.max(gy);
    }
    Some(v)
}

/// Émet l'ambiance autour de la caméra (pendant une partie).
pub fn update_ambient(
    time: Res<Time>,
    state: Res<GameState>,
    ambient: Option<ResMut<Ambient>>,
    mut ps: ResMut<ParticleSystem>,
    camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
) {
    let Some(mut a) = ambient else { return };
    if !a.enabled || state.game.is_none() {
        return;
    }
    a.acc += time.delta_secs().min(0.1);
    if a.acc < TICK {
        return;
    }
    let step = a.acc;
    a.acc = 0.0;
    let Some(v) = camera.iter().next().and_then(|(c, tf)| camera_view(c, tf)) else {
        return;
    };
    if !(v.w() > 0.0 && v.h() > 0.0) {
        return;
    }
    // tirages : nombre attendu par zone = taux × surface vue × durée ; chaque tirage teste la zone du point
    let area = (v.w() * v.h() / 1e6) as f32;
    a.spawn(&mut ps, 0, area * RATE_FIREFLY * step, &v, Zone::Jungle);
    a.spawn(&mut ps, 1, area * RATE_POLLEN * step, &v, Zone::Lane);
    a.spawn(&mut ps, 2, area * RATE_MIST * step, &v, Zone::River);
    a.spawn(&mut ps, 3, area * RATE_SPARKLE * step, &v, Zone::River);
    a.fountains(&mut ps, step, &v);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zones_de_la_carte() {
        assert_eq!(zone_at(-10.0, 50.0), Zone::None);
        assert_eq!(zone_at(FOUNTAINS.blue.x, FOUNTAINS.blue.y), Zone::None);
        assert_eq!(zone_at(3500.0, 3500.0), Zone::Lane, "voie du milieu");
        assert_eq!(
            zone_at(6000.0, 6000.0),
            Zone::River,
            "la rivière croise le milieu"
        );
        assert_eq!(zone_at(4500.0, 7500.0), Zone::River);
        assert_eq!(zone_at(4000.0, 6000.0), Zone::Jungle);
    }

    #[test]
    fn report_des_fractions_de_particules() {
        let mut ps = ParticleSystem::new(1024, Rng::new(3));
        let mut a = Ambient::new(&mut ps);
        let v = View {
            min_x: 0.0,
            min_y: 0.0,
            max_x: MAP_SIZE,
            max_y: MAP_SIZE,
        };
        a.spawn(&mut ps, 0, 0.4, &v, Zone::Jungle);
        assert_eq!(ps.emitted, 0);
        assert!((a.carry[0] - 0.4).abs() < 1e-6, "la fraction est reportée");
        a.spawn(&mut ps, 0, 50.0, &v, Zone::Jungle);
        assert!(ps.emitted > 0, "des lucioles dans la jungle");
    }
}
