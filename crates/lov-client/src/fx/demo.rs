//! Scène de démonstration des FX, sans serveur : `LOV_FXLAB=1 cargo run -p lov-client`.
//!
//! Équivalent minimal de la page `?fxlab` du client web pour le socle : une grille d'émetteurs, un par style
//! de particules standard, qui rejouent leur salve en boucle autour du centre de la caméra ; devant, une
//! rangée de décals au sol (télégraphies, onde de choc) ; derrière, les matériaux FX (bouclier fresnel, givre,
//! eau, modèle qui se dissout) et deux rubans de traînée en orbite.
//! `F9` bascule le post-traitement (qualité haute / basse) ; `LOV_POST=low` démarre en qualité basse.
//! `LOV_SCREENSHOT=chemin.png` enregistre une capture après 3 s (`LOV_SCREENSHOT_AT=4.5` pour choisir l'instant).

use std::f32::consts::TAU;

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

use super::particles::system::Blend;
use super::particles::{EmitOptions, EmitShape, ParticleSystem, StdStyles};
use super::post::{FxQuality, PostFxSettings};
use super::shaders::{
    DecalMaterial, DecalMesh, DecalShape, DissolveMaterial, FresnelMaterial, GroundDecal,
    IceMaterial, Ribbon, RibbonMaterial, RibbonMode, WaterMaterial, dissolvable, set_dissolve,
};
use crate::render::camera::CameraRig;

/// `LOV_FXLAB` défini (et non vide) : lancer la démo.
pub fn enabled() -> bool {
    std::env::var("LOV_FXLAB").is_ok_and(|v| !v.is_empty())
}

pub struct FxDemoPlugin;

impl Plugin for FxDemoPlugin {
    fn build(&self, app: &mut App) {
        if std::env::var("LOV_POST").as_deref() == Ok("low") {
            app.insert_resource(PostFxSettings {
                quality: FxQuality::Low,
                ..default()
            });
        }
        app.add_systems(
            Update,
            (
                replay,
                decals,
                ribbons,
                materials,
                toggle_post,
                screenshot,
                log_stats,
            ),
        );
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

/// Décals de la démo : (entité, matériau) dans l'ordre de `DECALS`.
#[derive(Default)]
struct DemoDecals(Vec<(GroundDecal, Handle<DecalMaterial>, Entity)>);

/// Rangée de décals devant la grille, animés sur la période des salves.
fn decals(
    mut commands: Commands,
    time: Res<Time>,
    rig: Res<CameraRig>,
    mesh: Res<DecalMesh>,
    mut materials: ResMut<Assets<DecalMaterial>>,
    mut transforms: Query<&mut Transform>,
    mut demo: Local<DemoDecals>,
) {
    let red = Vec3::new(1.0, 0.25, 0.2);
    let blue = Vec3::new(0.35, 0.65, 1.0);
    let gold = Vec3::new(1.0, 0.75, 0.3);
    if demo.0.is_empty() {
        for (i, (color, color2)) in [
            (red, Vec3::new(1.0, 0.7, 0.3)),
            (blue, blue),
            (gold, gold),
            (red, gold),
            (Vec3::new(0.05, 0.02, 0.08), Vec3::ZERO),
        ]
        .into_iter()
        .enumerate()
        {
            let decal = GroundDecal::default();
            let mut m = DecalMaterial::default();
            m.set_color(color, color2);
            let tf = Transform::from_xyz(i as f32, 0.0, 0.0);
            let handle = materials.add(m);
            let entity = commands.spawn(mesh.bundle(decal, handle.clone(), tf)).id();
            demo.0.push((decal, handle, entity));
        }
    }
    let t = (time.elapsed_secs() / PERIOD).fract();
    let at = |col: f32| rig.focus + Vec3::new((col - 2.0) * SPACING, 0.0, 2.5 * SPACING);
    for (i, (decal, handle, entity)) in demo.0.iter().enumerate() {
        let (Some(mut m), Ok(mut tf)) = (materials.get_mut(handle), transforms.get_mut(*entity))
        else {
            continue;
        };
        let p = at(i as f32);
        m.reset();
        *tf = match i {
            // 0 : télégraphie circulaire qui se remplit
            0 => {
                m.u.progress = t;
                decal.circle(&mut m, p, 1.6, DecalShape::Circle)
            }
            // 1 : onde de choc qui grandit et s'efface
            1 => {
                m.u.opacity = 1.0 - t;
                m.u.edge = 0.15;
                decal.circle(&mut m, p, 0.3 + 1.7 * t, DecalShape::Ring)
            }
            // 2 : ligne (skillshot) avec motif animé
            2 => {
                m.u.pattern = 1.0;
                m.u.fill = 0.3;
                decal.rect(
                    &mut m,
                    p - Vec3::new(1.5, 0.0, 1.0),
                    p + Vec3::new(1.5, 0.0, 1.0),
                    0.8,
                )
            }
            // 3 : cône qui se remplit
            3 => {
                m.u.progress = t;
                decal.cone(&mut m, p - Vec3::Z * 1.5, p + Vec3::Z, 3.0, 0.5)
            }
            // 4 : zone sombre persistante (mélange normal)
            _ => {
                m.set_blend(Blend::Normal);
                m.u.fill = 0.6;
                m.u.pattern = 0.5;
                decal.circle(&mut m, p, 1.4, DecalShape::Circle)
            }
        };
    }
}

/// Deux rubans en orbite derrière la grille : face caméra en hauteur, à plat au sol.
fn ribbons(
    mut commands: Commands,
    time: Res<Time>,
    rig: Res<CameraRig>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<RibbonMaterial>>,
    mut query: Query<&mut Ribbon>,
    mut spawned: Local<Vec<Entity>>,
) {
    if spawned.is_empty() {
        for (mode, color) in [
            (RibbonMode::Billboard, Vec3::new(1.0, 0.5, 0.15)),
            (RibbonMode::Flat, Vec3::new(0.35, 0.65, 1.0)),
        ] {
            let mut ribbon = Ribbon::new(32);
            ribbon.mode = mode;
            ribbon.width = 0.5;
            ribbon.life = 0.5;
            let mesh = meshes.add(ribbon.mesh());
            let material = materials.add(RibbonMaterial::new(color, None));
            spawned.push(commands.spawn(ribbon.bundle(mesh, material)).id());
        }
        return;
    }
    let a = time.elapsed_secs() * 3.0;
    for (k, entity) in spawned.iter().enumerate() {
        let Ok(mut ribbon) = query.get_mut(*entity) else {
            continue;
        };
        let (side, y) = if k == 0 { (-1.0, 1.5) } else { (1.0, 0.0) };
        let center = rig.focus + Vec3::new(side * 2.5 * SPACING / 2.0, y, -2.75 * SPACING);
        // le ruban au sol s'interrompt une partie du cycle : il s'efface puis repart à neuf (comme un
        // ruban rendu au pool par le runtime)
        if k == 1 && (a / TAU).fract() > 0.6 {
            if ribbon.alive() == 0 {
                ribbon.reset();
            }
            continue;
        }
        ribbon.push(center + Vec3::new(a.cos(), 0.0, a.sin()) * 2.0);
    }
}

/// Une pièce dissolvable de la démo, qui se défait et se reforme en boucle.
#[derive(Component)]
struct DemoDissolve;

/// Rangée de matériaux FX entre la grille et les rubans, posée une fois autour du centre de la caméra.
#[allow(clippy::too_many_arguments)] // un `ResMut<Assets<…>>` par matériau, comme toute scène Bevy
fn materials(
    mut commands: Commands,
    time: Res<Time>,
    rig: Res<CameraRig>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut fresnel: ResMut<Assets<FresnelMaterial>>,
    mut ice: ResMut<Assets<IceMaterial>>,
    mut water: ResMut<Assets<WaterMaterial>>,
    mut dissolve: ResMut<Assets<DissolveMaterial>>,
    pieces: Query<&MeshMaterial3d<DissolveMaterial>, With<DemoDissolve>>,
    mut done: Local<bool>,
) {
    if !*done {
        *done = true;
        let at =
            |col: f32, y: f32| rig.focus + Vec3::new((col - 2.0) * SPACING, y, -1.75 * SPACING);
        // 0 : bouclier fresnel à bandes
        let mut shield = FresnelMaterial::new(Vec3::new(0.35, 0.65, 1.0) * 1.5);
        shield.u.bands = 0.6;
        commands.spawn((
            Mesh3d(meshes.add(Sphere::new(1.2).mesh().uv(32, 16))),
            MeshMaterial3d(fresnel.add(shield)),
            Transform::from_translation(at(0.0, 1.2)),
        ));
        // 1 : éclats de givre (sphère grossière : facettes visibles)
        let shard = meshes.add(Sphere::new(0.5).mesh().uv(6, 4));
        let frost = ice.add(IceMaterial::default());
        for (dx, s, tilt) in [(-0.6, 1.0, 0.3), (0.5, 1.4, -0.4), (0.1, 0.8, 1.2)] {
            commands.spawn((
                Mesh3d(shard.clone()),
                MeshMaterial3d(frost.clone()),
                Transform::from_translation(at(1.0, 0.5 * s) + Vec3::X * dx)
                    .with_scale(Vec3::new(0.6, 1.6, 0.6) * s)
                    .with_rotation(Quat::from_rotation_z(tilt * 0.3)),
            ));
        }
        // 2 : flaque d'eau
        commands.spawn((
            Mesh3d(meshes.add(Plane3d::default().mesh().size(3.0, 3.0))),
            MeshMaterial3d(water.add(WaterMaterial::default())),
            Transform::from_translation(at(2.0, 0.04)),
        ));
        // 3 : colonne qui se dissout (avec son ombre)
        commands.spawn((
            DemoDissolve,
            Mesh3d(meshes.add(Capsule3d::new(0.5, 1.2))),
            MeshMaterial3d(dissolve.add(dissolvable(StandardMaterial {
                base_color: Color::srgb(0.75, 0.7, 0.65),
                ..default()
            }))),
            Transform::from_translation(at(3.0, 1.1)),
        ));
        // 4 : dôme fresnel doré, plein (base forte)
        let mut dome = FresnelMaterial::new(Vec3::new(1.0, 0.75, 0.3));
        dome.u.base = 0.15;
        dome.u.power = 1.5;
        commands.spawn((
            Mesh3d(meshes.add(Sphere::new(1.0).mesh().uv(32, 16))),
            MeshMaterial3d(fresnel.add(dome)),
            Transform::from_translation(at(4.0, 0.0)),
        ));
        return;
    }
    // dissolution en dents de scie sur deux périodes, avec une pause intacte
    let t = (time.elapsed_secs() / (2.0 * PERIOD)).fract();
    let amount = ((t - 0.2) / 0.7).clamp(0.0, 1.0);
    for handle in &pieces {
        if let Some(mut m) = dissolve.get_mut(&handle.0) {
            set_dissolve(&mut m, amount);
        }
    }
}

/// `F9` : bascule entre qualité haute (bloom, étalonnage) et basse, pour comparer.
fn toggle_post(keys: Res<ButtonInput<KeyCode>>, mut settings: ResMut<PostFxSettings>) {
    if keys.just_pressed(KeyCode::F9) {
        settings.quality = match settings.quality {
            FxQuality::High => FxQuality::Low,
            FxQuality::Low => FxQuality::High,
        };
        info!("post-traitement : {:?}", settings.quality);
    }
}

fn screenshot(mut commands: Commands, time: Res<Time>, mut done: Local<bool>) {
    let at = std::env::var("LOV_SCREENSHOT_AT")
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(3.0);
    if *done || time.elapsed_secs() < at {
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
