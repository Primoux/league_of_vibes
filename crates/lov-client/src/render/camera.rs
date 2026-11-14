//! Caméra de jeu : vue plongeante, défilement aux bords de l'écran et aux flèches, zoom à la molette,
//! Espace pour revenir sur son champion, Y pour verrouiller la caméra dessus.

use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;

use super::map::MAP_SIZE;
use super::{RENDER_SCALE, to_world};
use crate::net::{GameState, num};

const EDGE_PX: f32 = 14.0;
const PAN_SPEED: f32 = 32.0;
const DEFAULT_DISTANCE: f32 = 46.0;
/// Direction du point visé vers la caméra : environ 56° au-dessus de l'horizon.
const OFFSET: Vec3 = Vec3::new(0.0, 0.83, 0.56);

#[derive(Resource)]
pub struct CameraRig {
    pub focus: Vec3,
    pub distance: f32,
    pub locked: bool,
    centered_once: bool,
}

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(CameraRig {
            focus: to_world(Vec2::new(1500.0, 1500.0), 0.0),
            distance: DEFAULT_DISTANCE,
            locked: false,
            centered_once: false,
        })
        .add_systems(Startup, spawn_camera)
        .add_systems(Update, drive_camera);
    }
}

fn spawn_camera(mut commands: Commands) {
    // Focale longue et caméra éloignée : peu de déformation sur les bords, comme une vue de MOBA.
    commands.spawn((
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: 0.42,
            ..default()
        }),
        Transform::default(),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 9_000.0,
            ..default()
        },
        Transform::from_xyz(-6.0, 12.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    // Lumière d'appoint, pour que les faces à l'ombre ne soient pas noires.
    commands.spawn((
        DirectionalLight {
            illuminance: 2_500.0,
            ..default()
        },
        Transform::from_xyz(6.0, 6.0, -8.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn drive_camera(
    mut rig: ResMut<CameraRig>,
    mut camera: Single<&mut Transform, With<Camera3d>>,
    window: Single<&Window>,
    keys: Res<ButtonInput<KeyCode>>,
    scroll: Res<AccumulatedMouseScroll>,
    state: Res<GameState>,
    time: Res<Time>,
) {
    if keys.just_pressed(KeyCode::KeyY) {
        rig.locked = !rig.locked;
    }
    rig.distance = (rig.distance - scroll.delta.y * 3.0).clamp(20.0, 90.0);

    let me = state
        .me()
        .map(|e| to_world(Vec2::new(num(e, "x"), num(e, "y")), 0.0));
    let follow = rig.locked || keys.pressed(KeyCode::Space) || !rig.centered_once;
    if let Some(me) = me.filter(|_| follow) {
        rig.focus = me;
        rig.centered_once = true;
    } else {
        let mut pan = Vec2::ZERO;
        if let Some(cursor) = window.cursor_position() {
            pan.x += f32::from(cursor.x > window.width() - EDGE_PX) - f32::from(cursor.x < EDGE_PX);
            pan.y +=
                f32::from(cursor.y > window.height() - EDGE_PX) - f32::from(cursor.y < EDGE_PX);
        }
        pan.x += f32::from(keys.pressed(KeyCode::ArrowRight))
            - f32::from(keys.pressed(KeyCode::ArrowLeft));
        pan.y +=
            f32::from(keys.pressed(KeyCode::ArrowDown)) - f32::from(keys.pressed(KeyCode::ArrowUp));
        // Le bas de l'écran est +z en monde, la droite +x.
        let step = PAN_SPEED * (rig.distance / DEFAULT_DISTANCE) * time.delta_secs();
        rig.focus += Vec3::new(pan.x, 0.0, pan.y) * step;
    }
    let size = MAP_SIZE * RENDER_SCALE;
    rig.focus.x = rig.focus.x.clamp(0.0, size);
    rig.focus.z = rig.focus.z.clamp(-size, 0.0);

    **camera = Transform::from_translation(rig.focus + OFFSET * rig.distance)
        .looking_at(rig.focus, Vec3::Y);
}
