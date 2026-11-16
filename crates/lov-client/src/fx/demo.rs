//! Scène de démonstration des FX, sans serveur : `LOV_FXLAB=1 cargo run -p lov-client`.
//!
//! Équivalent minimal de la page `?fxlab` du client web pour le socle : une grille d'émetteurs, un par style
//! de particules standard, qui rejouent leur salve en boucle autour du centre de la caméra.
//! `LOV_SCREENSHOT=chemin.png` enregistre une capture après 3 s.

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

use super::particles::system::Blend;
use super::particles::{EmitOptions, EmitShape, ParticleSystem, StdStyles};
use crate::render::camera::CameraRig;

/// `LOV_FXLAB` défini (et non vide) : lancer la démo.
pub fn enabled() -> bool {
    std::env::var("LOV_FXLAB").is_ok_and(|v| !v.is_empty())
}

pub struct FxDemoPlugin;

impl Plugin for FxDemoPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (replay, screenshot, log_stats));
    }
}

const PERIOD: f32 = 1.4;
const SPACING: f32 = 4.0;

fn replay(
    time: Res<Time>,
    rig: Res<CameraRig>,
    s: Res<StdStyles>,
    mut ps: ResMut<ParticleSystem>,
    mut next: Local<f32>,
) {
    if time.elapsed_secs() < *next {
        return;
    }
    *next = time.elapsed_secs() + PERIOD;
    let gold = Vec3::new(1.0, 0.75, 0.3);
    let blue = Vec3::new(0.35, 0.65, 1.0);
    let at = |i: usize| {
        let (col, row) = ((i % 5) as f32 - 2.0, (i / 5) as f32 - 1.5);
        rig.focus + Vec3::new(col * SPACING, 0.3, row * SPACING)
    };
    let burst = |count: f32, life: [f32; 2], size: [f32; 2], speed: [f32; 2]| EmitOptions {
        count,
        life: life.into(),
        size: size.into(),
        speed: Some(speed.into()),
        ..default()
    };
    // 0 : éclat lumineux HDR (doit déclencher le bloom quand le post-traitement sera branché)
    ps.emit(
        s.glow_pop,
        at(0),
        &EmitOptions {
            tint: gold,
            intensity: 3.0,
            ..burst(6.0, [0.4, 0.7], [1.2, 2.0], [0.0, 0.5])
        },
    );
    // 1 : étincelles étirées en cône, avec gravité
    ps.emit(
        s.spark,
        at(1),
        &EmitOptions {
            shape: EmitShape::Cone,
            angle: 0.6,
            gravity: 9.0,
            stretch: 0.05,
            tint: gold,
            intensity: 2.5,
            ..burst(40.0, [0.4, 0.8], [0.12, 0.2], [5.0, 9.0])
        },
    );
    // 2 : fumée (lot normal)
    ps.emit(
        s.smoke,
        at(2),
        &EmitOptions {
            up: Some([0.4, 1.0].into()),
            radius: 0.6,
            ..burst(12.0, [1.0, 1.4], [1.0, 1.6], [0.1, 0.4])
        },
    );
    // 3 : flammes montantes
    ps.emit(
        s.flame,
        at(3),
        &EmitOptions {
            shape: EmitShape::Disc,
            radius: 0.5,
            up: Some([1.5, 3.0].into()),
            drag: 1.0,
            ..burst(30.0, [0.5, 0.9], [0.5, 0.9], [0.0, 0.3])
        },
    );
    // 4 : onde (anneau qui grandit)
    ps.emit(
        s.ring,
        at(4),
        &EmitOptions {
            tint: blue,
            intensity: 2.0,
            shape: EmitShape::Point,
            ..burst(1.0, [0.6, 0.6], [4.0, 4.0], [0.0, 0.0])
        },
    );
    // 5 : braises
    ps.emit(
        s.ember,
        at(5),
        &EmitOptions {
            tint: gold,
            intensity: 2.0,
            up: Some([1.0, 2.0].into()),
            drag: 1.5,
            ..burst(30.0, [0.8, 1.2], [0.08, 0.16], [1.0, 2.5])
        },
    );
    // 6 : poussière au sol
    ps.emit(
        s.dust,
        at(6),
        &EmitOptions {
            shape: EmitShape::Ring,
            radius: 0.8,
            drag: 2.0,
            ..burst(14.0, [0.8, 1.2], [0.8, 1.2], [1.5, 2.5])
        },
    );
    ps.emit(
        s.debris,
        at(6),
        &EmitOptions {
            tint: Vec3::new(0.35, 0.3, 0.25),
            gravity: 9.0,
            up: Some([2.0, 4.0].into()),
            spin: Some([-6.0, 6.0].into()),
            ..burst(8.0, [0.6, 0.9], [0.2, 0.35], [1.0, 2.5])
        },
    );
    // 7 : étoiles qui tournent
    ps.emit(
        s.star,
        at(7),
        &EmitOptions {
            tint: gold,
            intensity: 1.8,
            spin: Some([-4.0, 4.0].into()),
            drag: 2.0,
            ..burst(10.0, [0.8, 1.2], [0.3, 0.5], [1.5, 3.0])
        },
    );
    // 8 : flocons qui tombent
    ps.emit(
        s.snow,
        at(8),
        &EmitOptions {
            tint: blue,
            intensity: 1.5,
            shape: EmitShape::Disc,
            radius: 1.2,
            gravity: 0.6,
            spin: Some([-1.0, 1.0].into()),
            ..burst(14.0, [1.0, 1.3], [0.25, 0.4], [0.0, 0.2])
        },
    );
    // 9 : plumes (lot normal)
    ps.emit(
        s.feather,
        at(9),
        &EmitOptions {
            tint: Vec3::splat(0.2),
            gravity: 1.0,
            drag: 2.0,
            spin: Some([-3.0, 3.0].into()),
            up: Some([1.0, 2.0].into()),
            ..burst(10.0, [1.0, 1.3], [0.3, 0.45], [1.0, 2.0])
        },
    );
    // 10 : rune au sol
    ps.emit(
        s.rune,
        at(10),
        &EmitOptions {
            tint: blue,
            intensity: 2.0,
            shape: EmitShape::Point,
            spin: Some(1.5.into()),
            ..burst(1.0, [1.2, 1.2], [2.5, 2.5], [0.0, 0.0])
        },
    );
    // 11 : éclats
    ps.emit(
        s.shard,
        at(11),
        &EmitOptions {
            tint: blue,
            intensity: 1.5,
            gravity: 6.0,
            up: Some([2.0, 4.0].into()),
            spin: Some([-6.0, 6.0].into()),
            ..burst(12.0, [0.6, 1.0], [0.25, 0.4], [2.0, 3.5])
        },
    );
    // 12 : croix de soin
    ps.emit(
        s.cross,
        at(12),
        &EmitOptions {
            tint: Vec3::new(0.4, 1.0, 0.5),
            intensity: 1.8,
            up: Some([1.0, 1.6].into()),
            radius: 0.6,
            ..burst(6.0, [0.9, 1.2], [0.35, 0.5], [0.0, 0.2])
        },
    );
    // 13 : pièces d'or
    ps.emit(
        s.coin,
        at(13),
        &EmitOptions {
            tint: gold,
            intensity: 1.5,
            gravity: 8.0,
            up: Some([3.0, 5.0].into()),
            spin: Some([-5.0, 5.0].into()),
            ..burst(10.0, [0.8, 1.0], [0.25, 0.3], [0.5, 1.5])
        },
    );
    // 14 : tourbillon + scintillements
    ps.emit(
        s.swirl,
        at(14),
        &EmitOptions {
            tint: blue,
            intensity: 2.0,
            shape: EmitShape::Point,
            spin: Some(4.0.into()),
            ..burst(1.0, [1.0, 1.0], [2.0, 2.0], [0.0, 0.0])
        },
    );
    ps.emit(
        s.sparkles,
        at(14),
        &EmitOptions {
            tint: gold,
            intensity: 2.0,
            radius: 1.0,
            ..burst(4.0, [0.8, 1.2], [0.6, 0.9], [0.0, 0.3])
        },
    );
    // 15 : volutes sombres (lot normal)
    ps.emit(
        s.dark_wisp,
        at(15),
        &EmitOptions {
            up: Some([0.3, 0.8].into()),
            radius: 0.6,
            ..burst(10.0, [1.0, 1.4], [0.9, 1.4], [0.1, 0.4])
        },
    );
    // 16 : sang
    ps.emit(
        s.blood,
        at(16),
        &EmitOptions {
            gravity: 9.0,
            up: Some([1.5, 3.0].into()),
            ..burst(16.0, [0.5, 0.8], [0.08, 0.14], [1.0, 2.5])
        },
    );
    // 17 : lueur douce + 18 : traînée d'émission en ligne + 19 : sphère de braises
    ps.emit(
        s.glow,
        at(17),
        &EmitOptions {
            tint: blue,
            intensity: 1.5,
            radius: 0.5,
            ..burst(8.0, [0.8, 1.2], [0.8, 1.2], [0.0, 0.3])
        },
    );
    let line = EmitOptions {
        shape: EmitShape::Line,
        to: Some(at(18) + Vec3::new(2.0, 1.0, 0.0)),
        tint: gold,
        intensity: 2.0,
        ..burst(30.0, [0.6, 1.0], [0.15, 0.25], [0.0, 0.3])
    };
    ps.emit(s.glow, at(18) - Vec3::new(1.0, 0.0, 0.0), &line);
    ps.emit(
        s.ember,
        at(19) + Vec3::Y,
        &EmitOptions {
            shape: EmitShape::Sphere,
            radius: 1.0,
            tint: Vec3::new(1.0, 0.3, 0.8),
            intensity: 2.0,
            ..burst(40.0, [0.8, 1.2], [0.08, 0.14], [0.2, 0.6])
        },
    );
}

fn screenshot(mut commands: Commands, time: Res<Time>, mut done: Local<bool>) {
    if *done || time.elapsed_secs() < 3.0 {
        return;
    }
    *done = true;
    if let Ok(path) = std::env::var("LOV_SCREENSHOT") {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
    }
}

/// Une ligne de log toutes les 5 s : emplacements occupés par lot et total émis (budget de particules).
fn log_stats(time: Res<Time>, ps: Res<ParticleSystem>, mut next: Local<f32>) {
    if time.elapsed_secs() < *next {
        return;
    }
    *next = time.elapsed_secs() + 5.0;
    info!(
        "fx : {} / {} particules additives, {} / {} normales, {} emises",
        ps.batch(Blend::Add).used(),
        ps.batch(Blend::Add).capacity(),
        ps.batch(Blend::Normal).used(),
        ps.batch(Blend::Normal).capacity(),
        ps.emitted
    );
}
